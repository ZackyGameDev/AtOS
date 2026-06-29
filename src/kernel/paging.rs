#![allow(static_mut_refs)]

// TCR_EL1 register values for 4KB granule, 36-bit physical address space, inner shareable, write-back write-allocate cacheable memory
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".boot.constants")]
pub static TCR_EL1_VALUE: u64 = {

    pub const TCR_T0SZ: u64 = 25;
    pub const TCR_EPD0: u64 = 0 << 7;
    pub const TCR_IRGN0_WBWA: u64 = 0b01 << 8;
    pub const TCR_ORGN0_WBWA: u64 = 0b01 << 10;
    pub const TCR_SH0_INNER: u64 = 0b11 << 12;
    pub const TCR_TG0_4K: u64 = 0b00 << 14;
    pub const TCR_T1SZ: u64 = 25 << 16;
    pub const TCR_A1: u64 = 0 << 22;
    pub const TCR_EPD1: u64 = 0 << 23;
    pub const TCR_IRGN1_WBWA: u64 = 0b01 << 24;
    pub const TCR_ORGN1_WBWA: u64 = 0b01 << 26;
    pub const TCR_SH1_INNER: u64 = 0b11 << 28;
    pub const TCR_TG1_4K: u64 = 0b10 << 30;
    pub const TCR_IPS_36BIT: u64 = 0b001 << 32;

    TCR_T0SZ
    | TCR_EPD0
    | TCR_IRGN0_WBWA
    | TCR_ORGN0_WBWA
    | TCR_SH0_INNER
    | TCR_TG0_4K
    | TCR_T1SZ
    | TCR_A1
    | TCR_EPD1
    | TCR_IRGN1_WBWA
    | TCR_ORGN1_WBWA
    | TCR_SH1_INNER
    | TCR_TG1_4K
    | TCR_IPS_36BIT

};

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".boot.constants")]
pub static MAIR_EL1_VALUE: u64 = {
    pub const MAIR_ATTR_DEVICE: u64 = 0x00;
    pub const MAIR_ATTR_NORMAL: u64 = 0xFF;    
    (MAIR_ATTR_DEVICE << 0)
    | (MAIR_ATTR_NORMAL << 8)
};

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".boot.constants")]
pub static SCTLR_EL1_ENABLE_MMU: u64 = 1 << 0; // M = 1

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_ENTRIES: usize = PAGE_SIZE / 8; // each entry is 64 bits by ARM standard 

#[repr(C, align(4096))]
pub struct PageTable {
    pub entry: [u64; PAGE_ENTRIES], 
}

// the address at which an object of this struct exists, 
// is the physical address of the frame it represents.
// so basically a free frame will have a FreeFrame struct
// at the start of it. this saves the need to store the linked list
// at some other location 
#[repr(C)]
pub struct FreeFrame {
    pub next: Option<*mut FreeFrame>,
}

pub static mut FREE_FRAME_LIST: Option<*mut FreeFrame> = None;
pub struct PageAllocator;

impl PageAllocator {

    pub fn add_free_frame(addr_in_frame: usize) -> () {
        let free_frame_addr = addr_in_frame & !(PAGE_SIZE - 1);
        let free_frame = free_frame_addr as *mut FreeFrame;
        unsafe { (*free_frame).next = FREE_FRAME_LIST;
                 FREE_FRAME_LIST = Some(free_frame) };
    }

    // run at boot. marks all frames as free and adds them to the free frame list
    pub fn init_frames(first_free_frame_addr: usize, last_frame_addr_limit: usize) -> () {
        for frame_addr in (first_free_frame_addr..last_frame_addr_limit).step_by(PAGE_SIZE) {
            PageAllocator::add_free_frame(frame_addr);
        }
    }
}




/* ~~~ kernel TTBR1 EL1 paging (hardcoded) ~~~ */

// through the entire section we use const functions to generate the page tables at compile time.

const ATTR_DEVICE: u64 = 0 << 2;
const ATTR_NORMAL: u64 = 1 << 2;

const fn kernel_l2_block_descriptor(pa: u64, attr: u64) -> u64 {
    const VALID: u64 = 1 << 0;
    const BLOCK: u64 = 0;
    const AP_EL1_RW: u64 = 0b00 << 6;
    const SH_INNER: u64 = 0b10 << 8;
    const SH_NON: u64 = 0b00 << 8;
    const AF: u64 = 1 << 10;
    const PXN: u64 = 1 << 53;
    const UXN: u64 = 1 << 54;

    let sh =
    if attr == ATTR_DEVICE {
        SH_NON
    } else {
        SH_INNER
    };

    let pxn =
    if attr == ATTR_DEVICE {
        PXN
    } else {
        0
    };

    (pa & !((1u64 << 21) - 1))
        | VALID
        | BLOCK
        | attr
        | AP_EL1_RW
        | sh
        | AF
        | pxn
        | UXN
}

const fn kernel_l2_0() -> PageTable {
    let mut table = PageTable {
        entry: [0; PAGE_ENTRIES],
    };

    let mut i = 0;

    while i < PAGE_ENTRIES {
        let pa = (i as u64) << 21;

        let attr =
            if pa >= 0x3F00_0000 {
                ATTR_DEVICE
            } else {
                ATTR_NORMAL
            };

        table.entry[i] = kernel_l2_block_descriptor(pa, attr);

        i += 1;
    }

    table
}

const fn kernel_l2_1() -> PageTable {
    let mut table = PageTable {
        entry: [0; PAGE_ENTRIES],
    };

    let mut i = 0;

    while i < PAGE_ENTRIES {
        let pa = ((i+PAGE_ENTRIES) as u64) << 21;

        let attr =
            if pa >= 0x3F00_0000 {
                ATTR_DEVICE
            } else {
                ATTR_NORMAL
            };

        table.entry[i] = kernel_l2_block_descriptor(pa, attr);

        i += 1;
    }

    table
}

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".page_tables.kernel.l2")]
pub static PAGE_TABLE_KERNEL_L2_0: PageTable = kernel_l2_0();

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".page_tables.kernel.l2")]
pub static PAGE_TABLE_KERNEL_L2_1: PageTable = kernel_l2_1();

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".page_tables.kernel.l1")]
pub static mut PAGE_TABLE_KERNEL_L1: PageTable = PageTable {
    entry: [0; PAGE_ENTRIES],
};

// `PAGE_TABLE_KERNEL_L1.entry[0] = &PAGE_TABLE_KERNEL_L2_0` is set in entry.S at boot.
// and so is `PAGE_TABLE_KERNEL_L1.entry[1] = &PAGE_TABLE_KERNEL_L2_1`
// the reason we do not do it here is because we need the page table setup before jumping to rust.
// so we can't expect to setup the page table here in rust after jumping to it.
// this is also why we strictly use statics and const the entire time
// because they are setup at commpile time.