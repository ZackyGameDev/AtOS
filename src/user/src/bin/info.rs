#![no_std]
#![no_main]

use user::{entry, println};

pub fn main() {
    // IDEA: Have the logo and info printed out here kinda like fastfetch
    println!("AtOS v0");
}

entry!(main);
