use super::threads::*;
use containers::cpu_local::*;
use containers::queue::Queue;
use alloc::boxed::Box;
use alloc::arc::Arc;
use core::sync::atomic::Ordering;
use core::cell::RefCell;


pub struct Scheduler {
    thread_current: CPULocal<WrappedThread>,
    ready_queue: Queue<WrappedThread>,
    idle_thread: CPULocal<WrappedThread>,
    is_idling: CPULocal<bool>
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            thread_current: CPULocal::create(),
            ready_queue: Queue::create(),
            idle_thread: CPULocal::create(),
            is_idling: CPULocal::create()
        }
    }

    pub fn schedule(&self) {
        let prev: WrappedThread;
        if self.thread_current.get_mut().is_none() {
            prev = Arc::new(RefCell::new(KThread::boot_strap_thread()));
        } else {
            prev = self.thread_current.into_inner().unwrap(); // move the arc from thread_current
            if !self.is_idling.into_inner().unwrap_or(false) {
                if !prev.borrow().is_dead() {
                    self.ready_queue.enqueue(prev.clone()); // then enqueue
                } else {
                    //kprint!("Thread is dead. Count: {}\n", Arc::strong_count(&prev));
                }
            }
        }
        let next = match self.ready_queue.dequeue() {
            None => {
                self.get_idle()
            },
            Some(t) => {
                self.is_idling.set(false);
                t
            }
        };

        let ref_prev = prev.as_ptr();
        let ref_next = next.as_ptr();
        drop(prev);  // this is important for garbage collection
        self.thread_current.set(next);
        if ref_prev != ref_next {
            unsafe {
                (*ref_prev).switch_to(ref_next.as_mut().unwrap());
            }
        } else {
            unsafe { ::x86::shared::msr::wrmsr(::x86::shared::msr::IA32_X2APIC_EOI, 0); }
            unsafe { ::x86::shared::irq::enable(); }
        }

    }

    pub fn insert_thread(&self, t: WrappedThread) {
        self.ready_queue.enqueue(t);
    }

    pub fn sleep(t: WrappedThread) {}

    pub fn get_idle(&self) -> WrappedThread {
        if self.idle_thread.get_mut().is_none() {
            self.idle_thread.set(KThread::create(Scheduler::idle, "idle"));
        }
        self.is_idling.set(true);
        self.idle_thread.get_mut().unwrap().clone()
    }

    fn idle(_: usize) -> usize {
        unsafe {
            loop {
                //kprint!("gwa\n");
                asm!("sti; hlt");
            }
        }
    }
}

