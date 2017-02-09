use collections::string::String;
use collections::vec::Vec;
use alloc::boxed::Box;
use devices::ahci::HBA_CONTROLLER;
use devices::ahci::HBAPort;

lazy_static! {
    pub static ref BLOCK_DEVICES: Vec<Box<BlockDevice + Sync>> = block_init();
}

pub trait BlockDevice {
    fn identify(&self) -> String;

    fn read_block_raw(&self, buf: *mut u8, index: usize);

    fn write_block_raw(&self, buf: *mut u8, index: usize);
}

pub fn block_init() -> Vec<Box<BlockDevice + Sync>> {
    let mut ret: Vec<Box<BlockDevice + Sync>> = Vec::new();
    if HBA_CONTROLLER.is_some() {
        let mut ports: Vec<HBAPort> = HBA_CONTROLLER.as_ref().unwrap().test_ports();
        for p in ports.into_iter() {
            p.identify();
            ret.push(box p);
        }
    }
    ret
}