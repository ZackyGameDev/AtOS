#![no_std]
#![no_main]

use user::entry;
use user::stdlib::syscalls::show_os_info;

pub fn main() {
    // IDEA: Have the logo and info printed out here kinda like fastfetch
    show_os_info();
}

entry!(main);
