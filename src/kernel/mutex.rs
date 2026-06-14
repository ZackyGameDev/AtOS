// Mutexes

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
	    guard: Spinlock::new("mutex_guard"),
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
	    // Sleep (lock, sleeplock)
	}
	self.lock.set(true);
	if let Some(proc) = Process::get_current() {
	    self.pid.set(Some(proc.pid));
	}
	self.guard.release();
    }

    pub fn release(&self) {
	self.guard.acquire();
	
	self.lock.set(false);
	self.pid.set(None);

	// Wakeup all processes on this specific Mutex channel
	Scheduler::wakeup(self as *const Mutex as *const ());

	self.guard.release();
    }

    pub fn is_locked(&self) -> bool {
	self.lock.get()
    }
}
