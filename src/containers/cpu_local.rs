use devices::apic::*;
use core::cell::*;
use collections::vec::*;
use core::sync::atomic::Ordering;
use core::ops::{Deref, DerefMut};
use core::marker::Sync;

macro_rules! atomic_begin {
    () => {
        unsafe {
            asm! ("1: xbegin 1b" :::: "intel", "volatile");
        }
    };
}

macro_rules! atomic_end {
    () => {
        unsafe {
            asm! ("xend" :::: "intel", "volatile");
        }
    };
}


pub struct CPULocal<T> {
    data: UnsafeCell<Vec<Option<T>>>,
}

unsafe impl<T> Sync for CPULocal<T> {}

impl<T> CPULocal<T> {
    pub fn create() -> Self {
        let num_cpu = CPU_COUNT.load(Ordering::Relaxed);
        let mut vec = Vec::with_capacity(3 * num_cpu);
        for _ in 0..num_cpu {
            vec.push(None);
        }

        CPULocal {
            data: UnsafeCell::new(vec),
        }
    }

    pub fn get_mut(&self) -> Option<&mut T> {
        let cpu_id = get_cpu_id() as usize;
        let vec: &mut Vec<Option<T>> = unsafe { self.data.get().as_mut().unwrap() };

        if vec.len() - 1 < cpu_id {
            // index too large
            return None;
        }

        let v = vec[cpu_id].as_mut();
        if v.is_some() {
            return Some(v.unwrap());
        } else {
            return None;
        }
    }


    pub fn set(&self, val: T) {
        let cpu_id = get_cpu_id() as usize;
        let vec: &mut Vec<Option<T>> = unsafe { self.data.get().as_mut().unwrap() };
        atomic_begin!();
        if vec.len() - 1 < cpu_id {
            for _ in 0..(cpu_id - vec.len() + 1) {
                vec.push(None);
            }
        }
        atomic_end!();
        vec[cpu_id] = Some(val);
    }

    pub fn into_inner(&self) -> Option<T> {
        let cpu_id = get_cpu_id() as usize;
        let vec: &mut Vec<Option<T>> = unsafe { self.data.get().as_mut().unwrap() };

        if vec.len() - 1 < cpu_id {
            // index too large
            return None;
        }

        let v: &mut Option<T> = &mut vec[cpu_id];
        v.take()
    }
}

