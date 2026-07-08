use core::arch::asm;

// @Refactor(tanishqdaiya) remove that in the future since only main.rs uses it
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
