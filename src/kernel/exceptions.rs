use crate::kernel::processes::ProcessContext;
use crate::kernel::scheduler::Scheduler;
use crate::{dprintln, println};
use crate::kernel::syscalls;
use crate::kernel::interrupts::{Interrupts, InterruptSource};
use crate::kernel::timer::PhysicalTimer;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum ExceptionType {
    _SYNC,
    _IRQ,
    _FIQ,
    _SE,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExceptionSource {
    _EL1t,
    _EL1h,
    _EL064,
    _EL032,
}


#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExceptionContext {
    pub etype: ExceptionType, // u8
    pub esource: ExceptionSource, // u8
    pub _padding: [u8; 6], // because this struct follows c style repr
    pub x: [u64; 31],   // x0–x30
    pub elr: u64,
    pub spsr: u64,
    pub esr: u64,
    pub far: u64,
}

impl ExceptionContext {
    pub fn update_from_pctx(&mut self, pctx: &ProcessContext) {
        self.x.copy_from_slice(&pctx.x);
        self.elr = pctx.elr;
        self.spsr = pctx.spsr;
    }
}

fn print_exception_context(ctx: &ExceptionContext) -> () {
    let etype_str = match ctx.etype {
        ExceptionType::_SYNC => "SYNC",
        ExceptionType::_IRQ  => "IRQ",
        ExceptionType::_FIQ  => "FIQ",
        ExceptionType::_SE   => "SError",
    };
    let esource_str = match ctx.esource {
        ExceptionSource::_EL1t => "EL1t",
        ExceptionSource::_EL1h => "EL1h",
        ExceptionSource::_EL064 => "EL064",
        ExceptionSource::_EL032 => "EL032",
    };
    println!("=== Exception Context ===").unwrap();
    println!("Type : {} ({})", etype_str, ctx.etype as u8).unwrap();
    println!("Source : {} ({})", esource_str, ctx.esource as u8).unwrap();
    println!("ELR  : {:#018x}", ctx.elr).unwrap();
    println!("SPSR : {:#018x}", ctx.spsr).unwrap();
    println!("ESR  : {:#018x}", ctx.esr).unwrap();
    println!("FAR  : {:#018x}", ctx.far).unwrap();
    println!("Registers:").unwrap();
    for i in 0..31 {
        println!("  x{:02} = {:#018x}", i, ctx.x[i]).unwrap();
    }
    println!("=========================").unwrap();
}

macro_rules! unhandled_exception {
    ($ctx:expr) => {{
        println!("Unhandled exception detected!").unwrap();
        print_exception_context($ctx);
    }};
}

pub fn was_from_user_el0(ctx: &ExceptionContext) -> bool {
    ctx.esource == ExceptionSource::_EL064 || ctx.esource == ExceptionSource::_EL032
}

fn handle_sync_exception(ctx: &mut ExceptionContext) -> () {
    let exception_class = (ctx.esr >> 26) & 0x3f;

    match ctx.esource {
        ExceptionSource::_EL1h => {
            if exception_class == 0x15 && (ctx.esr & 0xffff) == 0 {
                dprintln!("[EXCEPTIONS] svc #0 from EL1 detected. starting scheduler...");
                Scheduler::reset_timer();
                Scheduler::schedule_next(ctx);
            } else {
                unhandled_exception!(ctx);
            }
        },

        ExceptionSource::_EL064=> {
            match exception_class {
                0x15 => { // it was an svc instruction
                    syscalls::handle_syscall(ctx);
                },
                0x07 => { 
                    println!("EXCEPTION: FP/SIMD functionality used when not enabled!").unwrap();
                },
                _ => unhandled_exception!(ctx),
            }
        },
        _ => unhandled_exception!(ctx),
    }
}

fn handle_irq_exception(ctx: &mut ExceptionContext) -> () {
    let mut irq_sources: u32 = Interrupts::pending_irq();

    if irq_sources & (InterruptSource::PhysicalNonSecureTimer as u32) != 0 {
        PhysicalTimer::handle_irq(ctx);
        irq_sources &= !(InterruptSource::PhysicalNonSecureTimer as u32);
    }

    if irq_sources > 0 {
        println!("Other Unhandled IRQ sources pending: {:#x}", irq_sources).unwrap();
        unhandled_exception!(ctx);
    }
}

// called by `exceptions.s`
#[unsafe(no_mangle)]
pub extern "C" fn handle_exception_el1(ctx: &mut ExceptionContext) {
    
    // if it came from EL0, then we need to update PCB of the process interrupted.
    if was_from_user_el0(ctx) {
        let sp_el0: u64;
        unsafe {
            // reading sp_el0
            core::arch::asm!(
                "mrs {val}, sp_el0",
                val = out(reg) sp_el0,
                options(nostack, preserves_flags)
            )
        }
        let new_pctx = ProcessContext::from_ectx(ctx, sp_el0);

        Scheduler::update_last_running_pctx(&new_pctx);
    }

    // println!("An exception has been detected :D").unwrap();
    
    // handling the exception based on the type and source.
    match ctx.etype {
        ExceptionType::_SYNC => handle_sync_exception(ctx),
        ExceptionType::_IRQ  => handle_irq_exception(ctx),
        _ => unhandled_exception!(ctx),
    }

    // next process is scheduled in timer irq handler

}

