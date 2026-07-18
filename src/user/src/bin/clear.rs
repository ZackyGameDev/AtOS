#![no_std]
#![no_main]

use user::{entry, print};

pub fn main() {
    // What the codes mean:
    // \x1b[2J  - Clears the screen
    // \x1b[1;1H - Explicitly sets cursor to Row 1, Column 1
    print!("\x1b[2J\x1b[1;1H").unwrap();
}

entry!(main);
