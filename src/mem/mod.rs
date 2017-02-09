use multiboot2;
use core::option::Option;
use core::slice;
use core::mem;
use core::cell;
use devices::vga;

pub mod bitmap;
pub mod frame;
pub mod paging;
pub mod heap_allocator;
pub mod alloc_stub;





pub static mut BOOTINFO: usize = 0;
lazy_static! {

    pub static ref FRAME: frame::FrameAllocator<'static> = unsafe {
        assert!(BOOTINFO != 0);
        let (base, size) = parse_multiboot(BOOTINFO);
        frame::FrameAllocator::new(base, size)
    };
}

pub fn parse_multiboot(paddr: usize) -> (usize, usize) {
    let bootinfo = unsafe {multiboot2::load(paddr)};
    let memtag = bootinfo.memory_map_tag().expect("cannot find memory tag");
    kprint!("memory info:\n");
    let mut mem_upper_bd: usize = 0;
    for area in memtag.memory_areas() {
        kprint!("start: 0x{:x}, length: 0x{:x}\n",
                 area.base_addr,
                 area.length);
        mem_upper_bd = (area.base_addr + area.length) as usize;
    }

    let elftag = bootinfo.elf_sections_tag().expect("cannot find elf tag");
    kprint!("elf sections loaded:\n");
    let mut mem_lower_bd: usize = 0;
    for section in elftag.sections() {
        if !section.is_allocated() {
            continue;
        }
        kprint!("section start: 0x{:x} end: 0x{:x}\n",
                section.start_address(),
                section.end_address());
        mem_lower_bd = section.end_address();
    }
    let boot_end = bootinfo.end_address();
    mem_lower_bd = if boot_end > mem_lower_bd {
        boot_end
    } else {
        mem_lower_bd
    };

    kprint!("available memory starts at 0x{:x}\n", mem_lower_bd);
    (mem_lower_bd,  mem_upper_bd - mem_lower_bd)
}
