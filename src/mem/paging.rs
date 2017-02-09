use x86::shared::control_regs::*;
use x86::shared::tlb;
use core::slice;
use core::mem::size_of;
use core::intrinsics::atomic_cxchg;

use bitflags;
use core::option;
use super::FRAME;

#[derive(Copy, Clone)]
pub struct Entry(usize);

bitflags! {
    pub flags EntryFlags: usize {

        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER_ACCESSIBLE = 1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const NO_CACHE =        1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const HUGE_PAGE =       1 << 7,
        const GLOBAL =          1 << 8,
        const NO_EXECUTE =      1 << 63,
    }
}

impl Entry {
    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn set_paddr(&mut self, paddr: usize) {
        assert!(paddr % 4096 == 0);
        self.0 |= paddr;
    }

    pub fn paddr(&self) -> usize {
        (self.0 >> 12) << 12
    }

    pub fn set_flags(&mut self, flags: EntryFlags) {
        self.0 = flags.bits() | self.paddr()
    }

    pub fn to_int(&self) -> usize {
        self.0
    }

    pub fn try_set_paddr(&mut self, paddr: usize) {
        let old: Entry = *self;
        let mut new: Entry = old;
        new.set_paddr(paddr);
        new.set_flags(PRESENT | WRITABLE);
        unsafe {
            atomic_cxchg(&mut self.0, old.0, new.0);
            ::core::intrinsics::atomic_fence();
        }
    }
}

pub fn page_map(vaddr: usize, paddr: usize) -> Option<usize> {
    match get_entry(vaddr, false) {
        Some(_) => panic!("vaddr already in use. vaddr = {:x}", vaddr),
        None => {
            let entry: &mut Entry = get_entry(vaddr, true).unwrap();
            entry.set_paddr(paddr);
            entry.set_flags(PRESENT | WRITABLE);
            unsafe {tlb::flush(vaddr)};
            Some(vaddr)
        }
    }
}

pub fn page_unmap(vaddr: usize) {
    match get_entry(vaddr, false) {
        Some(entry) => {
            entry.set_flags(EntryFlags::empty());
        },
        None => {
            return;
        }
    }
}

pub fn get_entry<'a>(vaddr: usize, create: bool) -> Option<&'a mut Entry> {
    let mut table: &mut [Entry; 512] = unsafe {get_table(cr3() as usize)};
    for level in (0..4).rev() {
        {
            let target = &mut table[get_index(vaddr, level)];
            //kprint!("{:x}\n", target.toInt());
            if target.flags().contains(PRESENT) == false {
                if !create {
                    return Option::None;
                } else {
                    if level == 0 {
                        let flags = target.flags();
                        target.set_flags(flags | PRESENT);
                        return unsafe { Option::Some(::core::intrinsics::transmute(target)) };
                    }
                    target.try_set_paddr(create_table());
                }
            }
            let flags = target.flags();
            target.set_flags(flags | PRESENT | WRITABLE);
        }
        table = unsafe {
            //kprint!("level = {}, addr = {:x}\n",level, table[get_index(vaddr, level)].paddr());
            get_table(table[get_index(vaddr, level)].paddr())
        };
    }
    Option::None
}

pub fn translate(vaddr: usize) -> Option<usize> {
    match get_entry(vaddr, false) {
        None => None,
        Some(e) => Some(e.paddr())
    }
}

fn get_index(vaddr: usize, level: u8) -> usize {
    let begin = 12 + level * 9;
    let end = 12 + level * 9 + 8;
    let mut temp = vaddr as usize;
    temp = temp << (63 - end);
    temp = temp >> (63 - end);
    temp = temp >> begin;
    temp
}

unsafe fn get_table<'a>(vaddr: usize) -> &'a mut [Entry; 512] {
    let intptr = (vaddr >> 12) << 12;
    &mut *(intptr as *mut _)
}

fn create_table() -> usize {
    //kprint!("gwa!");
    FRAME.alloc()
}

pub fn map_volatile(addr: usize) -> usize {
    let entry = get_entry(addr, true).unwrap();
    entry.set_paddr(addr);
    entry.set_flags(PRESENT | WRITABLE | NO_CACHE | WRITE_THROUGH);
    unsafe {
        tlb::flush(addr);
    }
    addr
}