#![no_std]
#![no_main]

mod kernel;

use kernel::utils::get_current_el;
use kernel::peripherals::Uart;
use kernel::timer::PhysicalTimer;
use kernel::interrupts::Interrupts;
use kernel::processes::{load_process, Process, ProcessState};
use kernel::spinlock::Spinlock;
use kernel::mutex::Mutex;

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

    // ===================
    // BEGIN Spinlock Test
    // ===================
    static LOCK_A: Spinlock = Spinlock::new("main_lock_a");
    static LOCK_B: Spinlock = Spinlock::new("main_lock_b");

    // Test #1
    println!("-> Verifying nesting rules").unwrap();
    Interrupts::irq_enable();
    assert!(Interrupts::irq_enabled(), "Setup failure: IRQs should be enabled");

    LOCK_A.acquire();
    assert!(!Interrupts::irq_enabled(), "Error: lock_a did not mask hardware IRQs");

    LOCK_B.acquire();
    assert!(!Interrupts::irq_enabled(), "Error: lock_b leaked hardware IRQs");
    LOCK_B.release();

    // Lock B dropped but lock A still held
    assert!(!Interrupts::irq_enabled(), "CRITICAL FAULT: Nested lock release leaked interrupts early!");
    LOCK_A.release();

    assert!(Interrupts::irq_enabled(), "Error: Outer lock release failed to restore active IRQs");
    println!("   [PASSED]").unwrap();

    // Test #2
    println!("-> Verifying original rules...").unwrap();
    Interrupts::irq_disable();
    assert!(!Interrupts::irq_enabled());

    LOCK_A.acquire();
    LOCK_A.release();

    // The relase must not force interrupts to be on if they were off initially
    assert!(!Interrupts::irq_enabled(), "CRITICAL FAULT: Lock forced IRQs active when they were originally masked!");

    // Restore to default enabled.
    Interrupts::irq_enable();
    println!("   [PASSED]").unwrap();


    // Test #3
    // Flip this boolean parameter manually to true to test lock's crash loop
    let run_destructive_check = false;
    if run_destructive_check {
        println!("-> Verifying single-core recursive deadlock safety trap...").unwrap();
        LOCK_A.acquire();
        println!("   First acquire won. Requesting second acquire on same lock (expecting panic)...").unwrap();
        
        // This will force a kernel crash
        LOCK_A.acquire();
        
        assert!(false, "CRITICAL FAULT: System bypassed recursive lock boundary check without a panic!");
    }

    println!("=== Spinlock checked. No problemo!~ ===").unwrap();
    // =================
    // END Spinlock Test
    // =================

    // ================
    // BEGIN Mutex Test
    // ================
    println!("== Begin Mutex Tests ==").unwrap();
    static TEST_MUTEX: Mutex = Mutex::new("main_test_mutex");

    // Test #1: Without Scheduler
    println!("-> Testing uncontended Mutex acquisition...").unwrap();
    assert!(!TEST_MUTEX.is_locked(), "Error: Mutex should've been unlocked");

    TEST_MUTEX.acquire();
    assert!(TEST_MUTEX.is_locked(), "Error: Mutex should've been locked after acquire");

    TEST_MUTEX.release();
    assert!(!TEST_MUTEX.is_locked(), "Error: Mutex should've been unlocked after release");
    println!("   [PASSED]").unwrap();

    // Test #2: Simulating sleep/wakeup situation
    // I was not quite sure about how to test this sleep/wakeup situation,
    // naturally, I decided to delegate this to an LLM. While I have reviewed
    // the following, I request other maintainers/reviewers to check it out.
    if let Some(proc_init) = Process::find_by_id(1) {
        let fake_channel = 0xCAFEBABE as *const ();
	proc_init.set_state(ProcessState::Blocked);
        proc_init.chan = fake_channel as u64;
	println!("   Process 'init' simulated sleeping on channel {:#x}", fake_channel as u64).unwrap();
	Scheduler::wakeup(fake_channel);
	assert_eq!(
            proc_init.state, 
            ProcessState::Ready, 
            "CRITICAL FAULT: Scheduler::wakeup failed to restore process back to Ready!"
        );
        assert_eq!(proc_init.chan, 0, "Error: Wakeup did not clear the process sleep channel");
        println!("   [PASSED]").unwrap();
    } else {
	println!("   [WARNING]: Could not find process 'init' to run state test.").unwrap();
    }
    println!("=== Mutex and Sleep/Wakeup tracking verified cleanly! ===\n").unwrap();
    // ================
    // END Mutex Test
    // ================

    println!("Starting the scheduler!").unwrap();
    Scheduler::start();
    
}

/* For input testing */
// #[unsafe(no_mangle)]
// pub extern "C" fn _rust_main() -> ! {
//     Uart.init();
    
//     // We keep interrupts OFF for this test so the timer doesn't take control away
    
//     println!("=== AtOS Raw Input Test ===").unwrap();
//     println!("Testing kernel space getline directly. Type a line and hit enter:").unwrap();

//     let mut test_buffer = [0u8; 64];

//     let bytes_read = kernel::input::getline(&mut test_buffer);

//     println!("\n--- Test Results ---").unwrap();
//     println!("Bytes read: {}", bytes_read).unwrap();
    
//     if let Ok(s) = core::str::from_utf8(&test_buffer[..bytes_read]) {
//         println!("Buffer contains: {}", s).unwrap();
//     } else {
//         println!("Buffer contains invalid UTF-8 data.").unwrap();
//     }

//     println!("Direct hardware testing complete. Halting CPU.").unwrap();
//     loop { core::hint::spin_loop(); }
// }

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
