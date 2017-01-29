#![feature(allocator)]
#![allocator]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(core_intrinsics)]
#![feature(associated_consts)]
#![no_std]
#![allow(unused)]
#![feature(collections)]
#![allow(private_no_mangle_fns)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(alloc)]
#[macro_use]
extern crate x86;
extern crate spin;
extern crate rlibc;
extern crate multiboot2;
extern crate bit_field;

extern crate alloc;
#[macro_use]
extern crate collections;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
#[macro_use]
mod devices;
mod mem;
mod interrupt;
use interrupt::descriptors;
use devices::serial;
use devices::vga;
use mem::paging;
use mem::heap_allocator;
use core::fmt::Write;
use collections::vec;
use core::intrinsics::transmute;
use collections::string::ToString;

#[no_mangle]
pub extern "C" fn kmain(bootinfo: usize) {
    let mut vwriter = vga::VgaWriter::new();
    vwriter.clear();
    serial::init();
    kprint!("multiboot_info = {:x}\n", bootinfo);

    //mem::parse_multiboot(bootinfo);
    unsafe{mem::BOOTINFO = bootinfo};
    mem::bitmap::test_bitmap();

    use alloc::boxed::Box;
    let heap_test = Box::new(42);

    descriptors::IDT.load();
    test_sse();

    let id = devices::apic::get_apic_id();
    kprint!("cpu local id {}\n", id);
    //unsafe {int!(3);}
    //kprint!("we are back!\n");
    for x in 0..10 {
        kprint!("fuck {} \n", x);
    }
    load_ap_bootstrap(0x1000);
    devices::apic::mp_init_broadcast(0x1000);
    loop {

    }
}

#[no_mangle]
pub extern "C" fn mp_main() {
    let id = devices::apic::get_apic_id();
    kprint!("cpu local id {}\n", id);
    let mut v = vec![0];
    for x in 0..300 {
        v.push(x);
    }

    for x in 0..100{
        serial::write_string((v[x].to_string() + "\n").as_str());

    }

    loop {}
}

#[no_mangle]
pub extern "C" fn create_stack() -> usize {
    descriptors::IDT.load();
    let ret = mem::FRAME.alloc_stack(2);
    kprint!("stack = 0x{:x}\n", ret);
    ret

}

extern {
    static mut mp_start: u8;
    static mut mp_end: u8;
}

fn load_ap_bootstrap(addr: u64) {
    unsafe {
        let distance: usize = transmute::<_, usize>(&mp_end) - transmute::<_, usize>(&mp_start);
        let ptr: *mut u8 = ::core::intrinsics::transmute(addr);
        rlibc::memmove(ptr, &mut mp_start, distance);
    }
}

fn test_sse() {
    let mut numbers: [u64; 4] = [1,2,3,4];
    let ptr = numbers.as_mut_ptr();
    unsafe {
        asm!("mov r8, 128000
              movq xmm0, r8
              mov r8, $0
              movups [r8], xmm0"
              : :"m"(ptr): "r8": "intel");
    }
    assert!(numbers[0] == 128000);
    kprint!("xmm registers working\n");
}

// These functions are used by the compiler, but not
// for a bare-bones hello world. These are normally
// provided by libstd.
#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

// This function may be needed based on the compilation target.
#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern "C" fn rust_eh_unwind_resume() {}

#[lang = "panic_fmt"]
#[no_mangle]
pub extern "C" fn rust_begin_panic(_msg: core::fmt::Arguments,
                                   _file: &'static str,
                                   _line: u32)
                                   -> ! {
    vga::VGAWRITER.lock().write_fmt(_msg);
    kprint!("\nat file {} line {}\n", _file, _line);
    serial::write_string("panic!");
    devices::apic::mp_abort_all()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}
