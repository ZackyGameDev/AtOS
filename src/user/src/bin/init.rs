#![no_std]
#![no_main]

use user::{entry, println};

fn main() {
    println!("hello this code is running in the init program!").unwrap();

    let mut x = 1;
    println!("x = {}", x).unwrap();
    x += 1;
    println!("x = {}", x).unwrap();

    for i in 0..100 {
        println!("init program is working, iteration {}", i).unwrap();
    }
    
    println!("init program is done working, it will now exit.").unwrap();
}


entry!(main);