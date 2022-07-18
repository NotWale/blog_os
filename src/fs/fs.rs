use alloc::string::String;

// Methods that every filesystem has to implement
pub trait Operations {
    // Create a directory in the current directory
    fn mkdir(&mut self, dname: &str, fscheck: bool);

    // Create a file in the current directory
    fn read_file(&mut self, fname: &str);

    // Write data into a file
    fn write_file(&mut self, msg: &str);

    // Create a File inside the current dir
    fn create_file(&mut self, fname: &str);

    // Remove the specified directory
    fn remove_dir(&mut self, dname: &str);

    // Remove the specified file
    fn remove_file(&mut self, fname: &str);

    // Get the current path as a string from the current filesystem
    fn get_path(&self) -> String;

    // List all the files and directories in the current directory
    fn ls(&self);

    // Change into the specified directory
    fn cd(&mut self, dname: &str) -> bool;

    fn get_sb_info(&self);
    fn get_path_inode(&self) -> i32;
    fn get_parent(&self) -> Option<i32>;
    fn get_sb_device(&self) -> String;
    fn speedtest(&mut self);
    fn speedtest2(&mut self);
}