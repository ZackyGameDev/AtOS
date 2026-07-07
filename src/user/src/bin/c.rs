#![no_std]
#![no_main]

use user::{entry, println, stdlib::syscalls::{fork, wait}};

fn main() {
    println!("hello this code is running in the c program!").unwrap();

    let mut x = 1;
    println!("x = {}", x).unwrap();
    x += 1;
    println!("x = {}", x).unwrap();

    for i in 0..20 {
        println!("c program is working, iteration {}", i).unwrap();
    }

    println!("c program will now fork and wait for the child to finish.").unwrap();
    match fork() {
        Ok(fc) => {
            if fc == 0 {
                println!("i'm c child! i finished early!").unwrap();
                panic!("Hi im c child. I just paniced here to check if my parent receives my exit code 1.");
            } else {
                println!("I'm c parent, now working").unwrap();
                for i in 0..15 {
                    println!("c parent working {}", i).unwrap();
                }
                match wait(None) {
                    Ok((pid, exit_code)) => {
                        println!("c parent waited for child process with pid {} to finish, it exited with code {}", pid, exit_code).unwrap();
                    }
                    Err(e) => {
                        println!("c parent program failed to wait for child process: {}", e).unwrap();
                    }
                }
                println!("c parent will now exit.").unwrap();
            }
        }
        Err(_) => {
            println!("fork() failed!").unwrap();
        }
    }
}


entry!(main);