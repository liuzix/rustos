use core::intrinsics::*;
use core::ptr::Unique;

pub struct MMIO<T> {
    reg: Unique<T>
}

impl<T> MMIO<T> {
    pub fn get(&self) -> T {
        unsafe {
            let ret = volatile_load(self.reg.get());
            //asm! ("clflush $0" ::"*m"(self.reg.get()) :: "volatile", "intel");
            ret
        }
    }

    pub fn set(&self, val: T) {
        unsafe {
            //asm! ("clflush $0" ::"*m"(self.reg.get()):: "volatile", "intel");
            volatile_store(self.reg.get() as *const _ as *mut _, val);
        }
    }

    pub fn new(address: *mut T) -> MMIO<T> {
        MMIO {
            reg: unsafe { Unique::new(address) }
        }
    }
}


