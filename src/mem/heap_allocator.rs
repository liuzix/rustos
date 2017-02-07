use spin::*;
use core::sync::atomic::*;
use core::intrinsics::transmute;
use core::mem::transmute_copy;
use core::ptr;
use core::mem::size_of;
use core::iter::*;
use core::marker::PhantomData;
use devices::serial;
use super::FRAME;

lazy_static! {
    pub static ref HEAP: HeapAllocator =
        HeapAllocator {
            arenas: AtomicPtr::new(ptr::null_mut()),
        };
}

macro_rules! pointer_sanity {
    ($ptr:expr) => {
        unsafe {assert!(transmute_copy::<_, usize>(&$ptr) < 0xFFFFFFFF);}
    };
}

macro_rules! aligned_size {
    ($t:ident) => {
        ((size_of::<$t>() - 1) / 16 + 1) * 16
    };
}

pub struct HeapAllocator {
    arenas: AtomicPtr<Arena>
}

struct Arena {
    next: AtomicPtr<Arena>,
    blocks: Mutex<*mut Block>
}

const BLOCK_MAGIC: usize = 0xdeadbeef;
pub struct Block {
    length: usize,
    free: bool,
    next: *mut Block,
    arena: *mut Arena,
    magic: usize
}

const MIN_BLOCK_SIZE: usize = 8;
const MAX_BLOCK_SIZE: usize = 3072;

impl HeapAllocator {
    pub fn allocate(&self, len: usize) -> usize {
        if len > MAX_BLOCK_SIZE {
            return self.allocate_huge(len)
        }
        if let Some(first) = unsafe { self.arenas.load(Ordering::SeqCst).as_mut() } {
            for current in first.iter() {
                match current.allocate(len) {
                    Some(x) => {
                        //    kprint!("allocate 0x{:x} size = {}\n", x as usize, len);
                        return x
                    },
                    None => continue,
                }
            }
        } else {
            self.arenas.store(Arena::new(), Ordering::SeqCst);
            return self.allocate(len);
        }

        let mut last: &Arena = unsafe { self.arenas.load(Ordering::SeqCst).as_mut().unwrap().iter() }.last().unwrap();
        let new_arena = Arena::new();

        let ret = new_arena.allocate(len).unwrap();
        while last.next.compare_and_swap(ptr::null_mut(), new_arena, Ordering::SeqCst).is_null() == false {
            let new_last = last.iter().last().unwrap();
            last = new_last;
        }
        //last.next.store(new_arena, Ordering::SeqCst);

        ret
    }

    pub fn deallocate(&self, ptr: *mut u8, len: usize) {
        //kprint!("free?\n");
        let block: &mut Block = unsafe {
            transmute::<_, *mut Block>(ptr.offset(0 - aligned_size!(Block) as isize))
                .as_mut().unwrap()
        };
        //kprint!("free 0x{:x} size = {}\n", ptr as usize, block.length);
        if block.length > MAX_BLOCK_SIZE {
            // is the a huge block?
            let page_head: *const _ = block;
            FRAME.dealloc_multiple(page_head as usize);
            return;
        }

        block.free();
    }

    pub fn allocate_huge(&self, len: usize) -> usize {
        kprint!("allocate_huge! len = {}\n", len);
        assert!(len > MAX_BLOCK_SIZE);
        let page_cnt: usize = (len + aligned_size!(Block) - 1) / 4096 + 1;
        assert!(page_cnt * 4096 >= len + aligned_size!(Block));
        let mut blk = Block::create(::mem::FRAME.alloc_multiple(page_cnt));
        blk.arena = ptr::null_mut();
        blk.free = false;
        blk.length = len;
        blk.next = ptr::null_mut();
        unsafe { transmute::<&mut Block, usize>(blk) + aligned_size!(Block) }
    }
}

struct ArenaIter<'a> {
    next: *const Arena,
    phantom: PhantomData<&'a Arena>,
}
impl<'a> Iterator for ArenaIter<'a>{
    type Item = &'a Arena;

    fn next(&mut self) -> Option<&'a Arena> {
        let res = self.next;
        self.next = unsafe { (*self.next).next.load(Ordering::SeqCst) };
        if !res.is_null() {
            Some(unsafe { res.as_ref().unwrap() })
        } else {
            None
        }
    }
}

impl Arena {
    pub fn new<'a>() -> &'a mut Arena {
        let addr = FRAME.alloc();
        let new_arena = unsafe {transmute::<usize, &mut Arena>(addr)};
        new_arena.next = AtomicPtr::new(ptr::null_mut());

        let new_block = Block::create(addr + aligned_size!(Arena));
        pointer_sanity!(new_block);
        new_block.length = 4096 - aligned_size!(Arena) - aligned_size!(Block);
        new_block.free = true;
        new_block.next = ptr::null_mut();
        new_block.arena = new_arena;
        new_block.magic = BLOCK_MAGIC;
        new_arena.blocks = Mutex::new(new_block);
        new_arena
    }

    pub fn allocate(&self, len: usize) -> Option<usize> {
        let mut len = ((len - 1) / 16) * 16 + 16;
        //kprint!("{:x}\n", unsafe {transmute::<_, usize>(&self.blocks)});
        //let try_lock = self.blocks.try_lock();
        //if try_lock.is_none() {
        //    return None;
        //}
        let mut guard: MutexGuard<*mut Block> = self.blocks.lock();
        //kprint!("gwa!\n");
        let mut r: *mut Block = *guard;
        let mut previous: Option<*mut Block> = None;
        while let Some(this_block) = unsafe {r.as_mut()} {
            //kprint!("this block = {:x}\n", unsafe {transmute_copy::<_, usize>(&this_block)});
            pointer_sanity!(this_block);
            assert!(this_block.magic == BLOCK_MAGIC);
            assert!(this_block.free == true);
            if this_block.length >= len {
                this_block.free = false;
                this_block.shrink_to_fit(len);
                if let Some(previous) = previous {

                    unsafe {previous.as_mut().unwrap()}.next = this_block.next;
                } else {
                    *guard = this_block.next;
                }
                //kprint!("gwa! 0x{:x}\n", (this_block as *mut Block) as usize);
                return Some(unsafe { transmute::<_, usize>(this_block) } + aligned_size!(Block));
            }
            previous = Some(r);
            r = this_block.next;

        }

        None
    }

    pub fn iter(&self) -> ArenaIter {
        ArenaIter {
            next: self,
            phantom: PhantomData
        }
    }
}


impl Block {
    pub fn create<'a>(addr: usize) -> &'a mut Block {
        unsafe {
            &mut *(addr as *mut Block)
        }
    }


    pub fn shrink_to_fit(&mut self, target: usize) {
        if self.length < aligned_size!(Block) + MIN_BLOCK_SIZE + target {
            return;
        } else {
            let offset = aligned_size!(Block) + target;
            let base_addr: usize = unsafe {transmute_copy::<&mut Block, _>(&self)};
            let new_block: &mut Block = Block::create(base_addr + offset);
            pointer_sanity!(new_block);
            new_block.next = self.next;
            new_block.length = self.length - aligned_size!(Block) - target;

            assert!(new_block.length < 4096);
            new_block.free = true;
            new_block.arena = self.arena;

            new_block.magic = BLOCK_MAGIC;
            self.next = new_block;


            unsafe {
                assert!(transmute_copy::<_, usize>(&new_block) - transmute::<_, usize>(self.arena) < 4096
                    && transmute_copy::<_, usize>(&new_block) > transmute::<_, usize>(self.arena));
            }
            self.length = target; // important
            return;
        }
    }

    pub fn free(&mut self) {
        assert!(!self.arena.is_null());
        assert!(self.magic == BLOCK_MAGIC);
        unsafe {
            assert!(transmute_copy::<_, usize>(&self) - transmute::<_, usize>(self.arena) < 4096
                && transmute_copy::<_, usize>(&self) > transmute::<_, usize>(self.arena));
        }
        // reinsert itself into arena
        let mut guard: MutexGuard<*mut Block> = unsafe { self.arena.as_mut().unwrap().blocks.lock() };

        let mut r: *mut Block = *guard;
        let mut previous: Option<*mut Block> = None;
        while let Some(this_block) = unsafe { r.as_mut() } {
            pointer_sanity!(this_block);
            assert!(this_block.magic == BLOCK_MAGIC);
            assert!(this_block.free == true);
            //kprint!("free\n");
            if !Block::pointer_comparison(this_block, self)
                || this_block.next == ptr::null_mut() {
                // found insertion point
                self.next = this_block;
                match previous {
                    // insert at the head

                    None => {
                        *guard = self;
                    },
                    Some(prev) => {
                        unsafe { prev.as_mut().unwrap().next = self; }
                    }
                }
                self.free = true;
                //self.try_merge();
                return;
            }


            previous = Some(r);
            r = this_block.next;
        }


        self.next = ptr::null_mut();
        match previous {
            // insert at the end


            None => {
                *guard = self;
            },
            Some(prev) => {
                unsafe { prev.as_mut().unwrap().next = self; }
            }
        }
        self.free = true;
    }

    ///
    /// try merge this block with the next
    /// if they are adjacent in memory
    ///
    fn try_merge(&mut self) {
        if self.next.is_null() {
            return;
        }

        if self.next as usize != self as *const Self as usize + self.length + aligned_size!(Block) {
            return;
        }

        //kprint!("malloc: merging!\n");

        let next_ref: &Block = unsafe { self.next.as_ref().unwrap() };
        assert!(next_ref.magic == BLOCK_MAGIC);
        assert!(next_ref.free == true);

        self.next = next_ref.next;
        self.length += aligned_size!(Block) + next_ref.length;
    }

    // pointer less-than
    fn pointer_comparison<T>(p1: *const T, p2: *const T) -> bool {
        unsafe {
            let i1: usize = transmute(p1);
            let i2: usize = transmute(p2);
            return i1 < i2;
        }
    }
}
