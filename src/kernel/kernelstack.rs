/*
There needs to be a separate kernel stack for each process when they 
execute syscalls. For that we are going to maintain the different process
stacks, and allocate them in this module.
*/

use core::ptr::{read_volatile, write_volatile};

use crate::kernel::{processes::MAX_PROCESSES, spinlock::Spinlock};
use crate::kernel::paging::{PageAllocator, PageTable};
use crate::kernel::paging::PAGE_TABLE_KERNEL_L1;
use crate::{dprintln, ttbr1_to_pa, ttbr1_to_va};

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

pub const KERNEL_STACK_REGION_START: u64 = 0xFFFF_FF80_8000_0000 + 0x4000; // first 0x4000 bytes are served for kernel stack
                                                                          // which will be treated like process 0's kernel stack. 

pub static mut KERNEL_STACKS_ALLOCATED: [Option<u64>; MAX_PROCESSES] = [None; MAX_PROCESSES];
// pub static KERNEL_STACK_TABLE_LOCK: Spinlock = Spinlock::new("kernel_stack_table");

// here None will represent that the stack region is free. Option<u64> will represent the pid of
// the process currently using that stack region. The index of the array will represent the stack number.

pub struct KernelStack;

impl KernelStack {

    pub fn duplicate_stack(src_pid: u64, dest_pid: u64, src_sp_el1: u64) -> Result<u64, &'static str> {
        let dst_stack_top = match Self::get_stack_top(dest_pid) {
            Ok(top) => top,
            Err(_) => Self::alloc_stack(dest_pid)?,
        };

        let active_size = Self::get_stack_usage(src_pid, src_sp_el1)?;
        let dst_sp_el1 = dst_stack_top - (active_size as u64);

        unsafe {
            core::ptr::copy_nonoverlapping(
                src_sp_el1 as *const u8,
                dst_sp_el1 as *mut u8,
                active_size,
            );
        }

        Ok(dst_sp_el1)
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

    pub fn get_stack_usage(pid: u64, sp_el1_val: u64) -> Result<usize, &'static str> {
        let stack_top = Self::get_stack_top(pid)?;

        if sp_el1_val > stack_top {
            return Err("[get_stack_usage] kernel stack underflow! (how did this happen bro)");
        }

        Ok((stack_top - sp_el1_val) as usize)
    }

    pub fn get_stack_top(pid: u64) -> Result<u64, &'static str> {
        let index = Self::get_stack_index(pid).ok_or("[get_stack_top] Process does not have a kernel stack allocated")?;
        let stack_start = KERNEL_STACK_REGION_START + (index as u64) * (KERNEL_STACK_SIZE as u64);
        Ok(stack_start + (KERNEL_STACK_SIZE as u64)) // Return the top of the stack
    }

    // the result can be used as sp_el1
    pub fn alloc_stack(pid: u64) -> Result<u64, &'static str> {
        dprintln!("[alloc_stack] Allocating kernel stack for pid {}", pid);
        if Self::get_stack_index(pid).is_some() {
            return Err("[alloc_stack] Process already has a kernel stack allocated");
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


                    let kernel_stack_top = kernel_stack_start + (KERNEL_STACK_SIZE as u64); 
                    
                    // testing if page was allocated
                    write_volatile((kernel_stack_top - 0x10) as *mut u64, 0x123456789ABCDEF0); // would cause page fault if page was not allocated properly
                    let value = read_volatile((kernel_stack_top - 0x10) as *const u64); 
                    dprintln!("[alloc_stack] Value read from kernel stack: {:<16X}", value);

                    dprintln!("[alloc_stack] Allocated kernel stack for pid {} at VA: {:<16X}", pid, kernel_stack_start);
                    return Ok(kernel_stack_top);
                }
            }
        }
        Err("[alloc_stack] No available kernel stacks")
    }

    pub fn free_stack(pid: u64) -> Result<(), &'static str> {
        let i = Self::get_stack_index(pid).ok_or({
            dprintln!("[free_stack] Process {} does not have a kernel stack allocated", pid);
            "[free_stack] Process does not have a kernel stack allocated"
        })?;
        unsafe {
            let kernel_stack_start = KERNEL_STACK_REGION_START + (i as u64) * (KERNEL_STACK_SIZE as u64);
            let total_pages = KERNEL_STACK_SIZE as u64 / 4096;

            let ttbr1_val = &raw const PAGE_TABLE_KERNEL_L1 as *const PageTable as u64;

            for page_i in 1..total_pages {
                let page_va = kernel_stack_start + (page_i * 4096);
                PageAllocator::free_page(page_va as usize, Some(ttbr1_val)); 
            }

            KERNEL_STACKS_ALLOCATED[i] = None;
            dprintln!("[free_stack] Freed kernel stack for pid {} at VA: {:<16X}", pid, kernel_stack_start);
            Ok(())
        }
    }
}