#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![feature(str_split_as_str)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]
#[allow(dead_code)]

extern crate alloc;

use blog_os::println;
use blog_os::task::{executor::Executor, keyboard, Task};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use blog_os::allocator;
    use blog_os::fs;
    use blog_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Hello World{}", "!");
    blog_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    //Filesystem init
    fs::svfs::init_vfs();
    //ProcFS Mount
    fs::procfs::init_procfs();  
    //testFS Mount for testing purposes
    fs::sysfs::init_sysfs(); 

    println!("Commands:");
    println!("mkdir <dirname> - Create new directory");
    println!("touch <filename> - Create new file");
    println!("read <filename> - Read a file");
    println!("write <filename> <text> - Write to a file");
    println!("cd <dirname> - Change directory");
    println!("ls - Display all files and directories in the current folder");
    println!("rmd <dirname> - Delete directory");
    println!("rmf <filename> - Delete file");
    println!("getinfo - Display info about current filesystem");
    println!("getpath - Show path inode number");
    println!("You can change the text and background color in proc/color");
  
    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    //executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
