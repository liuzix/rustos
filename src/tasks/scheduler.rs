use super::threads::KThread;
use containers::cpu_local::*;
use containers::queue::Queue;
use alloc::boxed::Box;
use alloc::arc::Arc;
use core::sync::atomic::Ordering;


pub struct Scheduler {
    thread_current: CPULocal<Arc<KThread>>,
    ready_queue: Queue<Arc<KThread>>,
    idle_thread: CPULocal<Arc<KThread>>,
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
        //kprint!("cpu {} scheduling!\n", ::devices::apic::get_cpu_id());
        ::devices::serial::write_string("scheduling\n");
        let prev: Arc<KThread>;
        if self.thread_current.get_mut().is_none() {
            kprint!("bootstrapping!\n");
            prev = Arc::new(KThread::boot_strap_thread());
        } else {
            //kprint!("we have a current thread!\n");
            prev = self.thread_current.into_inner().unwrap(); // move the arc from thread_current

            if !self.is_idling.into_inner().unwrap_or(false) {
                // if we are not idling
                //::devices::serial::write_string("Not idling, enqueue\n");
                if !prev.is_dead() {
                    //kprint!("enqueued something {}, {:x}\n", prev.name, prev.as_ref() as *const _ as usize);
                    self.ready_queue.enqueue(prev.clone()); // then enqueue
                }
            }
        }
        let next = match self.ready_queue.dequeue() {
            None => {
                //::devices::serial::write_string("dequeued nothing. start idling\n");
                self.get_idle()
            },
            Some(t) => {
                //kprint!("dequeued something {}\n", t.name);
                self.is_idling.set(false);
                t
            }
        };


        self.thread_current.set(next.clone());
        if !Arc::ptr_eq(&prev, &next) {
            ::devices::serial::write_string(next.name.as_str());
            unsafe { ::x86::shared::msr::wrmsr(::x86::shared::msr::IA32_X2APIC_EOI, 0) };
            unsafe { ::x86::shared::irq::enable(); }
            prev.switch_to(next.as_ref());
        }
        drop(prev)
    }

    pub fn insert_thread(&self, t: Arc<KThread>) {
        self.ready_queue.enqueue(t);
    }

    pub fn sleep(t: Arc<KThread>) {}

    pub fn get_idle(&self) -> Arc<KThread> {
        if self.idle_thread.get_mut().is_none() {
            self.idle_thread.set(KThread::create(Scheduler::idle, "idle"));
        }
        //kprint!("cpu {} idling\n", ::devices::apic::get_cpu_id());
        self.is_idling.set(true);
        self.idle_thread.get_mut().unwrap().clone()
    }

    fn idle(_: usize) -> usize {
        unsafe {
            loop {
                kprint!("gwa\n");
                asm!("sti; hlt");
            }
        }
    }
}

