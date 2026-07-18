/*

NOTE:

This is a temporary placeholder implementation.

Instead of a proper file system, right now I'm simply hardcoding things, such that 
there is only one single root directory, and it contains all the user program ELF
files in it to run. Of course, that is also not a proper file system, I will just 
emulate it for now where for a given file name i return appropriate ELF bytes: &[u8].

*/

use crate::{dprintln, kernel::processes::Process};

pub struct FileSystem;

impl FileSystem {
    pub fn read_file(file_name: &str) -> Option<&'static [u8]> {
        match file_name {
            "init" => Some(include_bytes!("../user/build/init")),
            "b" => Some(include_bytes!("../user/build/b")),
            "a" => Some(include_bytes!("../user/build/a")),
            "echo" => Some(include_bytes!("../user/build/echo")),
            "wc" => Some(include_bytes!("../user/build/wc")),
            "tetris" => Some(include_bytes!("../user/build/tetris")),
            "clear" => Some(include_bytes!("../user/build/clear")),
            "info" => Some(include_bytes!("../user/build/info")),
            "c" => Some(include_bytes!("../user/build/c")),
            _ => None,
        }
    }

    pub fn run_executable(file_name: &str, parent_pid: u64, args: &[&str]) -> Result<(), &'static str> {
        dprintln!("FileSystem: run_executable called with file_name: {}", file_name);
        if let Some(elf_bytes) = Self::read_file(file_name) {
            let pid = Process::spawn_from_elf(file_name, parent_pid, elf_bytes, args)?;
            dprintln!("[FILESYSTEM] Spawned process '{}' with PID {}", file_name, pid);
            Ok(())
        } else {
            Err("File not found")
        }
    }
}
