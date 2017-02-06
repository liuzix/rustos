use core::mem::size_of_val;
use core::mem::transmute_copy;
use core::intrinsics::transmute;
use core::cell::UnsafeCell;
use core::marker::Sync;

extern {
    static mut gdt_pointer: GDTPointer;
}

pub struct GDTCell(pub UnsafeCell<GDTController>);

unsafe impl Sync for GDTCell {}
///
/// This GDT structure is shared by all CPUs
///
lazy_static! {
        pub static ref GDT: GDTCell = GDTCell (UnsafeCell::new(
        unsafe {GDTController::from_raw_and_copy(&gdt_pointer)}));
}

#[repr(C, packed)]
struct GDTPointer {
    limit: u16,
    address: usize
}

const GDT_SIZE: usize = 64;

pub struct GDTController {
    table: [u64; GDT_SIZE],
    next_free: usize,
    ptr: GDTPointer
}

impl GDTController {
    ///
    /// Used to copy the bootstrap gdt to somewhere else
    ///
    fn from_raw_and_copy(raw_gdt_ptr: &GDTPointer) -> GDTController {
        let mut ret = GDTController {
            table: [0; GDT_SIZE],
            next_free: ((raw_gdt_ptr.limit + 1) / 8) as usize,
            ptr: GDTPointer {
                limit: GDT_SIZE as u16 * 8 - 1,
                address: 0,
            } // space reserved for install
        };
        // check capacity
        assert!(raw_gdt_ptr.limit < size_of_val(&ret.table) as u16);
        assert!(raw_gdt_ptr.limit % 8 == 7); // check length is legal
        let table_ptr: *mut u64 = ret.table.as_mut_ptr();
        unsafe {
            ::rlibc::memcpy(table_ptr as *mut u8,
                            raw_gdt_ptr.address as *const u8,
                            raw_gdt_ptr.limit as usize + 1);
        }
        ret
    }

    ///
    /// requires a static reference to install
    ///
    pub fn install(&'static mut self) {
        self.ptr.address = self.table.as_ptr() as usize;
        //kprint!("ptr = 0x{:x}\n", self.ptr.address);
        unsafe {
            asm!("lgdt [$0]" :: "r"(&self.ptr): "memory": "volatile", "intel");
        }
    }

    ///
    /// adds a descriptor to GDT
    ///
    pub fn add(&mut self, item: u64) -> usize {
        let index = unsafe {
            ::core::intrinsics::atomic_xadd(&mut self.next_free, 2)
        };
        unsafe { ::core::intrinsics::atomic_fence(); }
        assert!(index < GDT_SIZE);
        self.table[index] = item;
        index
    }
}