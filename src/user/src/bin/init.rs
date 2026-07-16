#![no_std]
#![no_main]

use user::{entry};
use user::stdlib::syswraps::spawn;

fn main() {
    // this is the process which tests out the fork and wait syscalls. spawning b and c processes afterwards.
    spawn("a").unwrap();
}


entry!(main);