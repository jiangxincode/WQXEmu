// PC1000 hardware machine implementation.
//
// The PC1000 is built around the SPDC1016-era 6502 SoC. Its ROM dump is
// stored linearly (no 16KB half swap, unlike the NC1020) and the bank
// window uses a different page order. The memory map and IO semantics
// follow the PC1000EMUX bus reference:
//
//  0x0000-0x003F  IO registers
//  0x0040-0x007F  zero-page switchable window (register 0x0F)
//  0x0080-0x1FFF  RAM page 0
//  0x2000-0x3FFF  RAM page 1
//  0x4000-0x5FFF  bank window page 0 (bank + 0x4000; RAM at boot)
//  0x6000-0x7FFF  bank window page 1 (bank + 0x6000; RAM mirror at boot)
//  0x8000-0x9FFF  bank window page 2 (bank + 0x0000)
//  0xA000-0xBFFF  bank window page 3 (bank + 0x2000)
//  0xC000-0xDFFF  BBS page (register 0x0A bits 0-3)
//  0xE000-0xFFFF  fixed BIOS page (volume bank 0 + 0x6000)
//
// ROM layout (12MB): obj1 + obj2 + obj3. Volume 0 covers obj1+obj2
// (8MB); volume 1 covers obj1 + obj3. NOR Flash is 512KB with 16 x 32KB
// banks and an SPR4096-style command interface (byte program, 4KB block
// erase, software ID, info block).

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
/// Canonical PC1000 ROM size (obj1 + obj2 + obj3, 12MB).
const ROM_SIZE_12M: usize = BANK_SIZE * 128 * 3;
/// ROM size accepted from the Android/PC1000EMUX layout (16MB buffer).
const ROM_SIZE_16M: usize = 0x1000000;
/// NOR Flash size (512KB).
const NOR_SIZE: usize = 0x80000;
/// Number of ROM banks per volume.
const NUM_ROM_BANKS: usize = 256;
/// Internal RAM size (8 pages x 8KB = 64KB).
const RAM_SIZE: usize = 0x10000;

/// PC1000 IO register indexes.
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
    pub const PORT3: usize = 0x0B;
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
    pub const VOLUME_SET: usize = 0x1A;
    pub const PORT0_OUT: usize = 0x41;
    pub const DSP_STAT: usize = 0x20;
    pub const DSP_RET_DATA: usize = 0x21;
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

/// PC1000 machine.
#[derive(Serialize, Deserialize)]
pub struct Pc1000Machine {
    ram: Vec<u8>,
    #[serde(skip)]
    rom: Vec<u8>,
    nor: Vec<u8>,
    #[serde(with = "BigArray")]
    nor_info_block: [u8; 0x100],
    /// NOR dump was stored half-swapped (swap back on save).
    nor_swapped: bool,
    #[serde(with = "BigArray")]
    io: [u8; 0x80],
    /// Volume bank base tables (bank index -> ROM offset).
    #[serde(with = "BigArray")]
    vol0: [usize; NUM_ROM_BANKS],
    #[serde(with = "BigArray")]
    vol1: [usize; NUM_ROM_BANKS],
    /// BBS page ROM offsets for the current volume.
    bbs: [usize; 16],
    /// Bank window pages 2-7.
    win: [Page; 8],
    /// Zero-page 0x40-0x7F base offset inside RAM.
    zp_off: usize,
    lcd_buff_addr: u16,

    // Keypad: key_posi[7-row] holds the pressed column bits.
    key_posi: [u8; 8],

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

    // DSP / audio
    dsp_sleep: bool,
    dsp_ret: u8,
    is_play_music: bool,
    music_sample: i16,

    nor_dirty: bool,
    audio: Audio,
    lcd: Lcd,
}

impl Pc1000Machine {
    /// Create the machine and load its ROM files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            ram: vec![0; RAM_SIZE],
            rom: Vec::new(),
            nor: vec![0xFF; NOR_SIZE],
            nor_info_block: [0; 0x100],
            nor_swapped: false,
            io: [0; 0x80],
            vol0: [0; NUM_ROM_BANKS],
            vol1: [0; NUM_ROM_BANKS],
            bbs: [0; 16],
            win: [Page::Ram(0); 8],
            zp_off: 0x40,
            lcd_buff_addr: 0x09C0,
            key_posi: [0; 8],
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
            dsp_sleep: false,
            dsp_ret: 0,
            is_play_music: false,
            music_sample: 1,
            nor_dirty: false,
            audio: Audio::new(),
            lcd: Lcd::new(),
        };
        machine.rom = vec![0; ROM_SIZE_12M];
        // Default to the canonical 12MB volume layout; load_rom_impl
        // reinitializes the tables when a dump is actually loaded.
        for b in 0..NUM_ROM_BANKS {
            machine.vol0[b] = b * BANK_SIZE;
        }
        for b in 0..128 {
            machine.vol1[b] = b * BANK_SIZE;
        }
        for b in 0..128 {
            machine.vol1[128 + b] = (256 + b) * BANK_SIZE;
        }
        machine.update_map();
        machine.load_rom(files)?;
        Ok(machine)
    }

    /// Load ROM and NOR files.
    fn load_rom_impl(&mut self, files: &RomFiles) -> Result<()> {
        if let Some(rom_path) = &files.rom {
            let data = std::fs::read(rom_path)
                .with_context(|| format!("Failed to read PC1000 ROM: {}", rom_path.display()))?;
            match data.len() {
                ROM_SIZE_12M => {
                    self.rom = data;
                    for b in 0..NUM_ROM_BANKS {
                        self.vol0[b] = b * BANK_SIZE;
                    }
                    for b in 0..128 {
                        // Volume 1 banks 0-127 share obj1 with volume 0.
                        self.vol1[b] = b * BANK_SIZE;
                    }
                    for b in 0..128 {
                        // Volume 1 banks 128-255 are obj3 (banks 256-383).
                        self.vol1[128 + b] = (256 + b) * BANK_SIZE;
                    }
                }
                ROM_SIZE_16M => {
                    // Android/PC1000EMUX layout: obj1 at 0, obj2 at 4MB,
                    // obj3 at 12MB (8-12MB unused).
                    self.rom = data;
                    for b in 0..NUM_ROM_BANKS {
                        self.vol0[b] = b * BANK_SIZE;
                    }
                    for b in 0..128 {
                        // Volume 1 banks 0-127 are unmapped in this layout.
                        self.vol1[b] = 0x800000;
                    }
                    for b in 0..128 {
                        self.vol1[128 + b] = (384 + b) * BANK_SIZE;
                    }
                }
                other => {
                    anyhow::bail!(
                        "PC1000 ROM size must be {} or {} bytes, got {}",
                        ROM_SIZE_12M,
                        ROM_SIZE_16M,
                        other
                    );
                }
            }
            log::info!(
                "PC1000 ROM loaded: {} bytes, reset vector 0x{:04X}",
                self.rom.len(),
                self.peek_u16(crate::cpu::RESET_VECTOR)
            );
        }
        if let Some(nor_path) = &files.nor {
            let data = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read PC1000 NOR: {}", nor_path.display()))?;
            if data.len() < NOR_SIZE {
                anyhow::bail!(
                    "PC1000 NOR file too small: expected {} bytes, got {}",
                    NOR_SIZE,
                    data.len()
                );
            }
            let n = self.nor.len().min(data.len());
            self.nor[..n].copy_from_slice(&data[..n]);
            self.nor_swapped = false;

            // Some dumps store each 32KB bank half-swapped (the NC1020
            // convention). Detect it from the info-block signature at
            // bank 0 offset 0x4000 ("*BD F0 D4 B6 BC FB") and swap back.
            let sig = [0x2A, 0xBD, 0xF0, 0xD4, 0xB6, 0xBC, 0xFB];
            if self.nor[0x4000..0x4007] != sig && self.nor[0x0000..0x0007] == sig {
                for off in (0..NOR_SIZE).step_by(BANK_SIZE) {
                    let mut bank = [0u8; BANK_SIZE];
                    bank[0x0000..0x4000].copy_from_slice(&self.nor[off + 0x4000..off + 0x8000]);
                    bank[0x4000..0x8000].copy_from_slice(&self.nor[off..off + 0x4000]);
                    self.nor[off..off + BANK_SIZE].copy_from_slice(&bank);
                }
                self.nor_swapped = true;
                log::debug!("PC1000 NOR: detected half-swapped dump, unswapped on load");
            }
            log::info!("PC1000 NOR loaded: {} bytes", self.nor.len());
        }
        // The physical NOR info block lives at bank 0 offset 0x4000.
        self.nor_info_block
            .copy_from_slice(&self.nor[0x4000..0x4100]);
        Ok(())
    }

    /// Recompute bank windows and BBS pages from the current registers.
    fn update_map(&mut self) {
        let bank = self.io[io_reg::BANK_SWITCH];
        let vol = self.io[io_reg::VOLUME] & 0x01;
        let roa = self.io[io_reg::BIOS_BSW] & 0x80 != 0;

        if roa {
            // NOR window: bank selects one of the 16 x 32KB NOR banks.
            let nb = (bank as usize & 0x0F) * BANK_SIZE;
            self.win[2] = Page::Nor(nb + 0x4000);
            self.win[3] = Page::Nor(nb + 0x6000);
            self.win[4] = Page::Nor(nb);
            self.win[5] = Page::Nor(nb + 0x2000);
        } else {
            // BROM window. Volume 1 mirrors volume 0 for banks 0-127 and
            // switches to obj3 for banks 128-255.
            let base = if vol == 0 {
                self.vol0[bank as usize]
            } else {
                self.vol1[bank as usize]
            };
            if bank == 0 {
                // Bank 0 maps 0x4000-0x7FFF onto internal RAM so the
                // firmware can copy itself into RAM after reset.
                self.win[2] = Page::Ram(0x4000);
                self.win[3] = Page::Ram(0x4000);
            } else {
                self.win[2] = Page::Rom(base + 0x4000);
                self.win[3] = Page::Rom(base + 0x6000);
            }
            self.win[4] = Page::Rom(base);
            self.win[5] = Page::Rom(base + 0x2000);
        }

        // BBS pages: banks 0-3 of the active volume, each split into
        // {+0x4000, +0x6000, +0x0000, +0x2000}.
        for i in 0..4 {
            let base = if vol == 0 { self.vol0[i] } else { self.vol1[i] };
            self.bbs[i * 4] = base + 0x4000;
            self.bbs[i * 4 + 1] = base + 0x6000;
            self.bbs[i * 4 + 2] = base;
            self.bbs[i * 4 + 3] = base + 0x2000;
        }

        let bbs_idx = (self.io[io_reg::BIOS_BSW] & 0x0F) as usize;
        self.win[6] = if bbs_idx == 1 {
            // BBS page 1 is internal RAM (0x4000-0x5FFF).
            Page::Ram(0x4000)
        } else {
            Page::Rom(self.bbs[bbs_idx])
        };
        // 0xE000-0xFFFF always shows volume bank 0 + 0x6000 (the BIOS).
        self.win[7] = Page::Rom(self.bbs[1]);
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
                // Reading the interrupt status clears the flags (the
                // upper two bits are latched and preserved).
                self.io[reg] &= 0xC0;
                value
            }
            0x04 => {
                self.timer0run = false;
                self.tm0v as u8
            }
            0x05 => {
                self.timer0run = true;
                self.tm0v as u8
            }
            0x06 => {
                self.timer1run = false;
                (self.tm1v2 / 2) as u8
            }
            0x07 => {
                self.timer1run = true;
                (self.tm1v2 / 2) as u8
            }
            0x0B => self.read_port3(),
            0x20 => {
                // DSP status: sleep flag + busy + transfer bit.
                let busy = if self.is_play_music { 0x30 } else { 0 };
                (if self.dsp_sleep { 0x80 } else { 0 }) | busy | 0x40
            }
            0x21 => {
                let ret = self.dsp_ret;
                self.dsp_ret = if ret == 0x5A { 0xFF } else { 0 };
                ret
            }
            _ => self.io[reg],
        }
    }

    /// IO register write.
    fn io_write(&mut self, addr: u8, value: u8) {
        let reg = addr as usize;
        match addr {
            0x00 => {
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
            0x07 | 0x19 => {
                self.io[reg] = value;
                self.check_play();
            }
            0x08 => {
                self.io[reg] = value;
                self.io[io_reg::PORT0_OUT] = value;
            }
            0x09 => {
                self.io[reg] = value;
                // Scan the keyboard: the selected port1 line drives the
                // corresponding matrix row into port0.
                let row = match value {
                    0x80 => Some(0),
                    0x40 => Some(1),
                    0x20 => Some(2),
                    0x10 => Some(3),
                    0x08 => Some(4),
                    0x04 => Some(5),
                    _ => None,
                };
                if let Some(row) = row {
                    self.io[io_reg::PORT0] = self.key_posi[row];
                }
            }
            0x0A => {
                self.io[reg] = value;
                self.update_map();
            }
            0x0D => {
                self.io[reg] = value;
                self.update_map();
            }
            0x06 => {
                self.io[reg] = value;
                // The PC1000 LCD controller address is driven solely by
                // the 0x06 register (address << 4); 0x0C is a timer/LCD
                // control register whose low bits are not part of the
                // framebuffer address on this model.
                self.lcd_buff_addr = (value as u16) << 4;
            }
            0x0C => {
                self.io[reg] = value;
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
            0x18 => {
                self.io[reg] = value;
                if self.is_play_music {
                    self.music_sample = if value & 0x80 != 0 { 1 } else { -1 };
                }
            }
            0x20 => {
                // DSP control: 0x40 reset / 0x80 wake-up.
                if value == 0x40 || value == 0x80 {
                    self.dsp_sleep = false;
                }
                self.io[reg] = value;
            }
            0x22 => {
                self.io[reg] = value;
            }
            0x23 => {
                self.io[reg] = value;
                let cmd = (value as u16) << 8 | self.io[io_reg::DSP_DATA_LOW] as u16;
                self.dsp_cmd(cmd);
            }
            _ => {
                self.io[reg] = value;
            }
        }
    }

    /// Port 3 keyboard read (power key and hotkey rows).
    fn read_port3(&self) -> u8 {
        let p0_dir = self.io[io_reg::ZP_BSW] & 0xF0;
        let p1_dir = self.io[io_reg::PORT1_DIR] & 0x03;
        let mut b0 = 1;
        if p0_dir == 0xF0 {
            let port0 = self.io[io_reg::PORT0_OUT];
            b0 = match port0 {
                0x00 => (self.key_posi[6] == 0 && self.key_posi[7] & 0x01 == 0) as u8,
                0x7F => (self.key_posi[6] & 0x80 == 0) as u8,
                0xBF => (self.key_posi[6] & 0x40 == 0) as u8,
                0xDF => (self.key_posi[6] & 0x20 == 0) as u8,
                0xEF => (self.key_posi[6] & 0x10 == 0) as u8,
                0xF7 => (self.key_posi[6] & 0x08 == 0) as u8,
                0xFB => (self.key_posi[6] & 0x04 == 0) as u8,
                0xFD => (self.key_posi[6] & 0x02 == 0) as u8,
                0xFE => (self.key_posi[6] & 0x01 == 0) as u8,
                0xFF => (self.key_posi[7] & 0x01 == 0) as u8,
                _ => 1,
            };
        }
        if b0 == 1 && p1_dir == 3 {
            let p1 = self.io[io_reg::PORT1];
            b0 = if p1 == 0xFE {
                if self.key_posi[7] & 0x02 == 0 {
                    1
                } else {
                    0
                }
            } else if p1 == 0xFD {
                if self.key_posi[7] & 0x04 == 0 {
                    1
                } else {
                    0
                }
            } else if p1 & 0x03 == 0 {
                if self.key_posi[7] & 0x06 == 0 {
                    1
                } else {
                    0
                }
            } else {
                b0
            };
        }
        (self.io[io_reg::PORT3] & 0xFE) | b0
    }

    /// Music playback enable check (port config 0x07 vs ctv 0x19).
    fn check_play(&mut self) {
        let play =
            self.io[io_reg::PORT_CONFIG] & 0x80 != 0 && self.io[io_reg::CTV_SELECT] & 0x80 == 0;
        self.is_play_music = play;
    }

    /// DSP command decoder.
    fn dsp_cmd(&mut self, cmd: u16) {
        match cmd {
            0x8000 => {
                // DSP sleep.
                self.dsp_sleep = true;
            }
            0xD001 => {
                // Status query; the next read of 0x21 returns 0x5A.
                self.dsp_ret = 0x5A;
            }
            _ => {}
        }
        if cmd == 0xFFFF {
            self.dsp_sleep = false;
        }
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
                    self.nor_dirty = true;
                    // PC1000 firmware polls the written value directly;
                    // the command state resets immediately.
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                }
                NorCmd::InfoByteProgram => {
                    self.nor_info_block[offset & 0xFF] &= value;
                    self.nor_dirty = true;
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
                    // Mass erase: all 16 NOR banks.
                    self.nor.fill(0xFF);
                    if self.fp_type == NorCmd::InfoOrBmassErase {
                        self.nor_info_block.fill(0xFF);
                    }
                    self.nor_dirty = true;
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                } else if self.fp_type == NorCmd::BlockOrMassErase && value == 0x30 {
                    // Block erase: 4KB aligned block (PC1000).
                    let block = offset & !0xFFF;
                    let end = (block + 0x1000).min(self.nor.len());
                    self.nor[block..end].fill(0xFF);
                    self.nor_dirty = true;
                    self.fp_step = 0;
                    self.fp_type = NorCmd::None;
                } else if self.fp_type == NorCmd::InfoOrBmassErase && value == 0x30 {
                    self.nor_info_block.fill(0xFF);
                    self.nor_dirty = true;
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

    fn reset_machine(&mut self) {
        self.ram.fill(0);
        self.io.fill(0);
        self.zp_off = 0x40;
        self.lcd_buff_addr = 0x09C0;
        self.key_posi = [0; 8];
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
        self.dsp_sleep = false;
        self.dsp_ret = 0;
        self.is_play_music = false;
        self.music_sample = 1;
        self.nor_dirty = false;
        self.audio.reset();
        self.lcd.reset();
        self.update_map();
    }
}

impl Machine for Pc1000Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Pc1000
    }

    fn reset(&mut self) {
        self.reset_machine();
        log::info!("PC1000 machine reset");
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        self.load_rom_impl(files)
    }

    fn set_key(&mut self, key_id: u8, pressed: bool) {
        // key_id encodes matrix position: row = key_id >> 3, col = key_id & 7.
        let row = (key_id >> 3) as usize;
        let col = (key_id & 7) as usize;
        if row >= 8 || col >= 8 {
            return;
        }
        let bit = 1u8 << col;
        let pos = 7 - row;
        if pressed {
            self.key_posi[pos] |= bit;
        } else {
            self.key_posi[pos] &= !bit;
        }
    }

    fn is_sleeping(&self) -> bool {
        false
    }

    fn step(&mut self, cpu: &mut Cpu) -> u64 {
        let mut bus = Pc1000Bus { machine: self };
        let cycles = cpu.step(&mut bus);
        self.tick_periodic(cpu, cycles);
        cycles
    }

    fn end_of_frame(&mut self, _cpu: &mut Cpu) {
        // Copy the LCD framebuffer from RAM at the configured address.
        let addr = (self.lcd_buff_addr as usize) & 0x3FFF;
        self.lcd.copy_from(&self.ram, addr);
        // Fill one frame of audio (square wave while the beeper plays).
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
            .with_context(|| format!("Failed to read PC1000 NOR: {}", path.display()))?;
        if data.len() < NOR_SIZE {
            anyhow::bail!(
                "PC1000 NOR file too small: expected {} bytes, got {}",
                NOR_SIZE,
                data.len()
            );
        }
        self.nor[..NOR_SIZE].copy_from_slice(&data[..NOR_SIZE]);
        self.nor_dirty = false;
        log::info!("PC1000 NOR loaded: {}", path.display());
        Ok(())
    }

    fn save_nor(&self, path: &std::path::Path) -> Result<()> {
        let mut output = self.nor.clone();
        if self.nor_swapped {
            // Write back in the original half-swapped dump format.
            for off in (0..NOR_SIZE).step_by(BANK_SIZE) {
                let mut bank = [0u8; BANK_SIZE];
                bank[0x0000..0x4000].copy_from_slice(&self.nor[off + 0x4000..off + 0x8000]);
                bank[0x4000..0x8000].copy_from_slice(&self.nor[off..off + 0x4000]);
                output[off..off + BANK_SIZE].copy_from_slice(&bank);
            }
        }
        std::fs::write(path, &output)
            .with_context(|| format!("Failed to write PC1000 NOR: {}", path.display()))?;
        log::info!("PC1000 NOR saved: {}", path.display());
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
        // The interrupt-enable shadow (0x40) is not part of the visible
        // IO window; rebuild the other dynamic state from scratch.
        self.io[0x40..].fill(0);
        if state.ram.len() >= RAM_SIZE {
            self.ram.copy_from_slice(&state.ram[..RAM_SIZE]);
        }
        self.lcd_buff_addr = state.lcd_addr as u16;
        self.lcd.lcd_addr = state.lcd_addr;
        self.zp_off = 0x40;
        self.key_posi = [0; 8];
        self.timer0run = false;
        self.timer1run = false;
        self.tm0v = 0;
        self.tm0r = 0;
        self.tm1v2 = 0;
        self.tma_value = 0;
        self.tma_reload = 0;
        self.dsp_sleep = false;
        self.dsp_ret = 0;
        self.is_play_music = false;
        self.update_map();
    }

    fn save_persistent_state(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).context("Failed to serialize PC1000 persistent state")
    }

    fn load_persistent_state(&mut self, data: &[u8]) -> Result<()> {
        let mut restored: Self =
            bincode::deserialize(data).context("Failed to deserialize PC1000 persistent state")?;
        restored.rom = std::mem::take(&mut self.rom);
        *self = restored;
        Ok(())
    }

    fn persistent_state_identity(&self) -> u64 {
        crate::save::persistent_identity(&self.rom)
    }
}

/// Bus adapter connecting the shared CPU to PC1000 hardware.
struct Pc1000Bus<'a> {
    machine: &'a mut Pc1000Machine,
}

impl<'a> CpuBus for Pc1000Bus<'a> {
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

    fn empty_machine() -> Pc1000Machine {
        Pc1000Machine::new(&RomFiles::new(None, None, None, None)).unwrap()
    }

    /// Build RomFiles from the repository-root roms/ directory, or None if
    /// the dumps are not present (tests then skip).
    fn pc1000_rom_files() -> Option<RomFiles> {
        let base = std::path::Path::new(r"E:\Code\WQXEmu\roms\pc1000");
        let rom = base.join("pc1000.rom");
        let nor = base.join("pc1000.fls");
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
        assert_eq!(m.map_addr(0x8000), Page::Rom(0x0000));
        assert_eq!(m.map_addr(0xA000), Page::Rom(0x2000));
        assert_eq!(m.map_addr(0xC000), Page::Rom(0x4000));
        assert_eq!(m.map_addr(0xE000), Page::Rom(0x6000));
        // Reset vector lives in the fixed BIOS page (bank 0 + 0x6000).
        assert_eq!(m.map_addr(0xFFFC), Page::Rom(0x7FFC));
    }

    #[test]
    fn bank_switch_selects_rom_bank() {
        let mut m = empty_machine();
        m.io_write(io_reg::BANK_SWITCH as u8, 0x01);
        // Bank 1: 0x4000 -> rom[0x8000 + 0x4000], 0x8000 -> rom[0x8000].
        assert_eq!(m.map_addr(0x4000), Page::Rom(0xC000));
        assert_eq!(m.map_addr(0x8000), Page::Rom(0x8000));
        // Volume 1, bank 0x80 -> obj3 bank 128 (offset 8MB).
        m.io_write(io_reg::VOLUME as u8, 0x01);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x80);
        assert_eq!(m.map_addr(0x8000), Page::Rom(0x800000));
    }

    #[test]
    fn roa_selects_nor_bank() {
        let mut m = empty_machine();
        m.io_write(io_reg::BIOS_BSW as u8, 0x80);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x03);
        assert_eq!(m.map_addr(0x4000), Page::Nor(3 * BANK_SIZE + 0x4000));
        assert_eq!(m.map_addr(0x8000), Page::Nor(3 * BANK_SIZE));
    }

    #[test]
    fn zero_page_switch() {
        let mut m = empty_machine();
        // Default: 0x40-0x7F maps to RAM 0x40-0x7F.
        assert_eq!(m.map_addr(0x40), Page::Ram(0x40));
        // Value 2: maps to RAM 0x00-0x3F.
        m.io_write(io_reg::ZP_BSW as u8, 0x02);
        assert_eq!(m.map_addr(0x40), Page::Ram(0x00));
        // Value 5: maps to RAM 0x240-0x27F.
        m.io_write(io_reg::ZP_BSW as u8, 0x05);
        assert_eq!(m.map_addr(0x40), Page::Ram(0x240));
    }

    #[test]
    fn keyboard_scan() {
        let mut m = empty_machine();
        m.set_key(5 << 3 | 2, true); // row 5, col 2
        m.io_write(io_reg::PORT1 as u8, 0x20); // select row 5 -> key_posi[2]
        assert_eq!(m.io[io_reg::PORT0], 0x04);
        m.set_key(5 << 3 | 2, false);
        m.io_write(io_reg::PORT1 as u8, 0x20);
        assert_eq!(m.io[io_reg::PORT0], 0x00);
    }

    #[test]
    fn nor_software_id() {
        let mut m = empty_machine();
        m.io_write(io_reg::BIOS_BSW as u8, 0x80);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0x90);
        assert_eq!(m.fp_type, NorCmd::SwId);
        assert_eq!(m.bus_read(0x8000), 0xBF);
        assert_eq!(m.bus_read(0x8001), 0xD7);
        m.bus_write(0x5555, 0xF0);
        assert_eq!(m.fp_type, NorCmd::None);
    }

    #[test]
    fn nor_byte_program_and_block_erase() {
        let mut m = empty_machine();
        m.io_write(io_reg::BIOS_BSW as u8, 0x80);
        m.io_write(io_reg::BANK_SWITCH as u8, 0x00);
        // Byte program: AA 55 A0 data.
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0xA0);
        m.bus_write(0x5234, 0x5A); // 0x5234 -> nor[0x4000 + 0x1234]
        assert_eq!(m.nor[0x5234], 0x5A);

        // 4KB block erase: AA 55 80 AA 55 30.
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x5555, 0x80);
        m.bus_write(0x5555, 0xAA);
        m.bus_write(0xAAAA, 0x55);
        m.bus_write(0x4234, 0x30); // erase block at nor[0x4000..0x4FFF]
        assert_eq!(m.nor[0x4234], 0xFF);
        assert_eq!(m.nor[0x5234], 0x5A); // next block untouched
    }

    #[test]
    fn nor_save_restores_linear_dump() {
        let mut m = empty_machine();
        m.nor[0x1234] = 0xAB;
        let dir = std::env::temp_dir().join("wqxemu_pc1000_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.fls");
        m.save_nor(&path).unwrap();

        let mut n = empty_machine();
        n.load_nor(&path).unwrap();
        assert_eq!(n.nor[0x1234], 0xAB);
        assert!(!n.nor_swapped);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn timer_start_stop_reads() {
        let mut m = empty_machine();
        m.io_write(io_reg::TIMER0_VAL as u8, 0x10);
        // Reading 0x05 starts timer0 and returns the current count.
        assert_eq!(m.bus_read(0x05), 0x10);
        assert!(m.timer0run);
        assert_eq!(m.bus_read(0x04), 0x10);
        assert!(!m.timer0run);
    }

    #[test]
    fn interrupt_status_read_clears() {
        let mut m = empty_machine();
        m.io[io_reg::INT_STATUS] = 0x38;
        assert_eq!(m.bus_read(0x01), 0x38);
        assert_eq!(m.io[io_reg::INT_STATUS], 0x00);
        // High bits are preserved.
        m.io[io_reg::INT_STATUS] = 0xC1;
        assert_eq!(m.bus_read(0x01), 0xC1);
        assert_eq!(m.io[io_reg::INT_STATUS], 0xC0);
    }

    #[test]
    fn boot_runs_real_firmware() {
        let Some(files) = pc1000_rom_files() else {
            eprintln!("skipping: PC1000 dumps not present");
            return;
        };
        let mut machine = Pc1000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));
        println!("reset vector: {:04X}", cpu.pc);
        assert_eq!(cpu.pc, 0xFFF4);

        // Run several frames; the CPU must keep executing instructions.
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
        let Some(files) = pc1000_rom_files() else {
            eprintln!("skipping: PC1000 dumps not present");
            return;
        };
        let mut machine = Pc1000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        let mut max_nonzero = 0;
        let mut last_pc = 0;
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
            if frame % 10 == 0 {
                println!(
                    "frame {:3}: pc={:04X} lcd_nonzero={} bank={:02X} roa={:02X} vol={:02X} io06={:02X} io0c={:02X} lcdaddr={:04X}",
                    frame,
                    cpu.pc,
                    nz,
                    machine.io[0x00],
                    machine.io[0x0A],
                    machine.io[0x0D],
                    machine.io[0x06],
                    machine.io[0x0C],
                    machine.lcd_buff_addr
                );
            }
            last_pc = cpu.pc;
        }
        println!("max LCD nonzero bytes: {}", max_nonzero);
        println!("final pc: {:04X}", last_pc);
        assert!(max_nonzero > 50, "boot should draw the logo/menu");
    }

    #[test]
    fn menu_responds_to_keys() {
        let Some(files) = pc1000_rom_files() else {
            eprintln!("skipping: PC1000 dumps not present");
            return;
        };
        let mut machine = Pc1000Machine::new(&files).unwrap();
        machine.reset();
        let mut cpu = Cpu::new();
        cpu.reset(machine.peek_u16(crate::cpu::RESET_VECTOR));

        // Boot to the menu.
        for _ in 0..100 {
            let mut acc = 0u64;
            while acc < crate::lcd::CYCLES_PER_FRAME {
                acc += machine.step(&mut cpu);
            }
            machine.end_of_frame(&mut cpu);
        }
        let before = machine.lcd.framebuffer_raw().to_vec();

        // Press Down (row 6, col 3) for a few frames, then release.
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
