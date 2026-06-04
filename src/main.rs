#![no_std]
#![no_main]

mod kernel;

use kernel::utils::get_current_el;
use kernel::peripherals::Uart;

// this is to read from the linker-- the end of kernel in memory and top of the stack. 
unsafe extern "C" {
    unsafe static _kernel_top: u8;
    unsafe static _stack_top: u8;
}

fn delay(mut count: u64) {
    while count > 0 {
        unsafe { core::arch::asm!("nop"); }
        count -= 1;
    }
}

// of course, i will later add a scheduler and stuff
// for now, im just manually loading a single process
// to a hardcoded location in memory and jumping to it
// for testing syscalls and el transitions and the sort.
fn load_and_run_init_process() {
    const INIT_PROCESS_IMAGE: &[u8] = include_bytes!("user/build/init.bin");
    const INIT_PROCESS_ADDR: usize = 0x200000;
    unsafe {
        core::ptr::copy_nonoverlapping(
            INIT_PROCESS_IMAGE.as_ptr(),
            INIT_PROCESS_ADDR as *mut u8,
            INIT_PROCESS_IMAGE.len(),
        );
    }

    // i got this entry point from the compiled init elf.
    const ENTRY_POINT: usize = 0x200064;
    const STACK_TOP: usize = (INIT_PROCESS_ADDR + INIT_PROCESS_IMAGE.len() + 0x4000) & !0xf; // 16 byte aligned stack top 
    // right now i have just hardcoded some stack pointer for EL0

    enter_user(ENTRY_POINT, STACK_TOP);
}
fn enter_user(entry_point: usize, stack_top: usize) {
    unsafe {
        core::arch::asm!(
            "
            msr sp_el0, {stack}
            msr elr_el1, {entry}
    
            mov x0, xzr
            msr spsr_el1, x0
    
            eret
            ",
            stack = in(reg) stack_top,
            entry = in(reg) entry_point,
            options(noreturn)
        );   
    }
}


#[unsafe(no_mangle)]
pub extern "C" fn _rust_main() -> ! {
    Uart.init();
    
    println!("Welcome to, AtOS.").unwrap();
    println!("Current EL is: EL{}", get_current_el()).unwrap();
    
    let kernel_end_addr = unsafe { &_kernel_top as *const u8 as usize };
    let stack_top_addr = unsafe { &_stack_top as *const u8 as usize };
    println!("Kernel ends at: {:#x}", kernel_end_addr).unwrap();
    println!("Kernel size: {} KB", kernel_end_addr / 1024).unwrap();
    println!("Stack top at: {:#x}", stack_top_addr).unwrap();

//    load_and_run_init_process();

    loop {
	let byte = Uart.read_byte();

	if byte == 127 || byte == 8 {
	    Uart.write_byte(8); // Move cursor left by one space
	    Uart.write_byte(32); // Print a space (overwriting the old character, effectively hiding it)
	    Uart.write_byte(8); // Move the cursor left again so that it stays at the position it is meant to stay in
	} else if byte == b'\r' || byte == b'\n' {
	    let _ = println!();
	} else  {
	    let _ = print!("{}", byte as char);
	}
    }
    
    // loop {
    //     println!("The end. Make sure to power off your RPi before disconnecting it :)").unwrap();
    //     loop {
    //         delay(1_000_000);
    //     }
    // }
}

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_panic: &PanicInfo) -> ! {
    println!("Some error happened...").unwrap();
    loop {delay(10_000_000)}
}
