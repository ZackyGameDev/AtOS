#![allow(static_mut_refs, unused)]

use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::paging::PageAllocator;
use crate::kernel::spinlock::Spinlock;
use crate::dprintln;

// Restrictions for current hardware
pub const MAX_PROCESSES: usize = 50;
pub const MAX_CPUS: usize = 4;

#[derive(Debug)]
pub struct Cpu {
    pub cid: usize,
    pub current_pid: Option<u64>,
    pub ncli: usize, // Depth of nested spinlocks
    pub were_interrupts_enabled: bool, // Interrupt state before first lock
}

pub static mut CPUS: [Cpu; MAX_CPUS] = [
    Cpu { cid: 0, current_pid: None, ncli: 0, were_interrupts_enabled: false },
    Cpu { cid: 1, current_pid: None, ncli: 0, were_interrupts_enabled: false },
    Cpu { cid: 2, current_pid: None, ncli: 0, were_interrupts_enabled: false },
    Cpu { cid: 3, current_pid: None, ncli: 0, were_interrupts_enabled: false },
];

impl Cpu {
    pub fn current() -> &'static mut Self {
        let mpidr: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr, options(nostack, preserves_flags));

            // Core ID is stored in "Affinity Level" (wtf is that name) bits 0-7
            let core_id = (mpidr & 0xFF) as usize;
            &mut CPUS[core_id]
        }
    }

    // Minimal interface mutations required by the scheduler
    #[inline(always)]
    pub fn set_current_process(&mut self, pid: u64) { self.current_pid = Some(pid); }

    #[inline(always)]
    pub fn clear_current_process(&mut self) { self.current_pid = None; }

}

#[inline(always)]
pub fn mycpu() -> &'static mut Cpu {
    Cpu::current()
}

// Process Table

pub static mut PROCESS_TABLE: [Option<Process>; MAX_PROCESSES] = [None; MAX_PROCESSES];
pub static mut NEXT_PID: u64 = 1; // 0 could be for kernel

// Global spinlock protecting PROCESS_TABLE and related parent/child checks.
pub static PROCESS_TABLE_LOCK: Spinlock = Spinlock::new("process_table");

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlockReason {
    WaitingForChild,
    // ... add more as needed
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessContext {
    pub x: [u64; 31],
    pub sp: u64,
    pub elr: u64, // address to jump to when jumping to this process
    pub spsr: u64,
    pub ttbr0: u64, // translation table base address for this process's user space
}

impl ProcessContext {
    pub const fn new(entry_point: u64, sp: u64, ttbr0: u64) -> Self {
        Self {
            x: [0; 31],
            sp,
            elr: entry_point,
            spsr: 0,
            ttbr0,
        }
    }

    pub fn from_ectx(ctx: &ExceptionContext) -> Self {
        let mut pctx = Self::new(ctx.elr, ctx.sp_el0, ctx.ttbr0);
        pctx.x.copy_from_slice(&ctx.x);
        pctx.spsr = ctx.spsr;
        pctx
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Process {
    pub pid: u64,
    pub name: [u8; 32],
    pub state: ProcessState,
    pub block_reason: Option<BlockReason>,
    pub parent_pid: u64, // initially this was &Parent but then i'd have to deal with rust lifetime complications
    pub pctx: ProcessContext,
    pub chan: u64,
    pub exit_code: i64, // for terminated processes
}

// Flat definition to be used by other functions to find a process (when possible)
fn find_index_by_id(pid: u64) -> Option<usize> {
    unsafe {
        for (i, slot) in PROCESS_TABLE.iter().enumerate() {
            if let Some(proc) = slot {
                if proc.pid == pid {
                    return Some(i);
                }
            }
        }
    }
    None
}

impl Process {
    pub fn new(name: &str, parent_pid: u64, entry_point: u64, sp: u64, ttbr0: u64) -> Self {
        let pid = unsafe {
            let id = NEXT_PID;
            NEXT_PID += 1;
            id
        };

        let mut name_bytes = [0u8; 32];
        let bytes = name.as_bytes();
        let len = if bytes.len() > 32 { 32 } else { bytes.len() };
        name_bytes[..len].copy_from_slice(&bytes[..len]);

        Self {
            pid,
            name: name_bytes,
            state: ProcessState::Ready,
            block_reason: None,
            parent_pid,
            pctx: ProcessContext::new(entry_point, sp, ttbr0),
            chan: 0,
            exit_code: 0,
        }
    }

    // Very self explanatory. This function loads an elf file into memory and
    // creates a process for it.  It returns the pid of the newly created
    // process.
    pub fn spawn_from_elf(name: &str, parent_pid: u64, bytes: &'static [u8]) -> Result<u64, &'static str> {
        let (entry_point, stack_top, ttbr0) = PageAllocator::load_elf(bytes)?;
        let process = Process::new(name, parent_pid, entry_point, stack_top, ttbr0);
        let pid = process.pid;

        add_process_to_ptable(process)?;
        Ok(pid)
    }

    pub fn fork(&self) -> Result<Process, &'static str> {
        let new_ttbr0 = PageAllocator::duplicate_va_space(Some(self.pctx.ttbr0))?;

        let new_pid = unsafe {
            let pid = NEXT_PID;
            NEXT_PID += 1;
            pid
        };

        let mut new_pctx = self.pctx;
        new_pctx.ttbr0 = new_ttbr0;

        dprintln!("[PROC_FORK] parent process is: {:?}", self);        
        dprintln!("[PROC_FORK] child process context will be: {:?}", new_pctx);

        // this function does NOT set the return value in x0 for child process or parent process.
        // that part is supposed to be done by the syscall implementation. as the decision for what
        // to return in which register is made there, not here.
        Ok(Self { pid: new_pid,
                  name: self.name,
                  state: ProcessState::Ready,
                  block_reason: None,
                  parent_pid: self.pid,
                  pctx: new_pctx,
                  chan: 0,
                  exit_code: 0, } )
    }

    pub fn exec(&mut self, elf_bytes: &'static [u8]) -> Result<(), &'static str> {
        let (entry_point, stack_top, new_ttbr0) = PageAllocator::load_elf(elf_bytes)?;

        // Free the old page table
        PageAllocator::free_page_table(Some(self.pctx.ttbr0));

        // Update the process context with the new entry point, stack pointer, and page table
        self.pctx = ProcessContext::new(entry_point, stack_top, new_ttbr0);

        Ok(())
    }

    pub fn block(&mut self, reason: BlockReason) {
        self.state = ProcessState::Blocked;
        self.block_reason = Some(reason);
    }

    pub fn terminate(&mut self, exit_code: i64) {
        self.exit_code = exit_code;
        self.state = ProcessState::Terminated; // \TODO currently terminated processes stay indefinitely process table.
        PageAllocator::free_page_table(Some(self.pctx.ttbr0));
    }

    // Minimal state mutation utils required by the scheduler and syscalls (in order to not break those completely on refactor)
    #[inline(always)]
    pub fn set_state(&mut self, state: ProcessState) { self.state = state; }

    #[inline(always)]
    pub fn set_pctx(&mut self, ctx: ProcessContext) { self.pctx = ctx; }


    // Utils
    pub fn get_current() -> Option<&'static mut Process> {
        let current_id = Cpu::current().current_pid?;
        Self::find_by_id(current_id)
    }

    pub fn find_by_id(pid: u64) -> Option<&'static mut Process> {
        let i = find_index_by_id(pid)?;
        unsafe { PROCESS_TABLE[i].as_mut() }
    }

    pub fn find_by_ptable_index(index: usize) -> Option<&'static mut Process> {
        if index >= MAX_PROCESSES { return None; }
        unsafe { PROCESS_TABLE[index].as_mut() }
    }
}

// Global Mutations
pub fn add_process_to_ptable(process: Process) -> Result<(), &'static str> {
    unsafe {
        for slot in PROCESS_TABLE.iter_mut() {
            if slot.is_none() {
                *slot = Some(process);
                return Ok(());
            }
        }
    }
    Err("Process table full")
}

pub fn remove_process_from_ptable(pid: u64) -> Result<(), &'static str> {
    if let Some(idx) = find_index_by_id(pid) {
        unsafe {
            PROCESS_TABLE[idx] = None;
        }
        return Ok(());
    }
    Err("Process not found")
}
