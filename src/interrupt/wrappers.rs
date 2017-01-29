use core::intrinsics::unreachable;

#[derive(Debug)]
#[repr(C)]
pub struct ExceptionStackFrame {
    pub instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

macro_rules! save_scratch_registers {
    () => {
        asm!("push rax
              push rcx
              push rdx
              push rsi
              push rdi
              push r8
              push r9
              push r10
              push r11
        " :::: "intel", "volatile");
    }
}

macro_rules! restore_scratch_registers {
    () => {
        asm!("pop r11
              pop r10
              pop r9
              pop r8
              pop rdi
              pop rsi
              pop rdx
              pop rcx
              pop rax
            " :::: "intel", "volatile");
    }
}

#[macro_export]
macro_rules! exception_handler {
    ($name:ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("mov rdi, rsp
                      add rdi, 9*8
                      call $0"
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame))
                      : "rdi" : "intel", "volatile");

                restore_scratch_registers!();
                asm!("iretq" :::: "intel", "volatile");
                unreachable!();
            }
        }
        wrapper
    }}
}

#[macro_export]
macro_rules! exception_handler_errorcode {
    ($name:ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                save_scratch_registers!();
                asm!("pop rsi
                      mov rdi, rsp
                      add rdi, 9*8
                      call $0"
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame, u64))
                      : "rdi", "rsi" : "intel", "volatile");

                restore_scratch_registers!();
                asm!("iretq" :::: "intel", "volatile");
                unreachable!();
            }
        }
        wrapper
    }}
}