#![no_std]
#![no_main]

use user::{entry, println};
use user::stdlib::syswraps::spawn;

fn main() {
    println!("init loaded, now spawning process a and echo for testing the scheduler.").unwrap();
    // this is the process which tests out the fork and wait syscalls. spawning b and c processes afterwards.
    spawn("a", &["test"]).unwrap();
    println!("spawned process a, now spawning process echo").unwrap();
    spawn("echo", &[]).unwrap();
}


entry!(main);