use core::ptr::{read_volatile, write_volatile};

pub struct Register<T> {
    address: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<T> Register<T> {
    pub const fn new(address: usize) -> Self {
        Self {
            address,
            _marker: core::marker::PhantomData,
        }
    }

    #[inline(always)]
    fn as_ptr(&self) -> *mut T {
        self.address as *mut T
    }

    #[inline(always)]
    pub fn read(&self) -> T {
        unsafe { read_volatile(self.as_ptr()) }
    }

    #[inline(always)]
    pub fn write(&self, value: T) {
        unsafe { write_volatile(self.as_ptr(), value) }
    }
}
