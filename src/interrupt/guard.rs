use x86::shared::flags::*;
use core::ops::Drop;

pub struct InterruptGuard {
    int_enabled: bool
}

impl InterruptGuard {
    pub fn disable_interrupt() -> InterruptGuard {
        let ret = InterruptGuard {
            int_enabled: flags().contains(FLAGS_IF)
        };
        unsafe {
            asm!("cli" :::: "volatile");
        }
        ret
    }
}


impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.int_enabled {
            unsafe {
                asm!("sti" :::: "volatile");
            }
        }
    }
}