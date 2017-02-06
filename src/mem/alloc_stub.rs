use super::heap_allocator::HEAP;
use core::intrinsics::transmute;
use core::mem::size_of;
use core::ptr::null_mut;
use rlibc::memmove;

#[no_mangle]
pub extern fn __rust_allocate(size: usize, _align: usize) -> *mut u8 {
    unsafe {transmute(HEAP.allocate(size))}
}


#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, _old_size: usize, _align: usize) {

    HEAP.deallocate(ptr, _old_size)
}

/*
#[no_mangle]
pub extern fn malloc (size: usize) -> *mut u8 {
    __rust_allocate(size, 8)
}*/

#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, _old_size: usize, size: usize,
                                _align: usize) -> *mut u8 {
    let new_addr = __rust_allocate(size, _align);
    if new_addr.is_null() {
        return null_mut();
    }
    unsafe {memmove(new_addr, ptr, _old_size);}
    new_addr

}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, old_size: usize,
                                        _size: usize, _align: usize) -> usize {
    old_size // this api is not supported by libc
}

#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}