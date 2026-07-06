// process B just for the sake of testing the scheduler with init and process B

#![no_std]
#![no_main]

use user::{entry, println};
use user::stdlib::syscalls::fork;

fn parent_do() {
    println!("This is the parent process doing work!").unwrap();

    for i in 0..20 {
        println!("parent process is working, iteration {}", i).unwrap();
    }
    
    println!("parent process is done working, it will now exit.").unwrap();
}

fn child_do() {
    println!("This is the child process doing work!").unwrap();

    for i in 0..20 {
        println!("child process is working, iteration {}", i).unwrap();
    }
    println!("child process is done working, it will now exit.").unwrap();
}

fn main() {
    println!("hello this code is running in process B!").unwrap();
    
    println!("Trying to fork in B!").unwrap();

    let fc = fork();
    if fc == 0 {
        child_do();
    } else if fc == -1 {
        println!("fork() returned -1, so this is the parent process and the fork failed!").unwrap();
    } else {
        parent_do();
    }
}

entry!(main);