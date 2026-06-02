use crate::println;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum ExceptionType {
    _SYNC,
    _IRQ,
    _FIQ,
    _SE,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum ExceptionSource {
    _EL1t,
    _EL1h,
    _EL064,
    _EL032,
}


#[repr(C)]
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

// called by `exceptions.s`
#[no_mangle]
pub extern "C" fn handle_exception_el1(ctx: &mut ExceptionContext) {
    
    println!("An exception has been detected :D").unwrap();
    
    // printing the full context for now.
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