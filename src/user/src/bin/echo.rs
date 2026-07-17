#![no_std]
#![no_main]

use user::{entry_args, print, println};

pub fn main(args: &[&str]) {
    for (i, arg) in args.iter().enumerate().skip(1) {
        if i > 1 {
            print!(" ").unwrap();
        }
        print!("{}", arg).unwrap();
    }

    println!("").unwrap();
}

entry_args!(main);
