use super::pci;
use super::mmio::MMIO;
use super::pci::PCI_DEVICES;
use mem::paging;
use collections::vec::Vec;
use collections::String;
use fs::block::BlockDevice;
use alloc::boxed::Box;
use core::default::Default;
use core::ptr::Unique;
use core::mem::{transmute, transmute_copy};
use mem::FRAME;
use core::str;
use core::slice;
use core::intrinsics::*;
use core::sync::atomic::*;

lazy_static! {
    pub static ref HBA_CONTROLLER: Option<HBAController> = global_init_ahci();
}

macro_rules! init_array (
    ($ty:ty, $len:expr, $val:expr) => (
        {
            let mut array: [$ty; $len] = unsafe { ::core::mem::uninitialized() };
            for i in array.iter_mut() {
                unsafe { ::core::ptr::write(i, $val); }
            }
            array
        }
    )
);

fn global_init_ahci() -> Option<HBAController> {
    for dev in PCI_DEVICES.iter() {
        if dev.class == 1 && dev.subclass == 6 {
            return Some(init_ahci_controller(*dev));
        }
    }
    return None;
}

pub fn global_HBA_status() {
    if HBA_CONTROLLER.is_some() {
        kprint!("Found HBA controller.\n");
        HBA_CONTROLLER.as_ref().unwrap().test_ports();
    } else {
        kprint!("No HBA controller found\n");
    }
}

pub struct HBAController {
    device: pci::PCIDevice,
    reg_base: usize,
    CAP: MMIO<u32>,
    GHC: MMIO<u32>,
    IS: MMIO<u32>,
    PI: MMIO<u32>,
}

pub struct HBAPort {
    controller: &'static HBAController,
    reg_base: usize,
    PxCLB: MMIO<u64>,
    PxFB: MMIO<u64>,
    PxIS: MMIO<u32>,
    PxIE: MMIO<u32>,
    PxCMD: MMIO<u32>,
    PxSACT: MMIO<u32>,
    PxCI: MMIO<u32>,
    PxSSTS: MMIO<u32>,
    PxTFD: MMIO<u32>,
    command_list: Unique<[CommandHeader; 32]>,
    received_fis: Unique<[u8; 256]>,
    slot_free_map: [AtomicBool; 32]
}

#[repr(C, packed)]
struct CommandHeader {
    flags: u16,
    PRDTL: u16,
    PRDBC: u32,
    CTBA: Unique<CommandTable>,
    pad: [u32; 4]
}

impl Default for CommandHeader {
    fn default() -> Self {
        CommandHeader {
            flags: 5,
            // command header length = 5
            PRDTL: 1,
            // one physical region descriptor
            PRDBC: 511,
            // transfers 512 bytes (one block) by default
            //CTBA: unsafe {Unique::new(Box::into_raw(box CommandTable::default()))},
            CTBA: unsafe {
                let p = FRAME.alloc() as *mut CommandTable;
                *p = CommandTable::default();
                Unique::new(p)
            },
            pad: [0; 4]
        }
    }
}

#[repr(C, packed)]
#[derive(Default)]
struct DMACommand {
    fis_type: u8,
    flags: u8,
    command: u8,
    feature: u8,

    lba_low_low: u16,
    lba_low_high: u8,
    device: u8,

    lba_high_low: u16,
    lba_high_high: u8,
    feature_high: u8,

    count: u16,
    icc: u8,
    control: u8,

    pad: u32
}

#[repr(C, packed)]
struct CommandTable {
    CFIS: DMACommand,
    pad: [u8; 108],
    database_address: u64,
    pad2: u32,
    byte_count: u32
}

impl Default for CommandTable {
    fn default() -> Self {
        CommandTable {
            CFIS: DMACommand::default(),
            pad: [0; 108],
            database_address: FRAME.alloc() as u64,
            pad2: 0,
            byte_count: 511 | 1 << 31
        }
    }
}


const AHCI_BA_OFFSET: u16 = 0x24;

fn init_ahci_controller(dev: pci::PCIDevice) -> HBAController {
    let base_address: usize = pci::pci_read32(dev.bus, dev.device, 0, AHCI_BA_OFFSET) as usize;
    kprint!("ahci base address 0x{:x}\n", base_address);
    paging::map_volatile(base_address);

    let ret = HBAController {
        device: dev,
        reg_base: base_address,
        CAP: MMIO::new(base_address as *mut u32),
        GHC: MMIO::new((base_address + 0x4) as *mut u32),
        IS: MMIO::new((base_address + 0x8) as *mut u32),
        PI: MMIO::new((base_address + 0xC) as *mut u32),
    };

    kprint!("ahci cap: 0x{:x}\n", ret.CAP.get());
    //kprint!("ahci ports: 0x{:x}\n", ret.PI.get());


    ret
}

impl HBAController {
    pub fn test_ports(&'static self) -> Vec<HBAPort> {
        let mut vec = Vec::new();
        let port_bits = self.PI.get();
        for x in 0..32 {
            if (port_bits >> x) & 1 == 1 {
                match HBAPort::new(self, x) {
                    Some(port) => {
                        vec.push(port)
                    },
                    None => {}
                }
            }
        }
        vec
    }
}

impl HBAPort {
    fn new(controller: &'static HBAController, num: usize) -> Option<HBAPort> {
        let base_address = controller.reg_base + 0x100 + num * 0x80;
        let SSTS = MMIO::new((base_address + 0x28) as *mut u32);
        if SSTS.get() & 0b1111 != 3 {
            return None;
        }

        let headers: &mut [CommandHeader; 32] = unsafe { (FRAME.alloc() as *mut _).as_mut().unwrap() };
        for x in 0..32 {
            headers[x] = CommandHeader::default();
        }

        let ret = HBAPort {
            controller: controller,
            reg_base: base_address,
            PxCLB: MMIO::new(base_address as *mut u64),
            PxFB: MMIO::new((base_address + 0x8) as *mut u64),
            PxIS: MMIO::new((base_address + 0x10) as *mut u32),
            PxIE: MMIO::new((base_address + 0x14) as *mut u32),
            PxCMD: MMIO::new((base_address + 0x18) as *mut u32),
            PxTFD: MMIO::new((base_address + 0x20) as *mut u32),
            PxSACT: MMIO::new((base_address + 0x34) as *mut u32),
            PxCI: MMIO::new((base_address + 0x38) as *mut u32),
            PxSSTS: MMIO::new((base_address + 0x28) as *mut u32),
            command_list: unsafe { Unique::new(headers) },
            received_fis: unsafe { Unique::new(FRAME.alloc() as *mut _) },
            slot_free_map: init_array!(AtomicBool, 32, AtomicBool::new(false))
        };


        kprint!("port {} is connected\n", num);
        ret.PxCMD.set(ret.PxCMD.get() & !1);
        ret.PxCMD.set(ret.PxCMD.get() & !(1 << 4)); // stop device

        unsafe {
            ret.PxCLB.set(ret.command_list.get().as_ptr() as u64);
            ret.PxFB.set(ret.received_fis.get() as *const _ as u64);
        }
        ret.PxCMD.set(ret.PxCMD.get() | 1 << 4); // start device
        ret.PxCMD.set(ret.PxCMD.get() | 1);
        Some(ret)
    }

    fn get_header(&self, i: usize) -> &mut CommandHeader {
        unsafe {
            transmute::<_, &mut _>(&self.command_list.get()[i])
        }
    }

    fn get_table(&self, i: usize) -> &mut CommandTable {
        unsafe {
            transmute::<_, &mut _>(self.get_header(i).CTBA.get())
        }
    }

    fn get_buf(&self, i: usize) -> *mut u8 {
        self.get_table(i).database_address as *mut u8
    }

    fn get_free_slot(&self) -> usize {
        loop {
            for i in 0..32 {
                if self.slot_free_map[i].load(Ordering::SeqCst) == false {
                    if self.slot_free_map[i].swap(true, Ordering::Acquire) == true {
                        continue;
                    } else {
                        return i;
                    }
                }
            }
        }
    }

    fn release_slot(&self, i: usize) {
        self.slot_free_map[i].store(false, Ordering::Release);
    }

    fn spin_on_slot(&self, i: usize) {
        loop {
            if self.PxCI.get() & (1 << i) == 0 {
                break;
            }
        }
    }

    fn spin_on_tag(&self, i: usize) {
        loop {
            if self.PxSACT.get() & (1 << i) == 0 {
                break;
            }
        }
    }

    fn wait_busy(&self) {
        loop {
            if self.PxTFD.get() & (1 << 7 | 1 << 3) == 0 {
                break;
            }
        }
    }

    fn dma_fallback(&self, block_num: usize, write: bool, slot: usize) {
        let fis = &mut self.get_table(slot).CFIS;
        fis.fis_type = 0x27;
        fis.flags = 1 << 7;
        fis.command = if !write { 0x25 } else { 0x35 };
        fis.feature = 0;
        fis.feature_high = 0;
        fis.count = 1;
        self.get_header(slot).flags = if write {
            self.get_header(slot).flags | 1 << 6
        } else {
            self.get_header(slot).flags & !(1 << 6)
        };


        use bit_field::BitField;
        fis.lba_low_low = block_num.get_bits(0..16) as u16;
        fis.lba_low_high = block_num.get_bits(16..24) as u8;
        fis.lba_high_low = block_num.get_bits(24..40) as u16;
        fis.lba_high_high = block_num.get_bits(40..48) as u8;
        fis.device = 1 << 6;

        //self.PxSACT.set(1 << slot);
        self.wait_busy();
        kprint!("PxTFD = 0x{:x}\n", self.PxTFD.get());
        self.PxCI.set(1 << slot);

        unsafe {
            //::devices::apic::micro_delay(10);
        }

        self.spin_on_slot(slot);
        kprint!("res: {:x}\n", self.PxIS.get());
        //self.PxIS.set(!0);
    }

    fn dma_transaction(&self, block_num: usize, write: bool, slot: usize) {
        let fis = &mut self.get_table(slot).CFIS;
        fis.fis_type = 0x27;
        fis.flags = 1 << 7;
        fis.command = if !write { 0x60 } else { 0x61 };
        fis.feature = 1;
        fis.feature_high = 0;
        fis.count = (slot as u16) << 3;
        self.get_header(slot).flags = if write {
            self.get_header(slot).flags | 1 << 6
        } else {
            self.get_header(slot).flags & !(1 << 6)
        };

        use bit_field::BitField;
        fis.lba_low_low = block_num.get_bits(0..16) as u16;
        fis.lba_low_high = block_num.get_bits(16..24) as u8;
        fis.lba_high_low = block_num.get_bits(24..40) as u16;
        fis.lba_high_high = block_num.get_bits(40..48) as u8;
        fis.device = 1 << 6;
        self.PxIS.set(!0);
        self.PxIE.set(!0);
        self.PxSACT.set(1 << slot);
        self.wait_busy();
        self.PxCI.set(1 << slot);
        self.spin_on_slot(slot);
        self.spin_on_tag(slot);
        //self.PxIS.set(!0);
    }
}

impl BlockDevice for HBAPort {
    fn identify(&self) -> String {
        let slot = self.get_free_slot();
        let fis = &mut self.get_table(slot).CFIS;
        fis.fis_type = 0x27;
        fis.flags = 1 << 7;
        fis.command = 0xEC;
        fis.device = 0;
        let res = self.PxIS.set(!0);
        self.PxCI.set(1 << slot);

        self.spin_on_slot(slot);
        let res = self.PxIS.get();

        kprint!("res: {:x}\n", res);


        let model = unsafe {
            str::from_utf8(slice::from_raw_parts(self.get_buf(0).offset(20), 20)).unwrap()
        };

        kprint!("model: {}\n", model);
        self.PxIS.set(!0);

        unsafe {
            let test_1: &mut usize = (FRAME.alloc() as *mut usize).as_mut().unwrap();
            *test_1 = 0x2333333;
            self.write_block_raw(transmute_copy(&test_1), 20);


            *test_1 = 0x0;
            self.read_block_raw(transmute_copy(&test_1), 20);
            kprint!("{:x}\n", *test_1);
        }

        return String::from(model);
    }

    fn read_block_raw(&self, buf: *mut u8, index: usize) {
        let slot = self.get_free_slot();
        unsafe { ::rlibc::memset(self.get_buf(slot), 0, 512) };
        self.dma_transaction(index, false, slot);
        unsafe {
            ::rlibc::memmove(buf, self.get_buf(slot), 512);
        }
        self.release_slot(slot);
    }

    fn write_block_raw(&self, buf: *mut u8, index: usize) {
        let slot = self.get_free_slot();
        unsafe {
            ::rlibc::memmove(self.get_buf(slot), buf, 512);
        }
        self.dma_transaction(index, true, slot);
        self.release_slot(slot);
    }
}

