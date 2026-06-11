use crate::{print, dprintln};
use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::processes::ProcessState;
use crate::kernel::scheduler::Scheduler;

pub fn handle_syscall(ctx: &mut ExceptionContext) -> () {
    let syscall_number: u16 = (ctx.esr & 0xffff) as u16; 

    match syscall_number {
        1 => sys_print(ctx).unwrap(),
        3 => sys_exit(ctx).unwrap(),
        _ => {
            print!("Unknown syscall: {}", ctx.x[8]).unwrap();
        }
    }
}

/* SYSCALL #1 -- PRINT */
fn sys_print(ctx: &ExceptionContext) -> core::fmt::Result {
    let ptr = ctx.x[0] as *const u8;
    let len = ctx.x[1] as usize;

    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let s = core::str::from_utf8(s).unwrap_or("");

    print!("{}", s).unwrap();
    Ok(())
}

/* SYSCALL #3 -- EXIT */
// expects x0 to have the exit code
fn sys_exit(ctx: &mut ExceptionContext) -> core::fmt::Result {
    let exit_code = ctx.x[0] as i32;
    dprintln!("[SYS_EXIT] Process exiting with code: {}", exit_code);

    unsafe {
        if let Some(current_process) = &mut crate::kernel::processes::PROCESS_TABLE[Scheduler::get_current_process_index()] {
            current_process.set_state(ProcessState::Terminated);
            dprintln!("[SYS_EXIT] Process pid {}, name \"{:?}\" terminated.", current_process.pid, current_process.name);
        }
    }

    Scheduler::schedule_next(ctx);
    Ok(())
}