#![allow(static_mut_refs, unused)]

use core::default;

use crate::{dprintln, kernel, print};
use crate::kernel::{
    exceptions::{self, ExceptionContext},
    paging::PageAllocator,
    spinlock::Spinlock,
    kernelstack::KernelStack,
};

pub const MAX_PROCESSES: usize = 50;
pub const MAX_CPUS: usize = 1; // for now

//
// CPU Abstraction
//

pub struct Cpu {
    pub cid: usize,
    pub current_pid: Option<u64>, // Tracking the running proc by id

    pub ncli: usize, // Depth of nested spinlocks held on this CPU
    pub interrupts_enabled: bool, // Were interrupts enabled BEFORE the very first lock?
}

impl Cpu {
    // This gets called by current to get raw physical address. get_instance()
    // doesn't know anything about CPU cores. Its only job is to manage static
    // memory. This is like returning the base address of the global Cpu
    // structs.
    fn get_instance() -> &'static mut [Cpu; MAX_CPUS] {
        static mut CPUS: [Cpu; MAX_CPUS] = [
            Cpu { cid: 0, ncli: 0, interrupts_enabled: false, current_pid: None }
        ];
        unsafe { &mut CPUS }
    }

    // This calls get_instance to get the base address and then (when we do have
    // multiple cores) indexes into the correct Cpu structure belonging to that
    // specific core.
    pub fn current() -> &'static mut Self {
        // For now. Later, we'll check mpidr_el1
        let cpus = Self::get_instance();
        &mut cpus[0]
    }

    pub fn set_current_process(&mut self, pid: u64) {
        self.current_pid = Some(pid);
    }

    pub fn clear_current_process(&mut self) {
        self.current_pid = None;
    }
}

// This will act as a nice utility function without going into Cpu methods.
pub fn mycpu() -> &'static mut Cpu {
    Cpu::current()
}

//
// Processes
//

pub static mut PROCESS_TABLE: [Option<Process>; MAX_PROCESSES] = [None; MAX_PROCESSES];
pub static mut NEXT_PID: u64 = 1; // 0 could be for kernel
// Global spinlock protecting PROCESS_TABLE and related parent/child checks.
pub static PROCESS_TABLE_LOCK: Spinlock = Spinlock::new("process_table");

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(unused)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(unused)]
pub enum BlockReason {
    WaitingForChild,
    // ... add more as needed
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessContext {
    pub x: [u64; 31],
    pub elr: u64, // address to jump to when jumping to this process
    pub spsr: u64,
    pub sp_el0: u64,
    pub sp_el1: u64,
    pub ttbr0: u64, // translation table base address for this process's user space
}

impl ProcessContext {
    pub fn new(entry_point: u64, user_sp: u64, kernel_sp: u64, ttbr0: u64) -> Self {
        Self { elr: entry_point, sp_el0: user_sp, sp_el1: kernel_sp, ttbr0, ..Default::default() }
    }

    pub fn from_ectx(ctx: &ExceptionContext) -> Self {
        let mut pctx = Self::new(ctx.elr, ctx.sp_el0, ctx.sp_el1, ctx.ttbr0);
        pctx.x.copy_from_slice(&ctx.x);
        pctx.spsr = ctx.spsr;
        pctx
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Process {
    // identity
    pub pid: u64,
    pub name: [u8; 32],
    pub state: ProcessState,
    pub block_reason: Option<BlockReason>,
    pub parent_pid: u64, // initially this was &Parent but then i'd have to deal with rust lifetime complications
    pub pctx: ProcessContext,
    pub chan: u64,
    pub exit_code: i64, // for terminated processes
}

impl Process {
    pub fn new(name: &str, parent_pid: u64, entry_point: u64, user_sp: u64, ttbr0: u64) -> Result<Self, &'static str> {
        let pid = unsafe {
            let pid = NEXT_PID;
            NEXT_PID += 1;
            pid
        };

        let mut name_bytes = [0u8; 32];
        let bytes = name.as_bytes();

        let len = core::cmp::min(bytes.len(), 32);
        name_bytes[..len].copy_from_slice(&bytes[..len]);

        let kernel_sp = KernelStack::alloc_stack(pid)?;

        dprintln!("New process created with PID: {}, name: '{}', entry_point: {:#x}, user_sp: {:#x}, kernel_sp: {:#x}, ttbr0: {:#x}",
                  pid, name, entry_point, user_sp, kernel_sp, ttbr0);

        Ok(Self { pid,
               name: name_bytes,
               state: ProcessState::Ready,
               block_reason: None,
               parent_pid,
               pctx: ProcessContext::new(entry_point, user_sp, kernel_sp, ttbr0),
               chan: 0,
               exit_code: 0, })
    }

    // very self explanatory. this function loads an elf file into memory and creates a process for it.
    // it returns the pid of the newly created process.
    pub fn spawn_from_elf(name: &str, parent_pid: u64, bytes: &'static [u8], args: &[&str]) -> Result<u64, &'static str> {
        let (entry_point, stack_top, ttbr0) = PageAllocator::load_elf(bytes)?;

        let mut process = match Process::new(name, parent_pid, entry_point, stack_top, ttbr0) {
            Ok(process) => process,
            Err(e) => {
                PageAllocator::free_page_table(Some(ttbr0)); // free page table of failed process.
                return Err(e)
            }
        };

        process.initialize_args(args)?;

        if let Err(e) = add_process_to_ptable(process) {
            PageAllocator::free_page_table(Some(ttbr0)); // free page table of failed process.
            return Err(e)
        }

        Ok(process.pid)
    }

    pub fn fork(&self) -> Result<Process, &'static str> {

        dprintln!("[PROC_FORK] duplicating process pid {}, name \"{:?}\"", self.pid, self.name);

        let new_ttbr0 = PageAllocator::duplicate_va_space(Some(self.pctx.ttbr0))?;

        let new_pid = unsafe {
            let pid = NEXT_PID;
            NEXT_PID += 1;
            pid
        };

        dprintln!("[PROC_FORK] now kernel dupe pid {}, name \"{:?}\" into new process pid {}", self.pid, self.name, new_pid);
        
        let new_kernel_sp = KernelStack::duplicate_stack(self.pid, new_pid, self.pctx.sp_el1)?;

        let mut new_pctx = self.pctx;
        new_pctx.ttbr0 = new_ttbr0;
        new_pctx.sp_el1 = new_kernel_sp;

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
    
    pub fn exec(&mut self, elf_bytes: &'static [u8], args: &[&str]) -> Result<(), &'static str> {
        let (entry_point, stack_top, new_ttbr0) = PageAllocator::load_elf(elf_bytes)?;

        // Free the old page table
        PageAllocator::free_page_table(Some(self.pctx.ttbr0));

        // clear stack
        KernelStack::queue_stack_to_free(self.pid)?;
        let new_kernel_sp = KernelStack::alloc_stack(self.pid)?; // allocate a new stack for the process

        // Update the process context with the new entry point, stack pointer, and page table
        self.pctx = ProcessContext::new(entry_point, stack_top, new_kernel_sp, new_ttbr0);
        self.initialize_args(args)?;

        Ok(())
    }
    
    pub fn initialize_args(&mut self, args: &[&str]) -> Result<(), &'static str> {
        let (argc, new_sp, sp_top) = self.push_args_to_stack(args)?;
        self.pctx.x[0] = argc;
        self.pctx.x[1] = new_sp;
        self.pctx.x[2] = sp_top;
        Ok(())
    }

    fn push_args_to_stack(&mut self, args: &[&str]) -> Result<(u64, u64, u64), &'static str> {
        // since the user's ttbr0 may not be loaded, we first write everything to a buffer

        /*
        
        For execution args, we will layout the arguments data on the user stack as follows:

        Let's say arguments are "program hello world"

        STACK TOP ADDRESS (initial sp)
        ┌────────────────────┐
        │ "program"          │  ← argv[0]
        ├────────────────────┤
        │ "hello"            │  ← argv[1]
        ├────────────────────┤
        │ "world"            │  ← argv[2]
        ├────────────────────┤
        │ padding            │
        ├────────────────────┤
        │ offset → "world"   │  ← &argv[2] - final sp
        ├────────────────────┤
        │ offset → "hello"   │  ← &argv[1] - final sp
        ├────────────────────┤
        │ offset → "program" │  ← &argv[0] - final sp
        └────────────────────┘
        Final SP
        
        Where the runtime is given the stack top, and final sp values, along with number of args (argc).
        
        */

        dprintln!("[PROC_ARGS] Pushing {} args to stack for process pid {}, name \"{:?}\"", args.len(), self.pid, self.name);

        const STACK_BUF_SIZE: usize = 4096;
        const MAX_ARGS: usize = 64;

        if args.len() > MAX_ARGS {
            return Err("too many arguments");
        }

        let mut image = [0u8; STACK_BUF_SIZE];
        let mut arg_starts = [0u64; MAX_ARGS];
        let mut sp = STACK_BUF_SIZE;

        for (i, arg) in args.iter().enumerate() {
            let bytes = arg.as_bytes();

            if sp < bytes.len() {
                return Err("arguments exceed stack");
            }

            sp -= bytes.len();
            image[sp..sp + bytes.len()].copy_from_slice(bytes);
            arg_starts[i] = sp as u64;
        }

        sp &= !0xF;

        if sp < args.len() * size_of::<u64>() {
            return Err("arguments exceed stack");
        }

        sp -= args.len() * size_of::<u64>();

        for (i, arg_start) in arg_starts[..args.len()].iter().enumerate() {
            let offset = *arg_start - sp as u64;
            let dst = sp + i * size_of::<u64>();

            image[dst..dst + 8].copy_from_slice(&offset.to_ne_bytes());
        }

        let stack_top = self.pctx.sp_el0;
        let new_sp = stack_top - (STACK_BUF_SIZE - sp) as u64;

        PageAllocator::copy_to_pages(
            &image[sp..],
            new_sp,
            Some(self.pctx.ttbr0),
        )?;

        self.pctx.sp_el0 = new_sp;

        Ok((args.len() as u64, new_sp, stack_top))
    }

    pub fn block(&mut self, reason: BlockReason) {
        self.set_state(ProcessState::Blocked);
        self.block_reason = Some(reason);
    }

    pub fn terminate(&mut self, exit_code: i64) {
        self.exit_code = exit_code;
        KernelStack::queue_stack_to_free(self.pid).unwrap();
        self.set_state(ProcessState::Terminated); // \TODO currently terminated processes stay indefinitely process table.
        PageAllocator::free_page_table(Some(self.pctx.ttbr0));
    }

    pub fn set_state(&mut self, new_state: ProcessState) {
        self.state = new_state;
    }

    pub fn set_pctx(&mut self, new_ctx: ProcessContext) {
        self.pctx = new_ctx;
    }

    pub fn get_current() -> Option<&'static mut Process> {
        let cpu = Cpu::current();
        let current_id = cpu.current_pid?;

        unsafe {
            for slot in PROCESS_TABLE.iter_mut() {
                if let Some(proc) = slot {
                    if proc.pid == current_id {
                        return Some(proc);
                    }
                }
            }
        }
        None
    }

    pub fn find_by_ptable_index(index: usize) -> Option<&'static mut Process> {
        if index >= MAX_PROCESSES {
            return None;
        }
        unsafe {
            PROCESS_TABLE[index].as_mut()
        }
    }

    pub fn find_by_id(pid: u64) -> Option<&'static mut Process> {
        unsafe {
            for slot in PROCESS_TABLE.iter_mut() {
                if let Some(proc) = slot {
                    if proc.pid == pid {
                        return Some(proc);
                    }
                }
            }
        }
        None
    }
}

pub fn add_process_to_ptable(process: Process) -> Result<(), &'static str> {
    unsafe {
        for slot in PROCESS_TABLE.iter_mut() {
            if slot.is_none() {
                *slot = Some(process);
                return Ok(());
            }
        }
    }
    Err("Process table is full")
}

pub fn remove_process_from_ptable(pid: u64) -> Result<(), &'static str> {
    unsafe {
        for slot in PROCESS_TABLE.iter_mut() {
            if let Some(proc) = slot {
                if proc.pid == pid {
                    *slot = None;
                    return Ok(());
                }
            }
        }
    }
    Err("Process not found")
}

/* 

Below methods are deprecated. They only worked before paging was implemented.

pub fn load_process(process_name: &str, parent_pid: u64, process_image: &'static [u8], process_addr: u64, entry_point: u64) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            process_image.as_ptr(),
            process_addr as *mut u8,
            process_image.len(),
        );
    }

    // i got this entry point from the compiled init elf.
    let stack_top: u64 = (process_addr + process_image.len() as u64 + 0x4000) & !0xf; // 16 byte aligned stack top
    // right now i have just hardcoded some stack pointer for EL0

    let process: Process = Process::new(process_name, parent_pid, entry_point, stack_top);

    if let Err(e) = add_process_to_ptable(process) {
        dprintln!("{}", e);
        panic!("load_process: {}", e);
    }

}

// Made a new one for "backwards compatibility" until it gets merged
pub fn load_elf_process(process_name: &str, parent_pid: u64, bytes: &'static [u8]) {
    let header = match Elf64Hdr::mkfrombytes(bytes) {
        Some(h) => h,
        None => {
            dprintln!("load_elf_process: invalid elf file header '{}'",
                      process_name);
            return;
        }
    };

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
            // Since paging is under construction, use the physical address
            let dst = ph.phyaddr;

            unsafe {
                let src = bytes.as_ptr().add(ph.offset as usize);

                // copy init code and data from elf to ram
                core::ptr::copy_nonoverlapping(
                    src,
                    dst as *mut u8,
                    ph.filesize as usize
                );

                if ph.memsize > ph.filesize {
                    let bss_start = dst + ph.filesize;
                    let bss_size = ph.memsize - ph.filesize;
                    core::ptr::write_bytes(bss_start as *mut u8, 0, bss_size as usize);
                }
            }

            // have segment_end used by the program to position stack
            let segment_end = dst + ph.memsize;
            if segment_end > max_allocated_addr {
                max_allocated_addr = segment_end;
            }

            loaded_any_segments = true;
        }
    }
    
    if !loaded_any_segments {
        dprintln!("load_elf_process: no loadable segment found in '{}'", process_name);
        return;
    }

    let entry_point = header.entry;

    // set stack top to just above the highest allocated program segment 16-byte aligned
    let stack_top: u64 = (max_allocated_addr + 0x4000) & !0xf;

    let process: Process = Process::new(process_name, parent_pid, entry_point, stack_top);
    if let Err(e) = add_process_to_ptable(process) {
        dprintln!("{}", e);
        panic!("load_elf_process: {}", e);
    }
}

*/
