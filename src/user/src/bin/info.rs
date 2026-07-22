#![no_std]
#![no_main]

use user::entry;
use user::stdlib::syscalls::show_os_info;

pub fn main() {
    show_os_info();
}

entry!(main);
