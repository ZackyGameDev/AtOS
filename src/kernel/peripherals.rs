#![allow(unused)]

use core::fmt::{Arguments, Result, Write};
use core::ptr::{read_volatile, write_volatile};

use crate::kernel::arch::registers::Register;
use crate::kernel::spinlock::Spinlock;

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
    lock: Spinlock,
}

impl Uart {
    pub const fn new() -> Self {
        Self {
            lock: Spinlock::new("uart_hardware_lock"),
        }
    }

    pub fn init(&self) {
        self.lock.acquire();

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

        self.lock.release();
    }
    
    pub fn write_byte(&self, c: u8) {
        self.lock.acquire();

        // spin until transmit fifo can accept atleast one byte (bit 5 empty)
        while (AUX_MU_LSR_REG.read() & 0x20) == 0 {
            core::hint::spin_loop();
        }
        AUX_MU_IO_REG.write(c as u32);

        self.lock.release();
    }

    pub fn read_byte(&self) -> u8 {
        self.lock.acquire();
        while (AUX_MU_LSR_REG.read() & 0x01) == 0 {
            core::hint::spin_loop();
        }
        let byte = (AUX_MU_IO_REG.read() & 0xFF) as u8;
        self.lock.release();
        byte
    }
}
