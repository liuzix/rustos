use core::slice;
use core::intrinsics;
use core::mem;
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

    pub fn set(&mut self, bit_index: usize, val: bool) {
        let off = Bitmap::cal_bit_offset(bit_index);
        loop {
            let byte: &u8 = self.get_byte(Bitmap::cal_byte_index(bit_index));
            let mut byte_temp: u8 = *byte;
            let old_byte: u8 = byte_temp;
            let mask = !(0b1 << off);
            byte_temp &= mask;
            byte_temp |= (val as u8) << off;
            let (_, success) = unsafe {
                intrinsics::atomic_cxchg(mem::transmute(byte), old_byte, byte_temp)
            };
            if success {
                return;
            }
        }
    }

    pub fn set_first_unused(&self) -> usize {
        let mut i: usize = 0;
        loop {
            let byte = self.0[i];
            if byte != !0u8 {
                let pos = (!byte).trailing_zeros();
                let new_byte = byte | (0b1 << pos);
                let (_, success) = unsafe {
                    intrinsics::atomic_cxchg(mem::transmute::<&u8, *mut u8>(&self.0[i]), byte, new_byte)
                };
                if !success {
                    continue;
                } else {
                    return i * 8 + pos as usize;
                }
            }
            i += 1;
        }
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
    let mut b = Bitmap::new(0x600000, 0x1000000);
    for i in 0..1000 {
        b.set(i * 9, true);
    }

    for i in 0..9000 {
        let r = b.get(i);

        if i % 9 == 0 {
            assert!(r);
        } else {
            assert!(!r);
        }

    }

    for _  in 0..100 {
        b.set_first_unused();
    }
    kprint!("bitmap test successful\n");
}