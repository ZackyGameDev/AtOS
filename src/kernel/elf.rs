pub const ELF_MAGIC: u32 = 0x464C457F; // 0x7F followed by E L F, but adjusted for little-endian
pub const PT_LOAD: u32 = 1; // Program Type: 1 (loaded)

// Standard ELF header for 64-bit machines.  You can get the spec from:
// https://www.sco.com/developers/gabi/2000-07-17/ch4.eheader.html
//
// A few rearrangements have been made to accomodate for easy access.  Anything
// before Elf64Hdr->machine, while being the same size, is not 1-1 with the
// spec.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Hdr {
    pub magic: u32, // must be equal to ELF_MAGIC
    pub elf: [u8; 12],
    pub r#type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64ProgHdr {
    pub r#type: u32,
    pub flags: u32,
    pub offset: u64,
    pub virtaddr: u64,
    pub phyaddr: u64,
    pub filesize: u64,
    pub memsize: u64,
    pub align: u64,
}

// The debugging was harder than the implementation on this one. The kernel
// panicked in all its glory at the mere sight of an ELF file.  In C, this
// would've just been a typecast and some alignment.
impl Elf64Hdr {
    pub fn verify(&self) -> bool {
        self.magic == ELF_MAGIC
    }

    pub fn mkfrombytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < core::mem::size_of::<Self>() {
            return None;
        }

        // This read_aligned was the real pain in the ass!  Before that, I did
        // &*(bytes.as_ptr() as *const Self), simple 'nuff right?  Wrong!
        // Because it is Rust and the language likes to have its own quirks.
        unsafe {
            let header = core::ptr::read_unaligned(bytes.as_ptr() as *const Self);
            if header.verify() {
                Some(header)
            } else {
                None
            }
        }
    }
}
