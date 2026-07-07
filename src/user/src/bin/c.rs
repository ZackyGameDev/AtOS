#![no_std]
#![no_main]

use user::{entry, println, stdlib::syscalls::{fork, wait}};

fn main() {
    println!("hello this code is running in the c program!").unwrap();
    for i in 0..20 {
        println!("c program is working, iteration {}", i).unwrap();
    }
    
    println!("c will now fork and wait for the child to finish.").unwrap();

    match fork() {
        Ok(fc) => {
            if fc == 0 {
                println!("c program is the child process.").unwrap();
                for i in 0..100 {
                    println!("c program child is working, iteration {}", i).unwrap();
                }
            } else {
                println!("c program is the parent process, it will now wait for the child to finish.").unwrap();
                wait(Some(fc));
                println!("c program parent is done waiting for the child to finish.").unwrap();
            }
        }
        Err(_) => {
            println!("fork() failed!").unwrap();
        }
    }
}

entry!(main);