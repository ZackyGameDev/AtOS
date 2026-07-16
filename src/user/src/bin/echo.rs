#![no_std]
#![no_main]

use user::{entry, print, println};
use user::stdlib::syscalls::{sys_readline};

pub fn main() {
    println!("-- Input Test --").unwrap();
    println!("Type something: ").unwrap();

    let mut input_buffer = [0u8; 64];
    let bytes_read = sys_readline(&mut input_buffer);
    if bytes_read > 0 {
	if let Ok(user_string) = core::str::from_utf8(&input_buffer[..bytes_read]) {
	    print!("Kernel said: {}", user_string).unwrap();
	} else {
            println!("Error: Input data was invalid utf-8 (somehow)").unwrap();
        }
    } else {
	println!("Error: Read 0 bytes from stdin").unwrap();
    }

    // loop {core::hint::spin_loop();}
}

entry!(main);
