#![allow(unused)]

use core::ptr::{read_volatile, write_volatile};
use crate::kernel::processes::mycpu;
use crate::kernel::arch::registers::Register;

// QA7 component documentation:
// https://github.com/Tekki/raspberrypi-documentation/blob/b1df6ea8e135254e5feb0c8bb036b2a18db8b859/hardware/raspberrypi/bcm2836/QA7_rev3.4.pdf

// QA7 Base for BCM2837 (RP3)
const QA7_BASE: usize = 0x4000_0000 + 0xFFFF_FF80_0000_0000;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum TimerInterruptSource {
    Physical          = 1 << 0,
    PhysicalNonSecure = 1 << 1,
    Hypervisor        = 1 << 2,
    Virtual           = 1 << 3,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum InterruptRoute {
    IRQ,
    FIQ,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum InterruptSource {
    PhysicalSecureTimer    = 1 << 0,
    PhysicalNonSecureTimer = 1 << 1,
    HypervisorTimer        = 1 << 2,
    VirtualTimer           = 1 << 3,
    Mailbox0               = 1 << 4,
    Mailbox1               = 1 << 5,
    Mailbox2               = 1 << 6,
    Mailbox3               = 1 << 7,
    GPU                    = 1 << 8,
    PMU                    = 1 << 9,
    AXI                    = 1 << 10,
    LocalTimer             = 1 << 11,
    PeripheralInterrupt    = 0x3F << 12, // last 6 bits are for peripheral interrupts

    // \TODO don't know yet how they are implemented and used so for now i just
    // wrote it this way so (irq_sources | PeripheralInterrupt) would give you
    // all the peripheral interrupts (presumably).
}

pub struct Interrupts;

// During refactoring, some of the code has not been tested as extensively as
// the older versions.  While I have tried my best to understand and
// re-implement the old methods, but it is entirely plausible that the code will
// not exactly be a 1-1 replacement and might miss some subtleties incorporated.
// I request the reviewers to apply more scrutiny while checking out the code.
impl Interrupts {
    //
    // CPU interrupt mask primitives
    //
    pub fn daif_unmask_all() {
        unsafe { core::arch::asm!("msr DAIFClr, 0b1111", options(nostack, preserves_flags)); }
    }

    pub fn daif_mask_all() {
        unsafe { core::arch::asm!("msr DAIFSet, 0b1111", options(nostack, preserves_flags)); }
    }

    pub fn irq_disable() {
        unsafe { core::arch::asm!("msr DAIFSet, 0b0010", options(nostack, preserves_flags)); }
    }

    pub fn irq_enable() {
        unsafe { core::arch::asm!("msr DAIFClr, 0b0010", options(nostack, preserves_flags)); }
    }

    pub fn irq_enabled() -> bool {
        let daif: u64;
        unsafe {
            core::arch::asm!("mrs {}, DAIF", out(reg) daif, options(nostack, preserves_flags));
        }
        ((daif >> 7) & 1) == 0
    }

    //
    // Per-core interrupt routing and tracking
    //

    // @Review(ZackyGameDev) some multi-core considerations have been made
    pub fn deroute_timer_interrupt(source: TimerInterruptSource) {
        let cpu_id = mycpu().cid;
        let mut disable_bit = source as u32;

        disable_bit = disable_bit | (disable_bit << 4);

        let reg: Register<u32> = Register::new(QA7_BASE + 0x40 + (cpu_id * 4));
        reg.write(reg.read() & !disable_bit);
    }

    pub fn route_timer_interrupt(source: TimerInterruptSource, route: InterruptRoute) {
        let cpu_id = mycpu().cid;
        Self::deroute_timer_interrupt(source);
        
        let mut enable_bit: u32 = source as u32;
        enable_bit = match route {
            InterruptRoute::IRQ => enable_bit,
            InterruptRoute::FIQ => enable_bit << 4,
        };

        let reg: Register<u32> = Register::new(QA7_BASE + 0x40 + (cpu_id * 4));
        reg.write(reg.read() | enable_bit);
    }

    pub fn pending_irq() -> u32 {
        // reads CORE_0_IRQ_SRC
        Register::new(QA7_BASE + 0x60 + (mycpu().cid * 4)).read()
    }

    pub fn pending_fiq() -> u32 {
        // reads CORE_0_FIQ_SRC
        Register::new(QA7_BASE + 0x70 + (mycpu().cid * 4)).read()
    }

    pub fn is_irq_pending(source: InterruptSource) -> bool {
        (Self::pending_irq() & (source as u32)) != 0
    }

    pub fn is_fiq_pending(source: InterruptSource) -> bool {
        (Self::pending_fiq() & (source as u32)) != 0
    }

    //
    // Concurrency
    //
    
    // push_off and pop_off are like enabling and disabling interrupts but it
    // doesn't just toggle blindly. Each push_off matches a pop_off. If
    // interrupts were originally off, these functions keep them off.
    pub fn push_off() {
        let enabled = Self::irq_enabled();
        Self::irq_disable();

        let c = mycpu();
        if c.ncli == 0 {
            c.were_interrupts_enabled = enabled;
        }
        c.ncli += 1;
    }

    pub fn pop_off() {
        let c = mycpu();

        if Self::irq_enabled() {
            panic!("pop_off: interrupts active when they should be masked");
        }

        if c.ncli < 1 {
            panic!("pop_off: nesting underflow!");
        }

        c.ncli -= 1;
        if c.ncli == 0 && c.were_interrupts_enabled {
            Self::irq_enable();
        }
    }
}
