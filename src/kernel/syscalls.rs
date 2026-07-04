use crate::kernel::input;
use crate::{print, dprintln};
use crate::kernel::exceptions::ExceptionContext;
use crate::kernel::processes::ProcessState;
use crate::kernel::scheduler::Scheduler;
use crate::kernel::paging::PageAllocator;

pub fn handle_syscall(ctx: &mut ExceptionContext) -> () {
    let syscall_number: u16 = (ctx.esr & 0xffff) as u16;

    match syscall_number {
        1 => sys_print(ctx).unwrap(),
        2 => sys_read(ctx).unwrap(),
        3 => sys_exit(ctx).unwrap(),
        _ => {
            print!("Unknown syscall: {}", syscall_number).unwrap();
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

// SYSCALL #2 -- READ */
pub fn sys_read(ctx: &mut ExceptionContext) -> core::fmt::Result {
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

    let r = input::getline(buf);
    ctx.x[0] = r as u64;

    Ok(())
}

/* SYSCALL #3 -- EXIT */
// expects x0 to have the exit code
fn sys_exit(ctx: &mut ExceptionContext) -> core::fmt::Result {
    let exit_code = ctx.x[0] as i32;
    dprintln!("[SYS_EXIT] Process exiting with code: {}", exit_code);

    unsafe {
        if let Some(current_process) = &mut crate::kernel::processes::PROCESS_TABLE[Scheduler::get_current_process_index()] {
            current_process.set_state(ProcessState::Terminated); // \TODO currently terminated processes stay indefinitely process table.
            PageAllocator::free_page_table(Some(current_process.pctx.ttbr0));
            dprintln!("[SYS_EXIT] Process pid {}, name \"{:?}\" terminated.", current_process.pid, current_process.name);
        }
    }

    Scheduler::schedule_next(ctx);
    Ok(())
}
