use x86::shared::io::*;
use core::iter::Iterator;
use collections::vec::Vec;

lazy_static! {
    pub static ref PCI_DEVICES: Vec<PCIDevice> = pci_init();
}


pub fn pci_init() -> Vec<PCIDevice> {
    let mut ret = Vec::new();
    for device in PCIIter::scan() {
        ret.push(device);
    }
    ret
}

#[derive(Debug, Copy, Clone)]
pub struct PCIDevice {
    pub bus: u16,
    pub device: u16,
    device_id: u16,
    vendor_id: u16,
    pub class: u8,
    pub subclass: u8,
}

pub struct PCIIter {
    bus: u16,
    device: u16
}

impl Iterator for PCIIter {
    type Item = PCIDevice;
    fn next(&mut self) -> Option<PCIDevice> {
        loop {
            let ret = check_device(self.bus, self.device);
            self.device = if self.device < 31 {
                self.device + 1
            } else {
                if self.bus == 255 {
                    return None;
                }
                self.bus += 1;
                0
            };
            if ret.is_none() {
                continue;
            } else {
                return ret;
            }
        }
    }
}

impl PCIIter {
    fn scan() -> PCIIter {
        PCIIter {
            bus: 0,
            device: 0
        }
    }
}

fn check_device(bus: u16, device: u16) -> Option<PCIDevice> {
    let vendor_id = pci_read32(bus, device, 0, 0) & 0xFFFF;
    if vendor_id == 0xFFFF {
        None
    } else {
        Some(PCIDevice {
            bus: bus,
            device: device,
            device_id: (pci_read32(bus, device, 0, 0) >> 16) as u16,
            vendor_id: vendor_id as u16,
            class: (pci_read32(bus, device, 0, 8) >> 24) as u8,
            subclass: ((pci_read32(bus, device, 0, 8) >> 16) & 0xF) as u8
        })
    }
}

fn convert_address(bus: u16, device: u16, func: u16, offset: u16) -> u32 {
    0x80000000 | (bus as u32) << 16 | (device as u32) << 11 | (func as u32) << 8 | offset as u32
}

const PCI_ADDRESS: u16 = 0xCF8;
const PCI_DATA: u16 = 0xCFC;

pub fn pci_read32(bus: u16, device: u16, func: u16, offset: u16) -> u32 {
    unsafe {
        outl(PCI_ADDRESS, convert_address(bus, device, func, offset));
        inl(PCI_DATA)
    }
}

pub fn pci_write32(data: u32, bus: u16, device: u16, func: u16, offset: u16) {
    unsafe {
        outl(PCI_ADDRESS, convert_address(bus, device, func, offset));
        outl(PCI_DATA, data);
    }
}
