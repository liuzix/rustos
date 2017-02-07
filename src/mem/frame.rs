use super::bitmap;
use core::cell::UnsafeCell;
use core::sync::atomic::*;
use super::paging::*;


const KER_LOWER_BOUND: u64 = 0xc0000000;

static FREE_ADDRESS: AtomicUsize = ATOMIC_USIZE_INIT;

unsafe impl<'a> Sync for FrameAllocator<'a> {}

pub struct FrameAllocator<'a> {
    available_base: usize,
    upper: usize,
    freemap: UnsafeCell<bitmap::Bitmap<'a>>,
}

impl<'a> FrameAllocator<'a> {
    pub fn new(base: usize, len_bytes: usize) -> FrameAllocator<'a> {
        FREE_ADDRESS.store(KER_LOWER_BOUND as usize, Ordering::SeqCst);
        FrameAllocator {
            freemap: UnsafeCell::new(bitmap::Bitmap::new(base, len_bytes / 4096)),
            available_base: ((base + (len_bytes / (4096 * 8))) / 4096 + 1) * 4096,
            upper: base + len_bytes,
        }
    }

    pub fn alloc_multiple(&self, cnt: usize) -> usize {
        let cur = FREE_ADDRESS.fetch_add(cnt * 4096, Ordering::SeqCst);
        for i in 0..cnt {
            let pframe = self.alloc();
            page_map(cur + i * 4096, pframe);
        }
        cur
    }

    pub fn dealloc_multiple(&self, cnt: usize) {

    }

    pub fn alloc_stack(&self, cnt_in_page: usize) -> usize {
        let ret = self.alloc_multiple(cnt_in_page + 1);
        page_unmap(ret);
        ret + ((cnt_in_page + 1) * 4096)
    }

    pub fn alloc(&self) -> usize {
        let fm: &mut bitmap::Bitmap = unsafe { &mut *self.freemap.get() };
        let pos = fm.set_first_unused();
        if self.available_base >= self.upper {
            panic!("OOM!");
        }
        let ret = self.available_base + 4096 * pos;
        //kprint!("new frame = 0x{:x}\n", ret);
        return ret;
    }

    pub fn dealloc(&self, addr: usize) {
        let fm: &mut bitmap::Bitmap = unsafe { &mut *self.freemap.get() };
        let pos = (addr - self.available_base) / 4096;
        assert!(fm.get(pos));
        fm.set(pos, false);
    }
}
