#![allow(unused_assignments)]

use crate::kernel::filesystem::FileSystem;
use crate::kernel::processes::add_process_to_ptable;
use crate::kernel::processes::{Process, PROCESS_TABLE, PROCESS_TABLE_LOCK, 
                                ProcessState, BlockReason, remove_process_from_ptable};
use crate::{dprintln, print};
use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::scheduler::{Scheduler};
use crate::kernel::io::KERNEL_IO;

pub fn handle_syscall(ctx: &mut ExceptionContext) -> () {
    let syscall_number: u16 = (ctx.esr & 0xffff) as u16;

    dprintln!("[SYSCALL] Syscall number: {}", syscall_number);

    match syscall_number {
        1 => sys_print(ctx).unwrap(),
        2 => sys_read(ctx).unwrap(),
        3 => sys_exit(ctx).unwrap(),
        4 => sys_fork(ctx).unwrap(),
        5 => sys_exec(ctx).unwrap(),
        6 => sys_wait(ctx).unwrap(),
        7 => sys_poll_char(ctx).unwrap(),
        8 => sys_sleep(ctx).unwrap(),
        9 => sys_print_os_info(ctx).unwrap(),
        10 => sys_p_info(ctx).unwrap(),
        _ => {
            print!("Unknown syscall: {}", syscall_number);
        }
    }
}

/* SYSCALL #1 -- PRINT */
fn sys_print(ctx: &ExceptionContext) -> Result<(), &'static str> {
    let ptr = ctx.x[0] as *const u8;
    let len = ctx.x[1] as usize;

    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let s = core::str::from_utf8(s).unwrap_or("");

    print!("{}", s);
    Ok(())
}

// SYSCALL #2 -- READ */
pub fn sys_read(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    let usr_bufp = ctx.x[0] as *mut u8;
    let buf_sz = ctx.x[1] as usize;

    // Invalid arguments, return 0 bytes read.
    if usr_bufp.is_null() || buf_sz == 0 {
        ctx.x[0] = 0;
        return Ok(());
    }

    let buf = unsafe {
        core::slice::from_raw_parts_mut(usr_bufp, buf_sz)
    };

    let r = KERNEL_IO.read(buf);
    ctx.x[0] = r as u64;

    Ok(())
}

/* SYSCALL #3 -- EXIT */
// expects x0 to have the exit code
fn sys_exit(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    let exit_code = ctx.x[0] as i64;
    dprintln!("[SYS_EXIT] Process exiting with code: {}", exit_code);

    if let Some(current_process) = Scheduler::get_current_process() {
        // Acquire process table lock to avoid races with wait()
        PROCESS_TABLE_LOCK.acquire();

        current_process.terminate(exit_code);
        let parent = current_process.parent_pid;
        dprintln!("[SYS_EXIT] Process pid {}, name \"{:?}\" terminated.", current_process.pid, current_process.name);

        // Wake up parent (if it's waiting). 
        if let Some(parent_proc) = Process::find_by_id(parent) {
            if parent_proc.state == ProcessState::Blocked && parent_proc.block_reason == Some(BlockReason::WaitingForChild) {
                let awaited_child = parent_proc.pctx.x[0] as u64;
                if awaited_child == 0 || current_process.pid == awaited_child { // am i the awaited child? 
                    dprintln!("[SYS_EXIT] Waking up waiting parent process pid {}.", parent);
                    parent_proc.set_state(ProcessState::Ready);
                    
                    // return params for wait() syscall
                    parent_proc.pctx.x[0] = current_process.pid; // child pid
                    parent_proc.pctx.x[1] = exit_code as u64; // child exit code
                    remove_process_from_ptable(current_process.pid)?;
                    dprintln!("[SYS_WAIT] SCENARIO B child process pid {}, exit code {}", current_process.pid, exit_code);

                }
            }
        } else {
            if parent != 0 { // zero is not an error because zero just means process was spawned by kernel, no parent.
                panic!("[SYS_EXIT] Parent process pid {} not found of child {}", parent, current_process.pid);
            }
        }

        PROCESS_TABLE_LOCK.release();
    } else {
        dprintln!("[SYS_EXIT] Process to terminate not found. (???)");
    }

    Scheduler::schedule_next(ctx);
    Ok(())
}

/* SYSCALL #4 -- FORK */
// x0 returns standard C styled return value: 0 for child, child's pid for parent, -1 for error
fn sys_fork(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    // we are not letting the user pass the pid of the new process.
    // for security reasons we will fetch the id of the caller process
    // here itself

    let current_process = match Scheduler::get_current_process() {
        Some(p) => p,
        None => {
            dprintln!("[SYS_FORK] Current process not found. (???)");
            ctx.x[0] = u64::MAX; // return -1 to indicate error
            return Ok(());
        }
    };

    dprintln!("[SYS_FORK] Forking process pid {}, name \"{:?}\"", current_process.pid, current_process.name);

    let mut child = current_process.fork()?;

    child.pctx.x[0] = 0; // child process returns 0
    ctx.x[0] = child.pid; // parent process returns child's pid

    add_process_to_ptable(child)?;

    Ok(())
}

/* SYSCALL #5 -- EXEC */
// the exact path to the file must be passed as str in x0, with len of str in x1.
// the file must be ELF executable.
fn sys_exec(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    const MAX_ARGS: usize = 32;
    const MAX_ARG_LEN: usize = 256;

    let ptrs = ctx.x[0] as *const u64;
    let lens = ctx.x[1] as *const u64;
    let argc = ctx.x[2] as usize;

    if ptrs.is_null() || lens.is_null() || argc == 0 || argc > MAX_ARGS {
        dprintln!("[SYS_EXEC] Invalid arguments.");
        ctx.x[0] = u64::MAX;
        return Ok(());
    }

    let mut storage = [[0u8; MAX_ARG_LEN]; MAX_ARGS];
    let mut lengths = [0usize; MAX_ARGS];
    let mut strings = [""; MAX_ARGS];

    // reading the pointers to buffers (storage)
    for i in 0..argc {
        let ptr = unsafe { *ptrs.add(i) } as *const u8;
        let len = unsafe { *lens.add(i) } as usize;

        if len > MAX_ARG_LEN || (len != 0 && ptr.is_null()) {
            dprintln!("[SYS_EXEC] Invalid string at index {}.", i);
            ctx.x[0] = u64::MAX;
            return Ok(());
        }

        if len != 0 {
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, storage[i].as_mut_ptr(), len);
            }
        }

        lengths[i] = len;
    }

    // building valid &str slices from the storage
    for i in 0..argc {
        strings[i] = match core::str::from_utf8(&storage[i][..lengths[i]]) {
            Ok(s) => s,
            Err(_) => {
                dprintln!("[SYS_EXEC] Non-UTF8 string at index {}.", i);
                ctx.x[0] = u64::MAX;
                return Ok(());
            }
        };
    }

    let path_str = strings[0];
    dprintln!("[SYS_EXEC] Attempting to execute file: {}", path_str);

    let Some(current_process) = Scheduler::get_current_process() else {
        dprintln!("[SYS_EXEC] Current process not found.");
        ctx.x[0] = u64::MAX;
        return Ok(());
    };

    let elf_bytes = match FileSystem::read_file(path_str) {
        Some(bytes) => bytes,
        None => {
            dprintln!("[SYS_EXEC] File not found: {}", path_str);
            ctx.x[0] = u64::MAX;
            return Ok(());
        }
    };

    dprintln!("[SYS_EXEC] Executing file: {} with {} args", path_str, argc);

    if let Err(e) = current_process.exec(elf_bytes, &strings[..argc]) {
        dprintln!("[SYS_EXEC] exec failed: {}", e);
        ctx.x[0] = u64::MAX;
        return Ok(());
    }

    current_process.name = [0; 32];
    let name_bytes = path_str.as_bytes();
    let name_len = core::cmp::min(name_bytes.len(), current_process.name.len());
    current_process.name[..name_len].copy_from_slice(&name_bytes[..name_len]);

    ctx.update_from_pctx(&current_process.pctx);

    dprintln!("[SYS_EXEC] Process executed new file: {:?}", current_process);

    Ok(())
}

// SYSCALL #6 -- WAIT
// x0: pid to wait for (0 = any child)
fn sys_wait(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    let target_pid = ctx.x[0] as u64; // 0 means any child

    let current_pid = match Scheduler::get_current_process() {
        Some(p) => p.pid,
        None => {
            ctx.x[0] = u64::MAX;
            return Ok(());
        }
    };

    // Lock the process table while we inspect child state to avoid lost wakeups
    PROCESS_TABLE_LOCK.acquire();

    // Scan for children matching target
    let mut found_child = false;
    unsafe {
        #[allow(static_mut_refs)]
        for slot in PROCESS_TABLE.iter_mut() {
            if let Some(proc) = slot {
                if proc.parent_pid == current_pid && (target_pid == 0 || proc.pid == target_pid) {
                    found_child = true;
                    if proc.state == ProcessState::Terminated {
                        let child_pid = proc.pid;
                        let child_exit_code = proc.exit_code;
                        // Reap the child slot
                        *slot = None;
                        PROCESS_TABLE_LOCK.release();
                        ctx.x[0] = child_pid;
                        ctx.x[1] = child_exit_code as u64;

                        dprintln!("[SYS_WAIT] SCENARIO A child process pid {}, exit code {}", child_pid, child_exit_code);
                        return Ok(());
                    }
                }
            }
        }
    }

    if !found_child {
        // No matching child exists
        PROCESS_TABLE_LOCK.release();
        ctx.x[0] = u64::MAX; // indicate error / no child
        return Ok(());
    }

    // Found at least one child but none terminated yet: block current process
    if let Some(curr) = Scheduler::get_current_process() {
        dprintln!("[SYS_WAIT] SCENARIO C blocking current process pid {} waiting for child pid {}", curr.pid, target_pid);
        curr.block(BlockReason::WaitingForChild);
        Scheduler::schedule_next(ctx);
        PROCESS_TABLE_LOCK.release();
        return Ok(()) 
    } else {
        PROCESS_TABLE_LOCK.release();
        return Err("Where did the current process go 😦");
    }
    
}

// Syscall #7
fn sys_poll_char(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    match KERNEL_IO.poll_char() {
        Some(c) => ctx.x[0] = c as u64,
        None => ctx.x[0] = 0,
    }
    Ok(())
}

// Syscall #8
fn sys_sleep(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    let ms = ctx.x[0];

    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }

    let start_ticks: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntpct_el0", out(reg) start_ticks);
    }

    let target_ticks = start_ticks + (freq * ms) / 1000;
    while unsafe {
        let current: u64;
        core::arch::asm!("mrs {}, cntpct_el0", out(reg) current);
        current
    } < target_ticks {
        core::hint::spin_loop();
    }

    Ok(())
}

fn sys_print_os_info(_ctx: &ExceptionContext) -> Result<(), &'static str> {
    print!("{}", crate::INTRO);
    Ok(())
}
fn sys_p_info(ctx: &mut ExceptionContext) -> Result<(), &'static str> {
    // this will be used by the running process to get meta information about itself

    if let Some(curr) = Scheduler::get_current_process() {
        curr.save_meta_to_ectx(ctx);
    } else {
        return Err("How did save info actually fail?")
    }

    Ok(())
}
