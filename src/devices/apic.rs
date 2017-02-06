use x86::bits64::cpuid;
use x86::shared::{msr, irq};
use x86::shared::io;
use bit_field::BitField;
use core::sync::atomic::*;

pub static CPU_COUNT: AtomicUsize = ATOMIC_USIZE_INIT;

pub fn mp_apic_init() -> u32 {
    CPU_COUNT.fetch_add(1, Ordering::Relaxed);
    let cpuid_res: u32 = cpuid::cpuid1(1).ecx;
    if !cpuid_res.get_bit(21) {
        panic!("No x2APIC");
    }

    unsafe {
        let mut ia32_apic_base: u64 = msr::rdmsr(msr::IA32_APIC_BASE);
        if ia32_apic_base.get_bits(10..12) != 0b11 {
            ia32_apic_base.set_bits(10..12, 0b11); // enable 2xAPIC mode
            msr::wrmsr(msr::IA32_APIC_BASE, ia32_apic_base);
        }
        // disable 8259 pic
        io::outb(0xa1, 0xff);
        io::outb(0x21, 0xff);
        msr::wrmsr(msr::IA32_X2APIC_EOI, 0);
        msr::wrmsr(msr::IA32_X2APIC_SIVR, 1 << 8 | 20); // setup spurious interrupt handler. Important!
        get_cpu_id()
    }
}

pub fn get_cpu_id() -> u32 {
    unsafe {
        let local_apic_id: u64 = msr::rdmsr(msr::IA32_X2APIC_APICID);
        local_apic_id as u32
    }
}


pub fn mp_abort_all() {
    unsafe {
        irq::disable();
        msr::wrmsr(msr::IA32_X2APIC_ICR, 0xc403c);
    }
}

pub fn mp_init_broadcast(entry_point: u64) {
    let vector_no: u64 = (entry_point >> 12) & 0xFF;
    unsafe {
        msr::wrmsr(msr::IA32_X2APIC_ICR, 0xc4500);
        micro_delay(10 * 1000); // 10ms
        msr::wrmsr(msr::IA32_X2APIC_ICR, 0xc4600 | vector_no);
        micro_delay(200);
        msr::wrmsr(msr::IA32_X2APIC_ICR, 0xc4600 | vector_no);
    }
}

/***
It will spin until the timer goes off
CANNOT be run in parallel!
***/
pub unsafe fn micro_delay(microseconds: u64) {
    // Channel 2 stuff I don't understand
    let mut port_61 = io::inb(0x61);
    port_61 &= 0xD;
    port_61 |= 0x1;
    io::outb(0x61, port_61);

    io::outb(0x43, 0xB0);

    let latch_value = (1193182 * microseconds) / 1000000;
    io::outb(0x42, (latch_value & 0xFF) as u8);
    io::outb(0x42, ((latch_value >> 8) & 0xFF) as u8);

    while io::inb(0x61) & 0x20 == 0 {
        //kprint!("{:x}\n", io::inb(0x61));
        unsafe {asm!("pause")};
    }

    port_61 = io::inb(0x61);
    port_61 &= 0xC;
    io::outb(0x61, port_61);
}

const TIMER_INTERVAL: usize = 25; // in ms
pub const TIMER_INTERRUPT_VEC: u8 = 32;

///
/// Enables Apic timer
///
pub fn enable_timer() {
    unsafe {
        msr::wrmsr(msr::IA32_X2APIC_DIV_CONF, 3);
        msr::wrmsr(msr::IA32_X2APIC_LVT_TIMER, 1 << 17 | TIMER_INTERRUPT_VEC as u64);
        let calib_init: usize = 10000000;
        msr::wrmsr(msr::IA32_X2APIC_INIT_COUNT, calib_init as u64);
        micro_delay(5 * 1000); // delay 1 ms
        let cur_count: usize = msr::rdmsr(msr::IA32_X2APIC_CUR_COUNT) as usize;
        msr::wrmsr(msr::IA32_X2APIC_INIT_COUNT, ((calib_init - cur_count) / 5 * TIMER_INTERVAL) as u64);
        ::x86::shared::irq::enable();
    }
}