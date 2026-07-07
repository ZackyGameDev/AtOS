#![allow(unused_assignments)]

use crate::kernel::filesystem::FileSystem;
use crate::kernel::processes::add_process_to_ptable;
use crate::kernel::processes::{PROCESS_TABLE, PROCESS_TABLE_LOCK, ProcessState, BlockReason};
use crate::{print, dprintln};
use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::scheduler::Scheduler;
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
    let exit_code = ctx.x[0] as i32;
    dprintln!("[SYS_EXIT] Process exiting with code: {}", exit_code);

    if let Some(current_process) = Scheduler::get_current_process() {
        // Acquire process table lock to avoid races with wait()
        PROCESS_TABLE_LOCK.acquire();

        current_process.terminate();
        let parent = current_process.parent_pid;
        dprintln!("[SYS_EXIT] Process pid {}, name \"{:?}\" terminated.", current_process.pid, current_process.name);

        // Wake up parent (if it's waiting). Use parent PID as the wake channel.
        Scheduler::wakeup(parent as usize as *const ());

        PROCESS_TABLE_LOCK.release();
    } else {
        dprintln!("[SYS_EXIT] Process to terminate not found. (???)");
    }

    Scheduler::schedule_next(ctx);
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

    loop {
        // Lock the process table while we inspect child state to avoid lost wakeups
        PROCESS_TABLE_LOCK.acquire();

        // Scan for children matching target
        let mut found_child = false;
        unsafe {
            let table_ptr = core::ptr::addr_of_mut!(PROCESS_TABLE) as *mut Option<crate::kernel::processes::Process>;
            for i in 0..crate::kernel::processes::MAX_PROCESSES {
                let slot = &mut *table_ptr.add(i);
                if let Some(proc) = slot {
                    if proc.parent_pid == current_pid && (target_pid == 0 || proc.pid == target_pid) {
                        found_child = true;
                        if proc.state == ProcessState::Terminated {
                            let child_pid = proc.pid;
                            // Reap the child slot
                            *slot = None;
                            PROCESS_TABLE_LOCK.release();
                            ctx.x[0] = child_pid;
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
            curr.set_state(ProcessState::Blocked);
            curr.block_reason = Some(BlockReason::WaitingForChild);
            curr.chan = current_pid as u64; // sleep channel is parent's pid
        } else {
            PROCESS_TABLE_LOCK.release();
            ctx.x[0] = u64::MAX;
            return Ok(());
        }

        // Sleep will release the PROCESS_TABLE_LOCK atomically while blocking
        Scheduler::sleep(current_pid as usize as *const (), &PROCESS_TABLE_LOCK);
        // When woken up, loop and re-check children
    }
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
    let path_ptr = ctx.x[0] as *const u8;
    let path_len = ctx.x[1] as usize;

    if path_ptr.is_null() || path_len == 0 {
        dprintln!("[SYS_EXEC] Invalid arguments: null pointer or zero length.");
        ctx.x[0] = u64::MAX; // technically the calling process will know exec failed by the fact that it didn't die yet.
                            // but we will return -1 to indicate error anyway. 
        return Ok(());
    }

    let path_slice = unsafe { core::slice::from_raw_parts(path_ptr, path_len) };
    let path_str = core::str::from_utf8(path_slice).unwrap_or("");

    dprintln!("[SYS_EXEC] Attempting to execute file: {}", path_str);

    if let Some(current_process) = Scheduler::get_current_process() {

        let mut name_bytes = [0u8; 32];
        let bytes = path_str.as_bytes();
        let len = core::cmp::min(bytes.len(), 32);
        name_bytes[..len].copy_from_slice(&bytes[..len]);

        let elf_bytes = match FileSystem::read_file(path_str) {
            Some(bytes) => bytes,
            None => {
                dprintln!("[SYS_EXEC] File not found: {}", path_str);
                ctx.x[0] = u64::MAX; // return -1 to indicate error
                return Ok(());
            }
        };

        current_process.exec(elf_bytes)?;
        current_process.name[..len].copy_from_slice(&name_bytes[..len]);
        
        ctx.update_from_pctx(&current_process.pctx);
    
        dprintln!("[SYS_EXEC] Process executed new file: {:?}", current_process);

    } else {
        dprintln!("[SYS_EXEC] Current process not found. (???)");
        ctx.x[0] = u64::MAX; // return -1 to indicate error
    }

    Ok(())
}