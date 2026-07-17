#![no_std]
#![no_main]

mod kernel;

use kernel::io::KERNEL_IO;
use kernel::timer::PhysicalTimer;
use kernel::interrupts::Interrupts;
use kernel::paging::PageAllocator;
use kernel::scheduler::Scheduler;
use kernel::filesystem::FileSystem;
use kernel::paging::{total_memory, available_memory};

// pub const DEBUG_PRINTS_ENABLED_MMU: bool = true;
pub static mut DEBUG_PRINTS_ENABLED_MMU: bool = false;

// pub const DEBUG_PRINTS_ENABLED: bool = true;
pub const DEBUG_PRINTS_ENABLED: bool = false;

// this is to read from the linker-- the end of kernel in memory and top of the stack.
unsafe extern "C" {
    unsafe static _kernel_top: u8;
    unsafe static _stack_top: u8;
}

fn show_welcome_ascii() {
    const INTRO: &str = 
        include_str!(concat!(env!("OUT_DIR"), "/atos_intro_generated.txt")); // obtained from build.rs

    println!("{INTRO}");
    
}

#[unsafe(no_mangle)]
pub extern "C" fn _rust_main() -> ! {
    
    // let kernel_end_addr = unsafe { &_kernel_top as *const u8 as usize };
    let stack_top_addr = unsafe { &_stack_top as *const u8 as usize };
    
    KERNEL_IO.init();
    PhysicalTimer::init_irq();
    Interrupts::daif_unmask_all();
    PageAllocator::init_frames(stack_top_addr + 0x2000, ttbr1_to_va!(0x3EFFE000)); // eye balled end of usable RAM space.
    // let kernel_root_sp = KernelStack::alloc_stack(0); // this is currently commented because it ends up being unused. But 
                                                        // there may be need for it later. so it is still here as a hint.
    
    show_welcome_ascii();
    println!("Total available memory: {} MB", available_memory() / (1024 * 1024));
    println!("Total usable memory: {} MB", total_memory() / (1024 * 1024));
    println!("\nWelcome, to AtOS...\n");

    // since fork and exec syscalls have been implemented,
    // init process itself can spawn other processes needed. 
    // so we only need to 
    FileSystem::run_executable("init", 0, &["init", "hi there", "this is test of args"]).unwrap();

    dprintln!("[main] Starting the scheduler!");
    Scheduler::start();

    // the_end() // this should never be reached, but still keeping it here if preceeding code is changed during testings
}

// usually, an os has a root user process which spawns all the other user processes.
// the scheduler should always have at least one process to switch to.
// But if there are no more processes in the process table, one could imagine
// that a scenario like that would only occur when the user exited out of all
// processes. in that case this is the function that the scheduler will call
// if it sees there are no more processes left to schedule.
pub fn the_end() -> ! {
    println!("----------THE END-----------");
    println!("All processes have completed/terminated.");
    println!("There is nothing left to do. You may power off your device now.");
    println!("----------------------------");
    PhysicalTimer::set_seconds(1);
    PhysicalTimer::disable();
    loop { core::hint::spin_loop(); }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(panic: &PanicInfo) -> ! {
    println!("--------KERNEL PANIC--------");
    if let Some(location) = panic.location() {
        println!("Location: {}:{}:{}", location.file(), location.line(), location.column());
    } else {
        println!("Location: Unknown location");
    }
    println!("Message:  {}", panic.message());
    println!("----------------------------");
    loop {core::hint::spin_loop(); }
}
