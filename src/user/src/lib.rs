#![no_std]

pub mod stdlib;

#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn _start() -> ! {
            $main();
            user::stdlib::syscalls::exit(0);
        }

        use core::panic::PanicInfo;
        #[panic_handler]
        fn panic(info: &PanicInfo) -> ! {
            println!("-----------PANIC------------").unwrap();
            if let Some(location) = info.location() {
                println!("Location: {}:{}:{}", location.file(), location.line(), location.column()).unwrap();
            } else {
                println!("Location: Unknown location").unwrap();
            }
            println!("Message:  {}", info.message()).unwrap();
            println!("----------------------------").unwrap();

            user::stdlib::syscalls::exit(1);
        }
    };
}