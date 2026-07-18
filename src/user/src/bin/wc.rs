#![no_std]
#![no_main]

use user::{entry, println};
use user::stdlib::syscalls::sys_readline;

pub fn main() {
    let mut buf = [0u8; 1024];

    let n = sys_readline(&mut buf);

    let mut lines = 0;
    let mut words = 0;
    let bytes = n;
    let mut in_word = false;

    for &c in &buf[..n] {
        if c == b'\n' {
            lines += 1;
        }

        if c.is_ascii_whitespace() {
            in_word = false;
        } else if !in_word {
            words += 1;
            in_word = true;
        }
    }

    println!("{} {} {}", lines, words, bytes).unwrap();
}

entry!(main);
