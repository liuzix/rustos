use alloc::arc::Arc;
use mem;
use collections::string::*;
use core::sync::atomic::*;
use core::ops::Drop;
use core::marker::Copy;

type DoThreadFunc = fn(usize) -> usize;


pub struct KThread {
    pub name: String,
    entry_point: DoThreadFunc,
    runnable: AtomicBool,
    running: AtomicBool,
    rsp: usize,
    dead: AtomicBool,
    //rip: usize,
}


pub fn new_thread(entry_point: DoThreadFunc, name: &str) -> Arc<KThread> {
    let ret = KThread::create(entry_point, name);
    super::SCHEDULER.insert_thread(ret.clone());
    ret
}

impl KThread {
    pub fn create(entry_point: DoThreadFunc, name: &str) -> Arc<KThread> {
        let mut ret = Arc::new(KThread {
            name: name.to_string(),
            entry_point: entry_point,
            runnable: ATOMIC_BOOL_INIT,
            running: ATOMIC_BOOL_INIT,
            rsp: mem::FRAME.alloc_stack(3) - 8,
            dead: ATOMIC_BOOL_INIT
        });
        //ret.rsp -= 8;
        unsafe {
            asm!("mov rbx, rsp
                   mov rsp, $0
                   push $1
                   push $2
                   push $3
                   mov $0, rsp
                   mov rsp, rbx"
                   : "+*m"(&ret.rsp): "r"(ret.as_ref()),
                      "r"(exit_stub as extern fn()),
                      "r"(ret.entry_point): "rbx": "intel", "volatile");
        }
        //kprint!("create 0x{:x}\n", ret.as_ref() as *const _ as usize);
        ret
    }

    pub fn boot_strap_thread() -> KThread {
        KThread {
            name: "bootstrap".to_string(),
            entry_point: unsafe { *(0 as *mut DoThreadFunc) },
            runnable: AtomicBool::new(true),
            running: AtomicBool::new(true),
            rsp: 0,
            dead: AtomicBool::new(false)
        }
    }


    pub fn suspend(&mut self) {
        if self.runnable.swap(false, Ordering::SeqCst) == false {
            // If already suspended
            return;
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn switch_to(&self, other: &Self) {
        unsafe {
            /*
            Calculate RIP for resuming.
            Push calculated RIP onto stack.
            Swtich stack.
            Pop new RIP off of stack.
            */
            //kprint!("{:x}\n", other.rsp);
            other.running.store(true, Ordering::SeqCst);
            asm!("lea rax, [rip + back]
                  push rax
                  mov $0, rsp
                  mov rbx, $1
                  mov rsp, $2
                  pop rax
                  mov byte ptr [ebx], 0
                  jmp rax
                  back:" : "=*m" (&self.rsp),
                           "=*m" (&self.running)
                         : "m" (other.rsp)
                         :: "intel", "volatile");
            //::x86::shared::irq::enable();
        }
    }

    fn on_exit(&self, ret: usize) {
        kprint!("Thread {} exits, return value = {}\n", self.name, ret as usize);
        kprint!("exit 0x{:x}\n", self as *const _ as usize);
        self.dead.store(true, Ordering::SeqCst);
        unsafe {
            //::x86::shared::irq::disable();
        }
        super::SCHEDULER.schedule();
        unreachable!();
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }
}


impl Drop for KThread {
    fn drop(&mut self) {
        kprint!("Dropping {}\n", self.name.as_str());
    }
}

#[naked]
extern "C" fn exit_stub() {
    unsafe {
        asm!("pop rdi
              mov rsi, rax
              test rsp, 0xf
              jz 1f
              push 0
              1: call $0" :: "i"(exit_landing_pad as extern fn (*mut KThread, usize)) :: "intel");
    }
    //unreachable!();
}

///
/// This function is needed because we cannot use local variables in
/// a naked function
///
extern "C" fn exit_landing_pad(this: *mut KThread, ret: usize) {
    kprint!("diu!\n");
    unsafe {
        (*this).on_exit(ret);
    }
    unreachable!();
}

