use alloc::string::String;
use crate::alloc::string::ToString;
use alloc::vec::Vec;
use crate::alloc::borrow::ToOwned;
use alloc::collections::btree_map::BTreeMap;
use alloc::boxed::Box;
use crate::{print, println};
use crate::fs;
use crate::fs::fs::Operations;
use crate::fs::svfs::Superblock;
use core::convert::TryInto;
use crate::pci;
use crate::fs::svfs::CURFS;
use crate::fs::svfs::FSLIST;
use crate::task::executor::UPTIME;
use crate::fs::svfs::File;
use crate::fs::svfs::Dir;

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

pub fn init_sysfs() {
    let sys_sb = Superblock {
        device: "sys".to_string(),
        filecount: 0,
        dircount: -1, // first directory is root
    };

    let mut curfs = 0;
    unsafe{ 
        curfs = fs::svfs::CURFS as usize;
    }

    let mut sysfs = FileSystem {
        m_sb: sys_sb,
        InodeCount: 0,
        MountCount: 0,
        DirMap: BTreeMap::new(),
        FileMap: BTreeMap::new(),
        Path: 0,
        ParentFS: Some(curfs.try_into().unwrap()),
    };

    // Create Files and Dirs inside proc and return to root
    sysfs.mkdir("testfs", false);
    sysfs.Path = 1;
    sysfs.create_file("pci");

    // Set path to root
    sysfs.Path = 1;

    mount_fs(sysfs);
}

impl Operations for FileSystem {
    // Create a directory in the current directory
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
    fn read_file(&mut self, fname: &str) {
        if fname == "pci" {
            let write = pci::busScan_r();
            self.write_file(&(fname.to_owned() + 
            " " + &write) );
            return
        }

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
        if fname == "pci" { println!("Cannot remove pci file!"); return }
        
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
            self.create_file(&(fname.to_owned() + &curval));
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
        self.create_file("speed");
        longstring = "speed ".to_string() + &longstring;
        self.write_file(&longstring);

        unsafe { println!("File with a string of size 400 created in {} seconds", (UPTIME-timeatstart).to_string()); }
    }
}