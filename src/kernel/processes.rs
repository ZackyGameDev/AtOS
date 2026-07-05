#![allow(static_mut_refs)]

use crate::kernel::exceptions::ExceptionContext;

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

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(unused)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
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
    pub fn new(entry_point: u64, sp: u64, ttbr0: u64) -> Self {
        Self { elr: entry_point, sp, ttbr0, ..Default::default() }
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
    // identity
    pub pid: u64,
    pub name: [u8; 32],
    pub state: ProcessState,
    pub parent_pid: u64, // initially this was &Parent but then i'd have to deal with rust lifetime complications
    pub pctx: ProcessContext,
    pub chan: u64,
}

impl Process {
    pub fn new(name: &str, parent_pid: u64, entry_point: u64, sp: u64, ttbr0: u64) -> Self {
        let pid = unsafe {
            let pid = NEXT_PID;
            NEXT_PID += 1;
            pid
        };

        let mut name_bytes = [0u8; 32];
        let bytes = name.as_bytes();

        let len = core::cmp::min(bytes.len(), 32);
        name_bytes[..len].copy_from_slice(&bytes[..len]);

        Self { pid,
               name: name_bytes,
               state: ProcessState::Ready,
               parent_pid,
               pctx: ProcessContext::new(entry_point, sp, ttbr0),
               chan: 0, }
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
