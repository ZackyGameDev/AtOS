#![allow(unused)]

use core::fmt::{Arguments, Result, Write};
use core::ptr::{read_volatile, write_volatile};

use crate::kernel::arch::registers::Register;
use crate::kernel::spinlock::Spinlock;
use crate::println;

// MMIO Base for BCM2837 (Raspberry Pi 3)
const MMIO_BASE: usize = 0x3F00_0000 | 0xFFFF_FF80_0000_0000;

// Base configuration for Auxiliaries
const AUXENB: Register<u32> = Register::new(MMIO_BASE + 0x0021_5004);

// Mini UART Registers
pub const AUX_MU_IO_REG:   Register<u32> = Register::new(MMIO_BASE + 0x0021_5040);
pub const AUX_MU_IER_REG:  Register<u32> = Register::new(MMIO_BASE + 0x0021_5044);
pub const AUX_MU_IIR_REG:  Register<u32> = Register::new(MMIO_BASE + 0x0021_5048);
pub const AUX_MU_LCR_REG:  Register<u32> = Register::new(MMIO_BASE + 0x0021_504C);
pub const AUX_MU_LSR_REG:  Register<u32> = Register::new(MMIO_BASE + 0x0021_5054);
pub const AUX_MU_CNTL_REG: Register<u32> = Register::new(MMIO_BASE + 0x0021_5060);
pub const AUX_MU_BAUD:     Register<u32> = Register::new(MMIO_BASE + 0x0021_5068);

// GPIO Registers
pub const GPFSEL1:   Register<u32> = Register::new(MMIO_BASE + 0x0020_0004);
pub const GPPUD:     Register<u32> = Register::new(MMIO_BASE + 0x0020_0094);
pub const GPPUDCLK0: Register<u32> = Register::new(MMIO_BASE + 0x0020_0098);

pub struct Uart {
    w_lock: Spinlock,
    r_lock: Spinlock,
}

impl Uart {
    pub const fn new() -> Self {
        Self {
            w_lock: Spinlock::new("uart_write_lock"),
            r_lock: Spinlock::new("uart_read_lock"),
        }
    }

    pub fn init(&self) {
        self.w_lock.acquire();
        self.r_lock.acquire();

        AUXENB.write(AUXENB.read() | 1); // enable mini-UART
        AUX_MU_CNTL_REG.write(0); // to disable t/r
        AUX_MU_IER_REG.write(0); // to disable interrupts
        AUX_MU_LCR_REG.write(3); // for 8-bit mode
        AUX_MU_IIR_REG.write(0x06); // clear FIFOs
        AUX_MU_BAUD.write(270); // 115200 baud at 250MHz and baud_rate = sys_clock_f/(8*(baud_rate_reg + 1))

        // Setup GPIO 14 & 15 to Alt Function 5
        let mask = (7 << 12) | (7 << 15);
        let val = (2 << 12) | (2 << 15);
        GPFSEL1.write((GPFSEL1.read() & !mask) | (val & mask));

        // Disable pull-up/down
        GPPUD.write(0);
        for _ in 0..150 { core::hint::spin_loop(); }
        GPPUDCLK0.write((1 << 14) | (1 << 15));
        for _ in 0..150 { core::hint::spin_loop(); }
        GPPUDCLK0.write(0);

        AUX_MU_CNTL_REG.write(3); // enable t/r

        self.r_lock.release();
        self.w_lock.release();
    }
    
    pub fn write_byte(&self, c: u8) {
        self.w_lock.acquire();

        // spin until transmit fifo can accept atleast one byte (bit 5 empty)
        while (AUX_MU_LSR_REG.read() & 0x20) == 0 {
            core::hint::spin_loop();
        }
        AUX_MU_IO_REG.write(c as u32);

        self.w_lock.release();
    }


    /* obsolete function */
    /* This function is unsafe from deadlocks and thus has been removed */
    // pub fn read_byte(&self) -> u8 {
    //     self.r_lock.acquire();
    //     while (AUX_MU_LSR_REG.read() & 0x01) == 0 {
    //         if let Some(current_process) = Scheduler::get_current_process() {
    //             Scheduler::sleep(&AUX_MU_LSR_REG as *const _ as *const (), BlockReason::AwaitingIO);
    //         } else {
    //             core::hint::spin_loop();
    //         }
    //     }
    //     let byte = (AUX_MU_IO_REG.read() & 0xFF) as u8;
    //     self.r_lock.release();
    //     byte
    // }

    // Checks if a character is available in the FIFO.
    // Returns Some(u8) if data is ready, or None immediately if the FIFO is empty.
    pub fn poll_byte(&self) -> Option<u8> {
        self.r_lock.acquire();

        if (AUX_MU_LSR_REG.read() & 0x01) == 0 {
            self.r_lock.release();
            return None;
        }

        let byte = (AUX_MU_IO_REG.read() & 0xFF) as u8;
        
        self.r_lock.release();
        Some(byte)
    }

    pub fn raw_write(&self, s: &str) {
        for byte in s.bytes() {
            // Write directly to the IO register without waiting for TX FIFO empty status
            AUX_MU_IO_REG.write(byte as u32);
        }
    }

    /// Associated raw function to write a string even without a `Uart` instance/reference.
    pub fn raw_write_str(s: &str) {
        for byte in s.bytes() {
            AUX_MU_IO_REG.write(byte as u32);
        }
    }
}
