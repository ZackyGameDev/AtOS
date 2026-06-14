#![allow(static_mut_refs)]

use crate::kernel::exceptions::ExceptionContext;

pub const MAX_PROCESSES: usize = 50;

//
// CPU Abstraction
//

pub struct Cpu {
    pub current_pid: Option<u64>, // Tracking the running proc by id
    
    pub ncli: usize, // Depth of nested spinlocks held on this CPU
    pub interrupts_enabled: bool, // Were interrupts enabled BEFORE the very first lock?
}

impl Cpu {
    fn get_instance() -> &'static mut Self {
	static mut MYCPU: Cpu = Cpu {
	    ncli: 0,
	    interrupts_enabled: false,
	    current_pid: None,
	};
	unsafe { &mut MYCPU }
    }

    pub fn current() -> &'static mut Self {
	Self::get_instance()
    }

    pub fn set_current_process(&mut self, pid: u64) {
	self.current_pid = Some(pid);
    }

    pub fn clear_current_process(&mut self) {
	self.current_pid = None;
    }
}

// @Todo(cleanup): You can remove this, but you'd have to fix the references.
pub fn mycpu() -> &'static mut Cpu {
    Cpu::current()
}

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
}

impl ProcessContext {
    pub fn new(entry_point: u64, sp: u64) -> Self {
        Self { elr: entry_point, sp, ..Default::default() }
    }

    pub fn from_ectx(ctx: &ExceptionContext, sp: u64) -> Self {
        let mut pctx = Self::new(ctx.elr, sp);
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
    pub fn new(name: &str, parent_pid: u64, entry_point: u64, sp: u64) -> Self {
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
	       pctx: ProcessContext::new(entry_point, sp),
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

fn add_process_to_ptable(process: Process) -> Result<(), &'static str> {
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

    let _ = add_process_to_ptable(process);

    // enter_user(entry_point, stack_top); // obsolete
}
