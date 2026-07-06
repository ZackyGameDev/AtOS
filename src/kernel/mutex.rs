// Mutexes
#![allow(unused)]

use core::cell::Cell;
use crate::kernel::spinlock::Spinlock;
use crate::kernel::scheduler::Scheduler;
use crate::kernel::processes::Process;

// Never in my life with C I've experienced so much bs that I have with Rust.
// Thank you, Rust.
pub struct Mutex {
    lock: Cell<bool>,
    guard: Spinlock,

    // For debugging:
    name: &'static str,
    pid: Cell<Option<u64>>, // Process holding the lock
}

unsafe impl Sync for Mutex {}

impl Mutex {
    pub const fn new(name: &'static str) -> Self {
        Self {
            lock: Cell::new(false),
            guard: Spinlock::new(name),
            name,
            pid: Cell::new(None),
        }
    }

    pub fn acquire(&self) {
        self.guard.acquire();
        while self.lock.get() {
            // We pass the address of this specific mutex instance as unique wait channel
            // This is the kind of code that keeps you up at night.
            Scheduler::sleep(self as *const Mutex as *const (), &self.guard);
        }
        self.lock.set(true);
        if let Some(proc) = Process::get_current() {
            self.pid.set(Some(proc.pid));
        }
        self.guard.release();
    }

    pub fn release(&self) {
        self.guard.acquire();

        if !self.lock.get() {
            panic!("Mutex release error: ({}) is already unlocked!", self.name);
        }

        if let Some(current_proc) = Process::get_current() {
            if self.pid.get() != Some(current_proc.pid) {
                panic!("Mutex release error: ({}) is owned by PID {:?}, but PID {} tried to release it!",
                       self.name, self.pid.get(), current_proc.pid);
            }
        }
        
        self.lock.set(false);
        self.pid.set(None);

        // Wakeup all processes on this specific Mutex channel
        Scheduler::wakeup(self as *const Mutex as *const ());

        self.guard.release();
    }

    pub fn is_locked(&self) -> bool {
        self.guard.acquire();
        let locked = self.lock.get();
        self.guard.release();
        locked
    }
}
