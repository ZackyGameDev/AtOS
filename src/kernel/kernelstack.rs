/*
There needs to be a separate kernel stack for each process when they 
execute syscalls. For that we are going to maintain the different process
stacks, and allocate them in this module.
*/

use crate::kernel::{processes::MAX_PROCESSES, spinlock::Spinlock};
use crate::kernel::paging::PageAllocator;
use crate::kernel::paging::PAGE_TABLE_KERNEL_L1;
use crate::ttbr1_to_pa;

// Each process will have a kernel stack of 16KB (4 pages)
pub const KERNEL_STACK_SIZE: usize = 0x4000; // this MUST be 4096 aligned to align with paging boundaries

// i'm not going to just make a mutable static array of stacks
// because that is a huge waste of memory

// since the first 2 GB of the ttbr1 VA space are already mapped,
// we will use the memory immediately after that region for 
// the kernel stacks. Which starts from VA `0xFFFF_FF80_8000_0000`. 
// So, `KERNEL_STACK_SIZE * MAX_PROCESSES` size of region starting
// from that address will be used for kernel stacks for each process.

// we only now need to keep track of which of those are allocated 
// and which are not being used.

pub const KERNEL_STACK_REGION_START: u64 = 0xFFFF_FF80_8000_0000;

pub static mut KERNEL_STACKS_ALLOCATED: [Option<u64>; MAX_PROCESSES] = [None; MAX_PROCESSES];
pub static KERNEL_STACK_TABLE_LOCK: Spinlock = Spinlock::new("kernel_stack_table");

// here None will represent that the stack region is free. Option<u64> will represent the pid of
// the process currently using that stack region. The index of the array will represent the stack number.

pub struct KernelStack;

impl KernelStack {

pub fn duplicate_stack(src_pid: u64, dest_pid: u64) -> Result<u64, &'static str> {
        let src_stack_index = Self::get_stack_index(src_pid).ok_or("Source process does not have a kernel stack allocated")?;
        let dest_stack_index = Self::get_stack_index(dest_pid).ok_or("Destination process does not have a kernel stack allocated")?;

        let src_stack_start = KERNEL_STACK_REGION_START + (src_stack_index as u64) * (KERNEL_STACK_SIZE as u64);
        let dest_stack_start = KERNEL_STACK_REGION_START + (dest_stack_index as u64) * (KERNEL_STACK_SIZE as u64);

        // Calculate the starting offset of the active stack area (skipping page 0)
        let active_stack_offset = 4096;
        let active_stack_size = KERNEL_STACK_SIZE - active_stack_offset;

        // Copy ONLY the mapped pages of the source stack to the destination stack
        unsafe {
            core::ptr::copy_nonoverlapping(
                (src_stack_start + active_stack_offset as u64) as *const u8,
                (dest_stack_start + active_stack_offset as u64) as *mut u8,
                active_stack_size,
            );
        }

        Ok(dest_stack_start + (KERNEL_STACK_SIZE as u64)) // Return the top of the destination stack
    }

    pub fn get_stack_index(pid: u64) -> Option<usize> {
        for i in 0..MAX_PROCESSES {
            unsafe {
                if KERNEL_STACKS_ALLOCATED[i] == Some(pid) {
                    return Some(i);
                }
            }
        }
        None
    }

    // the result can be used as sp_el1
    pub fn alloc_stack(pid: u64) -> Result<u64, &'static str> {
        if Self::get_stack_index(pid).is_some() {
            return Err("Process already has a kernel stack allocated");
        }

        for i in 0..MAX_PROCESSES {
            unsafe {
                if KERNEL_STACKS_ALLOCATED[i].is_none() {
                    KERNEL_STACKS_ALLOCATED[i] = Some(pid);
                    let kernel_stack_start = KERNEL_STACK_REGION_START + (i as u64) * (KERNEL_STACK_SIZE as u64);

                    let total_pages = KERNEL_STACK_SIZE as u64 / 4096;

                    for page_idx in 1..total_pages { // zero page is not allocated to defend from stack overflow.
                        let page_va = kernel_stack_start + (page_idx * 4096);
                        PageAllocator::alloc_page(page_va as usize, Some(ttbr1_to_pa!(core::ptr::addr_of!(PAGE_TABLE_KERNEL_L1)) as u64));
                    }

                    let kernel_stack_top = kernel_stack_start + (KERNEL_STACK_SIZE as u64); // when allocating on stack, the sp is  
                    return Ok(kernel_stack_top);                                                // decreased *first*, so no need to subtract 16
                }
            }
        }
        Err("No available kernel stacks")
    }

    pub fn free_stack(pid: u64) -> Result<(), &'static str> {
        for i in 0..MAX_PROCESSES {
            unsafe {
                if KERNEL_STACKS_ALLOCATED[i] == Some(pid) {
                    let kernel_stack_start = KERNEL_STACK_REGION_START + (i as u64) * (KERNEL_STACK_SIZE as u64);
                    let total_pages = KERNEL_STACK_SIZE as u64 / 4096;

                    for page_i in 1..total_pages {
                        let page_va = kernel_stack_start + (page_i * 4096);
                        PageAllocator::add_free_frame(page_va as usize);
                    }

                    KERNEL_STACKS_ALLOCATED[i] = None;
                    return Ok(());
                }
            }
        }
        Err("No active kernel stack found for the given PID")
    }
}