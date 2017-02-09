#![feature(allocator)]
#![allocator]
#![feature(alloc)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(core_intrinsics)]
#![feature(associated_consts)]
#![no_std]
#![allow(unused)]
#![allow(mutable_transmutes)]
#![feature(collections)]
#![allow(private_no_mangle_fns)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(box_syntax)]
#![feature(repr_simd)]
#![feature(i128_type)]
#![feature(link_llvm_intrinsics)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![feature(ptr_eq)]
#[macro_use]
extern crate alloc;
#[macro_use]
extern crate x86;
extern crate spin;
extern crate rlibc;
extern crate multiboot2;
extern crate bit_field;

// extern crate alloc;
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
mod tasks;
mod containers;
mod fs;
use interrupt::descriptors;
use tasks::threads::KThread;
use devices::serial;
use devices::vga;
use mem::paging;
use mem::heap_allocator;
use core::fmt::Write;
use collections::vec;
use core::intrinsics::transmute;
use collections::string::ToString;
use x86::shared::irq;
use containers::queue::Queue;
use containers::cpu_local::CPULocal;
use core::sync::atomic::*;


#[no_mangle]
pub extern "C" fn kmain(bootinfo: usize) {
    let mut vwriter = vga::VgaWriter::new();
    vwriter.clear();
    serial::init();
    kprint!("multiboot_info = {:x}\n", bootinfo);

    // mem::parse_multiboot(bootinfo);
    unsafe { mem::BOOTINFO = bootinfo };
    mem::bitmap::test_bitmap();

    use alloc::boxed::Box;
    let heap_test = Box::new(42);

    descriptors::IDT.load();

    test_sse();
    test_mapping();

    let id = devices::apic::mp_apic_init();
    kprint!("cpu local id {}\n", id);

    unsafe {
        //   int!(12);
        // let ptr = 0xfff000000 as *mut u64;
        // ptr = 0x666666;
        // kprint!("{:x}\n", *ptr);
    }
    // kprint!("we are back!\n");
    load_ap_bootstrap(0x1000);
    devices::apic::mp_init_broadcast(0x1000);

    // unsafe { devices::apic::micro_delay(1 * 1000); }
    for _ in 0..2 {
        unsafe {
            devices::apic::micro_delay(50 * 1000);
        }
    }

    kprint!("found {} PCI devices \n", devices::pci::PCI_DEVICES.len());
    // call this to initialize global AHCI
    //devices::ahci::global_HBA_status();
    kprint!("found {} block devices\n", fs::block::BLOCK_DEVICES.len());
    tasks::threads::new_thread(thread_test, "init");
    ::devices::apic::enable_timer();

    loop {}
}

lazy_static! {
    static ref clocal: CPULocal<usize> = CPULocal::create();
}

fn test_cpu_local() {
    for x in 0..1 {
        clocal.set(x);
        assert!(*clocal.get_mut().unwrap() == x);
    }
}

fn thread_test(val: usize) -> usize {
    kprint!("we are in a new thread! 0x{:x}\n", val);
    ::tasks::threads::new_thread(thread_test2, "Test2");
    ::tasks::threads::new_thread(thread_test2, "Test2");
    ::tasks::threads::new_thread(thread_test2, "Test2");
    ::tasks::threads::new_thread(thread_test2, "Test2");
    ::tasks::threads::new_thread(thread_test2, "Test2");
    ::tasks::threads::new_thread(thread_test2, "Test2");




    0x1
}

static i: AtomicUsize = ATOMIC_USIZE_INIT;

fn thread_test2(val: usize) -> usize {
    kprint!("we are in a new thread! 0x{:x}\n", val);
    test_parallel_block();
    /*
    let mut vec = collections::Vec::new();
    for x in 0..10000 {
        vec.push(x);
    }

    for x in 0..10000 {
        vec.pop();
    }
    kprint!("done!\n");
    */
    0x2
}

lazy_static! {
    static ref q: Queue<usize> = Queue::create();
}

#[no_mangle]
pub extern "C" fn mp_main() {
    unsafe { irq::enable() };

    let id = devices::apic::mp_apic_init();
    //kprint!("cpu local id {}\n", id);

    for _ in 0..3000 {
        unsafe {
            asm!("pause" :::: "volatile");
        }
    }

    ::devices::apic::enable_timer();

    loop {
        unsafe {
            asm!("sti; hlt;");
        }
    }
}

#[no_mangle]
pub extern "C" fn create_stack() -> usize {
    unsafe {
        x86::shared::tlb::flush_all();
    }
    descriptors::IDT.load();
    let ret = mem::FRAME.alloc_stack(2);
    // kprint!("stack = 0x{:x}\n", ret);
    ret

}

extern "C" {
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
    let mut numbers: [u64; 4] = [1, 2, 3, 4];
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

fn test_mapping() {
    unsafe {
        let a = mem::FRAME.alloc_multiple(2);
        let b = mem::FRAME.alloc_multiple(2);
        let k: *mut usize = transmute(a);
        let l: *mut usize = transmute(b);
        *k = 0x23333333;
        *l = 0x12345677;
        assert_eq!(*k, 0x23333333);
        assert_eq!(*l, 0x12345677);
    }
}

fn test_parallel_block() {
    let block = &fs::block::BLOCK_DEVICES[0];
    let test: &mut usize = unsafe { (mem::FRAME.alloc() as *mut usize).as_mut().unwrap() };
    for x in 0..10000 {
        unsafe {
            *test = x;
            block.write_block_raw(core::mem::transmute_copy(&test), x);

            *test = 0x0;
            block.read_block_raw(core::mem::transmute_copy(&test), x);
            //kprint!("{}\n",x);
        }

        assert_eq!(*test, x);
    }
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
    devices::apic::mp_abort_all();
    unsafe {
        irq::disable();
        asm!("cli; hlt;");
    }
    loop {}
    //
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

/// This is just stupid
#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    mem::alloc_stub::__rust_allocate(size, 16)
}

#[no_mangle]
pub extern "C" fn posix_memalign(size: usize, align: usize) -> *mut u8 {
    mem::alloc_stub::__rust_allocate(size, align)
}
