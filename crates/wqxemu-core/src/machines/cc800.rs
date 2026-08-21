// CC800 hardware machine implementation.
//
// The CC800 is an older SPDC1016-era 6502 dictionary that shares most of
// the PC1000/NC1020 SoC family. Its ROM and NOR dumps are stored with the
// 16KB-half-swapped convention (like the NC1020), and its bank window
// page order differs from the PC1000. The memory map follows the Sim800
// (NekoDriver) reference:
//
//  0x0000-0x003F  IO registers
//  0x0040-0x007F  zero-page switchable window (register 0x0F)
//  0x0080-0x1FFF  RAM page 0
//  0x2000-0x3FFF  RAM page 1
//  0x4000-0x5FFF  bank window page 0 (bank + 0x0000; RAM at boot)
//  0x6000-0x7FFF  bank window page 1 (bank + 0x2000; RAM mirror at boot)
//  0x8000-0x9FFF  bank window page 2 (bank + 0x4000)
//  0xA000-0xBFFF  bank window page 3 (bank + 0x6000)
//  0xC000-0xDFFF  BBS page (register 0x0A bits 0-3)
//  0xE000-0xFFFF  fixed BIOS page (volume bank 0 + 0x2000)
//
// ROM layout: 16MB `obj.bin` with half-swapped banks. Volume 0 covers
// banks 0-255 (first 8MB) and volume 1 covers banks 256-511. NOR Flash
// is 512KB with 16 x 32KB banks and the same half-swapped dump layout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use crate::audio::Audio;
use crate::cpu::{Cpu, CpuBus};
use crate::lcd::Lcd;
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::save::SaveState;
use crate::timer::CPU_FREQ;

/// ROM bank size (32KB).
const BANK_SIZE: usize = 0x8000;
/// CC800 ROM size (16MB obj.bin).
const ROM_SIZE: usize = 0x1000000;
/// NOR Flash size (512KB).
const NOR_SIZE: usize = 0x80000;
/// Number of ROM banks per volume.
const NUM_ROM_BANKS: usize = 256;
/// Internal RAM size (8 pages x 8KB = 64KB).
const RAM_SIZE: usize = 0x10000;

/// CC800 IO register indexes (shared with the PC1000/SPDC1016 family).
#[allow(dead_code)]
mod io_reg {
    pub const BANK_SWITCH: usize = 0x00;
    pub const INT_STATUS: usize = 0x01;
    pub const INT_ENABLE: usize = 0x40;
    pub const TIMER0_VAL: usize = 0x02;
    pub const TIMER1_VAL: usize = 0x03;
    pub const GENERAL_CTRL: usize = 0x04;
    pub const CLOCK_CTRL: usize = 0x05;
    pub const LCD_CONFIG: usize = 0x06;
    pub const PORT_CONFIG: usize = 0x07;
    pub const PORT0: usize = 0x08;
    pub const PORT1: usize = 0x09;
    pub const BIOS_BSW: usize = 0x0A;
    pub const LCD_CTRL: usize = 0x0B;
    pub const GENERAL_STATUS: usize = 0x0C;
    pub const VOLUME: usize = 0x0D;
    pub const DAC_DATA: usize = 0x0E;
    pub const ZP_BSW: usize = 0x0F;
    pub const TIMERA_VAL_L: usize = 0x10;
    pub const TIMERA_VAL_H: usize = 0x11;
    pub const TIMERAB_CTRL: usize = 0x14;
    pub const PORT1_DIR: usize = 0x15;
    pub const PORT2: usize = 0x17;
    pub const PORT4: usize = 0x18;
    pub const CTV_SELECT: usize = 0x19;
    pub const PORT0_OUT: usize = 0x41;
    pub const DSP_STAT: usize = 0x20;
    pub const DSP_DATA_LOW: usize = 0x22;
    pub const DSP_DATA_HI: usize = 0x23;
}

/// Interrupt status bits.
mod int_flags {
    pub const TIMER_A: u8 = 0x01;
    pub const TIME_BASE: u8 = 0x08;
    pub const TIMER0: u8 = 0x10;
    pub const TIMER1: u8 = 0x20;
}

/// Interrupt enable bits (shadow register 0x40).
mod int_enable {
    pub const TIMER_A: u8 = 0x01;
    pub const TIME_BASE: u8 = 0x08;
    pub const NMI: u8 = 0x10;
}

/// Periodic event frequencies (relative to the shared CPU clock).
const TICK_HZ: u64 = 576 * 50;
const TICK_CYCLES: u64 = CPU_FREQ as u64 / TICK_HZ;
const NMI_CYCLES: u64 = CPU_FREQ as u64 / 2;

/// NOR command state machine states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// A resolved 8KB memory window target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Page {
    Ram(usize),
    Rom(usize),
    Nor(usize),
}

/// CC800 machine.
#[derive(Serialize, Deserialize)]
pub struct Cc800Machine {
    ram: Vec<u8>,
    #[serde(skip)]
    rom: Vec<u8>,
    nor: Vec<u8>,
    #[serde(with = "BigArray")]
    nor_info_block: [u8; 0x100],
    #[serde(with = "BigArray")]
    io: [u8; 0x80],
    /// Volume bank base tables (bank index -> ROM offset).
    #[serde(with = "BigArray")]
    vol0: [usize; NUM_ROM_BANKS],
    #[serde(with = "BigArray")]
    vol1: [usize; NUM_ROM_BANKS],
    /// BBS page targets for the current volume.
    bbs: [Page; 16],
    /// Bank window pages 2-7.
    win: [Page; 8],
    /// Zero-page 0x40-0x7F base offset inside RAM.
    zp_off: usize,
    lcd_buff_addr: u16,

    // Keypad matrix: 0 = released, 1 = pressed, 2 = nonexistent key.
    keypad_matrix: [[u8; 8]; 8],
    w08_port0_ol: u8,
    r08_port0_id: u8,
    w09_port1_ol: u8,
    r09_port1_id: u8,
    w15_port1_dir: u8,

    // Timers
    timer0run: bool,
    timer1run: bool,
    tm0v: u16,
    tm0r: u16,
    tm1v2: u16,
    tma_value: u32,
    tma_reload: u32,
    tick_idx: u64,
    tick_cycles: u64,
    nmi_cycles: u64,

    // NOR programming state
    fp_step: u8,
    fp_type: NorCmd,

    // Audio / LCD-off standby
    is_play_music: bool,
    music_sample: i16,
    lcd_off: bool,
    do_warm_reset: bool,

    audio: Audio,
    lcd: Lcd,
}

impl Cc800Machine {
    /// Create the machine and load its ROM files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            ram: vec![0; RAM_SIZE],
            rom: vec![0; ROM_SIZE],
            nor: vec![0xFF; NOR_SIZE],
            nor_info_block: [0; 0x100],
            io: [0; 0x80],
            vol0: [0; NUM_ROM_BANKS],
            vol1: [0; NUM_ROM_BANKS],
            bbs: [Page::Ram(0); 16],
            win: [Page::Ram(0); 8],
            zp_off: 0x40,
            lcd_buff_addr: 0x09C0,
            keypad_matrix: [[2; 8]; 8],
            w08_port0_ol: 0,
            r08_port0_id: 0,
            w09_port1_ol: 0,
            r09_port1_id: 0,
            w15_port1_dir: 0,
            timer0run: false,
            timer1run: false,
            tm0v: 0,
            tm0r: 0,
            tm1v2: 0,
            tma_value: 0,
            tma_reload: 0,
            tick_idx: 0,
            tick_cycles: 0,
            nmi_cycles: 0,
            fp_step: 0,
            fp_type: NorCmd::None,
            is_play_music: false,
            music_sample: 1,
            lcd_off: false,
            do_warm_reset: false,
            audio: Audio::new(),
            lcd: Lcd::new(),
        };
        for b in 0..NUM_ROM_BANKS {
            machine.vol0[b] = b * BANK_SIZE;
            machine.vol1[b] = (NUM_ROM_BANKS + b) * BANK_SIZE;
        }
        machine.update_map();
        machine.load_rom(files)?;
        Ok(machine)
    }

    /// Load ROM and NOR files.
    fn load_rom_impl(&mut self, files: &RomFiles) -> Result<()> {
        if let Some(rom_path) = &files.rom {
            let data = std::fs::read(rom_path)
                .with_context(|| format!("Failed to read CC800 ROM: {}", rom_path.display()))?;
            if data.len() < ROM_SIZE {
                anyhow::bail!(
                    "CC800 ROM file too small: expected {} bytes, got {}",
                    ROM_SIZE,
                    data.len()
                );
            }
            // CC800 dumps store each 32KB bank with its 16KB halves
            // swapped. Swap them back so bank windows map linearly.
            for off in (0..ROM_SIZE).step_by(BANK_SIZE) {
                self.rom[off..off + 0x4000].copy_from_slice(&data[off + 0x4000..off + 0x8000]);
                self.rom[off + 0x4000..off + 0x8000].copy_from_slice(&data[off..off + 0x4000]);
            }
            log::info!(
                "CC800 ROM loaded: {} bytes, reset vector 0x{:04X}",
                self.rom.len(),
                self.peek_u16(crate::cpu::RESET_VECTOR)
            );
        }
        if let Some(nor_path) = &files.nor {
            let data = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read CC800 NOR: {}", nor_path.display()))?;
            if data.len() < NOR_SIZE {
                anyhow::bail!(
                    "CC800 NOR file too small: expected {} bytes, got {}",
                    NOR_SIZE,
                    data.len()
                );
            }
            for off in (0..NOR_SIZE).step_by(BANK_SIZE) {
                self.nor[off..off + 0x4000].copy_from_slice(&data[off + 0x4000..off + 0x8000]);
                self.nor[off + 0x4000..off + 0x8000].copy_from_slice(&data[off..off + 0x4000]);
            }
            log::info!("CC800 NOR loaded: {} bytes", self.nor.len());
        }
        // The physical NOR info block (bank 0 offset 0x4000 in the dump)
        // lands at offset 0x0000 after the swap.
        self.nor_info_block
            .copy_from_slice(&self.nor[0x0000..0x0100]);
        Ok(())
    }

    /// Recompute bank windows and BBS pages from the current registers.
    fn update_map(&mut self) {
        let bank = self.io[io_reg::BANK_SWITCH];
        let vol = self.io[io_reg::VOLUME] & 0x01;
        let roa = self.io[io_reg::BIOS_BSW] & 0x80 != 0;

        // Active volume base tables for BBS and BIOS pages.
        let vol_base = |b: usize| if vol == 0 { self.vol0[b] } else { self.vol1[b] };

        // BBS pages: bbs[0] = vol[0], bbs[1] = RAM/NOR special, then
        // banks 0-3 of the active volume split as {+0, +0x2000, +0x4000,
        // +0x6000}.
        self.bbs[0] = Page::Rom(vol_base(0));
        self.bbs[1] = if vol == 0 {
            Page::Ram(0x4000)
        } else {
            Page::Nor(0x2000)
        };
        self.bbs[2] = Page::Rom(vol_base(0) + 0x4000);
        self.bbs[3] = Page::Rom(vol_base(0) + 0x6000);
        for i in 0..3 {
            let base = vol_base(i + 1);
            self.bbs[i * 4 + 4] = Page::Rom(base);
            self.bbs[i * 4 + 5] = Page::Rom(base + 0x2000);
            self.bbs[i * 4 + 6] = Page::Rom(base + 0x4000);
            self.bbs[i * 4 + 7] = Page::Rom(base + 0x6000);
        }

        // Bank window 0x4000-0xBFFF.
        let base = if roa {
            (bank as usize & 0x0F) * BANK_SIZE
        } else if vol == 0 {
            self.vol0[bank as usize]
        } else {
            self.vol1[bank as usize]
        };
        if bank == 0 && !roa {
            if vol == 0 {
                // 0x4000-0x7FFF maps onto internal RAM (0x6000 mirrors
                // 0x4000) so the firmware can copy itself into RAM.
                self.win[2] = Page::Ram(0x4000);
                self.win[3] = Page::Ram(0x4000);
            } else {
                // 0x4000-0x7FFF is NOR page 0.
                self.win[2] = Page::Nor(0x0000);
                self.win[3] = Page::Nor(0x2000);
            }
        } else {
            self.win[2] = Page::from_base(roa, base);
            self.win[3] = Page::from_base(roa, base + 0x2000);
        }
        self.win[4] = Page::from_base(roa, base + 0x4000);
        self.win[5] = Page::from_base(roa, base + 0x6000);

        let bbs_idx = (self.io[io_reg::BIOS_BSW] & 0x0F) as usize;
        self.win[6] = self.bbs[bbs_idx];
        // 0xE000-0xFFFF always shows volume bank 0 + 0x2000 (the BIOS).
        self.win[7] = Page::Rom(vol_base(0) + 0x2000);
    }

    /// Resolve a CPU address to a memory window (no IO side effects).
    fn map_addr(&self, addr: u16) -> Page {
        match addr {
            0x0000..=0x007F => Page::Ram(self.zp_off + (addr as usize & 0x3F)),
            0x0080..=0x3FFF => Page::Ram(addr as usize),
            0x4000..=0xFFFF => {
                let page = (addr >> 13) as usize;
                let off = (addr & 0x1FFF) as usize;
                match self.win[page] {
                    Page::Ram(b) => Page::Ram(b + off),
                    Page::Rom(b) => Page::Rom(b + off),
                    Page::Nor(b) => Page::Nor(b + off),
                }
            }
        }
    }

    fn peek_impl(&self, addr: u16) -> u8 {
        if (addr as usize) < 0x40 {
            return self.io[addr as usize];
        }
        match self.map_addr(addr) {
            Page::Ram(o) => self.ram[o],
            Page::Rom(o) => self.rom[o],
            Page::Nor(o) => self.nor[o],
        }
    }

    /// Execute a CPU read (with IO side effects).
    fn bus_read(&mut self, addr: u16) -> u8 {
        if (addr as usize) < 0x40 {
            return self.io_read(addr as u8);
        }
        match self.map_addr(addr) {
            Page::Ram(o) => self.ram[o],
            Page::Rom(o) => self.rom[o],
            Page::Nor(o) => self.nor_read(addr, o),
        }
    }

    /// Execute a CPU write.
    fn bus_write(&mut self, addr: u16, value: u8) {
        if (addr as usize) < 0x40 {
            self.io_write(addr as u8, value);
            return;
        }
        match self.map_addr(addr) {
            Page::Ram(o) => self.ram[o] = value,
            Page::Nor(o) => self.nor_write(addr, o, value),
            Page::Rom(_) => {
                // ROM pages are read-only.
            }
        }
    }

    /// IO register read.
    fn io_read(&mut self, addr: u8) -> u8 {
        let reg = addr as usize;
        match addr {
            0x01 => {
                let value = self.io[reg];
                self.io[reg] &= 0xC0;
                value
            }
            0x04 => {
                self.timer0run = false;
                self.io[reg]
            }
            0x05 => {
                self.timer0run = true;
                self.io[reg]
            }
            0x06 => {
                self.timer1run = false;
                self.io[reg]
            }
            0x07 => {
                self.timer1run = true;
                self.io[reg]
            }
            0x08 => {
                self.update_keypad_registers();
                self.r08_port0_id
            }
            0x09 => {
                self.update_keypad_registers();
                self.r09_port1_id
            }
            _ => self.io[reg],
        }
    }

    /// IO register write.
    fn io_write(&mut self, addr: u8, value: u8) {
        let reg = addr as usize;
        match addr {
            0x00 | 0x0A | 0x0D => {
                self.io[reg] = value;
                self.update_map();
            }
            0x01 => {
                // Interrupt enable register (shadow byte 0x40).
                self.io[io_reg::INT_ENABLE] = value;
            }
            0x02 => {
                self.io[reg] = value;
                self.tm0v = (value as u16 + self.tm0r).min(255);
                self.tm0r = 0;
            }
            0x03 => {
                self.io[reg] = value;
                self.tm1v2 = value as u16 * 2;
            }
            0x05 => {
                // Clock control: the low nibble drives the LCD; when the
                // firmware clears it the device enters standby.
                if self.io[reg] & 0x08 != 0 && value & 0x0F == 0 {
                    self.lcd_off = true;
                }
                self.io[reg] = value;
            }
            0x06 => {
                self.io[reg] = value;
                self.lcd_buff_addr = (value as u16) << 4;
            }
            0x07 | 0x19 => {
                self.io[reg] = value;
                self.check_play();
            }
            0x08 => {
                self.io[reg] = value;
                self.w08_port0_ol = value;
                self.io[io_reg::PORT0_OUT] = value;
                // LCD dot-out direction bit follows the hotkey rows.
                let row6data = self.hotkey_row_data(1);
                let row7data = self.hotkey_row_data(0);
                if row6data == value || value == 0 || row7data == 0xFB {
                    self.io[io_reg::LCD_CTRL] &= 0xFE;
                } else {
                    self.io[io_reg::LCD_CTRL] |= 0x01;
                }
                self.update_keypad_registers();
            }
            0x09 => {
                self.io[reg] = value;
                self.w09_port1_ol = value;
                // Rows 6/7 (hotkeys and power) are inverted and read
                // directly; the other rows go through the port
                // conduction model in update_keypad_registers().
                if value == 0x02 {
                    let data = self.hotkey_row_data(1);
                    self.w08_port0_ol = data;
                    self.io[io_reg::PORT0] = data;
                } else if value == 0x01 {
                    let data = self.hotkey_row_data(0);
                    self.w08_port0_ol = data;
                    self.io[io_reg::PORT0] = data;
                }
                self.update_keypad_registers();
            }
            0x0E => {
                // DAC data (record input); only stored.
                self.io[reg] = value;
            }
            0x0F => {
                self.io[reg] = value;
                let zp = value & 0x07;
                self.zp_off = match zp {
                    0 => 0x40,
                    1..=3 => 0x00,
                    v => 0x200 + (v as usize - 4) * 0x40,
                };
            }
            0x10 => {
                self.io[reg] = value;
                self.tma_reload = (self.tma_reload & 0xFF00) | value as u32;
            }
            0x11 => {
                self.io[reg] = value;
                self.tma_reload = (self.tma_reload & 0x00FF) | (value as u32) << 8;
                self.tma_value = self.tma_reload;
            }
            0x15 => {
                self.io[reg] = value;
                self.w15_port1_dir = value;
                self.update_keypad_registers();
            }
            0x18 => {
                self.io[reg] = value;
                if self.is_play_music {
                    self.music_sample = if value & 0x80 != 0 { 1 } else { -1 };
                }
            }
            0x20 => {
                // JG WAV control: 0x80/0x40 reset the waveform buffer.
                self.io[reg] = if value == 0x80 || value == 0x40 {
                    0
                } else {
                    value
                };
            }
            0x23 => {
                self.io[reg] = value;
            }
            _ => {
                self.io[reg] = value;
            }
        }
    }

    /// Hotkey row data for the inverted rows 6/7 (all released = 0).
    fn hotkey_row_data(&self, row: usize) -> u8 {
        let mut bits = 0;
        for col in 0..8 {
            if self.keypad_matrix[row][col] != 1 {
                bits |= 1 << col;
            }
        }
        if bits == 0xFF {
            0
        } else {
            bits
        }
    }

    /// Port conduction update for the 8x8 matrix (SPDC1016 keypad).
    fn update_keypad_registers(&mut self) {
        let port1_control = self.w15_port1_dir;
        let port0_control = self.io[io_reg::ZP_BSW] & 0xF0;
        let mut port1_control_bit = 1u8;
        let mut tmp_dest0 = 0u8;
        let mut tmp_dest1 = 0u8;
        let port1_data = self.w09_port1_ol;
        let port0_data = self.w08_port0_ol;

        for y in 0..8usize {
            let y_send = port1_control & port1_control_bit != 0;
            let mut x_bit = 1u8;
            for x in 0..8usize {
                let port0_control_bit = if x < 2 {
                    x_bit << 4
                } else if x < 4 {
                    0x40
                } else {
                    0x80
                };
                let key = self.keypad_matrix[y][x];
                if y < 2 && (port1_data == 0x02 || port1_data == 0x01) {
                    // Inverted hotkey/power rows.
                    if y_send {
                        if key != 1
                            && port1_data & port1_control_bit != 0
                            && port0_control & port0_control_bit == 0
                        {
                            tmp_dest0 |= x_bit;
                        }
                    } else if key != 1
                        && port0_data & x_bit != 0
                        && port0_control & port0_control_bit != 0
                    {
                        tmp_dest1 |= x_bit;
                    }
                } else if key != 2 {
                    if y_send {
                        if key != 0
                            && port1_data & port1_control_bit != 0
                            && port0_control & port0_control_bit == 0
                        {
                            tmp_dest0 |= x_bit;
                        }
                    } else if key != 0
                        && port0_data & x_bit != 0
                        && port0_control & port0_control_bit != 0
                    {
                        tmp_dest1 |= x_bit;
                    }
                }
                x_bit <<= 1;
            }
            port1_control_bit <<= 1;
        }

        let mut port1_data = self.w09_port1_ol;
        let mut port0_data = self.w08_port0_ol;
        if port1_control != 0xFF {
            port1_data &= port1_control;
        }
        if port1_control != 0xF0 {
            let mut port0_mask = (port0_control >> 4) & 0x03;
            if port0_control & 0x40 != 0 {
                port0_mask |= 0x0C;
            }
            if port0_control & 0x80 != 0 {
                port0_mask |= 0xF0;
            }
            port0_data &= port0_mask;
        }
        port0_data |= tmp_dest0;
        port1_data |= tmp_dest1;
        self.r09_port1_id = port1_data;
        self.r08_port0_id = port0_data;
    }

    /// Music playback enable check (port config 0x07 vs ctv 0x19).
    fn check_play(&mut self) {
        let play =
            self.io[io_reg::PORT_CONFIG] & 0x80 != 0 && self.io[io_reg::CTV_SELECT] & 0x80 == 0;
        self.is_play_music = play;
    }

    /// NOR read with command side effects (software ID, info block...).
    fn nor_read(&self, addr: u16, offset: usize) -> u8 {
        match self.fp_type {
            NorCmd::InfoRead if self.fp_step == 3 => self.nor_info_block[offset % 0x100],
            NorCmd::SwId if self.fp_step == 3 => match addr {
                0x8000 => 0xBF,
                0x8001 => 0xD7,
                _ => 0xFF,
            },
            NorCmd::PollStatus if self.fp_step == 3 => 0x88,
            NorCmd::ByteProgram if self.fp_step == 4 => 0x88,
            NorCmd::BlockOrMassErase if self.fp_step == 6 => 0x88,
            _ => self.nor[offset],
        }
    }

    /// NOR command sequence state machine.
    fn nor_write(&mut self, addr: u16, offset: usize, value: u8) {
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
                self.fp_step = if self.fp_type != NorCmd::None { 3 } else { 0 };
            }
            3 => match self.fp_type {
                NorCmd::ByteProgram => {
                    self.nor[offset] &= value;
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                }
                NorCmd::InfoByteProgram => {
                    self.nor_info_block[offset & 0xFF] &= value;
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
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
                    self.nor.fill(0xFF);
                    if self.fp_type == NorCmd::InfoOrBmassErase {
                        self.nor_info_block.fill(0xFF);
                    }
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                } else if self.fp_type == NorCmd::BlockOrMassErase && value == 0x30 {
                    // Block erase: 4KB aligned block.
                    let block = offset & !0xFFF;
                    let end = (block + 0x1000).min(self.nor.len());
                    self.nor[block..end].fill(0xFF);
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                } else if self.fp_type == NorCmd::InfoOrBmassErase && value == 0x30 {
                    self.nor_info_block.fill(0xFF);
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                }
            }
            _ => {}
        }

        if value == 0xF0 {
            self.fp_step = 0;
            self.fp_type = NorCmd::None;
        }
    }

    /// Advance the periodic timer/interrupt sources.
    fn tick_periodic(&mut self, cpu: &mut Cpu, delta: u64) {
        self.tick_cycles += delta;
        while self.tick_cycles >= TICK_CYCLES {
            self.tick_cycles -= TICK_CYCLES;
            self.tick_idx = self.tick_idx.wrapping_add(1);

            // Timer A (record/play timer).
            let temp = self.io[io_reg::TIMERAB_CTRL] >> 4;
            if temp != 0 {
                self.tma_value += 256 >> temp;
                if self.tma_value >= 0x10000 {
                    self.tma_value = self.tma_reload;
                    if self.io[io_reg::INT_ENABLE] & int_enable::TIMER_A != 0 {
                        self.io[io_reg::INT_STATUS] |= int_flags::TIMER_A;
                        cpu.irq_pending = true;
                    }
                }
            }

            // Timer 0 (+64 per tick, overflow every 4 ticks).
            if self.timer0run {
                self.tm0v += 64;
                if self.tm0v >= 256 {
                    self.tm0v -= 256;
                    self.tm0r = self.tm0v;
                    self.io[io_reg::INT_STATUS] |= int_flags::TIMER0;
                    cpu.irq_pending = true;
                }
            }

            // Timer 1 (+0.5 per tick, overflow every 512 ticks).
            if self.timer1run {
                self.tm1v2 = self.tm1v2.wrapping_add(1);
                if self.tm1v2 >= 512 {
                    self.tm1v2 = 0;
                    self.io[io_reg::INT_STATUS] |= int_flags::TIMER1;
                    cpu.irq_pending = true;
                }
            }

            // Time base (~250 Hz, used for keyboard scanning).
            if self.tick_idx % 115 == 30
                && self.io[io_reg::INT_ENABLE] & int_enable::TIME_BASE == 0
                && self.io[io_reg::GENERAL_CTRL] & 0x0F != 0
            {
                self.io[io_reg::INT_STATUS] |= int_flags::TIME_BASE;
                cpu.irq_pending = true;
            }
        }

        // NMI at 2 Hz (time update interrupt).
        self.nmi_cycles += delta;
        if self.nmi_cycles >= NMI_CYCLES {
            self.nmi_cycles -= NMI_CYCLES;
            if self.io[io_reg::INT_ENABLE] & int_enable::NMI == 0 {
                cpu.nmi_pending = true;
            }
        }
    }

    /// Watchdog warm reset (power key / hotkey while the LCD is off).
    fn warm_reset(&mut self, cpu: &mut Cpu) {
        self.do_warm_reset = false;
        self.lcd_off = false;
        self.io[io_reg::INT_STATUS] |= int_flags::TIMER_A;
        self.io[io_reg::TIMER0_VAL] |= 0x01;
        cpu.reset(self.peek_u16(crate::cpu::RESET_VECTOR));
        log::info!("CC800 warm reset (wake from standby)");
    }

    fn reset_machine(&mut self) {
        self.ram.fill(0);
        self.io.fill(0);
        self.zp_off = 0x40;
        self.lcd_buff_addr = 0x09C0;
        self.keypad_matrix = [[2; 8]; 8];
        self.w08_port0_ol = 0;
        self.r08_port0_id = 0;
        self.w09_port1_ol = 0;
        self.r09_port1_id = 0;
        self.w15_port1_dir = 0;
        self.timer0run = false;
        self.timer1run = false;
        self.tm0v = 0;
        self.tm0r = 0;
        self.tm1v2 = 0;
        self.tma_value = 0;
        self.tma_reload = 0;
        self.tick_idx = 0;
        self.tick_cycles = 0;
        self.nmi_cycles = 0;
        self.fp_step = 0;
        self.fp_type = NorCmd::None;
        self.is_play_music = false;
        self.music_sample = 1;
        self.lcd_off = false;
        self.do_warm_reset = false;
        self.audio.reset();
        self.lcd.reset();
        self.update_map();
    }
}

impl Page {
    /// Build a window target for the given storage kind.
    fn from_base(roa: bool, offset: usize) -> Page {
        if roa {
            Page::Nor(offset)
        } else {
            Page::Rom(offset)
        }
    }
}

impl Machine for Cc800Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Cc800
    }

    fn reset(&mut self) {
        self.reset_machine();
        log::info!("CC800 machine reset");
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        self.load_rom_impl(files)
    }

    fn set_key(&mut self, key_id: u8, pressed: bool) {
        let row = (key_id >> 3) as usize;
        let col = (key_id & 7) as usize;
        if row >= 8 || col >= 8 {
            return;
        }
        self.keypad_matrix[row][col] = if pressed { 1 } else { 0 };
        // A press on the power/hotkey rows wakes the device from the
        // LCD-off standby (watchdog reset).
        if pressed && row < 2 && self.lcd_off {
            self.do_warm_reset = true;
        }
        if pressed && row == 0 && col == 2 {
            // ON/OFF key arms the standby flag.
            self.lcd_off = true;
        }
    }

    fn is_sleeping(&self) -> bool {
        self.lcd_off
    }

    fn step(&mut self, cpu: &mut Cpu) -> u64 {
        if self.do_warm_reset {
            self.warm_reset(cpu);
        }
        let mut bus = Cc800Bus { machine: self };
        let cycles = cpu.step(&mut bus);
        self.tick_periodic(cpu, cycles);
        cycles
    }

    fn end_of_frame(&mut self, _cpu: &mut Cpu) {
        // Copy the LCD framebuffer from RAM at the configured address.
        if !self.lcd_off {
            let addr = (self.lcd_buff_addr as usize) & 0x3FFF;
            self.lcd.copy_from(&self.ram, addr);
        }
        let level = if self.is_play_music {
            self.music_sample * 8192
        } else {
            0
        };
        self.audio.output_frame(level);
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

    fn load_nor(&mut self, path: &std::path::Path) -> Result<()> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read CC800 NOR: {}", path.display()))?;
        if data.len() < NOR_SIZE {
            anyhow::bail!(
                "CC800 NOR file too small: expected {} bytes, got {}",
                NOR_SIZE,
                data.len()
            );
        }
        for off in (0..NOR_SIZE).step_by(BANK_SIZE) {
            self.nor[off..off + 0x4000].copy_from_slice(&data[off + 0x4000..off + 0x8000]);
            self.nor[off + 0x4000..off + 0x8000].copy_from_slice(&data[off..off + 0x4000]);
        }
        self.nor_info_block
            .copy_from_slice(&self.nor[0x0000..0x0100]);
        log::info!("CC800 NOR loaded: {}", path.display());
        Ok(())
    }

    fn save_nor(&self, path: &std::path::Path) -> Result<()> {
        let mut output = vec![0u8; NOR_SIZE];
        for off in (0..NOR_SIZE).step_by(BANK_SIZE) {
            output[off + 0x4000..off + 0x8000].copy_from_slice(&self.nor[off..off + 0x4000]);
            output[off..off + 0x4000].copy_from_slice(&self.nor[off + 0x4000..off + 0x8000]);
        }
        std::fs::write(path, &output)
            .with_context(|| format!("Failed to write CC800 NOR: {}", path.display()))?;
        log::info!("CC800 NOR saved: {}", path.display());
        Ok(())
    }

    fn save_state(&self, cpu: &Cpu) -> SaveState {
        SaveState::new(
            cpu,
            &self.ram,
            &self.io[..0x40],
            &crate::timer::Timer::new(),
            &crate::input::Input::new(),
            &self.audio,
            &crate::flash::Flash::new(),
            self.lcd_buff_addr as u32,
        )
    }

    fn load_state(&mut self, cpu: &mut Cpu, state: &SaveState) {
        *cpu = state.cpu.clone();
        if state.io.len() >= 0x40 {
            self.io[..0x40].copy_from_slice(&state.io[..0x40]);
        }
        self.io[0x40..].fill(0);
        if state.ram.len() >= RAM_SIZE {
            self.ram.copy_from_slice(&state.ram[..RAM_SIZE]);
        }
        self.lcd_buff_addr = state.lcd_addr as u16;
        self.lcd.lcd_addr = state.lcd_addr;
        self.zp_off = 0x40;
        self.timer0run = false;
        self.timer1run = false;
        self.tm0v = 0;
        self.tm0r = 0;
        self.tm1v2 = 0;
        self.tma_value = 0;
        self.tma_reload = 0;
        self.lcd_off = false;
        self.do_warm_reset = false;
        self.update_map();
    }

    fn save_persistent_state(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).context("Failed to serialize CC800 persistent state")
    }

    fn load_persistent_state(&mut self, data: &[u8]) -> Result<()> {
        let mut restored: Self =
            bincode::deserialize(data).context("Failed to deserialize CC800 persistent state")?;
        restored.rom = std::mem::take(&mut self.rom);
        *self = restored;
        Ok(())
    }

    fn persistent_state_identity(&self) -> u64 {
        crate::save::persistent_identity(&self.rom)
    }
}

/// Bus adapter connecting the shared CPU to CC800 hardware.
struct Cc800Bus<'a> {
    machine: &'a mut Cc800Machine,
}

impl<'a> CpuBus for Cc800Bus<'a> {
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

    fn empty_machine() -> Cc800Machine {
        Cc800Machine::new(&RomFiles::new(None, None, None, None)).unwrap()
    }

    /// Build RomFiles from the repository-root roms/ directory, or None if
    /// the dumps are not present (tests then skip).
    fn cc800_rom_files() -> Option<RomFiles> {
        let base = std::path::Path::new(r"E:\Code\WQXEmu\roms\cc800");
        let rom = base.join("obj.bin");
        let nor = base.join("cc800.fls");
        if rom.exists() && nor.exists() {
            Some(RomFiles::new(Some(rom), Some(nor), None, None))
        } else {
            None
        }
    }

    #[test]
    fn bank0_boot_maps_ram_and_rom() {
        let m = empty_machine();
        // Initial state: bank 0, ROA=0, volume 0.
        assert_eq!(m.map_addr(0x4000), Page::Ram(0x4000));
        assert_eq!(m.map_addr(0x6000), Page::Ram(0x4000));
        assert_eq!(m.map_addr(0x8000), Page::Rom(0x4000));
        assert_eq!(m.map_addr(0xA000), Page::Rom(0x6000));
        assert_eq!(m.map_addr(0xC000), Page::Rom(0x0000));
        assert_eq!(m.map_addr(0xE000), Page::Rom(0x2000));
        // Reset vector lives in the fixed BIOS page (bank 0 + 0x2000).
        assert_eq!(m.map_addr(0xFFFC), Page::Rom(0x3FFC));
    }

    #[test]
    fn bank_switch_selects_rom_bank() {
        let mut m = empty_machine();
        m.io_write(io_reg::BANK_SWITCH as u8, 0x01);
        // Bank 1: 0x4000 -> rom[0x8000], 0x8000 -> rom[0xC000].
        assert_eq!(m.map_addr(0x4000), Page::Rom(0x8000));
        assert_eq!(m.map_addr(0x8000), Page::Rom(0xC000));
        // Volume 1, bank 0 -> vol1[0] = offset 8MB.
        m.io_write(io_reg::VOLUME as u8, 0x01);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        assert_eq!(m.map_addr(0x8000), Page::Rom(0x800000 + 0x4000));
        // Volume 1 bank 0 maps 0x4000-0x7FFF onto NOR page 0.
        assert_eq!(m.map_addr(0x4000), Page::Nor(0x0000));
        assert_eq!(m.map_addr(0x6000), Page::Nor(0x2000));
    }

    #[test]
    fn roa_selects_nor_bank() {
        let mut m = empty_machine();
        m.io_write(io_reg::BIOS_BSW as u8, 0x80);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x03);
        assert_eq!(m.map_addr(0x4000), Page::Nor(3 * BANK_SIZE));
        assert_eq!(m.map_addr(0x8000), Page::Nor(3 * BANK_SIZE + 0x4000));
    }

    #[test]
    fn zero_page_switch() {
        let mut m = empty_machine();
        assert_eq!(m.map_addr(0x40), Page::Ram(0x40));
        m.io_write(io_reg::ZP_BSW as u8, 0x02);
        assert_eq!(m.map_addr(0x40), Page::Ram(0x00));
        m.io_write(io_reg::ZP_BSW as u8, 0x05);
        assert_eq!(m.map_addr(0x40), Page::Ram(0x240));
    }

    #[test]
    fn keyboard_scan_rows() {
        let mut m = empty_machine();
        // Firmware scan: port1 drives rows, port0 columns receive.
        m.io_write(io_reg::PORT1_DIR as u8, 0xFF);
        m.io_write(io_reg::PORT1 as u8, 0x20); // select row 5
        m.io_write(io_reg::ZP_BSW as u8, 0x00); // port0 all inputs
        m.set_key(5 << 3 | 2, true); // row 5, col 2
        m.update_keypad_registers();
        assert_eq!(m.r08_port0_id, 0x04); // column 2 conducted
        m.set_key(5 << 3 | 2, false);
        m.update_keypad_registers();
        assert_eq!(m.r08_port0_id, 0x00);

        // Hotkey row (row 1 = matrix[1]) via the inverted 0x02 scan.
        m.set_key(1 << 3 | 3, true);
        m.io_write(io_reg::PORT1 as u8, 0x02);
        let data = m.hotkey_row_data(1);
        assert_eq!(data & 0x08, 0); // pressed bit is cleared
        assert_ne!(data, 0xFF);
    }

    #[test]
    fn nor_byte_program() {
        let mut m = empty_machine();
        m.io_write(io_reg::BIOS_BSW as u8, 0x80);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0xA0);
        m.bus_write(0x4234, 0x5A); // 0x4234 -> nor[0x0000 + 0x0234]
        assert_eq!(m.nor[0x0234], 0x5A);
    }

    #[test]
    fn timer_start_stop_reads() {
        let mut m = empty_machine();
        m.io_write(io_reg::CLOCK_CTRL as u8, 0x77);
        // CC800 timer reads return the corresponding IO register.
        assert_eq!(m.bus_read(0x05), 0x77); // start timer0
        assert!(m.timer0run);
        m.io_write(io_reg::GENERAL_CTRL as u8, 0x55);
        assert_eq!(m.bus_read(0x04), 0x55); // stop timer0
        assert!(!m.timer0run);
    }

    #[test]
    fn boot_runs_real_firmware() {
        let Some(files) = cc800_rom_files() else {
            eprintln!("skipping: CC800 dumps not present");
            return;
        };
        let mut machine = Cc800Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));
        println!("reset vector: {:04X}", cpu.pc);
        assert_eq!(cpu.pc, 0xFFF4);

        let mut steps = 0u64;
        for _ in 0..3 {
            let target = crate::lcd::CYCLES_PER_FRAME;
            let mut acc = 0u64;
            while acc < target {
                let pc_before = cpu.pc;
                let c = machine.step(&mut cpu);
                assert!(c > 0, "step returned 0 at pc={:04X}", pc_before);
                acc += c;
                steps += 1;
            }
            machine.end_of_frame(&mut cpu);
        }
        println!("3 frames, {} instructions, pc={:04X}", steps, cpu.pc);
        assert!(cpu.pc != 0);
    }

    #[test]
    fn boot_draws_lcd() {
        let Some(files) = cc800_rom_files() else {
            eprintln!("skipping: CC800 dumps not present");
            return;
        };
        let mut machine = Cc800Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        let mut max_nonzero = 0;
        for frame in 0..120 {
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
            if frame % 20 == 0 {
                println!(
                    "frame {:3}: pc={:04X} lcd_nonzero={} bank={:02X} roa={:02X} vol={:02X}",
                    frame, cpu.pc, nz, machine.io[0x00], machine.io[0x0A], machine.io[0x0D]
                );
            }
        }
        println!("max LCD nonzero bytes: {}", max_nonzero);
        assert!(max_nonzero > 50, "boot should draw the logo/menu");
    }

    #[test]
    fn menu_responds_to_keys() {
        let Some(files) = cc800_rom_files() else {
            eprintln!("skipping: CC800 dumps not present");
            return;
        };
        let mut machine = Cc800Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        for _ in 0..100 {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
        }
        let before = machine.lcd.framebuffer_raw().to_vec();

        machine.set_key(6 << 3 | 3, true);
        for _ in 0..8 {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
        }
        machine.set_key(6 << 3 | 3, false);
        for _ in 0..8 {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
        }
        let after = machine.lcd.framebuffer_raw().to_vec();

        let diff = before
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a != b)
            .count();
        println!("LCD bytes changed after Down key: {}", diff);
        assert!(diff > 0, "pressing Down should change the menu selection");
    }
}
