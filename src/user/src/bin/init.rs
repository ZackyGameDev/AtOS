#![no_std]
#![no_main]

use user::{entry, stdlib::{syscalls::{exit, wait}, syswraps::spawn}};

fn main() {
    spawn("shell", &[]).expect("failed to spawn shell");

    loop {
        match wait(None) {
            Ok(_) => continue,
            Err(_) => exit(0), // no more children then exit.
        }
    }
}

entry!(main);