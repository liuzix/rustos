use x86::bits64::task::*;
use x86::shared::task::load_tr;
use x86::shared::segmentation::*;
use x86::shared::dtables::*;

use core::intrinsics::transmute;
use core::mem::size_of;
use alloc::boxed;
use core::sync::atomic::*;
use collections::string::ToString;
use x86::shared::PrivilegeLevel;


#[macro_use]
use super::wrappers::ExceptionStackFrame;

use bit_field::BitField;
pub struct Idt([Entry; 64]);


lazy_static! {
    pub static ref IDT: Idt = {
        let mut idt = Idt::new();
        for i in 0..64 {
            idt.set_handler(i, exception_handler!(default_handler));
        }

        idt.set_handler(3, exception_handler!(debug_handler));
        idt.set_handler(8, exception_handler!(double_handler)).set_stack_index(1);
        idt.set_handler(13, exception_handler_errorcode!(gp_handler)).set_stack_index(0);
        idt.set_handler(14, exception_handler_errorcode!(page_fault_handler)).set_stack_index(2);
        idt.set_handler(60, exception_handler!(abort_handler));
        idt.set_handler(::devices::apic::TIMER_INTERRUPT_VEC, exception_handler!(timer_handler));

        idt
    };


}

extern "C" fn debug_handler(fr: &ExceptionStackFrame) {
    kprint!("int 3!\n");
    let mut rsp: u64;
    unsafe {
        asm!("mov $0, rsp" : "=r"(rsp) :: : "intel");
    }
    kprint!("current rsp = 0x{:x}\n", rsp);
    kprint!("{:#?}\n", fr);

}

extern "C" fn gp_handler(fr: &ExceptionStackFrame, ec: u64) {
    ::devices::vga::vga_force_unlock();
    ::devices::apic::mp_abort_all();
    //::devices::serial::write_string(fr.stack_pointer.to_string().as_str());
    one_fence!();
    kprint!("GP fault at rip = 0x{:x}\n", fr.instruction_pointer);
    kprint!("error_code: {} \n{:#?}\n", ec, fr);
    loop {}
}

extern "C" fn page_fault_handler(fr: &ExceptionStackFrame, ec: u64) {
    ::devices::vga::vga_force_unlock();
    ::devices::apic::mp_abort_all();
    //::devices::serial::write_string(fr.stack_pointer.to_string().as_str());
    one_fence!();
    kprint!("Page fault at rip = 0x{:x}\n", fr.instruction_pointer);
    kprint!("error_code: {} \n{:#?}\n", ec, fr);
    loop {}
}

extern "C" fn default_handler(fr: &ExceptionStackFrame) {
    ::devices::apic::mp_abort_all();
    ::devices::serial::write_string("fuck?");
    //unsafe {
    //    loop {
    //        asm! ("hlt");
    //    }
    //}
    ::devices::vga::vga_force_unlock();

    //one_fence!();
    kprint!("I don't know what is wrong.\n");
    kprint!("{:#?}\n", fr);
    panic!("rip = 0x{:x}", fr.instruction_pointer);
}

extern "C" fn double_handler(fr: &ExceptionStackFrame) {
    ::devices::vga::vga_force_unlock();
    ::devices::apic::mp_abort_all();
    one_fence!();
    kprint!("Double fault!!!\n");
    kprint!("{:#?}\n", fr);
    panic!("rip = 0x{:x}", fr.instruction_pointer);
}


extern "C" fn abort_handler(fr: &ExceptionStackFrame) {
    unsafe { asm!("cli; hlt;") };
}

extern "C" fn timer_handler(fr: &ExceptionStackFrame) {
    ::devices::serial::write_char('!');
    unsafe {
        // ::x86::shared::msr::wrmsr(::x86::shared::msr::IA32_X2APIC_EOI, 0);
    }
    ::tasks::SCHEDULER.schedule();
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
                base: self,
                limit: (size_of::<Idt>() - 1) as u16
            };
            //lidt(&ptr);
            asm!("lidt [$0]" :: "r"(&ptr) :: "intel");
            //;
            install_tss_table();


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
            gdt_selector: SegmentSelector::new(0, PrivilegeLevel::Ring0) | RPL_0,
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
        self.0.set_bits(0..3, index + 1);
        self
    }
}

#[repr(C, packed)]
struct TSSDescriptor {
    limit: u16,
    base_lo: u16,
    base_mi: u8,
    flags: u16,
    base_hi: u8
}

const TSS_OFFSET_IN_GDT: usize = 24;

pub unsafe fn install_tss_table() {
    //kprint!("setting tss\n");
    let tss = get_tss_table();
    let mut descriptor = TSSDescriptor {
        // 0 entries to be set later
        limit: size_of::<TaskStateSegment>() as u16 - 1,
        base_lo: (transmute::<_, usize>(tss) & 0xFFFF) as u16,
        base_mi: ((transmute::<_, usize>(tss)) >> 16 & 0xFF) as u8,
        base_hi: ((transmute::<_, usize>(tss)) >> 32 & 0xFF) as u8,
        flags: 0b0000000010001001
    };
    use super::gdt;
    let gdt_ref: &mut gdt::GDTController = gdt::GDT.0.get().as_mut().unwrap();
    let tss_index = gdt_ref.add(transmute::<TSSDescriptor, u64>(descriptor));
    gdt_ref.install();
    load_tr(SegmentSelector::new(tss_index as u16, PrivilegeLevel::Ring0))
}

pub fn get_tss_table<'a>() -> &'a TaskStateSegment {
    let tss: &mut TaskStateSegment = unsafe { transmute(::mem::FRAME.alloc()) };
    *tss = TaskStateSegment::new();
    tss.ist[0] = ::mem::FRAME.alloc_stack(2) as u64;
    tss.ist[1] = ::mem::FRAME.alloc_stack(2) as u64;
    tss.ist[2] = ::mem::FRAME.alloc_stack(2) as u64;
    //kprint!("ist[0] = 0x{:x}\n", tss.ist[0]);
    //kprint!("ist[1] = 0x{:x}\n", tss.ist[1]);
    //kprint!("ist[2] = 0x{:x}\n", tss.ist[2]);
    tss
}