// println and print macros, they're basically macros which will take
// the format string and arguments, format them, and then call the _print
// function in peripherals.rs which will write the formatted arguments as typical println and print functions

// this file cannot reference kernel crate, the entire user directory will technically be a separate crate. so the println function here, what it will do is it will take the arguments that the user gives, and then build a string out of it, paste that string somewhere in memory, and the use svc to trap into EL1 while giving the address to the string and the type of syscall required

use core::{fmt, fmt::Write};


/* ~~~ POINTLESS PREMPT TO KERNEL ~~~ */
// This is just for user program to trap into the kernel, without
// necessarily any request.
// I assign the syscall number 0 to this (svc #0)
pub fn trap() {
    unsafe {
        core::arch::asm!("svc #0");
    }
}

/* ~~~ STDIO ~~~ */
// For printing or getting input from the stdio (UART).
// printing is assigned syscall number 1 (svc #1),
// and getting input is assigned syscall number 2 (svc #2)
// \TODO INPUT HANDLING
pub struct Stdout;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe {
            core::arch::asm!(
                "svc #1",
                in("x0") s.as_ptr(),
                in("x1") s.len(),
            );
        }

        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) -> fmt::Result {
    Stdout.write_fmt(args)
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::stdlib::syscalls::_print(
            core::format_args!($($arg)*)
        )
    });
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => ({
        $crate::stdlib::syscalls::_print(
            core::format_args!(
                "{}\n",
                core::format_args!($($arg)*)
            )
        )
    });
}

pub fn sys_readline(buf: &mut [u8]) -> usize {
    let p = buf.as_mut_ptr() as u64;
    let l = buf.len() as u64;
    let mut r: u64;

    unsafe {
        core::arch::asm!("svc #2", // @Todo(tanishqdaiya): Find a better way to implement this. A C way would be to simply have a bunch of defines and a trap function call
                         inout("x0") p => r, // we're using x0 to pass arg and then read back into x0
                         in("x1") l,
                         clobber_abi("C"));
    }

    r as usize
}

/* ~~~ PROCESS CONTROL ~~~ */
// exit is assigned syscall number 3 (svc #3)
pub fn exit(exit_code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "svc #3",
            in("x0") exit_code,
            options(noreturn)
        );
    }
}


// fork is assigned syscall number 4 (svc #4)
pub fn fork() -> i64 {
    let mut r: u64 = 12;

    unsafe {
        core::arch::asm!(
            "svc #4",
            out("x0") r,
            clobber_abi("C")
        );
    }

    r as i64
}