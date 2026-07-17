/*

TERMINOLOGY REFERENCE:

FRAME refers to the physical memory block.
PAGE refers to the virtual memory block of the process. (common lingo)

FRAME_PA refers to the physical address of some frame on the physical RAM, 
it may or may not be currently used right now.

FRAME_VA refers to the virtual address of some frame, in the ttbr1 virtual
address space. So basically kernel's virtual address space. 

so VA in frame always refers to a virtual address in ttbr1 space, 
which represents the physical memory as it is.

PAGE_VA refers to the virtual address of some page, in the ttbr0 virtual 
address space, so basically the process's virtual address space.

so a VA in page always refers to a virtual address in ttbr0 space, which is the 
virtualized memory for the process.

in ttbr0 you find the clean, virtualized memory for the user process to freely use

in ttbr1 space you get a look of the actual 1GB of physical RAM in the same order
as the physical RAM exists. 

*/

#![allow(static_mut_refs)]

use crate::{ttbr1_to_pa, ttbr1_to_va, mprintln};
use crate::kernel::elf::{Elf64Hdr, Elf64ProgHdr, PT_LOAD};

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
pub static mut FREE_FRAME_COUNT: usize = 0;
pub static mut TOTAL_FRAME_COUNT: usize = 0;
pub struct PageAllocator;

// \TODO add a method to keep track of how many frames are used/available
impl PageAllocator {

    // this was originally written in kernel::processes:load_elf_process
    // however that function is now deprecated because it was written before 
    // paging or virtualization was implemented. i have moved it here and modified it to use
    // the page allocator to load process in virtual memory and handle the translation table
    pub fn load_elf(bytes: &'static [u8]) -> Result<(u64, u64, u64), &'static str> {
        let header = match Elf64Hdr::mkfrombytes(bytes) {
            Some(h) => h,
            None => {
                mprintln!("load_elf_process: invalid elf file header");
                return Err("Invalid ELF file header");
            }
        };

        // yummy page table 
        let ttbr0: u64 = Self::get_free_frame_pa().expect("No free frames available for new process") as u64;

        let mut loaded_any_segments = false;
        let mut max_allocated_addr = 0u64;

        let ph_size = core::mem::size_of::<Elf64ProgHdr>();
        let start = header.phoff as usize;
        let count = header.phnum as usize;

        for i in 0..count {
            let offset = start + (i * ph_size);
            if offset + ph_size > bytes.len() {
                break;
            }

            let ph = unsafe {
                core::ptr::read_unaligned(bytes.as_ptr().add(offset) as *const Elf64ProgHdr)
            };

            if ph.r#type == PT_LOAD {
                let va_start = ph.virtaddr as usize;
                let va_end = va_start + ph.memsize as usize;
                let file_end = va_start + ph.filesize as usize;

                let mut current_page_va = va_start & !0xFFF;

                while current_page_va < va_end {
                    let _frame_va = ttbr1_to_va!(
                        Self::alloc_page(current_page_va, Some(ttbr0))
                    ) as *mut u8;

                    current_page_va += 4096;
                }

                if ph.filesize > 0 {
                    Self::copy_to_pages(
                        &bytes[ph.offset as usize .. (ph.offset as usize + ph.filesize as usize)],
                        va_start as u64,
                        Some(ttbr0),
                    ).expect("load_elf: copy_to_pages failed");
                }

                if file_end < va_end {
                    let mut zero_va = file_end;

                    while zero_va < va_end {
                        let frame_va = Self::ttbr0_to_ttbr1(zero_va, Some(ttbr0))
                            .expect("load_elf: missing page while zeroing BSS") as *mut u8;

                        let offset_in_page = zero_va & 0xFFF;
                        let bytes_to_zero = core::cmp::min(4096 - offset_in_page, va_end - zero_va);

                        unsafe {
                            core::ptr::write_bytes(frame_va.add(offset_in_page), 0, bytes_to_zero);
                        }

                        zero_va += bytes_to_zero;
                    }
                }

                if va_end as u64 > max_allocated_addr {
                    max_allocated_addr = va_end as u64;
                }
                loaded_any_segments = true;
            }
        }
        
        if !loaded_any_segments {
            mprintln!("load_elf_process: no loadable segment found in elf");
            return Err("No loadable segments found in ELF file");
        }

        let entry_point = header.entry;
        // set stack top to just above the highest allocated program segment 16-byte aligned
        let stack_top: u64 = (max_allocated_addr + 0x10000) & !0xf; 

        // giving the user stack 8 pages. with unallocated gaurd page between stack area and everythign else.

        for i in 0..8 {
            let stack_page_va = stack_top - (i * 4096) - 1;
            Self::alloc_page(stack_page_va as usize, Some(ttbr0));
        }



        // Self::alloc_page(stack_top as usize, Some(ttbr0));
        // Self::alloc_page(stack_top as usize - PAGE_SIZE, Some(ttbr0)); // allocate 2 pages for stack right now \TODO make this dynamic 

        Ok((entry_point, stack_top, ttbr0))

    }

    pub fn copy_to_pages(src: &[u8], dst_va: u64, ttbr0_val: Option<u64>) -> Result<(), &'static str> {

        let mut src_offset = 0usize;
        let mut current_va = dst_va as usize;

        while src_offset < src.len() {

            let frame_va = match Self::ttbr0_to_ttbr1(current_va, ttbr0_val) {
                Some(va) => va as *mut u8,
                None => return Err("Destination page not mapped"),
            };

            let offset_in_page = current_va & 0xFFF;
            let bytes_this_page = core::cmp::min(
                PAGE_SIZE - offset_in_page,
                src.len() - src_offset,
            );

            unsafe {
                core::ptr::copy_nonoverlapping(
                    src.as_ptr().add(src_offset),
                    frame_va.add(offset_in_page),
                    bytes_this_page,
                );
            }

            src_offset += bytes_this_page;
            current_va += bytes_this_page;
        }

        Ok(())
    }
    
    // sometimes, the ttbr0 of a process might not be the one which is loaded. so 
    // you cannot write to that process's va space directly, you need to a ttbr1
    // address which points to the same physical frame in ttbr1 for kernel to access 
    pub fn ttbr0_to_ttbr1(va: usize, ttbr0_val: Option<u64>) -> Option<usize> {
        Self::ttbr0_to_pa(va, ttbr0_val).map(|pa| ttbr1_to_va!(pa) as usize)
    }

    // gets the physical address of a virtual address according to given/loaded ttbr0 table
    // since ttbr0 is not identity mapped (unlike ttbr1), we have to traverse the table levels to 
    // get the physical adddress
    pub fn ttbr0_to_pa(va: usize, ttbr0_val: Option<u64>) -> Option<u64> {
        let translation_table_pa = match ttbr0_val {
            Some(pa) => pa,
            None => {
                let mut ttbr0: u64;
                unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0) };
                ttbr0 & 0x0000_FFFF_FFFF_F000
            }
        };

        let l1_i = (va >> 30) & 0x1FF;
        let l2_i = (va >> 21) & 0x1FF;
        let l3_i = (va >> 12) & 0x1FF;
        let offset = va & 0xFFF;

        // trailing to the l3 entry with checks
        unsafe {
            let l1 = ttbr1_to_va!(translation_table_pa) as *const PageTable;
            if (*l1).entry[l1_i] & 0b11 != 0b11 { return None; /* invalid entry/unhandled block situation */ } 
            let l2 = ttbr1_to_va!((*l1).entry[l1_i] & 0x0000_FFFF_FFFF_F000) as *const PageTable;
            if (*l2).entry[l2_i] & 0b11 != 0b11 { return None; /* invalid entry/unhandled block situation */ }
            let l3 = ttbr1_to_va!((*l2).entry[l2_i] & 0x0000_FFFF_FFFF_F000) as *const PageTable;
            let l3_entry = (*l3).entry[l3_i];
            if l3_entry & 0b11 != 0b11 { return None; /* invalid entry */ }
            Some((l3_entry & 0x0000_FFFF_FFFF_F000) + offset as u64)
        }  
    } 

    // this is for fork syscall. duplicates the entire virtual address space of a process
    // and creates an appropriate translation table for the new virtual address space.
    // a deep copy is created. with new frames being allocated for the new va space.
    // but having the same data as the corresponding previous va space frames.
    pub fn duplicate_va_space(ttbr0_val: Option<u64>) -> Result<u64, &'static str> {
        let src_translation_table_pa = match ttbr0_val {
            Some(pa) => pa,
            None => {
                let mut ttbr0: u64;
                unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0) };
                ttbr0 & 0x0000_FFFF_FFFF_F000
            }
        };

        let dst_translation_table_pa = match Self::get_free_frame_pa() {
            Some(pa) => pa as u64,
            None => return Err("No more memory to allocate new translation table for child process"),
        };

        mprintln!("[PAGE_ALLOC] Duplicating va space from ttbr0 {:#x} to new ttbr0 {:#x}", src_translation_table_pa, dst_translation_table_pa);

        unsafe {
            let dst_l1 = ttbr1_to_va!(dst_translation_table_pa) as *mut PageTable;
            (*dst_l1).entry.fill(0);
        }

        mprintln!("[PAGE_ALLOC] Starting recursive duplication of page tables...");

        if let Err(e) = Self::do_duplicate_table_recursive(src_translation_table_pa, dst_translation_table_pa, 1) {
            // Rollback: If we ran out of memory halfway through, free everything we've allocated so far
            Self::free_page_table(Some(dst_translation_table_pa));
            return Err(e);
        }

        Ok(dst_translation_table_pa)
    }

    // \TODO if memory is exhausted halfway through, the frames allocated so far are not freed.
    // if you're THAT out of memory then you probably have bigger issues than that, but none the less
    // this is a bug.
    fn do_duplicate_table_recursive(src_ttpa: u64, dst_ttpa: u64, level: u8) -> Result<(), &'static str> {
        if level > 3 {
            return Ok(());
        }

        mprintln!("[PAGE_ALLOC] Duplicating page table at level {}: src_ttpa {:#x}, dst_ttpa {:#x}", level, src_ttpa, dst_ttpa);

        let src_table = ttbr1_to_va!(src_ttpa) as *const PageTable;
        let dst_table = ttbr1_to_va!(dst_ttpa) as *mut PageTable;

        for i in 0..PAGE_ENTRIES {
            let src_entry = unsafe { (*src_table).entry[i] };
            
            // Check if the entry is valid (0b11)
            if src_entry & 0b11 == 0b11 { 
                let old_pa = src_entry & 0x0000_FFFF_FFFF_F000;
                let flags = src_entry & !0x0000_FFFF_FFFF_F000; // Isolate lower/upper attribute bits

                if level < 3 {
                    let new_table_pa = match Self::get_free_frame_pa() {
                        Some(pa) => pa as u64,
                        None => return Err("Out of memory: failed to allocate intermediate page table"),
                    };

                    mprintln!("[PAGE_ALLOC] Allocated new page table at level {}: new_table_pa {:#x} for src_entry {:#x}", level, new_table_pa, src_entry);

                    unsafe {
                        (*(ttbr1_to_va!(new_table_pa) as *mut PageTable)).entry.fill(0);
                        (*dst_table).entry[i] = new_table_pa | flags;
                    }

                    Self::do_duplicate_table_recursive(old_pa, new_table_pa, level + 1)?;
                } else {
                    let new_frame_pa = match Self::get_free_frame_pa() {
                        Some(pa) => pa as u64,
                        None => return Err("Out of memory: failed to allocate data frame for child process"),
                    };

                    unsafe {
                        let src_ptr = ttbr1_to_va!(old_pa) as *const u8;
                        let dst_ptr = ttbr1_to_va!(new_frame_pa) as *mut u8;
                        
                        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 4096);

                        (*dst_table).entry[i] = new_frame_pa | flags;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn free_page_table(ttbr0_val: Option<u64>) {
        let translation_table_pa = match ttbr0_val {
            Some(pa) => pa,
            None => {
                let mut ttbr0: u64;
                unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0) };
                ttbr0 & 0x0000_FFFF_FFFF_F000
            }
        };

        // recursively traverse the entire table for allocated valid page entries 
        Self::do_free_table_recursive(translation_table_pa, 1);        
    }

    fn do_free_table_recursive(ttpa: u64, level: u8) {
        if level > 3 {
            return;
        }

        mprintln!("[PAGE_ALLOC] Freeing page table at level {}: ttpa {:#x}", level, ttpa);

        let table = ttbr1_to_va!(ttpa) as *mut PageTable;
        for i in 0..PAGE_ENTRIES {
            let entry = unsafe { (*table).entry[i] };
            if entry & 0b11 == 0b11 { // valid entry
                if level < 3 {
                    Self::do_free_table_recursive(entry & 0x0000_FFFF_FFFF_F000, level + 1);
                } else {
                    unsafe { (*table).entry[i] = 0; } // invalidate the entry
                    Self::add_free_frame(ttbr1_to_va!(entry & 0x0000_FFFF_FFFF_F000));
                }
            }
        }
        
        Self::add_free_frame(ttbr1_to_va!(ttpa));        
    }

    // this one basically takes in virtual address of a frame and then 
    // frees it by first invalidating the page table entry by zeroing it,
    // and then adding the frame itself to the free frame list.
    pub fn free_page(va_in_page: usize, ttbr0_val: Option<u64>) {
        let translation_table_pa = match ttbr0_val {
            Some(pa) => pa,
            None => {
                let mut ttbr0: u64;
                unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0) };
                ttbr0 & 0x0000_FFFF_FFFF_F000
            }
        };

        let l1_i = (va_in_page >> 30) & 0x1FF;
        let l2_i = (va_in_page >> 21) & 0x1FF;
        let l3_i = (va_in_page >> 12) & 0x1FF;

        unsafe {
            let l1 = ttbr1_to_va!(translation_table_pa) as *mut PageTable;
            if (*l1).entry[l1_i] & 0b11 != 0b11 { return; } // invalid entry
            let l2 = ttbr1_to_va!((*l1).entry[l1_i] & 0x0000_FFFF_FFFF_F000) as *mut PageTable;
            if (*l2).entry[l2_i] & 0b11 != 0b11 { return; } // invalid entry
            let l3 = ttbr1_to_va!((*l2).entry[l2_i] & 0x0000_FFFF_FFFF_F000) as *mut PageTable;
            let l3_entry = (*l3).entry[l3_i];
            if l3_entry & 0b11 != 0b11 { return; } // invalid entry

            // Invalidate the page table entry
            (*l3).entry[l3_i] = 0;

            // Add the frame to the free frame list
            Self::add_free_frame(ttbr1_to_va!(l3_entry & 0x0000_FFFF_FFFF_F000));
        }
    }

    // basically takes in a virtual address (from ttbr0 va range) and allocates a page for 
    // it in the given/loaded ttbr0 translation table. if the page is already allocated, it panics.
    // returns the physical address of the frame in which the page is allocated.
    pub fn alloc_page(va: usize, ttbr0_val: Option<u64>) -> u64 {
        let translation_table_pa = match ttbr0_val {
            Some(pa) => pa,
            None => {
                let mut ttbr0: u64;
                unsafe { core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0) };
                ttbr0 & 0x0000_FFFF_FFFF_F000
            }
        };

        if let Some(_) = Self::ttbr0_to_pa(va, ttbr0_val) {
            panic!("Page already allocated at va: {:#x}", va);
        } 

        let l1_i = (va >> 30) & 0x1FF;
        let l2_i = (va >> 21) & 0x1FF;
        let l3_i = (va >> 12) & 0x1FF;

        const VALID: u64 = 1 << 0;
        const PAGE: u64 = 1 << 1;
        // 0b00 = EL1 RW, EL0 No Access 
        // 0b01 = EL1 RW, EL0 RW        
        const AP_EL0_RW: u64 = 0b01 << 6; 
        const SH_INNER: u64 = 0b10 << 8;
        const AF: u64 = 1 << 10;
        const PXN: u64 = 1 << 53; // don't try to run user space code in el1!!
        // const UXN: u64 = 1 << 54;
        const NG: u64 = 1 << 11;

        // trailing to the l3 entry with checks, but if any entry is invalid, we allocate a new page table for it and continue
        unsafe {
            let l1 = ttbr1_to_va!(translation_table_pa) as *mut PageTable;
            if (*l1).entry[l1_i] & 0b11 != 0b11 {  // if invalid, i.e. lower level table doesn't exist 
                Self::get_free_frame_pa().map(|new_l2_pa| {
                    (*l1).entry[l1_i] = new_l2_pa as u64 | 0b11; // valid, table
                    (*(ttbr1_to_va!(new_l2_pa) as *mut PageTable)).entry.fill(0); // zero out the new table
                }).expect("No free frames available for new L2 page table");
            } 

            let l2 = ttbr1_to_va!((*l1).entry[l1_i] & 0x0000_FFFF_FFFF_F000) as *mut PageTable;
            if (*l2).entry[l2_i] & 0b11 != 0b11 {  // if invalid, i.e. lower level table doesn't exist 
                Self::get_free_frame_pa().map(|new_l3_pa| {
                    (*l2).entry[l2_i] = new_l3_pa as u64 | 0b11; // valid, table
                    (*(ttbr1_to_va!(new_l3_pa) as *mut PageTable)).entry.fill(0); // zero out the new table
                }).expect("No free frames available for new L3 page table");
            } 

            let l3 = ttbr1_to_va!((*l2).entry[l2_i] & 0x0000_FFFF_FFFF_F000) as *mut PageTable;
            let l3_entry = (*l3).entry[l3_i];
            if l3_entry & 0b11 != 0b11 { 
                Self::get_free_frame_pa().map(|frame| {
                    (*l3).entry[l3_i] = frame as u64 | VALID | PAGE | AP_EL0_RW | SH_INNER | AF | PXN | NG; // valid, table
                }).expect("No free frames available for new page table entry");
            } else {
                panic!("Page already allocated at va: {:#x}", va);
            }

            (*l3).entry[l3_i] & 0x0000_FFFF_FFFF_F000
        }

    }
    
    // this function most certainly pops a free frame from the free list in fact. 
    // make sure any frame you ask you eventually return to the free list i suppose! 
    fn get_free_frame_pa() -> Option<usize> {
        unsafe {
            if let Some(free_frame) = FREE_FRAME_LIST {
                let free_frame_va = free_frame as usize;
                FREE_FRAME_LIST = (*free_frame).next;
                FREE_FRAME_COUNT -= 1;
                Some(ttbr1_to_pa!(free_frame_va))
            } else {
                None
            }
        }
    }

    fn zero_frame(frame_va: usize) {
        unsafe {
            core::ptr::write_bytes(frame_va as *mut u8, 0, PAGE_SIZE);
        }
    }
    
    pub fn add_free_frame(va_in_frame: usize) -> () {
        let free_frame_va = va_in_frame & !(PAGE_SIZE - 1);
        let free_frame = free_frame_va as *mut FreeFrame;
        Self::zero_frame(free_frame_va);
        mprintln!("[PAGE_ALLOC] Adding free frame at va: {:#x}", free_frame_va);
        unsafe { (*free_frame).next = FREE_FRAME_LIST;
                 FREE_FRAME_LIST = Some(free_frame);
                 FREE_FRAME_COUNT += 1; };
    }

    // run at boot. marks all frames as free and adds them to the free frame list
    pub fn init_frames(first_free_frame_va: usize, last_frame_va_limit: usize) -> () {
        for frame_va in (first_free_frame_va..last_frame_va_limit).step_by(PAGE_SIZE) {
            PageAllocator::add_free_frame(frame_va);
        }
        unsafe {TOTAL_FRAME_COUNT = FREE_FRAME_COUNT;}
    }
}

pub fn available_memory() -> usize {
    unsafe { FREE_FRAME_COUNT * PAGE_SIZE }
}

pub fn total_memory() -> usize {
    unsafe { TOTAL_FRAME_COUNT * PAGE_SIZE }
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

// now finally some macros to assist with the kernel va and pa conversions
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