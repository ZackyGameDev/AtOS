// process B just for the sake of testing the scheduler with init and process B

#![no_std]
#![no_main]

use user::{entry, println};

fn main() {
    println!("hello this code is running in process B!").unwrap();
    
    let mut x = 1;
    println!("x = {}", x).unwrap();
    x += 1;
    println!("x = {}", x).unwrap();
    
    for i in 0..100 {
        println!("process B is working, iteration {}", i).unwrap();
    }

    println!("process B is done working, it will now exit.").unwrap();
    // loop {println!("This is b looping forever!").unwrap(); core::hint::spin_loop();}
}

entry!(main);