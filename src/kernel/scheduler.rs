use core::panic;

use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::processes::{Cpu, PROCESS_TABLE, ProcessContext, MAX_PROCESSES, ProcessState};
use crate::kernel::timer::PhysicalTimer;
use crate::kernel::spinlock::Spinlock;
use crate::kernel::processes::Process;
use crate::dprintln;
use crate::the_end;

pub static mut CURRENT_PROCESS: usize = 1; // last scheduled process index in process table
pub const TIMESLICE_MILISECONDS: u64 = 1; 

// we implement xv6 similar round robin
pub struct Scheduler;

impl Scheduler {

    pub fn start() -> ! {
        dprintln!("[SCHEDULER] Starting scheduler...");
        unsafe {
            core::arch::asm!("svc #0", options(noreturn));
        }
    }

    pub fn get_current_process_index() -> usize {
        unsafe { CURRENT_PROCESS }
    }

    pub fn schedule_next(ectx: &mut ExceptionContext) {
        dprintln!("[SCHEDULER] Scheduling next process...");
        if let Some(next_process) = Self::choose_next_process() {
            Self::load_pctx(next_process, ectx);
        } else {
            dprintln!("[SCHEDULER] No process left to schedule!");
            the_end();
        }
    }

    fn choose_next_process() -> Option<usize> {
        unsafe {
            // if current progress has not finished its time slice, continue it
            if let Some(current_process) = PROCESS_TABLE[CURRENT_PROCESS] {
                if current_process.state == ProcessState::Running {
                    return Some(CURRENT_PROCESS as usize);
                }
            }
            // otherwise scan forward circularly for next process
            for i in 1..(MAX_PROCESSES+1) { // +1, so if no other processes are found, it will circle back to current process
                let idx = (CURRENT_PROCESS + i) % MAX_PROCESSES;
                if let Some(process) = PROCESS_TABLE[idx] {
                    if process.state == ProcessState::Ready {
                        CURRENT_PROCESS = idx;
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    fn load_pctx(pidx: usize, ectx: &mut ExceptionContext) {
        unsafe {
            if let Some(process) = &mut PROCESS_TABLE[pidx] {
                CURRENT_PROCESS = pidx;
                process.set_state(ProcessState::Running);

		// Sync struct Cpu abstraction with PID
		Cpu::current().set_current_process(process.pid);
		
                ectx.update_from_pctx(&process.pctx);
                core::arch::asm!("msr SP_EL0, {sp}", sp = in(reg) process.pctx.sp);
            } else {
                panic!("in Scheduler::load_pctx(), process not found!");
                // panic, because this function is only called in schedule() and ONLY after choose_next_process() returns Some(pidx). so if for some mysterious reason the process just disappeared after being chosen, we might want kernel to scream.
            }
        }
    }

    pub fn update_last_running_pctx(new_pctx: &ProcessContext) {
        unsafe {
            if let Some(current_process) = &mut PROCESS_TABLE[CURRENT_PROCESS] {
                current_process.set_pctx(*new_pctx);
            } else {
                panic!("Last running process disappeared for unaccounted reason!");
                // panic, because this function is going to only exclusively called
                // in handle_exception_el1. and ONLY in the case when the exception
                // came from EL0. which would be our last scheduled user process.
                // so if for some mysterious reason the process just disappeared
                // after an exception came from it, we might want kernel to scream.
            }
        }
    }

    pub fn reset_timer() {
        PhysicalTimer::set_milliseconds(TIMESLICE_MILISECONDS);
        PhysicalTimer::enable();
    }

    pub fn timeslice_up() {
        // updating current processs from running to ready.
        unsafe {
            if let Some(current_process) = &mut PROCESS_TABLE[CURRENT_PROCESS] {
                current_process.set_state(ProcessState::Ready);
            } else {
                panic!("Current process disappeared for unaccounted reason!");
            }
        }

        Self::reset_timer();
    }

    pub fn sleep(channel: *const (), mutex_guard: &Spinlock) {
	unsafe {
	    if let Some(current_process) = &mut PROCESS_TABLE[CURRENT_PROCESS] {
                current_process.set_state(ProcessState::Blocked);
                current_process.chan = channel as u64;
            } else {
                panic!("Scheduler::sleep() was called when no process was active!");
            }

	    // Clear Cpu tracker because the process is no longer running
	    Cpu::current().clear_current_process();

	    mutex_guard.release();
	    
	    core::arch::asm!("svc #0");
	}
    }

    pub fn wakeup(channel: *const ()) {
	let channel_addr = channel as u64;
	unsafe {
	    let table_ptr = core::ptr::addr_of_mut!(PROCESS_TABLE) as *mut Option<Process>;
	    for i in 0..MAX_PROCESSES {
		let slot = &mut *table_ptr.add(i);
		if let Some(process) = slot {
		    if process.state == ProcessState::Blocked && process.chan == channel_addr {
			process.set_state(ProcessState::Ready);
			process.chan = 0;
		    }
		}
	    }
	}
    }
}
