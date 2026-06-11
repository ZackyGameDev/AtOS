#![allow(dead_code)]

use crate::dprintln;
use crate::kernel::interrupts::{Interrupts, TimerInterruptSource, InterruptRoute};
use crate::kernel::exceptions::{ExceptionContext, was_from_user_el0};
use crate::kernel::scheduler::Scheduler;

pub struct PhysicalTimer;

impl PhysicalTimer {
    pub fn init_irq(){
        Interrupts::route_timer_interrupt(TimerInterruptSource::PhysicalNonSecure, InterruptRoute::IRQ);
    }

    // \TODO Physical timers IRQ could be triggered as secure or non secure. 
    // later you may want to add a argument here which lets you differentiate between
    // the two. but for now we only use non secure world so it's fine to just 
    // handle it. 
    pub fn handle_irq(ctx: &mut ExceptionContext) { 
        dprintln!("[P_TIMER] Physical Timer Interrupt Fired!");
        dprintln!("[P_TIMER] Calling Scheduler");
        PhysicalTimer::set_seconds(1); // disable irq by immedaitely scheduling it into the future
        PhysicalTimer::disable();

        if was_from_user_el0(ctx) {
            Scheduler::timeslice_up();
            Scheduler::schedule_next(ctx);
        } else {
            dprintln!("[P_TIMER] Timer went off in kernel mode. Not switching context.");
            Scheduler::reset_timer();
        }

    }


    #[inline(always)]
    pub fn read_cnt() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!(
                "mrs {val}, CNTPCT_EL0",
                val = out(reg) value,
                options(nostack, preserves_flags)
            );
        }
        value
    }

    #[inline(always)]
    pub fn read_frq() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!(
                "mrs {val}, CNTFRQ_EL0",
                val = out(reg) value,
                options(nostack, preserves_flags)
            );
        }
        value
    }

    #[inline(always)]
    pub fn read_ctl() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!(
                "mrs {val}, CNTP_CTL_EL0",
                "isb",
                val = out(reg) value,
                options(nostack, preserves_flags)
            );
        }
        value
    }

    #[inline(always)]
    fn write_ctl(val: u64) {
        unsafe {
            core::arch::asm!(
                "msr CNTP_CTL_EL0, {val}",
                "isb",
                val = in(reg) val,
                options(nostack, preserves_flags)
            );
        }
    }

    #[inline(always)]
    pub fn enable() {
        let mut ctl = Self::read_ctl();
        ctl |= 1 << 0; 
        Self::write_ctl(ctl);
    }

    #[inline(always)]
    pub fn disable() {
        let mut ctl = Self::read_ctl();
        ctl &= !1; 
        Self::write_ctl(ctl);
    }

    #[inline(always)]
    pub fn mask_int() {
        let mut ctl = Self::read_ctl();
        ctl |= 1 << 1;
        Self::write_ctl(ctl);
    }

    #[inline(always)]
    pub fn unmask_int() {
        let mut ctl = Self::read_ctl();
        ctl &= !(1 << 1);
        Self::write_ctl(ctl);
    }

    #[inline(always)]
    pub fn set_cval(new_cval: u64) {
        unsafe {
            core::arch::asm!(
                "msr CNTP_CVAL_EL0, {val}",
                "isb",
                val = in(reg) new_cval,
            );
        }
    }

    #[inline(always)]
    pub fn set_tval(new_tval: i32) {
        unsafe {
            core::arch::asm!(
                "msr CNTP_TVAL_EL0, {val:w}",
                "isb",
                val = in(reg) new_tval,
            );
        }
    }

    #[inline(always)]
    pub fn start() {
        Self::unmask_int();
        Self::enable();
    }

    pub fn set_seconds(seconds: u64) {
        let freq = Self::read_frq();
        let now = Self::read_cnt();

        let ticks = freq
            .checked_mul(seconds)
            .expect("Physical Timer Seconds Overflow.");

        let cval = now
            .checked_add(ticks)
            .expect("Physical Timer CVAL Overflow.");

        Self::set_cval(cval);
        dprintln!("[TIMER] p cval was set.. {} seconds", seconds);
    }

    pub fn set_milliseconds(milliseconds: u64) {
        let freq = Self::read_frq();
        let now = Self::read_cnt();

        let ticks = freq
            .checked_mul(milliseconds)
            .expect("Physical Timer Milliseconds Overflow.")
            / 1000;

        let cval = now
            .checked_add(ticks)
            .expect("Physical Timer CVAL Overflow.");

        Self::set_cval(cval);
        dprintln!("[TIMER] p cval was set.. {} ms", milliseconds);
    }
}