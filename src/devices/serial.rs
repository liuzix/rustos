use x86::shared::{io, irq};
use spin::Mutex;
use interrupt::guard::InterruptGuard;


static SERIAL_LOCK: Mutex<()> = Mutex::new(());

const PORT: u16 = 0x3f8;
pub fn init() {
    let g = SERIAL_LOCK.lock();
    unsafe {
        io::outb(PORT+1, 0x00);
        io::outb(PORT+3, 0x80);
        io::outb(PORT+0, 0x03);
        io::outb(PORT+1, 0x00);
        io::outb(PORT+3, 0x03);
        io::outb(PORT+2, 0xC7);
        io::outb(PORT+4, 0x0B);
    }
    drop(g);
}

fn is_transmit_empty() -> u8 {
    unsafe {io::inb(PORT+5) & 0x20}
}

pub fn write_char(a: char) {
    while is_transmit_empty() == 0 {};
    unsafe {io::outb(PORT, a as u8)}
}

pub fn write_string(s: &str) {
    let guard = InterruptGuard::disable_interrupt();
    let g = SERIAL_LOCK.lock();
    for c in s.chars() {
        write_char(c);
    }
    drop(g);
}