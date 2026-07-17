#![allow(unused)]

use core::fmt::{Arguments, Write};
use crate::kernel::mutex::Mutex;
use crate::kernel::peripherals::Uart;

pub struct KernelIO {
    lock: Mutex,
    device: Uart,
}

unsafe impl Sync for KernelIO {}

impl KernelIO {
    pub const fn new() -> Self {
        Self {
            lock: Mutex::new("kernel_io"),
            device: Uart::new(),
        }
    }

    pub fn init(&self) {
        self.lock.acquire();
        self.device.init();
        self.lock.release();
    }

    pub fn getch(&self) -> u8 {
        loop {
            self.lock.acquire();
            let c = self.device.read_byte();

            if c == 8 || c == 127 {
                self.lock.release();
                return c;
            }
            
            if c == b'\r' {
                self.device.write_byte(b'\n');
                self.lock.release();
                return b'\n';
            }

            if c == b'\n' {
                self.lock.release();
                continue;
            }

            self.device.write_byte(c);
            self.lock.release();
            return c;
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> usize {
        let mut r = 0;
        while r < buf.len() {
            let c = self.getch();
            match c {
                // Handle backspace and delete
                8 | 127 => {
                    if r > 0 {
                        r -= 1;

                        self.lock.acquire();
                        self.device.write_byte(8);
                        self.device.write_byte(b' ');
                        self.device.write_byte(8);
                        self.lock.release();
                    }
                }

                _ => {
                    buf[r] = c;
                    r += 1;

                    if c == b'\n' {
                        break;
                    }
                }
            }
        }
        r
    }

    pub fn write(&self, buf: &mut [u8]) {
        self.lock.acquire();
        for &byte in buf.iter() {
            if byte == b'\n' {
                self.device.write_byte(b'\r');
            }
            self.device.write_byte(byte);
        }
        self.lock.release();
    }

    pub fn print(&self, args: Arguments) {
        struct IOWriter<'a>(&'a Uart);

        impl Write for IOWriter<'_> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                for b in s.bytes() {
                    if b == b'\n' {
                        self.0.write_byte(b'\r');
                    }
                    self.0.write_byte(b);
                }
                Ok(())
            }
        }

        self.lock.acquire();
        let mut writer = IOWriter(&self.device);
        let _ = writer.write_fmt(args);
        self.lock.release();
    }
}

pub static KERNEL_IO: KernelIO = KernelIO::new();

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::kernel::io::KERNEL_IO.print(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::kernel::io::KERNEL_IO.print(core::format_args!("{}\n", core::format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {
        if $crate::DEBUG_PRINTS_ENABLED {
            $crate::println!("      {}", format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! mprintln {
    ($($arg:tt)*) => {
        if unsafe { $crate::DEBUG_PRINTS_ENABLED_MMU } {
            $crate::println!("      {}", format_args!($($arg)*));
        }
    };
}

