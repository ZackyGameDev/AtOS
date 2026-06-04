use core::fmt::{Arguments, Result, Write};
use core::ptr::{read_volatile, write_volatile};

// MMIO Base for BCM2837 (Raspberry Pi 3)
const MMIO_BASE:       usize    = 0x3F00_0000;

// Base configuration for Auxiliaries
const AUXENB:          *mut u32 = (MMIO_BASE + 0x0021_5004) as *mut u32;

// Mini UART Registers
const AUX_MU_IO_REG:   *mut u32 = (MMIO_BASE + 0x0021_5040) as *mut u32;
const AUX_MU_IER_REG:  *mut u32 = (MMIO_BASE + 0x0021_5044) as *mut u32;
const AUX_MU_IIR_REG:  *mut u32 = (MMIO_BASE + 0x0021_5048) as *mut u32;
const AUX_MU_LCR_REG:  *mut u32 = (MMIO_BASE + 0x0021_504C) as *mut u32;
const AUX_MU_LSR_REG:  *mut u32 = (MMIO_BASE + 0x0021_5054) as *mut u32;
const AUX_MU_CNTL_REG: *mut u32 = (MMIO_BASE + 0x0021_5060) as *mut u32;
const AUX_MU_BAUD:     *mut u32 = (MMIO_BASE + 0x0021_5068) as *mut u32;

// GPIO Registers
const GPFSEL1:         *mut u32 = (MMIO_BASE + 0x0020_0004) as *mut u32;
const GPPUD:           *mut u32 = (MMIO_BASE + 0x0020_0094) as *mut u32;
const GPPUDCLK0:       *mut u32 = (MMIO_BASE + 0x0020_0098) as *mut u32;

pub struct Uart;

impl Uart {
    // This time we'll be more explicit
    pub fn init(&self) {
	unsafe {
	    write_volatile(AUXENB, read_volatile(AUXENB) | 1); // enable Mini UART
	    write_volatile(AUX_MU_CNTL_REG, 0); // to disable t/r
	    write_volatile(AUX_MU_IER_REG, 0); // to disable interrupts
	    write_volatile(AUX_MU_LCR_REG, 3); // for 8-bit mode
	    write_volatile(AUX_MU_IIR_REG, 0x06); // clear FIFOs
	    write_volatile(AUX_MU_BAUD, 270); // 115200 baud at 250MHz and baud_rate = sys_clock_f/(8*(baud_rate_reg + 1))

	    // Setup GPIO 14 & 15 to Alt Function 5
            let mut gpfsel = read_volatile(GPFSEL1);
            gpfsel &= !((7 << 12) | (7 << 15));
            gpfsel |= (2 << 12) | (2 << 15);
            write_volatile(GPFSEL1, gpfsel);

	    // Disable pull-up/down
	    write_volatile(GPPUD, 0);
            for _ in 0..150 { core::hint::spin_loop(); }
            write_volatile(GPPUDCLK0, (1 << 14) | (1 << 15));
            for _ in 0..150 { core::hint::spin_loop(); }
            write_volatile(GPPUDCLK0, 0);

	    write_volatile(AUX_MU_CNTL_REG, 3); // enable t/r
	}
    }
    
    pub fn write_byte(&self, c: u8) {
        unsafe {
            while (read_volatile(AUX_MU_LSR_REG) & 0x20) == 0 { core::arch::asm!("nop"); }
            write_volatile(AUX_MU_IO_REG, c as u32);
        }
    }

    // Will fix it later
    pub fn read_byte(&self) -> u8 {
	unsafe {
	    while (read_volatile(AUX_MU_LSR_REG) & 0x01) == 0 { core::hint::spin_loop(); }
	    (read_volatile(AUX_MU_IO_REG) & 0xFF) as u8
	}
    }
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
	    if b == b'\n' { self.write_byte(b'\r'); }
            self.write_byte(b);
        }
        Ok(())
    }
}

pub fn _print(args: Arguments) -> Result {
    let mut uart = Uart;
    uart.write_fmt(args)
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::kernel::peripherals::_print(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
	$crate::print!("\n")
    };
    
    ($($args:tt)*) => {
        $crate::kernel::peripherals::_print(core::format_args!($($args)*))
	    .and_then(|_| $crate::print!("\n"))
    };
}
