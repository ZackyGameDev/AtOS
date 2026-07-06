/*

NOTE:

This is a temporary placeholder implementation.

Instead of a proper file system, right now I'm simply hardcoding things, such that 
there is only one single root directory, and it contains all the user program ELF
files in it to run. Of course, that is also not a proper file system, I will just 
emulate it for now where for a given file name i return appropriate ELF bytes: &[u8].

*/

use crate::{dprintln, kernel::paging::PageAllocator};

pub struct FileSystem;

impl FileSystem {
    pub fn read_file(file_name: &str) -> Option<&'static [u8]> {
        match file_name {
            "init" => Some(include_bytes!("../user/build/init")),
            "b" => Some(include_bytes!("../user/build/b")),
            _ => None,
        }
    }

    pub fn run_executable(file_name: &str) -> Result<(), &'static str> {
        dprintln!("FileSystem: run_executable called with file_name: {}", file_name);
        if let Some(elf_bytes) = Self::read_file(file_name) {
            PageAllocator::load_elf_process(file_name, 0, elf_bytes);
            Ok(())
        } else {
            Err("File not found")
        }
    }
}
