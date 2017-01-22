use core::slice;
use spin::Mutex;


pub struct Bitmap<'a> (&'a mut [u8]);

impl<'a> Bitmap<'a> {
    pub fn new<'b>(addr: usize, cap: usize) -> Bitmap<'b> {
        unsafe { Bitmap(slice::from_raw_parts_mut(addr as *mut _, cap / 8)) }
    }

    pub fn get(&self, bit_index: usize) -> bool {
        let byte = self.get_byte(Bitmap::cal_byte_index(bit_index));
        let off = Bitmap::cal_bit_offset(bit_index);
        ((*byte >> off) & 0b1 == 1)
    }

    

    #[inline]
    fn get_byte_mut(&mut self, index: usize) -> &mut u8 {
        &mut self.0[index]
    }

    #[inline]
    fn get_byte(&self, index: usize) -> &u8 {
        &self.0[index]
    }

    #[inline]
    fn cal_byte_index(bit_index: usize) -> usize {
        bit_index / 8
    }

    #[inline]
    fn cal_bit_offset(bit_index: usize) -> usize {
        bit_index % 8
    }
}

pub fn test_bitmap() {
    let b = Bitmap::new(0x600000, 0x1000000);
}