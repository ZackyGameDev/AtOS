use core::panic;

use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::processes::{Cpu, PROCESS_TABLE, ProcessContext, MAX_PROCESSES, ProcessState, MAX_CPUS, mycpu};
use crate::kernel::timer::PhysicalTimer;
use crate::kernel::spinlock::Spinlock;
use crate::kernel::processes::Process;
use crate::dprintln;
use crate::the_end;

// Track the last scheduled process index individually for each CPU core to prevent SMP corruption.
pub static mut CURRENT_PROCESSES: [usize; MAX_CPUS] = [0; MAX_CPUS];
pub const TIMESLICE_MILISECONDS: u64 = 9;

// we implement xv6 similar round robin
pub struct Scheduler;

impl Scheduler {
    pub fn start() -> ! {
        dprintln!("[SCHEDULER] Starting scheduler on Core {}...", mycpu().cid);
        unsafe {
            core::arch::asm!("svc #0", options(noreturn));
        }
    }

    pub fn get_current_process_index() -> usize {
        unsafe {
            let core_id = Cpu::current().cid;
            CURRENT_PROCESSES[core_id]
        }
    }

    pub fn get_current_process() -> Option<&'static mut Process> {
        Process::find_by_ptable_index(Self::get_current_process_index())
    }

    pub fn schedule_next(ectx: &mut ExceptionContext) {
        dprintln!("[SCHEDULER] Scheduling next process...");
        if let Some(next_process) = Self::choose_next_process() {
            Self::load_pctx(next_process, ectx);
        } else {
            dprintln!("[SCHEDULER] No process left to schedule on Core {}!", mycpu().cid);
            the_end();
        }
    }

    fn choose_next_process() -> Option<usize> {
        unsafe {
            let core_id = mycpu().cid;
            let current_index = CURRENT_PROCESSES[core_id];
            
            // if current progress has not finished its time slice, continue it
            if let Some(current_process) = &PROCESS_TABLE[current_index] {
                if current_process.state == ProcessState::Running {
                    return Some(current_index);
                }
            }
            // otherwise scan forward circularly for next process
            for i in 1..(MAX_PROCESSES+1) { // +1, so if no other processes are found, it will circle back to current process
                let idx = (current_index + i) % MAX_PROCESSES;
                if let Some(process) = &PROCESS_TABLE[idx] {
                    if process.state == ProcessState::Ready {
                        CURRENT_PROCESSES[core_id] = idx;
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    fn load_pctx(pidx: usize, ectx: &mut ExceptionContext) {
        unsafe {
            let core_id = mycpu().cid;
            if let Some(process) = PROCESS_TABLE[pidx].as_mut() {
                CURRENT_PROCESSES[core_id] = pidx;
                process.set_state(ProcessState::Running);

                // Sync struct Cpu abstraction with PID
                Cpu::current().set_current_process(process.pid);
                ectx.update_from_pctx(&process.pctx);
            } else {
                panic!("[Core {}] in Scheduler::load_pctx(), process not found!", mycpu().cid);
                // panic, because this function is only called in schedule() and 
                // ONLY after choose_next_process() returns Some(pidx). so if for 
                // some mysterious reason the process just disappeared after being
                // chosen, we might want kernel to scream.
            }
        }
    }

    pub fn update_last_running_pctx(new_pctx: &ProcessContext) {
        let current_index = Self::get_current_process_index();
        unsafe {
            if let Some(process) = PROCESS_TABLE[current_index].as_mut() {
                process.set_pctx(*new_pctx);
            } else {
                panic!("Last running process at index {} disappeared for unaccounted reason!", current_index);
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
        let current_index = Self::get_current_process_index();
        unsafe {
            if let Some(current_process) = PROCESS_TABLE[current_index].as_mut() {
                current_process.set_state(ProcessState::Ready);
            } else {
                panic!("Current process disappeared for unaccounted reason!");
            }
        }

        Self::reset_timer();
    }

    pub fn sleep(channel: *const (), mutex_guard: &Spinlock) {
        let current_index = Self::get_current_process_index();
        unsafe {
            if let Some(current_process) = PROCESS_TABLE[current_index].as_mut() {
                current_process.set_state(ProcessState::Blocked);
                current_process.chan = channel as u64;
            } else {
                panic!("Scheduler::sleep() was called when no process was active!");
            }

            // Clear Cpu tracker because the process is no longer running
            Cpu::current().clear_current_process();

            mutex_guard.release();

            core::arch::asm!("svc #0");

            mutex_guard.acquire();
        }
    }

    pub fn wakeup(channel: *const ()) {
        let channel_addr = channel as u64;
        for i in 0..MAX_PROCESSES {
            unsafe {
                let slot_ptr = core::ptr::addr_of_mut!(PROCESS_TABLE[i]);
                if let Some(process) = (*slot_ptr).as_mut() {
                    if process.state == ProcessState::Blocked && process.chan == channel_addr {
                        process.set_state(ProcessState::Ready);
                        process.chan = 0;
                    }
                }
            }
        }
    }
}
