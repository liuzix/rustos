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

pub struct HeapAllocator {
    arenas: AtomicPtr<Arena>
}

struct Arena {
    next: AtomicPtr<Arena>,
    blocks: Mutex<*mut Block>
}


pub struct Block {
    length: usize,
    free: bool,
    next: *mut Block,
    arena: *mut Arena
}

const MIN_BLOCK_SIZE: usize = 8;
const MAX_BLOCK_SIZE: usize = 3072;

impl HeapAllocator {
    pub fn allocate(&self, len: usize) -> usize {
        if let Some(first) = unsafe {self.arenas.load(Ordering::Relaxed).as_mut()} {
            for current in first.iter() {
                match current.allocate(len) {
                    Some(x) => return x,
                    None => continue,
                }
            }
        } else {
            self.arenas.store(Arena::new(), Ordering::SeqCst);
            return self.allocate(len);
        }

        let last = unsafe {self.arenas.load(Ordering::Relaxed).as_mut().unwrap().iter()}.last().unwrap();
        let new_arena = Arena::new();
        last.next.store(new_arena, Ordering::SeqCst);
        return new_arena.allocate(len).unwrap();
        //last.allocate(len).unwrap()
    }

    pub fn deallocate(&self, ptr: *mut u8, len: usize) {
        if len >= 3072 { // is the a huge block?

        }
        let block: &mut Block = unsafe {
                transmute::<_,*mut Block> (ptr.offset(-1))
                    .as_mut().unwrap()
        };
    }
}

struct ArenaIter<'a> {
    next: *mut Arena,
    phantom: PhantomData<&'a mut Arena>,
}
impl<'a> Iterator for ArenaIter<'a>{
    type Item = &'a mut Arena;

    fn next(&mut self) -> Option<&'a mut Arena> {
        let res = self.next;
        self.next = unsafe {(*self.next).next.load(Ordering::Relaxed)};
        if !res.is_null() {
            Some(unsafe {res.as_mut().unwrap()})
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

        let new_block = Block::create(addr);
        new_block.length = 4096 - size_of::<Arena>();
        new_block.free = true;
        new_block.next = ptr::null_mut();
        new_block.arena = new_arena;
        new_arena.blocks = Mutex::new(new_block);
        new_arena
    }

    pub fn allocate(&self, len: usize) -> Option<usize> {
        //kprint!("{:x}\n", unsafe {transmute::<_, usize>(&self.blocks)});
        let mut guard: MutexGuard<*mut Block> = self.blocks.lock();
        //kprint!("gwa!\n");
        let mut r: *mut Block = *guard;
        let mut previous: Option<*mut Block> = None;
        while let Some(this_block) = unsafe {r.as_mut()} {

            assert!(this_block.free == true);
            if this_block.length >= len {
                this_block.free = false;
                if let Some(previous) = previous {
                    unsafe {previous.as_mut().unwrap()}.next = this_block.next;
                } else {
                    *guard = this_block.next;
                }
                this_block.shrink_to_fit(len);
                return Some(unsafe { transmute::<_, usize>(this_block) } + size_of::<Block>());
            }
            previous = Some(r);
            r = this_block.next;

        }

        None
    }

    pub fn iter(&mut self) -> ArenaIter {
        ArenaIter {
            next: self,
            phantom: PhantomData
        }
    }
}


impl Block {
    pub fn create<'a>(addr: usize) -> &'a mut Block {
        unsafe {
            &mut *(addr as *mut Block).offset(1)
        }
    }


    pub fn shrink_to_fit(&mut self, target: usize) {
        if self.length < size_of::<Block>() + MIN_BLOCK_SIZE + target {
            return;
        } else {

            let offset = size_of::<Block>() + target;
            let base_addr: usize = unsafe {transmute_copy::<&mut Block, _>(&self)};
            let new_block: &mut Block = Block::create(base_addr + offset);
            new_block.next = self.next;
            new_block.length = self.length - size_of::<Block>() - target;
            new_block.free = true;
            new_block.arena = self.arena;
            self.next = new_block;
            return;
        }
    }

    pub fn free(&mut self) {

    }
}