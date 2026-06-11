#![no_std]
#![no_main]

mod kernel;

use kernel::utils::get_current_el;
use kernel::peripherals::Uart;
use kernel::timer::PhysicalTimer;
use kernel::interrupts::Interrupts;
use kernel::processes::{load_process};

// pub const DEBUG_PRINTS_ENABLED: bool = true;  
pub const DEBUG_PRINTS_ENABLED: bool = false;  

// this is to read from the linker-- the end of kernel in memory and top of the stack. 
unsafe extern "C" {
    unsafe static _kernel_top: u8;
    unsafe static _stack_top: u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn _rust_main() -> ! {
    Uart.init();
    PhysicalTimer::init_irq();
    Interrupts::daif_unmask_all();
    
    println!("Welcome to, AtOS.").unwrap();
    println!("Current EL is: EL{}", get_current_el()).unwrap();
    
    let kernel_end_addr = unsafe { &_kernel_top as *const u8 as usize };
    let stack_top_addr = unsafe { &_stack_top as *const u8 as usize };
    println!("Kernel ends at: {:#x}", kernel_end_addr).unwrap();
    println!("Kernel size: {} KB", kernel_end_addr / 1024).unwrap();
    println!("Stack top at: {:#x}", stack_top_addr).unwrap();

    println!("Kernel done. Loading two processes for demonstration.").unwrap();

    let frq = PhysicalTimer::read_frq();

    println!("Physical Timer frequency: {} Hz", frq).unwrap();

    let process_a_image: &'static [u8] = include_bytes!("user/init.bin");
    let process_b_image: &'static [u8] = include_bytes!("user/b.bin");

    load_process("init", 0, process_a_image, 0x200000, 0x2002e0);
    load_process("process b", 0, process_b_image, 0x500000, 0x500500);

    println!("Starting the scheduler!").unwrap();
    Scheduler::start();

    loop { core::hint::spin_loop(); }
}

// usually, an os has a root user process which spawns all the other user processes. 
// the scheduler should always have at least one process to switch to.
// But if there are no more processes in the process table, one could imagine
// that a scenario like that would only occur when the user exited out of all
// processes. in that case this is the function that the scheduler will call
// if it sees there are no more processes left to schedule.
pub fn the_end() -> ! {
    println!("All processes have completed/terminated.").unwrap();
    println!("There is nothing left to do. You may power off your device now.").unwrap();
    PhysicalTimer::set_seconds(1);
    PhysicalTimer::disable();
    loop { core::hint::spin_loop(); }
}

use core::panic::PanicInfo;

use crate::kernel::scheduler::Scheduler;
#[panic_handler]
fn panic(panic: &PanicInfo) -> ! {
    println!("Kernel Panicked!: {}", panic).unwrap();
    loop {core::hint::spin_loop(); }
}
