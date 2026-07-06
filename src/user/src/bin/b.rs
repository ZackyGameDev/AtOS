// process B just for the sake of testing the scheduler with init and process B

#![no_std]
#![no_main]

use user::{entry, println};
use user::stdlib::syscalls::fork;

fn main() {
    println!("hello this code is running in process B!").unwrap();
    
    println!("Trying to fork in B!").unwrap();

    let fc = fork();
    if fc == 0 {
        println!("fork() returned 0, so this is the child process!").unwrap();

        for i in 0..100 {
            println!("child process is working, iteration {}", i).unwrap();
        }

        println!("process B is done working, it will now exit.").unwrap();

    } else if fc == -1 {
        println!("fork() returned -1, so this is the parent process and the fork failed!").unwrap();
    } else {
        println!("fork() returned {}, so this is the parent process!", fc).unwrap();

        for i in 0..100 {
            println!("parent process is working, iteration {}", i).unwrap();
        }
    }
    // loop {println!("This is b looping forever!").unwrap(); core::hint::spin_loop();}
}

entry!(main);