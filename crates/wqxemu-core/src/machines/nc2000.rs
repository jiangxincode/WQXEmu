// NC2000 hardware machine implementation.
//
// The NC2000 boots from NOR Flash plus NAND Flash (no big ROM dump).
// Banks 0x00-0x0F select NOR pages, banks 0x80+ select extended RAM, and
// the fixed BIOS page at 0xE000-0xFFFF is NOR bank 0. The IO subsystem
// follows the SPDC1016 register model (timers, ports, LCD address, RTC,
// DSP, NAND controller).

use anyhow::{Context, Result};

use crate::audio::Audio;
use crate::cpu::{Cpu, CpuBus};
use crate::lcd::Lcd;
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::save::SaveState;

/// NOR page count (16 x 32KB = 512KB).
const NUM_NOR_PAGES: usize = 0x10;
/// NAND main area page count (65536 pages x 528 bytes).
const NUM_NAND_PAGES: usize = 65536;
/// NAND0 first-plane page count.
const NAND0_PAGES: usize = 64;
/// NAND page size (512 data + 16 spare).
const NAND_PAGE_SIZE: usize = 528;
/// NOR page size (32KB).
const NOR_PAGE_SIZE: usize = 0x8000;
/// Internal RAM size (24K) used by the 0x2000 window.
const INTERNAL_RAM_SIZE: usize = 0x6000;

/// IO register indexes used by the SPDC1016 model.
#[allow(dead_code)]
mod io_reg {
    pub const BANK_SWITCH: usize = 0x00;
    pub const INT_STATUS: usize = 0x01;
    pub const TIMER0_VAL: usize = 0x02;
    pub const TIMER1_VAL: usize = 0x03;
    pub const GENERAL_CTRL: usize = 0x04;
    pub const CLOCK_CTRL: usize = 0x05;
    pub const LCD_CONFIG: usize = 0x06;
    pub const PORT_CONFIG: usize = 0x07;
    pub const PORT0: usize = 0x08;
    pub const PORT1: usize = 0x09;
    pub const BIOS_BSW: usize = 0x0A;
    pub const PORT3: usize = 0x0B;
    pub const LCD_TIMER_CTRL: usize = 0x0C;
    pub const VOLUME: usize = 0x0D;
    pub const ZP_BSW: usize = 0x0F;
    pub const TIMERA_VAL_L: usize = 0x10;
    pub const TIMERA_VAL_H: usize = 0x11;
    pub const TIMERB_VAL_L: usize = 0x12;
    pub const TIMERB_VAL_H: usize = 0x13;
    pub const TIMERAB_CTRL: usize = 0x14;
    pub const PORT1_DIR: usize = 0x15;
    pub const PORT2: usize = 0x17;
    pub const PORT4: usize = 0x18;
    pub const CTV_SELECT: usize = 0x19;
    pub const VOLUME_SET: usize = 0x1A;
    pub const BATTERY: usize = 0x1C;
    pub const NAND_DATA: usize = 0x29;
    pub const DSP_STAT: usize = 0x30;
    pub const DSP_RET_DATA: usize = 0x31;
    pub const DSP_DATA_LOW: usize = 0x32;
    pub const DSP_CMD: usize = 0x33;
    pub const RTC_3A: usize = 0x3A;
    pub const RTC_3B: usize = 0x3B;
    pub const RTC_3C: usize = 0x3C;
    pub const RTC_3D: usize = 0x3D;
    pub const RTC_IDX: usize = 0x3E;
    pub const RTC_DATA: usize = 0x3F;
}

/// Extended register indexes (accessed via 0x3E/0x3F).
#[allow(dead_code)]
mod ext_reg {
    pub const TR_S: usize = 0x00;
    pub const TR_M: usize = 0x01;
    pub const TR_H: usize = 0x02;
    pub const TR_D: usize = 0x03;
    pub const TR_MS: usize = 0x04;
    pub const AR_S: usize = 0x05;
    pub const AR_M: usize = 0x06;
    pub const AR_H: usize = 0x07;
    pub const RTC_CTRL: usize = 0x0A;
    pub const INT_CLEAR: usize = 0x0B;
    pub const P0_PU: usize = 0x24;
}

/// NOR command state machine states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NorCmd {
    None,
    SwId,
    ByteProgram,
    BlockOrMassErase,
    InfoByteProgram,
    InfoOrBmassErase,
    InfoRead,
    PollStatus,
}

/// NC2000 machine.
pub struct Nc2000Machine {
    io: [u8; 0x40],
    ext_reg: [u8; 256],
    ram: Vec<u8>,
    ext_ram: Vec<u8>,
    ram_b: [u8; 0x2000],
    ram_b2: [u8; 0x2000],
    nor: Vec<u8>,
    nand: Vec<u8>,
    nor_info_block: [u8; 0x100],

    // NOR state
    fp_step: u8,
    fp_type: NorCmd,

    // NAND state
    nand_cmd: Vec<u8>,
    nand_addr: Vec<u8>,
    nand_data: Vec<u8>,
    nand_read_cnt: usize,

    // Keypad / ports
    keypad_matrix: [[bool; 8]; 8],
    w08_port0_ol: u8,
    r08_port0_id: u8,
    w09_port1_ol: u8,
    r09_port1_id: u8,
    w15_port1_dir: u8,
    w04_b46_ptype: u8,
    rw0f_b4_dir00: bool,
    rw0f_b5_dir01: bool,
    rw0f_b6_dir023: bool,
    rw0f_b7_dir047: bool,

    // Timers
    timer0run: bool,
    timer1run: bool,
    timer0ticks: u32,
    timer1ticks: u32,
    w0c_b67_tmodesl: u8,
    w0c_b45_tm0s: u8,
    w0c_b23_tm1s: u8,
    w0c_b345_tms: u8,
    tma_value: u32,
    tma_reload: u32,
    inner_interrupt_control: u8,

    // DSP
    dsp_ret_data: i32,
    dsp_sleep: bool,
    dsp_trans: bool,
    dsp_data_low: u8,
    dsp_0xd0: bool,
    dsp_0x7001_0x7002: bool,
    dsp_data_feeded: bool,

    // LCD
    lcd: Lcd,
    lcd_buff_addr: u16,
    lcd_buff_addr_mask: u16,

    // RTC / interrupt vectors
    rtc_256_counter: u8,
    iv_queue: Vec<u8>,
    tb_cycles: u64,
    rtc_cycles: u64,
    do_warm_reset: bool,

    audio: Audio,
    cycles: u64,
}

impl Nc2000Machine {
    /// Create the machine and load its NOR / NAND files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            io: [0; 0x40],
            ext_reg: [0; 256],
            ram: vec![0; 0x10000],
            ext_ram: vec![0; 0x8000],
            ram_b: [0; 0x2000],
            ram_b2: [0; 0x2000],
            nor: vec![0xFF; NUM_NOR_PAGES * NOR_PAGE_SIZE],
            nand: vec![0xFF; (NAND0_PAGES + NUM_NAND_PAGES) * NAND_PAGE_SIZE],
            nor_info_block: [0; 0x100],
            fp_step: 0,
            fp_type: NorCmd::None,
            nand_cmd: Vec::new(),
            nand_addr: Vec::new(),
            nand_data: Vec::new(),
            nand_read_cnt: 0,
            keypad_matrix: [[false; 8]; 8],
            w08_port0_ol: 0,
            r08_port0_id: 0,
            w09_port1_ol: 0,
            r09_port1_id: 0,
            w15_port1_dir: 0,
            w04_b46_ptype: 0,
            rw0f_b4_dir00: false,
            rw0f_b5_dir01: false,
            rw0f_b6_dir023: false,
            rw0f_b7_dir047: false,
            timer0run: false,
            timer1run: false,
            timer0ticks: 0,
            timer1ticks: 0,
            w0c_b67_tmodesl: 0,
            w0c_b45_tm0s: 0,
            w0c_b23_tm1s: 0,
            w0c_b345_tms: 0,
            tma_value: 0,
            tma_reload: 0,
            inner_interrupt_control: 0,
            dsp_ret_data: -1,
            dsp_sleep: false,
            dsp_trans: false,
            dsp_data_low: 0,
            dsp_0xd0: false,
            dsp_0x7001_0x7002: false,
            dsp_data_feeded: false,
            lcd: Lcd::new(),
            lcd_buff_addr: 0,
            lcd_buff_addr_mask: 0x3FFF,
            rtc_256_counter: 0,
            iv_queue: Vec::new(),
            tb_cycles: 0,
            rtc_cycles: 0,
            do_warm_reset: false,
            audio: Audio::new(),
            cycles: 0,
        };

        machine.load_rom(files)?;
        Ok(machine)
    }

    fn load_rom_impl(&mut self, files: &RomFiles) -> Result<()> {
        // NOR is loaded in physical order (no bank swap) for NC2000.
        if let Some(nor_path) = &files.nor {
            let data = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read NC2000 NOR: {}", nor_path.display()))?;
            if data.len() < self.nor.len() {
                anyhow::bail!(
                    "NOR file too small: expected at least {} bytes, got {}",
                    self.nor.len(),
                    data.len()
                );
            }
            let len = self.nor.len();
            self.nor[..len].copy_from_slice(&data[..len]);
        }
        if let Some(nand_path) = &files.nand {
            let data = std::fs::read(nand_path)
                .with_context(|| format!("Failed to read NC2000 NAND: {}", nand_path.display()))?;
            self.load_main_nand(&data);
        }
        if let Some(nand0_path) = &files.nand0 {
            let data = std::fs::read(nand0_path).with_context(|| {
                format!("Failed to read NC2000 NAND0: {}", nand0_path.display())
            })?;
            self.load_first_nand_plane(&data);
        }
        Ok(())
    }

    fn load_main_nand(&mut self, data: &[u8]) {
        let start = NAND0_PAGES * NAND_PAGE_SIZE;
        let n = (self.nand.len() - start).min(data.len());
        self.nand[start..start + n].copy_from_slice(&data[..n]);
    }

    fn load_first_nand_plane(&mut self, data: &[u8]) {
        let n = (NAND0_PAGES * NAND_PAGE_SIZE).min(data.len());
        self.nand[..n].copy_from_slice(&data[..n]);
    }

    /// Get the current bank window base for a bank index.
    fn get_bank(&self, bank_idx: u8) -> Option<BankTarget> {
        if (bank_idx as usize) < NUM_NOR_PAGES {
            Some(BankTarget::Nor(bank_idx as usize * NOR_PAGE_SIZE))
        } else if bank_idx >= 0x80 {
            Some(BankTarget::ExtRam)
        } else {
            None
        }
    }

    /// Resolve a CPU address to a memory target (read path).
    fn map_read(&self, addr: u16) -> Option<MemTarget> {
        if (addr as usize) < 0x40 {
            return Some(MemTarget::Io(addr as usize));
        }
        if addr < 0x2000 {
            return Some(MemTarget::Ram(addr as usize));
        }
        if addr < 0x4000 {
            // 0x2000-0x3FFF: RAM page 1 or bank RAM (io[0x0D] bit 2)
            let off = (addr & 0x1FFF) as usize;
            return if self.io[io_reg::VOLUME] & 0x04 != 0 {
                Some(MemTarget::RamB(off))
            } else {
                Some(MemTarget::Ram(0x2000 + off))
            };
        }
        if addr < 0xC000 {
            // 0x4000-0xBFFF: bank window
            let bank_idx = self.io[io_reg::BANK_SWITCH];
            let bank_off = match addr {
                0x4000..=0x5FFF => 0x4000,
                0x6000..=0x7FFF => 0x6000,
                0x8000..=0x9FFF => 0x0000,
                _ => 0x2000,
            };
            let page_off = (addr & 0x1FFF) as usize;
            return self.get_bank(bank_idx).map(|bank| match bank {
                BankTarget::Nor(base) => MemTarget::Nor(base + bank_off + page_off),
                BankTarget::ExtRam => MemTarget::ExtRam(bank_off + page_off),
            });
        }
        if addr < 0xE000 {
            // 0xC000-0xDFFF: BBS page
            let bbs_idx = (self.io[io_reg::BIOS_BSW] & 0x0F) as usize;
            let page_off = (addr & 0x1FFF) as usize;
            if bbs_idx == 1 {
                return Some(MemTarget::Ram(0x4000 + page_off));
            }
            return self.bbs_page_target(bbs_idx, page_off);
        }
        // 0xE000-0xFFFF: fixed BIOS page (bbs_pages[1] = nor_banks[0]+0x6000)
        let page_off = (addr & 0x1FFF) as usize;
        Some(MemTarget::Nor(0x6000 + page_off))
    }

    /// bbs_pages[i*4+k] = nor_banks[i] + {0x4000, 0x6000, 0x0000, 0x2000}.
    fn bbs_page_target(&self, idx: usize, page_off: usize) -> Option<MemTarget> {
        let nor_page = idx / 4;
        let sub = idx % 4;
        if nor_page >= NUM_NOR_PAGES {
            return None;
        }
        let base = nor_page * NOR_PAGE_SIZE;
        let off = match sub {
            0 => 0x4000,
            1 => 0x6000,
            2 => 0x0000,
            _ => 0x2000,
        };
        Some(MemTarget::Nor(base + off + page_off))
    }

    fn peek_impl(&self, addr: u16) -> u8 {
        match self.map_read(addr) {
            Some(MemTarget::Io(i)) => self.io[i],
            Some(MemTarget::Ram(o)) => self.ram[o],
            Some(MemTarget::RamB(o)) => self.ram_b[o],
            Some(MemTarget::ExtRam(o)) => self.ext_ram[o],
            Some(MemTarget::Nor(o)) => self.nor_read(addr, o),
            None => 0,
        }
    }

    /// Execute a CPU read (with NOR command side effects).
    fn bus_read(&mut self, addr: u16) -> u8 {
        if (addr as usize) < 0x40 {
            return self.io_read(addr as u8);
        }
        match self.map_read(addr) {
            Some(MemTarget::Ram(o)) => self.ram[o],
            Some(MemTarget::RamB(o)) => self.ram_b[o],
            Some(MemTarget::ExtRam(o)) => self.ext_ram[o],
            Some(MemTarget::Nor(o)) => self.nor_read(addr, o),
            Some(MemTarget::Io(i)) => self.io[i],
            None => 0,
        }
    }

    /// NOR read with command state machine (info block, ID, status).
    fn nor_read(&self, addr: u16, offset: usize) -> u8 {
        match self.fp_type {
            NorCmd::InfoRead if self.fp_step == 3 => self.nor_info_block[offset % 0x100],
            NorCmd::SwId if self.fp_step == 3 => match addr {
                0x8000 => 0xC7,
                0x8001 => 0xD7,
                _ => 0xFF,
            },
            NorCmd::PollStatus if self.fp_step == 3 => 0x88,
            NorCmd::ByteProgram if self.fp_step == 4 => 0x88,
            NorCmd::BlockOrMassErase if self.fp_step == 6 => 0x88,
            _ => self.nor[offset],
        }
    }

    /// Execute a CPU write.
    fn bus_write(&mut self, addr: u16, value: u8) {
        if (addr as usize) < 0x40 {
            self.io_write(addr as u8, value);
            return;
        }
        match self.map_read(addr) {
            Some(MemTarget::Ram(o)) => self.ram[o] = value,
            Some(MemTarget::RamB(o)) => self.ram_b[o] = value,
            Some(MemTarget::ExtRam(o)) => self.ext_ram[o] = value,
            Some(MemTarget::Nor(o)) => self.nor_write(addr, o, value),
            Some(MemTarget::Io(_)) | None => {}
        }
    }

    /// NOR command sequence state machine.
    fn nor_write(&mut self, addr: u16, offset: usize, value: u8) {
        let bank_idx = self.io[io_reg::BANK_SWITCH];
        if (bank_idx as usize) >= NUM_NOR_PAGES {
            return;
        }

        let addr_is_5555 = addr == 0x5555 || addr == 0xD555;

        match self.fp_step {
            0 if addr_is_5555 && value == 0xAA => {
                self.fp_step = 1;
            }
            1 => {
                if addr == 0xAAAA && value == 0x55 {
                    self.fp_step = 2;
                } else {
                    self.fp_step = 0;
                }
            }
            2 if addr_is_5555 => {
                self.fp_type = match value {
                    0x90 => NorCmd::SwId,
                    0xA0 => NorCmd::ByteProgram,
                    0x80 => NorCmd::BlockOrMassErase,
                    0xA8 => NorCmd::InfoByteProgram,
                    0x88 => NorCmd::InfoOrBmassErase,
                    0x78 => NorCmd::InfoRead,
                    0x70 => NorCmd::PollStatus,
                    _ => NorCmd::None,
                };
                if self.fp_type != NorCmd::None {
                    self.fp_step = 3;
                } else {
                    self.fp_step = 0;
                }
            }
            3 => match self.fp_type {
                NorCmd::ByteProgram => {
                    self.nor[offset] &= value;
                    self.fp_step = 4;
                }
                NorCmd::InfoByteProgram => {
                    self.nor_info_block[offset & 0xFF] &= value;
                    self.fp_step = 4;
                }
                NorCmd::BlockOrMassErase | NorCmd::InfoOrBmassErase => {
                    if addr_is_5555 && value == 0xAA {
                        self.fp_step = 4;
                    }
                }
                _ => self.fp_step = 0,
            },
            4 if matches!(
                self.fp_type,
                NorCmd::BlockOrMassErase | NorCmd::InfoOrBmassErase
            ) && addr == 0xAAAA
                && value == 0x55 =>
            {
                self.fp_step = 5;
            }
            5 => {
                if addr_is_5555 && value == 0x10 {
                    // Mass erase
                    self.nor.fill(0xFF);
                    if self.fp_type == NorCmd::InfoOrBmassErase {
                        self.nor_info_block.fill(0xFF);
                    }
                    self.fp_step = 6;
                } else if self.fp_type == NorCmd::BlockOrMassErase && value == 0x30 {
                    // Block erase: 2KB aligned block
                    let block = offset & !0x7FF;
                    let end = (block + 0x800).min(self.nor.len());
                    self.nor[block..end].fill(0xFF);
                    self.fp_step = 6;
                } else if self.fp_type == NorCmd::InfoOrBmassErase && value == 0x30 {
                    self.nor_info_block.fill(0xFF);
                    self.fp_step = 6;
                }
            }
            _ => {}
        }

        if value == 0xF0 {
            self.fp_step = 0;
            self.fp_type = NorCmd::None;
        }
    }

    /// NAND command sequence state machine (write side).
    fn nand_write(&mut self, value: u8) {
        let cle = self.io[io_reg::PORT4] & 0x01 != 0;
        let ale = self.io[io_reg::PORT4] & 0x02 != 0;

        if cle && ale {
            return;
        }

        if cle {
            match value {
                0xFF => {
                    self.clear_nand_status();
                }
                0x00 | 0x01 | 0x50 | 0x60 | 0x70 | 0x90 => {
                    self.clear_nand_status();
                    self.nand_cmd.push(value);
                }
                0x10 => {
                    // Program confirm
                    self.nand_program();
                    self.clear_nand_status();
                }
                0xD0
                    // Erase confirm
                    if self.nand_cmd.len() == 1
                        && self.nand_cmd[0] == 0x60
                        && self.nand_addr.len() == 3
                    => {
                        let low = self.nand_addr[0] as u32;
                        let mid = self.nand_addr[1] as u32;
                        let high = (self.nand_addr[2] & 0x01) as u32;
                        let page = (high * 256 * 256 + mid * 256 + low) as usize;
                        let final_off = page * NAND_PAGE_SIZE;
                        if final_off + 32 * NAND_PAGE_SIZE <= self.nand.len() {
                            let end = final_off + 32 * NAND_PAGE_SIZE;
                            for b in self.nand[final_off..end].iter_mut() {
                                *b = 0xFF;
                            }
                        }
                        self.nand_read_cnt += 1;
                        self.clear_nand_status();
                    }
                0x80
                    if !self.nand_cmd.is_empty() => {
                        self.nand_cmd.push(value);
                    }
                _ => {}
            }
            return;
        }

        if ale {
            if !self.nand_cmd.is_empty() {
                self.nand_addr.push(value);
            }
            return;
        }

        // Data
        if !self.nand_cmd.is_empty() {
            self.nand_data.push(value);
        }
    }

    /// Program a page (0x80 ... data ... 0x10).
    fn nand_program(&mut self) {
        if self.nand_cmd.len() < 2 || self.nand_addr.len() != 4 {
            return;
        }
        let low = self.nand_addr[0] as usize;
        let mid = self.nand_addr[1] as usize;
        let high = self.nand_addr[2] as usize;
        let a25 = (self.nand_addr[3] & 0x01) as usize;
        let page = a25 * 256 * 256 + high * 256 + mid;

        if self.nand_cmd[0] == 0x50 {
            // Spare area program (16 bytes)
            if self.nand_data.len() == 16 {
                let final_off = page * NAND_PAGE_SIZE + low + 512;
                for (i, &b) in self.nand_data.iter().enumerate() {
                    let idx = final_off + i;
                    if idx < self.nand.len() {
                        self.nand[idx] &= b;
                    }
                }
            }
        } else if self.nand_cmd[0] == 0x00 && self.nand_data.len() == NAND_PAGE_SIZE {
            let final_off = page * NAND_PAGE_SIZE + low;
            for (i, &b) in self.nand_data.iter().enumerate() {
                let idx = final_off + i;
                if idx < self.nand.len() {
                    self.nand[idx] &= b;
                }
            }
        }
    }

    /// NAND read side.
    fn nand_read(&mut self) -> u8 {
        // Status read after a long time
        if self.nand_cmd.len() == 1 && self.nand_cmd[0] == 0x70 {
            self.clear_nand_status();
            return 0x40;
        }
        // Manufacturer / device ID
        if self.nand_cmd.len() == 1
            && self.nand_cmd[0] == 0x90
            && self.nand_addr.len() == 1
            && self.nand_addr[0] == 0x00
        {
            if self.nand_read_cnt == 0 {
                self.nand_read_cnt += 1;
                return 0xEC;
            }
            if self.nand_read_cnt == 1 {
                self.clear_nand_status();
                return 0x75;
            }
            return 0;
        }

        // Read main / spare
        if self.nand_cmd.len() == 1
            && matches!(self.nand_cmd[0], 0x00 | 0x01 | 0x50)
            && self.nand_addr.len() == 4
        {
            let low = self.nand_addr[0] as usize;
            let mid = self.nand_addr[1] as usize;
            let high = self.nand_addr[2] as usize;
            let a25 = (self.nand_addr[3] & 0x01) as usize;
            let page = a25 * 256 * 256 + high * 256 + mid;

            let mut col = low;
            if self.nand_cmd[0] == 0x01 {
                col += 256;
            } else if self.nand_cmd[0] == 0x50 {
                col += 512;
            }
            let idx = page * NAND_PAGE_SIZE + col + self.nand_read_cnt;
            self.nand_read_cnt += 1;
            return if idx < self.nand.len() {
                self.nand[idx]
            } else {
                0xFF
            };
        }

        0xFF
    }

    fn clear_nand_status(&mut self) {
        self.nand_cmd.clear();
        self.nand_addr.clear();
        self.nand_data.clear();
        self.nand_read_cnt = 0;
    }

    /// IO register read (SPDC1016 semantics).
    fn io_read(&mut self, addr: u8) -> u8 {
        match addr as usize {
            io_reg::TIMER0_VAL => {
                if addr as usize == io_reg::GENERAL_CTRL {
                    return self.io[addr as usize];
                }
                self.io[addr as usize]
            }
            0x04 => {
                // Reading 0x04 stops timer0
                self.timer0run = false;
                self.io[0x02]
            }
            0x05 => {
                // Reading 0x05 starts timer0
                self.timer0run = true;
                self.timer0ticks = 0;
                self.io[0x02]
            }
            0x06 => {
                // Reading 0x06 stops timer1
                self.timer1run = false;
                self.io[0x03]
            }
            0x07 => {
                // Reading 0x07 starts timer1
                self.timer1run = true;
                self.timer1ticks = 0;
                self.io[0x03]
            }
            io_reg::PORT0 => {
                self.update_keypad_registers();
                self.r08_port0_id
            }
            io_reg::PORT1 => {
                self.update_keypad_registers();
                self.r09_port1_id
            }
            io_reg::PORT4 => self.io[addr as usize],
            io_reg::BATTERY => {
                let level = self.io[addr as usize] & 0x1F;
                if level >= 11 {
                    self.io[addr as usize] | 0x20
                } else {
                    self.io[addr as usize] & !0x20
                }
            }
            io_reg::DSP_STAT => self.dsp_stat(),
            io_reg::DSP_RET_DATA => {
                if self.dsp_ret_data == -1 {
                    0xFF
                } else {
                    let ret = self.dsp_ret_data as u8;
                    self.dsp_ret_data = -1;
                    ret
                }
            }
            io_reg::NAND_DATA => self.nand_read_mut(),
            io_reg::RTC_3A | io_reg::RTC_3B | io_reg::RTC_3C | io_reg::RTC_3D => {
                self.read_rtc_3x(addr)
            }
            io_reg::RTC_DATA => {
                let idx = self.io[io_reg::RTC_IDX] as usize;
                if idx < 256 {
                    self.ext_reg[idx]
                } else {
                    0
                }
            }
            io_reg::INT_STATUS => {
                let t = self.io[addr as usize];
                self.io[addr as usize] &= 0xC0;
                t
            }
            _ => self.io[addr as usize],
        }
    }

    fn nand_read_mut(&mut self) -> u8 {
        if self.io[io_reg::PORT4] & 0x40 != 0 {
            return 0xFF;
        }
        self.nand_read()
    }

    fn dsp_stat(&self) -> u8 {
        let mut value = 0u8;
        if self.dsp_sleep {
            value |= 0x80;
        }
        if !self.dsp_sleep {
            // Simulate sound busy consuming data
            if self.dsp_data_feeded {
                value |= 0x30;
            } else {
                // simulate consumed
            }
        }
        if self.dsp_0xd0 || self.dsp_0x7001_0x7002 || (self.dsp_trans && self.dsp_ret_data != -1) {
            value |= 0x40;
        }
        value
    }

    fn read_rtc_3x(&self, addr: u8) -> u8 {
        match addr as usize {
            io_reg::RTC_3A => 0,
            io_reg::RTC_3B => {
                if self.ext_reg[ext_reg::RTC_CTRL] & 0x03 == 0 {
                    0
                } else {
                    self.io[addr as usize]
                }
            }
            io_reg::RTC_3C => 0,
            io_reg::RTC_3D => self.io[addr as usize],
            _ => 0,
        }
    }

    /// IO register write (SPDC1016 semantics).
    fn io_write(&mut self, addr: u8, value: u8) {
        match addr as usize {
            io_reg::BANK_SWITCH | io_reg::BIOS_BSW | io_reg::VOLUME => {
                self.io[addr as usize] = value;
            }
            io_reg::INT_STATUS => {
                self.inner_interrupt_control = value;
            }
            0x04 => {
                self.io[addr as usize] = value;
                self.w04_b46_ptype = (value >> 4) & 0x07;
            }
            0x05 => {
                self.io[addr as usize] = value;
            }
            0x06 => {
                // LCD start address bits 11..4
                let t = ((value as u16) << 4) & 0x0FF0;
                self.lcd_buff_addr = (self.lcd_buff_addr & !0x0FF0) | t;
                self.io[addr as usize] = value;
            }
            0x07 => {
                self.io[addr as usize] = value;
            }
            io_reg::PORT0 => {
                self.w08_port0_ol = value;
                // Pass-through to input data based on direction bits
                let mut mask = 0u8;
                let mut data = 0u8;
                if self.rw0f_b4_dir00 {
                    mask |= 0x01;
                    data |= value & 0x01;
                }
                if self.rw0f_b5_dir01 {
                    mask |= 0x02;
                    data |= value & 0x02;
                }
                if self.rw0f_b6_dir023 {
                    mask |= 0x0C;
                    data |= value & 0x0C;
                }
                if self.rw0f_b7_dir047 {
                    mask |= 0xF0;
                    data |= value & 0xF0;
                }
                self.r08_port0_id = (self.r08_port0_id & !mask) | data;
                self.update_keypad_registers();
            }
            io_reg::PORT1 => {
                self.w09_port1_ol = value;
                if self.w04_b46_ptype == 0 || self.w04_b46_ptype == 5 {
                    let mut mask = 0u8;
                    let mut data = 0u8;
                    if self.w04_b46_ptype == 0 {
                        mask |= self.w15_port1_dir & 0x0F;
                        data |= value & 0x0F;
                    }
                    mask |= self.w15_port1_dir & 0xF0;
                    data |= value & 0xF0;
                    self.r09_port1_id = (self.r09_port1_id & !mask) | data;
                }
                self.update_keypad_registers();
            }
            io_reg::PORT3 => {
                // Port3 configures the LCD address mask and LCD control
                // (cpf/lcden); b6b5 selects how many address bits are used.
                let b6b5 = (value & 0x60) >> 5;
                self.lcd_buff_addr_mask = 0x3FFF >> b6b5;
                self.io[addr as usize] = (value & 0xFE) | (self.io[addr as usize] & 1);
            }
            io_reg::LCD_TIMER_CTRL => {
                let t = ((value as u16) & 0x03) << 12;
                self.lcd_buff_addr = (self.lcd_buff_addr & !0x3000) | t;
                self.w0c_b67_tmodesl = value >> 6;
                if self.w0c_b67_tmodesl == 1 {
                    self.w0c_b45_tm0s = (value >> 4) & 3;
                    self.w0c_b23_tm1s = (value >> 2) & 3;
                } else {
                    self.w0c_b345_tms = (value >> 3) & 7;
                }
                self.io[addr as usize] = value;
            }
            io_reg::ZP_BSW => {
                self.io[addr as usize] = value;
                let v = value & 0x07;
                self.rw0f_b4_dir00 = v & 0x10 != 0;
                self.rw0f_b5_dir01 = v & 0x20 != 0;
                self.rw0f_b6_dir023 = v & 0x40 != 0;
                self.rw0f_b7_dir047 = v & 0x80 != 0;
            }
            io_reg::TIMERA_VAL_L => {
                self.tma_reload = (self.tma_reload & 0xFF00) | value as u32;
            }
            io_reg::TIMERA_VAL_H => {
                self.tma_reload = (self.tma_reload & 0xFF) | ((value as u32) << 8);
                self.tma_value = self.tma_reload;
            }
            io_reg::TIMERAB_CTRL => {
                self.io[addr as usize] = value;
            }
            io_reg::PORT1_DIR => {
                self.w15_port1_dir = value;
            }
            io_reg::PORT4 => {
                self.io[addr as usize] = value;
            }
            io_reg::CTV_SELECT => {
                self.io[addr as usize] = value;
            }
            io_reg::VOLUME_SET => {
                self.io[addr as usize] = value & 0x7F;
            }
            io_reg::BATTERY => {
                self.io[addr as usize] = value;
            }
            io_reg::NAND_DATA => {
                self.nand_write(value);
            }
            io_reg::DSP_STAT => {
                if value & 0xC0 != 0 {
                    self.dsp_sleep = false;
                }
                if value & 0x40 != 0 {
                    self.dsp_ret_data = -1;
                    self.dsp_data_feeded = false;
                    self.dsp_trans = false;
                    self.dsp_0x7001_0x7002 = false;
                    self.dsp_0xd0 = false;
                }
            }
            io_reg::DSP_DATA_LOW => {
                self.dsp_data_low = value;
            }
            io_reg::DSP_CMD => {
                self.dsp_cmd(value);
            }
            io_reg::RTC_3A | io_reg::RTC_3B | io_reg::RTC_3C | io_reg::RTC_3D => {
                self.write_rtc_3x(addr, value);
            }
            io_reg::RTC_IDX => {
                self.io[addr as usize] = value;
            }
            io_reg::RTC_DATA => {
                let idx = self.io[io_reg::RTC_IDX] as usize;
                self.io[addr as usize] = value;
                if idx == ext_reg::RTC_CTRL {
                    self.ext_reg[idx] = value;
                } else if idx == ext_reg::INT_CLEAR {
                    self.iv_queue.retain(|&iv| {
                        (value & 0x01) == 0
                            || iv != 0x00 && (value & 0x02) == 0
                            || iv != 0x02 && (value & 0x04) == 0
                            || iv != 0x01
                    });
                    self.ext_reg[idx] = value & 0xF8;
                } else if idx < 7 {
                    if self.ext_reg[ext_reg::INT_CLEAR] & 0x80 == 0 {
                        self.ext_reg[idx] = value;
                    }
                } else if idx < 256 {
                    self.ext_reg[idx] = value;
                }
            }
            _ => {
                self.io[addr as usize] = value;
            }
        }
    }

    fn write_rtc_3x(&mut self, addr: u8, value: u8) {
        self.io[addr as usize] = value;
    }

    fn dsp_cmd(&mut self, value: u8) {
        let low = self.dsp_data_low;
        let cmd = ((value as u16) << 8) | low as u16;

        if cmd == 0xFFFF {
            self.dsp_trans = false;
        }
        if cmd == 0x8000 {
            self.dsp_sleep = true;
        }
        if self.dsp_trans {
            self.dsp_ret_data = low as i32;
            return;
        }

        if value == 0xD0 {
            self.dsp_ret_data = 0x5A;
            self.dsp_0xd0 = true;
        } else if value > 0x60 {
            self.dsp_0xd0 = false;
        }

        if value == 0x70 {
            if low == 0x01 || low == 0x02 {
                self.dsp_0x7001_0x7002 = true;
                self.dsp_ret_data = 0x06;
            } else {
                self.dsp_0x7001_0x7002 = false;
            }
        } else if value >= 0x60 {
            self.dsp_0x7001_0x7002 = false;
        }

        if cmd == 0x7004 {
            self.dsp_trans = true;
        }

        self.dsp_data_feeded = true;
    }

    /// Recompute port0/port1 input data from the keypad matrix.
    fn update_keypad_registers(&mut self) {
        let port1_control = self.w15_port1_dir;
        let port0_control = ((self.rw0f_b4_dir00 as u8) << 4)
            | ((self.rw0f_b5_dir01 as u8) << 5)
            | ((self.rw0f_b6_dir023 as u8) << 6)
            | ((self.rw0f_b7_dir047 as u8) << 7);
        let pull_high = (!self.ext_reg[ext_reg::P0_PU]) & 0x0F;

        let mut tmpdest0 = pull_high;
        let mut tmpdest1 = 0u8;
        let mut port1_control_bit = 0x01u8;

        for y in 0..8 {
            let ysend = port1_control & port1_control_bit != 0;
            let mut xbit = 0x01u8;
            for x in 0..8 {
                let port0_control_bit = if x < 2 {
                    xbit << 4
                } else if x < 4 {
                    0x40
                } else {
                    0x80
                };
                let xsend = port0_control & port0_control_bit != 0;

                if ysend != xsend {
                    if ysend {
                        // port1 -> port0
                        if self.keypad_matrix[y][x] {
                            if self.w09_port1_ol & port1_control_bit != 0 {
                                tmpdest0 |= xbit;
                            } else if (xbit & 0x0F) != 0
                                && (self.ext_reg[ext_reg::P0_PU] & xbit) == 0
                            {
                                tmpdest0 &= !xbit;
                            }
                        }
                    } else {
                        // port0 -> port1
                        if self.keypad_matrix[y][x] && self.w08_port0_ol & xbit != 0 {
                            tmpdest1 |= port1_control_bit;
                        }
                    }
                }
                xbit <<= 1;
            }
            port1_control_bit <<= 1;
        }

        // Port1: clear receive bits
        let mut port1_data = if port1_control != 0xFF {
            self.r09_port1_id & port1_control
        } else {
            self.r09_port1_id
        };
        port1_data |= tmpdest1;

        // Port0: clear receive bits
        let mut port0_data = if port0_control != 0xF0 {
            let port0_mask = (port0_control >> 4) & 0x03;
            let mask = if port0_control & 0x40 != 0 {
                port0_mask | 0x0C
            } else {
                port0_mask
            };
            let mask = if port0_control & 0x80 != 0 {
                mask | 0xF0
            } else {
                mask
            };
            self.r08_port0_id & mask
        } else {
            self.r08_port0_id
        };
        port0_data |= tmpdest0;

        self.r09_port1_id = port1_data;
        self.r08_port0_id = port0_data;
    }

    /// Advance timer0/timer1 counters; returns true if an IRQ fired.
    fn keep_timer01(&mut self, cpu_tick: u32) -> bool {
        let mut need_irq = false;

        if self.timer0run {
            self.timer0ticks += cpu_tick;
            let mul0 = 1 + if self.w0c_b67_tmodesl == 1 {
                self.w0c_b45_tm0s as u32
            } else {
                self.w0c_b345_tms as u32
            };
            let inc0 = self.timer0ticks >> mul0;
            if inc0 > 0 {
                self.timer0ticks -= inc0 << mul0;
            }

            if self.w0c_b67_tmodesl == 1 || self.w0c_b67_tmodesl == 0 {
                let newt = self.io[io_reg::TIMER0_VAL] as u16 + inc0 as u16;
                let overflow = newt > 0xFF;
                if overflow && (self.w0c_b67_tmodesl == 1 || self.timer1run) {
                    self.io[io_reg::INT_STATUS] |= 0x10;
                    need_irq = true;
                }
                self.io[io_reg::TIMER0_VAL] = if self.w0c_b67_tmodesl == 1 {
                    newt as u8
                } else {
                    (newt + self.io[io_reg::TIMER1_VAL] as u16) as u8
                };
            }
            if self.w0c_b67_tmodesl == 2 {
                let newt = self.io[io_reg::TIMER0_VAL] as u16 + inc0 as u16;
                self.io[io_reg::TIMER0_VAL] = newt as u8;
                if newt > 0xFF {
                    let newt1 = self.io[io_reg::TIMER1_VAL] as u16 + (newt >> 8);
                    if newt1 > 0xFF {
                        self.io[io_reg::INT_STATUS] |= 0x20;
                        need_irq = true;
                    }
                    self.io[io_reg::TIMER1_VAL] = newt1 as u8;
                }
            }
            if self.w0c_b67_tmodesl == 3 {
                let newt = self.io[io_reg::TIMER0_VAL] as u16 + inc0 as u16;
                self.io[io_reg::TIMER0_VAL] = newt as u8;
                if newt > 0xFF {
                    self.io[io_reg::INT_STATUS] |= 0x10;
                    need_irq = true;
                    if self.timer1run {
                        let newt1 = self.io[io_reg::TIMER1_VAL] as u16 + (newt >> 8);
                        if newt1 > 0xFF {
                            self.io[io_reg::INT_STATUS] |= 0x20;
                            need_irq = true;
                        }
                        self.io[io_reg::TIMER1_VAL] = newt1 as u8;
                    }
                }
            }
        }

        // Timer 1, mode 1 only
        if self.timer1run && self.w0c_b67_tmodesl == 1 {
            self.timer1ticks += cpu_tick;
            let inc1 = self.timer1ticks >> ((self.w0c_b23_tm1s as u32 + 1) * 2);
            if inc1 > 0 {
                self.timer1ticks -= inc1 << ((self.w0c_b23_tm1s as u32 + 1) * 2);
            }
            let newt = self.io[io_reg::TIMER1_VAL] as u16 + inc1 as u16;
            if newt > 0xFF {
                self.io[io_reg::INT_STATUS] |= 0x20;
                need_irq = true;
            }
            self.io[io_reg::TIMER1_VAL] = newt as u8;
        }

        need_irq
    }

    /// Timer A (used by record/play; fires an interrupt).
    fn set_timer_a(&mut self) -> bool {
        let temp = self.io[io_reg::TIMERAB_CTRL] >> 4;
        if temp != 0 {
            self.tma_value += 256 >> temp;
            if self.tma_value >= 0x10000 {
                self.tma_value = self.tma_reload;
                if self.inner_interrupt_control & 1 != 0 {
                    self.io[io_reg::INT_STATUS] |= 1;
                    return true;
                }
            }
        }
        false
    }

    fn time_base_enable(&self) -> bool {
        self.io[io_reg::GENERAL_CTRL] & 0x0F != 0
    }

    fn nmi_enable(&self) -> bool {
        self.inner_interrupt_control & 0x10 == 0
    }

    /// Clock is switched off when io[0x05] CKS bits (5..7) == 7.
    /// The firmware writes 0xFF here to enter standby; the CPU is then
    /// suspended until a key wakes the device (warm reset).
    fn is_clk_off(&self) -> bool {
        self.io[io_reg::CLOCK_CTRL] >> 5 == 7
    }

    /// Wake the device from standby: restore the clock and reset the CPU.
    fn warm_reset(&mut self, cpu: &mut Cpu) {
        self.io[io_reg::CLOCK_CTRL] &= 0x1F;
        self.do_warm_reset = false;
        cpu.reset(self.peek_u16(crate::cpu::RESET_VECTOR));
        log::info!("NC2000 warm reset (wake from standby)");
    }

    fn bump_rtc(&mut self) {
        self.ext_reg[ext_reg::TR_S] = self.ext_reg[ext_reg::TR_S].wrapping_add(1);
        if self.ext_reg[ext_reg::TR_S] == 60 {
            self.ext_reg[ext_reg::TR_S] = 0;
            self.ext_reg[ext_reg::TR_M] = self.ext_reg[ext_reg::TR_M].wrapping_add(1);
            if self.ext_reg[ext_reg::TR_M] == 60 {
                self.ext_reg[ext_reg::TR_M] = 0;
                self.ext_reg[ext_reg::TR_H] = self.ext_reg[ext_reg::TR_H].wrapping_add(1);
                if self.ext_reg[ext_reg::TR_H] == 24 {
                    self.ext_reg[ext_reg::TR_H] = 0;
                    self.ext_reg[ext_reg::TR_D] = self.ext_reg[ext_reg::TR_D].wrapping_add(1);
                }
            }
        }
    }

    fn chk_alarm(&self) -> bool {
        let mut alm = false;
        if self.ext_reg[ext_reg::AR_H] & 0x80 != 0 {
            alm = true;
            if (self.ext_reg[ext_reg::AR_H] & 0x1F) != (self.ext_reg[ext_reg::TR_H] & 0x1F) {
                return false;
            }
        }
        if self.ext_reg[ext_reg::AR_M] & 0x80 != 0 {
            alm = true;
            if (self.ext_reg[ext_reg::AR_M] & 0x3F) != (self.ext_reg[ext_reg::TR_M] & 0x3F) {
                return false;
            }
        }
        if self.ext_reg[ext_reg::AR_S] & 0x80 != 0 {
            alm = true;
            if (self.ext_reg[ext_reg::AR_S] & 0x3F) != (self.ext_reg[ext_reg::TR_S] & 0x3F) {
                return false;
            }
        }
        alm
    }

    fn put_iv(&mut self, iv: u8) {
        if !self.iv_queue.contains(&iv) {
            self.iv_queue.push(iv);
        }
    }

    fn peek_iv(&self) -> Option<u8> {
        self.iv_queue.first().copied()
    }

    /// Advance periodic events based on elapsed cycles since last tick.
    fn tick_periodic(&mut self, cpu: &mut Cpu, delta: u32) {
        // Timer0/1
        if self.keep_timer01(delta) {
            cpu.irq_pending = true;
        }

        // Timer A at 576*50 Hz
        let ta_cycles = crate::timer::CPU_FREQ / (576 * 50);
        self.cycles = self.cycles.wrapping_add(delta as u64);
        while self.cycles >= ta_cycles as u64 {
            self.cycles -= ta_cycles as u64;
            if self.set_timer_a() {
                cpu.irq_pending = true;
            }
        }

        // Timebase ~185 Hz
        self.tb_cycles += delta as u64;
        let tb_cycles = (crate::timer::CPU_FREQ / 185) as u64;
        while self.tb_cycles >= tb_cycles {
            self.tb_cycles -= tb_cycles;
            if self.time_base_enable() {
                self.io[io_reg::INT_STATUS] |= 0x08;
                cpu.irq_pending = true;
            }
        }

        // RTC 256 Hz
        self.rtc_cycles += delta as u64;
        let rtc_cycles = (crate::timer::CPU_FREQ / 256) as u64;
        while self.rtc_cycles >= rtc_cycles {
            self.rtc_cycles -= rtc_cycles;
            self.rtc_256_counter = self.rtc_256_counter.wrapping_add(1);
            let cnt = self.rtc_256_counter;
            if cnt == 0 {
                self.bump_rtc();
            }
            if cnt.is_multiple_of(128) {
                if cnt == 0 && self.ext_reg[ext_reg::RTC_CTRL] & 0x02 != 0 && self.chk_alarm() {
                    self.put_iv(0x02);
                }
                if self.ext_reg[ext_reg::RTC_CTRL] & 0x01 != 0 {
                    self.put_iv(0x00);
                }
            }
            if cnt % 128 == 64 && self.nmi_enable() {
                cpu.nmi_pending = true;
            }
            if let Some(iv) = self.peek_iv() {
                let _ = iv;
                cpu.irq_pending = true;
            }
        }
    }

    fn reset_machine(&mut self) {
        self.io.fill(0);
        self.ext_reg.fill(0);
        self.ram.fill(0);
        self.ext_ram.fill(0);
        self.ram_b.fill(0);
        self.ram_b2.fill(0);
        self.fp_step = 0;
        self.fp_type = NorCmd::None;
        self.clear_nand_status();
        self.keypad_matrix = [[false; 8]; 8];
        self.w08_port0_ol = 0;
        self.r08_port0_id = 0;
        self.w09_port1_ol = 0;
        self.r09_port1_id = 0;
        self.w15_port1_dir = 0;
        self.timer0run = false;
        self.timer1run = false;
        self.timer0ticks = 0;
        self.timer1ticks = 0;
        self.inner_interrupt_control = 0;
        self.dsp_ret_data = -1;
        self.dsp_sleep = false;
        self.dsp_trans = false;
        self.dsp_data_feeded = false;
        self.lcd.reset();
        self.lcd_buff_addr = 0;
        self.lcd_buff_addr_mask = 0x3FFF;
        self.rtc_256_counter = 0;
        self.iv_queue.clear();
        self.do_warm_reset = false;
        self.audio.reset();
        self.cycles = 0;
        self.tb_cycles = 0;
        self.rtc_cycles = 0;
    }
}

/// Bank window targets.
#[derive(Clone, Copy)]
enum BankTarget {
    Nor(usize),
    ExtRam,
}

/// Resolved memory target.
#[derive(Clone, Copy)]
enum MemTarget {
    Io(usize),
    Ram(usize),
    RamB(usize),
    ExtRam(usize),
    Nor(usize),
}

impl Machine for Nc2000Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Nc2000
    }

    fn reset(&mut self) {
        self.reset_machine();
        log::info!("NC2000 machine reset");
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        self.load_rom_impl(files)
    }

    fn set_key(&mut self, key_id: u8, pressed: bool) {
        // key_id encodes matrix position: row = key_id >> 3, col = key_id & 7
        let row = (key_id >> 3) as usize;
        let col = (key_id & 7) as usize;
        if row < 8 && col < 8 {
            self.keypad_matrix[row][col] = pressed;
            self.update_keypad_registers();
        }
        // Any key press on columns 0/1 wakes the device from standby
        // (matching the reference implementation).
        if pressed && col < 2 && self.is_clk_off() {
            self.do_warm_reset = true;
        }
    }

    fn is_sleeping(&self) -> bool {
        false
    }

    fn step(&mut self, cpu: &mut Cpu) -> u64 {
        if self.do_warm_reset {
            self.warm_reset(cpu);
        }

        // With the clock off the CPU is suspended; still advance the
        // periodic timers (RTC keeps running while in standby).
        let cycles = if self.is_clk_off() {
            crate::lcd::CYCLES_PER_FRAME
        } else {
            let mut bus = Nc2000Bus { machine: self };
            cpu.step(&mut bus)
        };

        self.tick_periodic(cpu, cycles as u32);
        cycles
    }

    fn end_of_frame(&mut self, _cpu: &mut Cpu) {
        // Copy LCD framebuffer from RAM at the configured address.
        let addr = (self.lcd_buff_addr & self.lcd_buff_addr_mask) as usize;
        self.lcd.copy_from(&self.ram, addr);
    }

    fn peek(&self, addr: u16) -> u8 {
        self.peek_impl(addr)
    }

    fn peek_u16(&self, addr: u16) -> u16 {
        let lo = self.peek(addr) as u16;
        let hi = self.peek(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn lcd(&self) -> &Lcd {
        &self.lcd
    }

    fn lcd_mut(&mut self) -> &mut Lcd {
        &mut self.lcd
    }

    fn drain_audio(&mut self, out: &mut Vec<i16>) {
        self.audio.drain_audio(out);
    }

    fn save_state(&self, cpu: &Cpu) -> SaveState {
        SaveState::new(
            cpu,
            &self.ram,
            &self.io,
            &crate::timer::Timer::new(),
            &crate::input::Input::new(),
            &self.audio,
            &crate::flash::Flash::new(),
            self.lcd_buff_addr as u32,
        )
    }

    fn load_state(&mut self, cpu: &mut Cpu, state: &SaveState) {
        *cpu = state.cpu.clone();
        if state.io.len() == 0x40 {
            self.io.copy_from_slice(&state.io);
        }
        if state.ram.len() >= INTERNAL_RAM_SIZE {
            self.ram[..INTERNAL_RAM_SIZE].copy_from_slice(&state.ram[..INTERNAL_RAM_SIZE]);
        }
        self.lcd_buff_addr = state.lcd_addr as u16;
    }
}

/// Bus adapter connecting the shared CPU to NC2000 hardware.
struct Nc2000Bus<'a> {
    machine: &'a mut Nc2000Machine,
}

impl<'a> CpuBus for Nc2000Bus<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        self.machine.bus_read(addr)
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.machine.bus_write(addr, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_BOOT_FRAMES: u32 = 2000;

    fn empty_machine() -> Nc2000Machine {
        Nc2000Machine::new(&RomFiles::new(None, None, None, None)).unwrap()
    }

    /// Build RomFiles from the repository-root roms/ directory, or None if
    /// the dumps are not present (tests then skip).
    fn nc2000_rom_files() -> Option<RomFiles> {
        let base = std::path::Path::new(r"E:\Code\WQXEmu\roms\nc2000");
        let nor = base.join("nc2000.nor");
        let nand = base.join("nc2000.nand");
        let nand0 = base.join("nc2000.nand0");
        if nor.exists() && nand.exists() && nand0.exists() {
            Some(RomFiles::new(None, Some(nor), Some(nand), Some(nand0)))
        } else {
            None
        }
    }

    #[test]
    fn nor_software_id() {
        let mut m = empty_machine();
        // Bank 0 selects NOR page 0.
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        // SW_ID command sequence: AA 55 90
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0x90);
        assert_eq!(m.fp_type, NorCmd::SwId);
        assert_eq!(m.bus_read(0x8000), 0xC7);
        assert_eq!(m.bus_read(0x8001), 0xD7);
        // Exit ID mode
        m.bus_write(0x5555, 0xF0);
        assert_eq!(m.fp_type, NorCmd::None);
    }

    #[test]
    fn nor_byte_program() {
        let mut m = empty_machine();
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        let offset = 0x4000 + 0x1234; // within NOR page 0 window
        assert_eq!(m.nor[0x1234], 0xFF);
        // BYTE_PROGRAM: AA 55 A0 then data
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0xA0);
        m.bus_write(offset as u16, 0x5A);
        // CPU 0x5234 maps to NOR offset 0x4000 + 0x1234
        assert_eq!(m.nor[0x5234], 0x5A);
    }

    #[test]
    fn nor_block_erase() {
        let mut m = empty_machine();
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        m.nor[0x4000] = 0x00;
        m.nor[0x47FF] = 0x00;
        // BLOCK_OR_MASS_ERASE: AA 55 80 AA 55 30
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0x80);
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x4555, 0x30); // erase 2KB block at 0x4000
        assert_eq!(m.nor[0x4000], 0xFF);
        assert_eq!(m.nor[0x47FF], 0xFF);
    }

    #[test]
    fn bank_window_nor_and_ext_ram() {
        let mut m = empty_machine();
        // Bank 0 -> NOR page 0
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        m.nor[0x4000] = 0xAB;
        assert_eq!(m.bus_read(0x4000), 0xAB);
        // Bank 0x80 -> extended RAM
        m.io_write(io_reg::BANK_SWITCH as u8, 0x80);
        m.bus_write(0x4000, 0xCD);
        // CPU 0x4000 maps to ext_ram + 0x4000 (memmap[2] = bank + 0x4000)
        assert_eq!(m.ext_ram[0x4000], 0xCD);
        assert_eq!(m.bus_read(0x4000), 0xCD);
        // 0xE000-0xFFFF fixed BIOS = NOR page 0 + 0x6000
        m.nor[0x6000 + 0x1FFC] = 0x12;
        m.nor[0x6000 + 0x1FFD] = 0x34;
        assert_eq!(m.bus_read(0xFFFC), 0x12);
        assert_eq!(m.peek_u16(0xFFFC), 0x3412);
    }

    #[test]
    fn nand_read_page() {
        let mut m = empty_machine();
        // Fill page 5 with known data
        m.nand[5 * NAND_PAGE_SIZE] = 0x11;
        m.nand[5 * NAND_PAGE_SIZE + 511] = 0x22;
        m.nand[5 * NAND_PAGE_SIZE + 512] = 0x33;

        // CLE on (io[0x18] bit0), command 0x00 = read main area
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        // ALE on (bit1), address: col=0, row=5, 0, 0
        m.io_write(io_reg::PORT4 as u8, 0x02);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x05);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        // CLE/ALE off
        m.io_write(io_reg::PORT4 as u8, 0x00);

        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x11);
        // Sequential read to the end of the page
        for _ in 1..511 {
            m.bus_read(io_reg::NAND_DATA as u16);
        }
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x22);
        // Spare area (command 0x50)
        m.clear_nand_status();
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x50);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x05);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x33);
    }

    #[test]
    fn nand_program_and_erase() {
        let mut m = empty_machine();
        // Program page 3
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x80);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x03);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        for i in 0..NAND_PAGE_SIZE {
            m.io_write(io_reg::NAND_DATA as u8, (i & 0xFF) as u8);
        }
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x10);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.nand[3 * NAND_PAGE_SIZE + 10], 0x0A);

        // Erase block containing page 3 (32 pages per block)
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x60);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0xD0);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.nand[3 * NAND_PAGE_SIZE + 10], 0xFF);
    }

    #[test]
    fn nand_status_and_id() {
        let mut m = empty_machine();
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x70);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x40);

        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x90);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0xEC);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x75);
    }

    #[test]
    fn nand_images_share_physical_page_space() {
        let mut m = empty_machine();
        m.load_main_nand(&[0x22]);
        m.load_first_nand_plane(&[0x11]);

        // Read physical page 0 from the first-plane image.
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        for value in [0x00, 0x00, 0x00, 0x00] {
            m.io_write(io_reg::NAND_DATA as u8, value);
        }
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x11);

        // Read physical page 64 from the start of the main image.
        m.io_write(io_reg::PORT4 as u8, 0x01);
        m.io_write(io_reg::NAND_DATA as u8, 0x00);
        m.io_write(io_reg::PORT4 as u8, 0x02);
        for value in [0x00, NAND0_PAGES as u8, 0x00, 0x00] {
            m.io_write(io_reg::NAND_DATA as u8, value);
        }
        m.io_write(io_reg::PORT4 as u8, 0x00);
        assert_eq!(m.bus_read(io_reg::NAND_DATA as u16), 0x22);
    }

    #[test]
    fn keypad_matrix_to_port() {
        let mut m = empty_machine();
        // Firmware scan: port1 drives all rows, port0 P00-P03 receive,
        // P04-P07 drive columns.
        m.w15_port1_dir = 0xFF;
        m.w09_port1_ol = 0xFF;
        m.rw0f_b7_dir047 = true; // P04-P07 output
        m.w08_port0_ol = 0x00;
        // No pull-up on P00-P03 so idle input reads 0
        m.ext_reg[ext_reg::P0_PU] = 0x0F;

        // Press key at matrix (row 2, col 0)
        m.set_key(2 << 3, true);
        m.update_keypad_registers();
        // Port0 input should reflect the pressed key: P00 bit set
        assert_ne!(m.r08_port0_id & 0x01, 0);

        // Release
        m.set_key(2 << 3, false);
        m.update_keypad_registers();
        assert_eq!(m.r08_port0_id & 0x01, 0);
    }

    #[test]
    fn step_advances_cycles() {
        // Boot with the real NC2000 dump files and verify the CPU advances.
        let Some(files) = nc2000_rom_files() else {
            eprintln!("skipping: NC2000 dumps not present");
            return;
        };
        let mut machine = Nc2000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        let mut total = 0u64;
        let mut steps = 0u32;
        for _ in 0..10000 {
            let pc_before = cpu.pc;
            let c = machine.step(&mut cpu);
            if c == 0 {
                panic!(
                    "step returned 0 cycles: pc_before={:04X} pc_after={:04X}",
                    pc_before, cpu.pc
                );
            }
            total += c;
            steps += 1;
        }
        println!("10000 steps, total cycles = {}", total);
        assert!(total >= 10000);
        assert!(steps == 10000);
    }

    #[test]
    fn run_frame_completes() {
        let Some(files) = nc2000_rom_files() else {
            eprintln!("skipping: NC2000 dumps not present");
            return;
        };
        let mut machine = Nc2000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        let start = std::time::Instant::now();
        for _ in 0..5 {
            // replicate Emulator::run_frame
            let target = crate::lcd::CYCLES_PER_FRAME;
            let mut acc = 0u64;
            let mut steps = 0u64;
            while acc < target {
                acc += machine.step(&mut cpu);
                steps += 1;
            }
            println!("frame done: cycles={} steps={}", acc, steps);
        }
        println!("5 frames took {:?}", start.elapsed());
    }

    #[test]
    fn standby_and_wake() {
        let Some(files) = nc2000_rom_files() else {
            eprintln!("skipping: NC2000 dumps not present");
            return;
        };
        let mut machine = Nc2000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        // Run until the firmware enters standby (clock off).
        let mut frames = 0;
        while !machine.is_clk_off() && frames < MAX_BOOT_FRAMES {
            let target = crate::lcd::CYCLES_PER_FRAME;
            let mut acc = 0u64;
            while acc < target {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
            frames += 1;
        }
        assert!(machine.is_clk_off(), "firmware did not enter standby");
        println!("entered standby after {} frames", frames);

        // Press a wake key (column 0) and step: warm reset should fire.
        machine.set_key(0, true);
        assert!(machine.do_warm_reset);
        let pc_before = cpu.pc;
        let _ = machine.step(&mut cpu);
        assert!(!machine.is_clk_off(), "clock should be restored");
        assert!(!machine.do_warm_reset);
        // After warm reset the CPU restarts at the reset vector; the same
        // step may already have executed the first instruction.
        assert_ne!(pc_before, cpu.pc);
        assert_ne!(cpu.pc, 0xE314, "CPU should leave the standby loop");

        // Continue running: CPU should execute again after wake.
        let target = crate::lcd::CYCLES_PER_FRAME;
        let mut acc = 0u64;
        while acc < target {
            acc += machine.step(&mut cpu);
        }
        assert!(cpu.pc != 0);
    }

    #[test]
    fn hotkey_wakes_device() {
        let Some(files) = nc2000_rom_files() else {
            eprintln!("skipping: NC2000 dumps not present");
            return;
        };
        let mut machine = Nc2000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        // Run until standby
        let mut frames = 0;
        while !machine.is_clk_off() && frames < MAX_BOOT_FRAMES {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
            frames += 1;
        }
        assert!(machine.is_clk_off());

        // F5 (English-Chinese) sits at matrix (row 3, col 1).
        machine.set_key(3 << 3 | 1, true);
        assert!(machine.do_warm_reset);
        let pc_before = cpu.pc;
        let _ = machine.step(&mut cpu);
        assert!(!machine.is_clk_off(), "clock should be restored");
        assert!(!machine.do_warm_reset);
        assert_ne!(pc_before, cpu.pc);
    }

    #[test]
    fn cold_boot_lcd_trace() {
        let Some(files) = nc2000_rom_files() else {
            eprintln!("skipping: NC2000 dumps not present");
            return;
        };
        let mut machine = Nc2000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        // The firmware draws the clock screen before entering standby.
        let mut max_nonzero = 0;
        for _ in 0..150 {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
            let nz = machine
                .lcd
                .framebuffer_raw()
                .iter()
                .filter(|&&b| b != 0)
                .count();
            max_nonzero = max_nonzero.max(nz);
        }
        assert!(
            max_nonzero > 50,
            "cold boot should draw the clock screen, max nonzero {}",
            max_nonzero
        );
    }
}
