use core::sync::atomic::*;
use alloc::boxed::*;
use core::ptr;
use core::intrinsics::*;
use core::mem::*;


pub struct Queue<T> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
}


struct Node<T> {
    data: Option<T>,
    ref_cnt: AtomicUsize,
    next: AtomicPtr<Node<T>>,
    retired: AtomicBool
}


impl<T> Queue<T> {
    pub fn create() -> Queue<T> {
        let dummy = Box::into_raw(box Node {
            data: None,
            ref_cnt: AtomicUsize::new(0),
            next: AtomicPtr::new(ptr::null_mut()),
            retired: ATOMIC_BOOL_INIT
        });


        Queue {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy)
        }
    }

    pub fn enqueue(&self, val: T) {
        let new_node = box Node {
            data: Some(val),
            ref_cnt: AtomicUsize::new(1),
            next: AtomicPtr::new(ptr::null_mut()),
            retired: AtomicBool::new(false)
        };


        let ptr_new_node = Box::into_raw(new_node);

        loop {
            let tail = self.tail.load(Ordering::SeqCst); // we need to guarantee tail is not empty
            let new_grown_tail = unsafe {
                tail.as_mut().unwrap().next.compare_and_swap(ptr::null_mut(), ptr_new_node, Ordering::SeqCst)
            };
            if new_grown_tail == ptr::null_mut() {
                self.tail.compare_and_swap(tail, ptr_new_node, Ordering::SeqCst);
                release(tail);
                return;
            } else {
                self.tail.compare_and_swap(tail, new_grown_tail, Ordering::SeqCst);
                release(tail);
            }
        }
    }


    pub fn dequeue(&self) -> Option<T> {
        loop {
            let head = safe_read(&self.head).unwrap();
            let next_res = safe_read(&head.next);
            match next_res {
                Some(next) => {
                    let raw_head = unsafe { transmute_copy(&head) };
                    if self.head.compare_and_swap(raw_head, next, Ordering::SeqCst) != raw_head {
                        release(next);
                        continue;
                    }
                    let ret = (&mut next.data).take();
                    head.retired.store(true, Ordering::SeqCst);
                    unsafe {
                        release(raw_head.as_mut().unwrap());
                    }
                    return ret;
                },
                None => { return None; }
            }
        }
    }
}


fn safe_read<T>(atomic_ptr: &AtomicPtr<Node<T>>) -> Option<&mut Node<T>> {
    unsafe { asm!("1: xbegin 1b" ::: "rax", "memory": "intel", "volatile"); }
    let raw_ptr: *mut Node<T> = atomic_ptr.load(Ordering::Relaxed);
    unsafe {
        match raw_ptr.as_mut() {
            Some(r) => {
                r.ref_cnt.fetch_add(1, Ordering::Acquire);
                asm!("xend" :::: "intel", "volatile");
                return Some(r)
            },

            None => {
                asm!("xend" :::: "intel", "volatile");
                return None
            }
        }
    }
}

extern "C" fn transaction_failed(cause: usize) {
    panic!("Transactional Memory Failed!! {:x}\n", cause);
}

fn release<T>(ptr: *mut Node<T>) {
    unsafe {
        match ptr.as_mut() {
            Some(r) => {
                let cur_count = r.ref_cnt.fetch_sub(1, Ordering::Release);
                if r.retired.load(Ordering::SeqCst) == true && cur_count == 0 {
                    drop(Box::from_raw(r));
                }
            },
            None => {}
        };
    }
}