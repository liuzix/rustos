use core::ptr::Unique;
use rlibc;
use core::fmt;
use spin::Mutex;
use core::mem;
use core::intrinsics;

pub static VGAWRITER: Mutex<VgaWriter> = Mutex::new(VgaWriter::new());

pub fn print(args : fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = VGAWRITER.lock();
    writer.write_fmt(args);
}

pub fn vga_force_unlock() {
    unsafe {
        VGAWRITER.force_unlock();
    }
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ({
        $crate::devices::vga::print(format_args!($($arg)*));
    });
}

#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy)]
struct ColorCode(u8);

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

impl ScreenChar {
    pub fn volatile_set(&mut self, another: ScreenChar) {
        unsafe {
            intrinsics::volatile_store(self, another)
        }
    }
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct VgaWriter {
    row: usize,
    col: usize,
    buf: Unique<Buffer>,
}

impl VgaWriter {
    // return a vga writer who's buffer is the default vgabuff
    pub const fn new() -> VgaWriter {
        VgaWriter {
            row: 0,
            col: 0,
            buf: unsafe { Unique::new(0xb8000 as *mut _) },
        }
    }

    pub fn clear(&mut self) {
        use core::mem;
        unsafe {
            // zeroing out vga text buffer
            let ptr: *mut u8 = mem::transmute_copy(&self.buf.get_mut());
            rlibc::memset(ptr, 0, mem::size_of_val(&self.getbuffer().chars));
        }
    }

    pub fn putchar(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            _ => {
                let color = ColorCode::new(Color::White, Color::Black);
                self.getbuffer().chars[self.row][self.col] = ScreenChar {
                    ascii_character: c as u8,
                    color_code: color,
                };
                self.next();
            }
        }
    }

    fn getbuffer(&mut self) -> &mut Buffer {
        unsafe { self.buf.get_mut() }
    }

    fn next(&mut self) {
        self.col += 1;
        if self.col == BUFFER_WIDTH {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.row += 1;
        self.col = 0;
        if self.row == BUFFER_HEIGHT {
            self.scroll();
        }
    } 

    fn scroll(&mut self) {
        self.row -= 1;
        let buf = self.getbuffer();
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {

                buf.chars[row - 1][col] = buf.chars[row][col];
            }
        }

        for col in 0..BUFFER_WIDTH {
            // volatile_set prevents SSE optimization.
            // SIMD writes to video memory causes kvm to crash
            buf.chars[BUFFER_HEIGHT-1][col]
                .volatile_set (ScreenChar {ascii_character: 0, color_code: ColorCode(0)});
        }

    }
}

impl fmt::Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for c in s.chars() {
            self.putchar(c);
        }
        return Ok(());
    }
}
