#![no_std]
#![no_main]

use user::{entry_args, println, stdlib::syscalls::{fork, wait}};
use user::stdlib::syswraps::spawn;

fn main(args: &[&str]) {
    println!("hello this code is running in the 'a' program!").unwrap();
    println!("args passed to 'a' program: {:?}", args).unwrap();

    let mut x = 1;
    println!("x = {}", x).unwrap();
    x += 1;
    println!("x = {}", x).unwrap();

    for i in 0..20 {
        println!("'a' program is working, iteration {}", i).unwrap();
    }

    println!("'a' program will now fork and wait for the child to finish.").unwrap();
    match fork() {
        Ok(fc) => {
            if fc == 0 {
                for i in 0..10 {
                    println!("child working {}", i).unwrap();
                }
                println!("child is done working, it will now exit.").unwrap();
            } else {
                println!("I'm parent, now waiting for child to finish").unwrap();
                match wait(None) {
                    Ok((pid, exit_code)) => {
                        println!("parent waited for child process with pid {} to finish, it exited with code {}", pid, exit_code).unwrap();
                    }
                    Err(e) => {
                        println!("parent program failed to wait for child process: {}", e).unwrap();
                    }
                }
                println!("parent will now exit.").unwrap();
                println!("before exiting, spawning process c for next test.").unwrap();
                spawn("c", &[]).unwrap();
            }
        }
        Err(_) => {
            println!("fork() failed!").unwrap();
        }
    }
}


entry_args!(main);