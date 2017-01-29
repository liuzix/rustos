use x86::segmentation::*;
use x86::dtables::*;
use core::intrinsics::transmute;
use core::mem::size_of;

#[macro_use]
use super::wrappers::ExceptionStackFrame;

use bit_field::BitField;
pub struct Idt([Entry; 64]);

lazy_static! {
    pub static ref IDT: Idt = {
        let mut idt = Idt::new();
        for i in 0..64 {
            idt.set_handler(3, exception_handler!(default_handler));
        }
        idt.set_handler(3, exception_handler!(debug_handler));
        idt.set_handler(13, exception_handler_errorcode!(GP_handler));
        idt.set_handler(33, exception_handler!(abort_handler));
        idt
    };
}

extern "C" fn debug_handler(fr: &ExceptionStackFrame) {
    kprint!("int 3!\n");
    kprint!("{:#?}\n", fr);
}

extern "C" fn GP_handler(fr: &ExceptionStackFrame, ec: u64) {
    ::devices::vga::vga_force_unlock();
    kprint!("GP fault!\n");
    kprint!("error_code: {} \n{:#?}\n", ec, fr);
    panic!("GP fault at rip = 0x{:x}", fr.instruction_pointer);
}

extern "C" fn default_handler(fr: &ExceptionStackFrame) {
    ::devices::vga::vga_force_unlock();
    kprint!("I don't know what is wrong.");
    kprint!("{:#?}\n", fr);
    panic!("rip = 0x{:x}", fr.instruction_pointer);
}


extern "C" fn abort_handler(fr: &ExceptionStackFrame) {
    kprint!("abort!\n");
    loop {
        unsafe {
            asm!("hlt" :::: "volatile");
        }
    }
}


pub type HandlerFunc = extern "C" fn() -> !;

impl Idt {
    pub fn new() -> Idt {
        Idt([Entry::missing(); 64])
    }

    pub fn set_handler(&mut self, vec_no: u8, handler: HandlerFunc) -> &mut EntryOptions{
        self.0[vec_no as usize] = Entry::new(cs(), handler);
        &mut self.0[vec_no as usize].options
    }

    pub fn load(&self) {
        unsafe {
            let ptr = DescriptorTablePointer {
                base: transmute::<&Idt, u64>(self),
                limit: (size_of::<Idt>() - 1) as u16
            };
            lidt(&ptr)
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: SegmentSelector,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl Entry {
    fn missing() -> Self {
        Entry {
            gdt_selector: SegmentSelector::new(0) | RPL_0,
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: EntryOptions::minimal(),
            reserved: 0,
        }
    }

    fn new(gdt_selector: SegmentSelector, handler: HandlerFunc) -> Self {
        let pointer = handler as u64;
        Entry {
            gdt_selector: gdt_selector,
            pointer_low: pointer as u16,
            pointer_middle: (pointer >> 16) as u16,
            pointer_high: (pointer >> 32) as u32,
            options: EntryOptions::new(),
            reserved: 0,
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(u16);

impl EntryOptions {
    fn minimal() -> Self {
        let mut options = 0;
        options.set_bits(9..12, 0b111); // 'must-be-one' bits
        EntryOptions(options)
    }

    fn new() -> Self {
        let mut options = Self::minimal();
        options.set_present(true).disable_interrupts(true);
        options
    }

    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    pub fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.0.set_bit(8, !disable);
        self
    }

    pub fn set_privilege_level(&mut self, dpl: u16) -> &mut Self {
        self.0.set_bits(13..15, dpl);
        self
    }

    pub fn set_stack_index(&mut self, index: u16) -> &mut Self {
        self.0.set_bits(0..3, index);
        self
    }
}

