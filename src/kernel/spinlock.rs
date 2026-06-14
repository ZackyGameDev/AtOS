// Spinlocks

use core::sync::atomic::{AtomicBool, Ordering};
use crate::Interrupts;

pub struct Spinlock {
    lock: AtomicBool,

    // For debugging:
    // Besides the name of the lock, we should also store the CPU context
    // it is in when we actually work with multiple cores.
    name: &'static str, // Name of the lock
}

impl Spinlock {
    pub const fn new(name: &'static str) -> Self {
	Self {
	    lock: AtomicBool::new(false),
	    name,
	}
    }

    pub fn acquire(&self) {
	Interrupts::push_off();

	// Prevent the exact same lock from deadlocking itself.
	// If it is already locked on a single core, we'd spin forever.
	if self.lock.load(Ordering::Relaxed) {
	    panic!("acquire({})", self.name);
	}

	while self.lock.compare_exchange(false,
					 true,
					 Ordering::Acquire,
					 Ordering::Relaxed).is_err() {
	    core::hint::spin_loop();
	}
    }

    pub fn release(&self) {
	if !self.lock.load(Ordering::Relaxed) {
	    panic!("release({})", self.name);
	}
	
	self.lock.store(false, Ordering::Release);
	Interrupts::pop_off();
    }
}

