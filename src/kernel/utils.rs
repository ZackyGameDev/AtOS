use core::arch::asm;

pub fn get_current_el() -> u64 {
    let el: u64;
    unsafe {
        asm!(
            "mrs {0}, CurrentEL",
            "lsr {0}, {0}, #2",
            out(reg) el,
        );
    }
    el
}

#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {
        if crate::DEBUG_PRINTS_ENABLED {
            crate::println!($($arg)*).unwrap();
        }
    };
}

#[macro_export]
macro_rules! ttbr1_to_va {
    ($addr:expr) => {
        // Casts to usize and applies bitwise OR to set the higher-half prefix
        (($addr) as usize) | 0xffffff8000000000usize
    };
}

#[macro_export]
macro_rules! ttbr1_to_pa {
    ($addr:expr) => {
        // Casts to usize and masks out everything except the lower 39 bits
        (($addr) as usize) & 0x0000007FFFFFFFFFusize
    };
}