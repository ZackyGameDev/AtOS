#![no_std]
#![no_main]

use user::{entry_args, println};
use user::stdlib::syswraps::spawn;

fn main(args: &[&str]) {
    println!("init process started with {} args: {:?}", args.len(), args).unwrap();
    // this is the process which tests out the fork and wait syscalls. spawning b and c processes afterwards.
    spawn("a", &["test"]).unwrap();
    println!("spawned process a, now spawning process echo").unwrap();
    spawn("echo", &[]).unwrap();
}


entry_args!(main);