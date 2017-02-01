use x86::cpuid;
use x86::msr;
use x86::io;
use bit_field::BitField;

pub fn get_apic_id() -> u32 {
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
        msr::wrmsr(msr::IA32_X2APIC_SIVR, 1 << 8 | 20);
        let local_apic_id: u64 = msr::rdmsr(msr::IA32_X2APIC_APICID);
        local_apic_id as u32
    }
}

pub fn mp_abort_all() {
    unsafe {
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
***/
pub unsafe fn micro_delay(microseconds: u64) {
    // Channel 2 stuff I don't understand
    let mut port_61 = io::inb(0x61);
    port_61 &= 0xD;
    port_61 |= 0x1;
    io::outb(0x61, port_61);

    io::outb(0x43, 0xB0);

    let latch_value = 1193182 * microseconds / 1000000;
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