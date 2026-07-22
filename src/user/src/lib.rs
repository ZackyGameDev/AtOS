#![no_std]

pub static INFO_PRINTS_ENABLED: bool = true;

pub mod stdlib;

/* -- RUNTIME -- */
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        user::runtime_entry!($main, false);
        user::runtime_panic_handler!();
    };
}

// alternate entry, if args are required
#[macro_export]
macro_rules! entry_args {
    ($main:path) => {
        user::runtime_entry!($main, true);
        user::runtime_panic_handler!();
    };
}

#[macro_export]
macro_rules! print_meta { () => {
    if user::INFO_PRINTS_ENABLED {
        let mut pid: u64;
        let mut ppid: u64;
        (pid, ppid) = user::stdlib::syscalls::get_p_info().unwrap();
    
        user::println!("new process spawned with PID: {}, from parent PID: {}", pid, ppid).unwrap();
    }

};}

#[macro_export]
macro_rules! runtime_entry {
    ($main:path, false) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn _start(_: u64, _: u64, _: u64) -> ! {
            // user::print_meta!();
            $main();
            user::stdlib::syscalls::exit(0);
        }
    };

    ($main:path, true) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn _start(argc: u64, final_sp: u64, stack_bottom: u64) -> ! {
            // user::print_meta!();
            user::parse_args!(argc, final_sp, stack_bottom, argv, args);
            $main(args);
            user::stdlib::syscalls::exit(0);
        }
    };
}

#[macro_export]
macro_rules! runtime_panic_handler { () => {

    use core::panic::PanicInfo;
    #[panic_handler]
    fn panic(info: &PanicInfo) -> ! {
        user::println!("-----------PANIC------------").unwrap();
        if let Some(location) = info.location() {
            user::println!("Location: {}:{}:{}", location.file(), location.line(), location.column()).unwrap();
        } else {
            user::println!("Location: Unknown location").unwrap();
        }
        user::println!("Message:  {}", info.message()).unwrap();
        user::println!("----------------------------").unwrap();

        user::stdlib::syscalls::exit(1);
    }

};}

#[macro_export]
macro_rules! parse_args {
    ($argc:expr, $final_sp:expr, $stack_top:expr, $argv:ident, $args:ident) => {

        /*
        
        The args are passed to the user program's stack as follows:

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
            Final SP (last added item on stack)
            
            Where the runtime is given the stack top, and final sp values, along with number of args (argc).
         */

        let argc = $argc as usize;

        let offsets = unsafe {
            core::slice::from_raw_parts($final_sp as *const u64, argc)
        };

        let $argv = core::array::from_fn::<_, 64, _>(|i| {
            if i >= argc {
                return "";
            }

            let start = $final_sp + offsets[i];
            let len = if i == 0 {
                $stack_top - start
            } else {
                offsets[i - 1] - offsets[i]
            };

            unsafe {
                core::str::from_utf8(core::slice::from_raw_parts(
                    start as *const u8,
                    len as usize,
                )).expect("kernel passed invalid UTF-8")
            }
        });

        let $args = &$argv[..argc];
    };
}
