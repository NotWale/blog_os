use crate::alloc::string::ToString;
use alloc::string::String;
use alloc::format;
use alloc::collections::btree_map::BTreeMap;
use alloc::boxed::Box;
use crate::{print, println};
use crate::fs;
use crate::fs::fs::Operations;
use crate::fs::svfs::Superblock;
use crate::fs::svfs::Dir;
use crate::fs::svfs::File;
use core::convert::TryInto;
use crate::alloc::borrow::ToOwned;
use crate::allocator;
use crate::task::executor::UPTIME;
use crate::fs::svfs::CURFS;
use crate::fs::svfs::FSLIST;
use crate::vga_buffer;
use core::sync::atomic::{AtomicBool, Ordering};

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

pub fn mount_fs(fs: FileSystem){
    let curdevice = fs.m_sb.device.clone();
    let mut curfs = 0;
    unsafe{ 
        curfs = CURFS as usize;
    }

    // Make a new directory with the name of the fs    
    let name = &fs.m_sb.device;
    match &mut FSLIST.lock().get_mut(curfs) {
        Some(fs) => fs.mkdir(name, true), // true = its a directory that contains a new fs
        None => { println!("Could not get current filesystem!"); return}
    };

    // Mount it
    let getpos = FSLIST.lock().len();
    FSLIST.lock().insert(getpos, Box::new(fs));
}

pub fn init_procfs() {
    let proc_sb = Superblock {
        device: "proc".to_string(),
        filecount: 0,
        dircount: -1, // first directory is root
    };

    let mut curfs = 0;
    unsafe{ 
        curfs = fs::svfs::CURFS as usize;
    }

    let mut procfs = FileSystem {
        m_sb: proc_sb,
        InodeCount: 0,
        MountCount: 0,
        DirMap: BTreeMap::new(),
        FileMap: BTreeMap::new(),
        Path: 0,
        ParentFS: Some(curfs.try_into().unwrap()),
    };

    // Create Files and Dirs inside proc and return to root
    procfs.mkdir("proc", false);
    procfs.Path = 1;
    procfs.create_file_o("meminfo");
    procfs.create_file_o("mounts");
    procfs.create_file_o("uptime");
    procfs.create_file_o("color");
    procfs.write_file_o("color Black Yellow");
    procfs.mkdir("sys", false);
    procfs.Path = 5; // sys
    procfs.mkdir("fs", false);
    procfs.Path = 6; // fs
    procfs.create_file_o("inode-state");

    // Set path to root
    procfs.Path = 1;

    mount_fs(procfs);
}

trait Superuser {
    fn write_file_o(&mut self, msg: &str);
    fn create_file_o(&mut self, fname: &str);
}

// File Operation(s) only performed by the system itself
impl Superuser for FileSystem {
    // Create a File from system
    fn create_file_o(&mut self, fname: &str) {
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
    // Write data into a file from system
    fn write_file_o(&mut self, msg: &str) {
        let mut split = msg.splitn(2, ' ');
        // Split up string into two 
        let fname = split.next();  
        let content = split.next();
        let mut curinode = 0;

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
}

// Standard Filesystem Operations
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
        if fscheck { self.MountCount = self.MountCount-1; }
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
    fn read_file(&mut self, name: &str) {
        if name == "meminfo" {
            self.write_file_o(&(name.to_owned() + 
            " Heap Size: " + &allocator::HEAP_SIZE.to_string() +
            " bytes\nHeap Start: 0x" +  &(format!("{:X}", &allocator::HEAP_START)) ),);
        }

        if name == "uptime" {
            unsafe {
                self.write_file_o(&(name.to_owned() + 
                " System running time: " + &UPTIME.to_string() + " seconds."));
            }
        }

        if name == "mounts" {
            self.write_file_o(&(name.to_owned() + 
            " Currently mounted filesystem: " + &self.m_sb.device));
        }

        if name == "inode-state" {
            let mut list = "".to_string();
            for (inode, dir) in &self.DirMap {
                list = list.to_owned() + &inode.to_string() + "       " + &dir.dirname + "\n";
            }
            for (inode, file) in &self.FileMap {
                list = list.to_owned() + &inode.to_string() + "       " + &file.filename + "\n";
            }

            self.write_file_o(&(name.to_owned() + 
            " InodeCount: " + &self.InodeCount.to_string() +
            "\nCurrent Path: " + &self.Path.to_string() +
            "\nInode   Dirname/Filename\n" + &list));
        }

        let mut curinode = 0;
        // Check if file with this name exists
        if !self.DirMap.get(&self.Path).is_none() {
            match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == name) {
                Some(s) => { curinode = *s.0; },
                None => { println!("Cannot find {} inside the current dir.", name); return },
            };
        } 

        // Search for file inside the FileMap and read contents from it
        match self.FileMap.get_mut(&curinode) {
            Some(s) => { println!("{}", s.Data); },
            None => { println!("Cannot find {} in the global FileMap.", name); return },
        };
    }

    // Write data into a file
    fn write_file(&mut self, name: &str) {
        let mut split = name.splitn(2, ' ');
        // Split up string into two 
        let fname = split.next();  
        let content = split.next();
        let mut curinode = 0;
        if content.is_none() { println!("Usage: write <filename> <message>"); return }

        if fname.unwrap() != "color" {
            // Check if file with this name exists
            if !self.DirMap.get(&self.Path).is_none() {
                match self.DirMap.get(&self.Path).unwrap().FileList.iter().find(|&s| *s.1 == fname.unwrap()) {
                    Some(s) => { curinode = *s.0; },
                    None => { println!("Cannot find {} inside the current dir.", fname.unwrap()); return },
                };
            } 

            // Search for file inside the FileMap and refuse to write contents to it
            match self.FileMap.get_mut(&curinode) {
                Some(s) => { println!("This file is read-only!"); return },
                None => { println!("Cannot find {} in the global FileMap.", fname.unwrap()); return },
            };
        }

        // color specific
        let s = name.clone();
        let mut split = s.splitn(3, ' ');
        // Split up string into two 
        let fnam = split.next(); 
        let fgcolor = split.next();
        let bgcolor = split.next();

        if !fgcolor.is_none(){
            if !bgcolor.is_none() {
                self.write_file_o(&(name.to_owned() + " " + fgcolor.unwrap() + " " + bgcolor.unwrap()));
                let guard1 = *vga_buffer::FGCOLOR.lock() = fgcolor.unwrap().to_string().to_owned();
                let guard2 = *vga_buffer::BGCOLOR.lock() = bgcolor.unwrap().to_string().to_owned();
            }
        } 

        vga_buffer::SWITCHCOLOR.store(true, Ordering::Relaxed);
    }

    fn create_file(&mut self, fname: &str) {
        println!("Cannot add files in this filesystem!");
    }

    fn remove_dir(&mut self, dname: &str) {
        println!("Cannot remove directories in this filesystem!");
    }

    fn remove_file(&mut self, fname: &str) {
        println!("Cannot remove files in this filesystem!");
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
            // If root -> switch to parent fs
            if self.Path == 1 {
                if !self.ParentFS.is_none(){
                    unsafe { CURFS = self.ParentFS.unwrap(); }
                    return false
                }
                return false
            }
            // else
            if !self.DirMap.get(&self.Path).is_none() {
                if !self.DirMap.get(&self.Path).unwrap().parentdir.is_none() {
                    self.Path = self.DirMap.get(&self.Path).unwrap().parentdir.unwrap();
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
        let mut timeatstart = 0;

        unsafe {
            timeatstart = UPTIME;
        }
        
        for n in 1..1000 {
            let curval = n.to_string();
            self.create_file_o(&(fname.to_owned() + &curval));
        }

        unsafe { println!("Created 500 empty files in {} seconds", (UPTIME-timeatstart).to_string()); timeatstart = UPTIME; }

        self.cd("..");
    }
    fn speedtest2(&mut self) {
        // Crate long string
        let mut longstring = "test".to_string();
        for _i in 1..100 {
            longstring = longstring.to_owned() + &longstring;
        }

        let mut timeatstart = 0;

        unsafe {
            timeatstart = UPTIME;
        }

        // Write long string into a file
        self.create_file_o("speed");
        longstring = "speed ".to_string() + &longstring;
        self.write_file_o(&longstring);

        unsafe { println!("File with a string of size 400 created in {} seconds", (UPTIME-timeatstart).to_string()); }
    }
}