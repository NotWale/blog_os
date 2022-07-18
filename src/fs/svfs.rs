#![allow(non_snake_case)]
use lazy_static::lazy_static;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use spin::Mutex;
use alloc::collections::btree_map::BTreeMap;
use crate::alloc::borrow::ToOwned;
use crate::{print, println};
use crate::alloc::string::ToString;
use crate::fs::fs::Operations;
use core::arch::x86_64::_rdtsc;

pub static mut CURFS: i32 = 0; // The current filesystem
lazy_static! {
    pub static ref FSLIST: Mutex<Vec<Box<dyn Operations + Send>>> = Mutex::new(Vec::new()); // Vector of mounted Filesystems, as trait objects
}

// --- Structs ---
#[derive(Clone)]
pub struct FileSystem {
    pub m_sb: Superblock, // Mounted Superblock
    pub InodeCount: i32, // Generates unique inode numbers
    pub MountCount: i32, // Count for mounted filesystems
    pub DirMap: BTreeMap<i32, Dir>, // Table of Dirs with associated Inodes
    pub FileMap: BTreeMap<i32, File>, // Table of Files with associated Inodes
    pub Path: i32, // Current path inside the current filesystem as inode
    pub ParentFS: Option<i32>, // Parent Filesystem where the current one is mounted on
}

// Filesystem Metadata
#[derive(Clone)]
pub struct Superblock {
    pub device: String, // Name of Filesystem(Device)
    pub filecount: i32, // Counter for files
    pub dircount: i32, // Counter for directories
}

#[derive(Clone)]
pub struct Dir {
    pub dirname: String, // current name of the dir
    pub DirList: BTreeMap<i32, String>, // Map of dirs inside the dir
    pub FileList: BTreeMap<i32, String>, // Map of files inside the dir
    pub parentdir: Option<i32>, // parent dir inode nr (which might not exist)
}

#[derive(Clone)]
pub struct File {
    pub filename: String, // current name of the file
    pub Data: String, // String to store the content of the file
}

// --- Impl ---

impl Superblock {
    pub fn get_info(&self) {
        println!("Device: {}\nFilecount: {}\nDircount: {}", self.device, self.filecount, self.dircount);
        unsafe { println!("Current time-stamp counter: {}", _rdtsc()); }
    }
    pub fn get_device(&self) -> String {
        self.device.clone()
    }
}

impl Operations for FileSystem {
    // Create a new directory in the current directory
    fn mkdir(&mut self, dname: &str, fscheck: bool) {
        // Check if directory with this name already exists
        if !self.DirMap.get(&self.Path).is_none() {
            let findsame = match self.DirMap.get(&self.Path).unwrap().DirList.iter().find(|&s| *s.1 == dname) {
                Some(s) => { println!("This directory already exists. Please choose a different name."); return },
                None => (),
            };
        }  

        // Create a new Inode for this dir
        let mut cur_inode_nr = self.MountCount;
        if fscheck { self.MountCount = self.MountCount-1; } // The negative mountcount is used as inodes in the case of mounted filesystems
        if !fscheck { 
            self.InodeCount += 1;
            cur_inode_nr = self.InodeCount;
        }

        // Create a new Dir struct which stores information about its files/dirs
        let newdir = Dir {
            dirname: dname.to_string(),
            DirList: BTreeMap::new(),
            FileList: BTreeMap::new(),
            parentdir: Some(self.Path),
        };

        // Store it inside the DirMap
        self.DirMap.insert(cur_inode_nr, newdir.clone());
        self.m_sb.dircount = self.m_sb.dircount + 1;

        // Add this dir to the dirlist of the dir in the current PATH
        if !self.DirMap.get_mut(&self.Path).is_none() {
            self.DirMap.get_mut(&self.Path).unwrap().DirList.insert(cur_inode_nr, newdir.dirname.to_string());
        }
    }

    // Create a file in the current directory
    fn read_file(&mut self, fname: &str) {
        let mut curinode = 0;
        // Check if file with this name exists
        if !self.DirMap.get(&self.Path).is_none() {
            match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == fname) {
                Some(s) => { curinode = *s.0; },
                None => { println!("Cannot find {} inside the current dir.", fname); return },
            };
        } 

        // Search for file inside the FileMap and read contents from it
        match self.FileMap.get_mut(&curinode) {
            Some(s) => { println!("{}", s.Data); },
            None => { println!("Cannot find {} in the global FileMap.", fname); return },
        };
    }

    // Write data into a file
    fn write_file(&mut self, msg: &str) {
        let mut split = msg.splitn(2, ' ');
        // Split up string into two 
        let fname = split.next();  
        let content = split.next();
        let mut curinode = 0;
        if content.is_none() { println!("Usage: write <filename> <message>"); return }

        // Check if file with this name exists
        if !self.DirMap.get(&self.Path).is_none() {
            match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == fname.unwrap()) {
                Some(s) => { curinode = *s.0; },
                None => { println!("Cannot find {} inside the current dir.", fname.unwrap()); return },
            };
        } 

        // Search for file inside the FileMap and write the contents to it
        match self.FileMap.get_mut(&curinode) {
            Some(s) => { s.Data = content.unwrap().to_string(); },
            None => { println!("Cannot find {} in the global FileMap.", fname.unwrap()); return },
        };
    }

    // Create a File inside the current dir
    fn create_file(&mut self, fname: &str) {
        // Check if file with this name already exists
        if !self.DirMap.get(&self.Path).is_none() {
            let findsame = match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == fname) {
                Some(s) => { println!("This file already exists. Please choose a different name."); return },
                None => (),
            };
        } 

        self.InodeCount += 1;
        let cur_inode_nr = self.InodeCount;

        // Create a new File struct
        let newfile = File {
            filename: fname.to_string(),
            Data: "".to_string(),
        };

        self.FileMap.insert(cur_inode_nr, newfile.clone());
        self.m_sb.filecount = self.m_sb.filecount + 1;

        // Add this file to the filelist of the curdir
        self.DirMap.get_mut(&self.Path).unwrap().FileList.insert(cur_inode_nr, newfile.filename);
    }

    // Remove the specified directory
    fn remove_dir(&mut self, dname: &str) {
        let mut cur_inode_nr = 0;
        // Check if directory with this name exists
        if !self.DirMap.get(&self.Path).is_none() {
            let findsame = match self.DirMap.get(&self.Path).unwrap().DirList.iter().find(|&s| *s.1 == dname) {
                Some(s) => { cur_inode_nr = *s.0 },
                None => { println!("Cannot find a dir with that name"); return },
            };
        }  

        self.m_sb.dircount = self.m_sb.dircount - 1;
        // Remove the directory from DirMap
        self.DirMap.remove(&cur_inode_nr);
        // Remove the directory from the current DirList
        self.DirMap.get_mut(&self.Path).unwrap().DirList.remove(&cur_inode_nr);
    }

    // Remove the specified file
    fn remove_file(&mut self, fname: &str) {
        let mut cur_inode_nr = 0;
        // Check if file with this name exists
        if !self.DirMap.get(&self.Path).is_none() {
            let findsame = match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == fname) {
                Some(s) => { cur_inode_nr = *s.0 },
                None => { println!("Cannot find a file with that name"); return },
            };
        } 

        self.m_sb.filecount = self.m_sb.filecount - 1;
        // Remove the file from global FileMap
        self.FileMap.remove(&cur_inode_nr);
        // Remove the file from the current FileList inside the current dir
        self.DirMap.get_mut(&self.Path).unwrap().FileList.remove(&cur_inode_nr);
    }

    // Get the current path as a string from the current filesystem
    fn get_path(&self) -> String {  
        let mut curpath = "".to_string();
        let mut cur = self.Path;

        if !self.DirMap.get(&cur).is_none() {
            if !self.DirMap.get(&cur).unwrap().parentdir.is_none() {
                // Traverse through the dirmap and append current dir to current path
                while cur != 1 { // This has to be set to root -> 1 = root
                    curpath = self.DirMap.get(&cur).unwrap().dirname.clone() + "/" + &curpath;
                    cur = self.DirMap.get(&cur).unwrap().parentdir.unwrap();
                }   
            } else { return curpath }
        }
        
        if self.m_sb.device == "SVFS" { return "/".to_owned() + &curpath }
        self.m_sb.device.to_string() + "/" + &curpath
    }

    // List all the files and directories in the current directory
    fn ls(&self) {
        if !self.DirMap.get(&self.Path).is_none() {
            for val in self.DirMap.get(&self.Path).unwrap().DirList.iter() {
                println!("d - {}", val.1);
            }
            for val in self.DirMap.get(&self.Path).unwrap().FileList.iter() {
                println!("f - {}", val.1);
            }
        }  
    }

    // Change into the specified directory
    fn cd(&mut self, dname: &str) -> bool {
        let mut found = false;

        // Go back a dir
        if dname == ".." {
            if !self.DirMap.get(&self.Path).is_none() {
                if !self.DirMap.get(&self.Path).unwrap().parentdir.is_none() {
                    self.Path = self.DirMap.get(&self.Path).unwrap().parentdir.unwrap();
                    // Path <= 0 should never be reached! So this is just a hacky way of preventing that
                    if self.Path <= 0 { self.Path = 1; }
                    return false
                } else { return false }
            }

        }

        // Find the Dir with dname in the Filelist of the current dir
        if !self.DirMap.get(&self.Path).is_none() {
            if let Some(str) = self.DirMap.get(&self.Path).unwrap().DirList.iter().find(|&s| *s.1 == dname) {
                found = true;
            }
        }

        // Search through the dirmap for the correct inode and assign PATH to it
        if found { 
            let getpath = match self.DirMap.get(&self.Path).unwrap().DirList.iter().find(|&s| *s.1 == dname) {
                Some(x) => *x.0,
                None => -99
            };
            if getpath == -99 { println!("Couldn't find {} in dirlist.", dname); return false }
            // check for new fs
            if getpath <= 0 { 
                return true
            }

            self.Path = getpath;
        }
        else if !found { println!("Couldn't find {} in the current dir.", dname); return false } 
        return false
    }
    
    fn get_sb_info(&self) {
        self.m_sb.get_info();
    }
    fn get_sb_device(&self) -> String {
        self.m_sb.get_device()
    }
    fn get_path_inode(&self) -> i32 {
        self.Path
    }
    fn get_parent(&self) -> Option<i32> {
        self.ParentFS
    }
    fn speedtest(&mut self) {
        self.mkdir("speedtest", false);
        self.cd("speedtest");
        // Create 1000 Files
        let fname = "test";
        
        let mut curtime: u64 = 0;
        unsafe { 
            curtime = _rdtsc(); 
        }

        for n in 1..1000 {
            let curval = n.to_string();
            self.create_file(&(fname.to_owned() + &curval));
        }
        
        unsafe { println!("Created 1000 empty files in {} cycles", _rdtsc()-curtime); }

        self.cd("..");
    }
    // If for some reason an allocation error is thrown -> lower the amount of for loops (e.g. 1..13)
    fn speedtest2(&mut self) {
        // Crate long string
        let mut longstring = "ABCDEFGHIJ".to_string();
        for _i in 1..14 {
            longstring = longstring.to_owned() + &longstring;
        }

        let mut curtime: u64 = 0;
        unsafe { 
            curtime = _rdtsc(); 
        }

        // Write long string into a file
        self.create_file("speed");
        longstring = "speed ".to_string() + &longstring;
        self.write_file(&longstring);

        unsafe { println!("File with a string of size {} created in {} cycles", longstring.len(), _rdtsc()-curtime); }
    }
}

// --- Functions ---

pub fn execute_cmd(msg: String) {
    let fullstr = msg.trim_end();
    let mut cmd = "";
    let mut name = "";
    let mut curfs = 0;

    unsafe{ 
        curfs = CURFS as usize;
    }

    if fullstr.chars().count()>9 // speedtest2
    {
        // Speed Test 2
        cmd = &fullstr[..10];
        if cmd == "speedtest2" { FSLIST.lock().get_mut(curfs).unwrap().speedtest2(); return }
    }

    if fullstr.chars().count()>8 // speedtest
    {
        // Speed Test
        cmd = &fullstr[..9];
        if cmd == "speedtest" { FSLIST.lock().get_mut(curfs).unwrap().speedtest(); return }
    }

    if fullstr.chars().count()>6 // mkdir, touch, write,
    {
        cmd = &fullstr[..5];
        name = &fullstr[6..];
        if cmd == "mkdir" { 
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.mkdir(name, false),
                None => { println!("Could not get current filesystem!"); return}
            };
            return 
        }

        if cmd == "touch" { 
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.create_file(name),
                None => { println!("Could not get current filesystem!"); return}
            };
            return
        }

        if cmd == "write" {
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.write_file(name),
                None => { println!("Could not get current filesystem!"); return}
            };
            return
        }

        cmd = &fullstr[..7];
        if cmd == "getinfo" { FSLIST.lock().get(curfs).unwrap().get_sb_info(); return }
        if cmd == "getpath" { println!("Current Path: {}", FSLIST.lock().get(curfs).unwrap().get_path_inode()); return }
    }

    if fullstr.chars().count()>5 { // read
        cmd = &fullstr[..4];
        name = &fullstr[5..];

        if cmd == "read" {
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.read_file(name),
                None => { println!("Could not get current filesystem!"); return}
            };
            return
        }
    }

    if fullstr.chars().count()>3{ // rm
        cmd = &fullstr[..3];
        name = &fullstr[4..];
        if cmd == "rmf" {
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.remove_file(name),
                None => { println!("Could not get current filesystem!"); return}
            };
            return
        }
        if cmd == "rmd" {
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.remove_dir(name),
                None => { println!("Could not get current filesystem!"); return}
            };
            return
        }
    }

    let mut foundfs = false;

    if fullstr.chars().count()>2 // cd
    {
        cmd = &fullstr[..2];
        name = &fullstr[3..];
        if cmd == "cd" { 
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => foundfs = fs.cd(name),
                None => { println!("Could not get current filesystem!"); return}
            };
        }

        // Switch FS if found a fs
        if foundfs { getcurfs(name); return };
    }
    
    if fullstr.chars().count()>1 { // ls
        cmd = &fullstr[..2];
        if cmd == "ls" { 
            match &mut FSLIST.lock().get_mut(curfs) {
                Some(fs) => fs.ls(),
                None => { println!("Could not get current filesystem!"); return}
            };
            return 
        }
    }
}

// Get the full path as a string while traversing through every filesystem back to root
pub fn getcurpath() -> String {
    let mut curpath = "".to_string();

    let mut curfsnr = 0;

    unsafe{ 
        curfsnr = CURFS as usize;
    }

    if !FSLIST.lock().get(curfsnr).is_none(){
        match FSLIST.lock().get(curfsnr) {
            Some(x) => { curpath = x.get_path() + &curpath; },
            None => ()
        };

        // check for parent fs
        // This is a very hacky way of doing things but it seems to work
        if !FSLIST.lock().get(curfsnr).unwrap().get_parent().is_none() {
            curfsnr = FSLIST.lock().get(curfsnr).unwrap().get_parent().unwrap() as usize;
            match FSLIST.lock().get(curfsnr) {
                Some(x) => { curpath = x.get_path() + &curpath; },
                None => ()
            };
        }
    }

    curpath
}

// This switches to the correct filesystem
pub fn getcurfs(dname: &str) {
    let filesystem = match FSLIST.lock().iter().position(|x| x.get_sb_device() == dname) {
        Some(x) => unsafe { CURFS = x as i32; println!("Switched to {}", dname); },
        None => ()
    };
}

// --- Init ---

pub fn init_vfs() {
    // Create a superblock for the svfs itself
    let sb = Superblock {
        device: "SVFS".to_string(),
        filecount: 0,
        dircount: -1, // first directory is root
    };

    sb.get_info();

    let mut svfs = FileSystem {
        m_sb: sb,
        InodeCount: 0,
        MountCount: 0,
        DirMap: BTreeMap::new(),
        FileMap: BTreeMap::new(),
        Path: 0,
        ParentFS: None,
    };

    // Create root
    svfs.mkdir("/", false);
    // Set path to root
    // This is important or else the system crashes!
    svfs.Path = 1;

    // Mount it
    let getpos = FSLIST.lock().len();
    FSLIST.lock().insert(getpos, Box::new(svfs));
    unsafe { CURFS = getpos as i32; }
}

