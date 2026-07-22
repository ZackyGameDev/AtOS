#![no_std]
#![no_main]

use user::{println, entry, stdlib::{syscalls::{exit, wait, get_p_info}, syswraps::spawn}};

fn main() {
    
    if let Ok((pid, _)) = get_p_info() {
        if pid != 1 {
            println!("spawning 'init' program from user space is not permitted! 'init' program is quitting...").unwrap();
            exit(1);
        }
    }

    spawn("shell", &[]).expect("failed to spawn shell");

    loop {
        match wait(None) {
            Ok(_) => continue,
            Err(_) => exit(0), // no more children then exit.
        }
    }
}

entry!(main);

