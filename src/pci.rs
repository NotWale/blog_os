extern crate alloc;
use alloc::string::String;
use crate::alloc::string::ToString;
use crate::{print, println};
use x86_64::structures::port::PortWrite;
use x86_64::structures::port::PortRead;
use core::arch::x86_64::_rdtsc;

pub static CFA: u16 = 0xcf8;
pub static CFD: u16 = 0xcfc;

pub fn busScan() {
    for b in 0..1 { // bus
        for d in 0..31 {
            let devID: u32 = readPCI(b,d,0,0);
            if devID != 0 && devID != 4294967295 { // since its an unsigned integer, the number to check for is 4294967295 instead of -1
                let classCode = readPCI(b, d, 0, 8) >> 8;
                print!("PCI device at bus: {}", b);
				println!(", device: {}", d);

                print!("devID={}", alloc::format!("{:#X}", devID >> 8) );
                print!(", vendorID={}", alloc::format!("{:#X}", devID&0xFF) );
                print!(", class code={}", alloc::format!("{:#X}", classCode >> 16) );
                print!(", sub code={}", alloc::format!("{:#X}", (classCode>>8)&0xFF)) ;

                let irq = readPCI(b, d, 0, 0x3C);
                println!(", irq={}", alloc::format!("{:#X}", irq&0xF) );
            }
        }
    }
}

pub fn busScan_r() -> String {
    // Time measurement
    let mut curtime: u64 = 0;
    unsafe { 
            curtime = _rdtsc(); 
    }

    let mut full: String = "".to_string();

    for b in 0..1 { // bus
        for d in 0..31 {
            let devID: u32 = readPCI(b,d,0,0);
            if devID != 0 && devID != 4294967295 { // since its an unsigned integer, the number to check for is 4294967295 instead of -1
                let classCode = readPCI(b, d, 0, 8) >> 8;
                full = full + "PCI device at bus: " + &b.to_string();
                full = full + ", device: " + &d.to_string();

                full = full + "\ndevID=" + &alloc::format!("{:#X}", devID >> 8);
                full = full + ", vendorID=" + &alloc::format!("{:#X}", devID&0xFF);
                full = full + ", class code=" + &alloc::format!("{:#X}", classCode >> 16);
                full = full + ", sub code=" + &alloc::format!("{:#X}", (classCode>>8)&0xFF);

                let irq = readPCI(b, d, 0, 0x3C);
                full = full + ", irq=" + &alloc::format!("{:#X}", irq&0xF);
                full = full + "\n";
            }
        }
    }
    
    println!("{}", full);
    unsafe { println!("PCI bus scan performed in {} cycles", _rdtsc()-curtime); }
    full
}

pub fn readPCI(bus: u32, device: u32, func: u32, offset: u32) -> u32 {
    let addr: u32 = 0x80000000 |    			// Active Config-Data				 			
			        ((bus    & 0xFF) << 16) |  
					((device & 0x1F) << 11) |
					((func   & 0x07) <<  8) |   
					(offset  & 0xFC);
    unsafe{
        PortWrite::write_to_port(CFA, addr); // output, write operation on port CFA with addr as data  
        let value = PortRead::read_from_port(CFD); // Read Config-Data
        value
    }
}