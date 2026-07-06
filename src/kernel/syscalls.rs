#![allow(unused_assignments)]

use crate::kernel::processes::Process;
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
        current_process.terminate();
        dprintln!("[SYS_EXIT] Process pid {}, name \"{:?}\" terminated.", current_process.pid, current_process.name);
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

    let mut child = current_process.fork()?;

    child.pctx.x[0] = 0; // child process returns 0
    ctx.x[0] = child.pid; // parent process returns child's pid

    Ok(())
}