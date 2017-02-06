use core::intrinsics::unreachable;

#[derive(Debug)]
#[repr(C, packed)]
pub struct ExceptionStackFrame {
    pub registers: ExceptionRegisters,
    pub error_code: u64,
    pub instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    pub stack_pointer: u64,
    stack_segment: u64,

}

#[derive(Debug)]
#[repr(C, packed)]
pub struct ExceptionRegisters {
    r11: usize,
    r10: usize,
    r9: usize,
    r8: usize,
    rdi: usize,
    rsi: usize,
    rdx: usize,
    rcx: usize,
    rbx: usize,
    rax: usize,
    rbp: usize
}


macro_rules! save_scratch_registers {
    () => {
        asm!("push rbp
              mov rbp, rsp
              push rax
              push rbx
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
              pop rbx
              pop rax
              pop rbp
            " :::: "intel", "volatile");
    }
}

#[macro_export]
macro_rules! exception_handler {
    ($name:ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!("push 0" :::: "intel");
                save_scratch_registers!();
                asm!("mov rdi, rsp
                      mov rsi, rsp
                      sub rsp, 8
                      call $0
                      add rsp, 8
                      "
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame))
                      : "rdi" : "intel", "volatile");

                restore_scratch_registers!();
                asm!("add rsp, 8; iretq" :::: "intel", "volatile");
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
                asm!("mov rsi, [rsp + 11*8]
                      mov rdi, rsp
                      sub rsp, 8
                      call $0
                      "
                      :: "i"($name as extern "C" fn(&ExceptionStackFrame, u64))
                      : "rdi", "rsi" : "intel", "volatile");

                restore_scratch_registers!();
                asm!("add rsp, 8; iretq" :::: "intel", "volatile");
                unreachable!();
            }
        }
        wrapper
    }}
}

pub static mut HAS_RUN: u32 = 0;
#[macro_export]
macro_rules! one_fence {
    () => {

        unsafe {
            asm! ("mov eax, 1
                   xchg eax, [$0]
                   mfence
                   test eax, eax
                   jz 1f
                   cli
                   hlt
                   1:
                  " ::"r"(&::interrupt::wrappers::HAS_RUN): "eax", "memory": "intel", "volatile")
        }
    };
}
