// Memory system with bank switching for NC1020.
//
// Memory map:
//   0x0000-0x003F: IO registers (handled by io.rs)
//   0x0040-0x007F: Zero page bank-switchable area (controlled by register 0x0F)
//   0x0080-0x1FFF: Zero page + Stack + RAM page 0
//   0x2000-0x3FFF: RAM page 1 (or page 2 based on register 0x0A bit2)
//   0x4000-0x5FFF: Bank window page 0 (switchable via register 0x00)
//   0x6000-0x7FFF: Bank window page 1
//
// Bank switching:
//   Register 0x00: Selects current bank (0x00-0x1F = NOR, 0x80-0xFF = ROM)
//   Register 0x0A: ROA/BBS control (bit[0:3] selects bbs_pages for 0x6000)
//   Register 0x0D: Volume select (bit0=vol1, bit1=vol2)
//   Register 0x0F: Zero page 0x40 area switch

use serde::{Deserialize, Serialize};

/// Total RAM size (32KB)
pub const RAM_SIZE: usize = 0x8000;
/// ROM bank size (32KB)
pub const BANK_SIZE: usize = 0x8000;
/// ROM size: 512KB x 3 volumes = 1.5MB
pub const ROM_SIZE: usize = 0x8000 * 0x300;
/// NOR Flash size: 512KB (32 banks x 32KB)
pub const NOR_SIZE: usize = 0x8000 * 0x20;
/// Number of ROM volumes
pub const ROM_VOLUME_COUNT: usize = 3;
/// ROM volume size (512KB = 128 banks x 4KB? No, 128 banks x 32KB = 4MB... let me recalc)
/// Actually ROM_SIZE = 0x8000 * 0x300 = 32KB * 768 = 24MB? That seems too large.
/// Looking at the reference: rom_volume0/1/2 each have 0x100 entries of 0x8000 bytes
/// So each volume = 256 * 32KB = 8MB, total = 24MB? That's huge.
/// Let me re-read: ROM_SIZE = 0x8000 * 0x300 = 32KB * 768 = 24MB
/// But the volumes are indexed 0x80-0xFF (128 entries), so 128 * 32KB = 4MB per volume
/// The reference code has ROM_SIZE = 0x8000 * 0x300 which is 768 banks of 32KB
/// Volume 0: banks 0x00-0xFF (256 banks), Volume 1: 0x100-0x1FF, Volume 2: 0x200-0x2FF
/// But only banks 0x80-0xFF are accessed (128 banks per volume)
/// So effective ROM per volume = 128 * 32KB = 4MB
///
/// For simplicity, let's store the full ROM and index into it.
pub const ROM_BANKS: usize = 0x300; // 768 banks total (256 per volume)

/// Number of NOR banks
pub const NOR_BANKS: usize = 0x20; // 32 banks

/// Memory page size (8KB)
pub const PAGE_SIZE: usize = 0x2000;

/// IO register limit (addresses below this are IO)
pub const IO_LIMIT: u16 = 0x40;

/// Memory system state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    /// Main RAM (32KB)
    pub ram: Vec<u8>,
    /// IO registers (64 bytes, at 0x0000-0x003F)
    pub io: Vec<u8>,
    /// Backup for zero page switch (register 0x0F)
    pub bak_40: Vec<u8>,
    /// Current memory page mapping (8 pages of 8KB each)
    page_map: [u8; 8],
    /// BBS pages (16 entries, each pointing to a RAM/ROM page)
    pub bbs_pages: Vec<Vec<u8>>,
    /// Current bank index (from register 0x00)
    current_bank: u8,
    /// Current volume index (from register 0x0D)
    current_volume: u8,
    /// Current ROA/BBS value (from register 0x0A)
    current_roa_bbs: u8,
    /// Current zero page switch value (from register 0x0F)
    current_zp_switch: u8,
    /// Whether bank region is writable (NOR Flash)
    bank_writable: bool,
}

impl Memory {
    /// Create a new memory system
    pub fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            io: vec![0; 0x40],
            bak_40: vec![0; 0x40],
            page_map: [0; 8],
            bbs_pages: vec![vec![0; PAGE_SIZE]; 16],
            current_bank: 0,
            current_volume: 0,
            current_roa_bbs: 0,
            current_zp_switch: 0,
            bank_writable: false,
        }
    }

    /// Reset memory to initial state
    pub fn reset(&mut self) {
        self.ram.fill(0);
        self.io.fill(0);
        self.bak_40.fill(0);
        self.current_bank = 0;
        self.current_volume = 0;
        self.current_roa_bbs = 0;
        self.current_zp_switch = 0;
        self.bank_writable = false;
    }

    /// Read a byte from the address (without IO interception)
    pub fn peek(&self, addr: u16) -> u8 {
        let page = (addr >> 13) as usize;
        let offset = (addr & 0x1FFF) as usize;
        match page {
            0 => self.ram[offset],
            1 => self.ram[0x2000 + offset],
            _ => {
                // For banked regions, we need to look up the actual bank
                // This is handled by the emulator's read() method
                0
            }
        }
    }

    /// Write a byte to the address (without IO interception)
    pub fn poke(&mut self, addr: u16, value: u8) {
        let page = (addr >> 13) as usize;
        let offset = (addr & 0x1FFF) as usize;
        match page {
            0 => self.ram[offset] = value,
            1 => self.ram[0x2000 + offset] = value,
            _ => {} // Bank writes handled by emulator
        }
    }

    /// Get the zero page 0x40-0x7F area based on register 0x0F
    pub fn get_zp40_ptr(&self, index: u8) -> usize {
        match index & 0x07 {
            0 => 0x0040, // RAM[0x40]
            1 => 0x00C0, // RAM[0xC0] (stack area, but used as bank switch target)
            2 => 0x4040, // RAM page 2 + 0x40
            3 => 0x6040, // RAM page 3 + 0x40
            4 => 0x2040, // RAM page 1 + 0x40
            5 => 0x4040, // Same as 2
            6 => 0x6040, // Same as 3
            7 => 0x2040, // Same as 1
            _ => 0x0040,
        }
    }

    /// Handle zero page switch (register 0x0F write)
    pub fn switch_zp40(&mut self, new_value: u8) {
        let old_value = self.current_zp_switch;
        let new_idx = new_value & 0x07;
        let old_idx = old_value & 0x07;

        if new_idx != old_idx {
            if old_idx != 0 {
                // Save current 0x40-0x7F to the old location
                let old_ptr = self.get_zp40_ptr(old_idx);
                self.ram.copy_within(0x40..0x80, old_ptr);
                // Restore from new location
                let new_ptr = self.get_zp40_ptr(new_idx);
                if new_idx == 0 {
                    // Load from bak_40
                    self.bak_40.copy_from_slice(&self.ram[0x40..0x80]);
                    self.ram[0x40..0x80].copy_from_slice(&self.bak_40);
                } else {
                    // Save current 0x40-0x7F to bak_40
                    self.bak_40.copy_from_slice(&self.ram[0x40..0x80]);
                    // Load from new location
                    self.ram.copy_within(new_ptr..new_ptr + 0x40, 0x40);
                }
            } else {
                // old was 0, save to bak_40
                self.bak_40.copy_from_slice(&self.ram[0x40..0x80]);
                let new_ptr = self.get_zp40_ptr(new_idx);
                self.ram.copy_within(new_ptr..new_ptr + 0x40, 0x40);
            }
        }
        self.current_zp_switch = new_value;
    }

    /// Update BBS pages based on current volume
    pub fn update_bbs_pages(&mut self, rom: &[u8], nor: &[u8]) {
        let volume_offset = match self.current_volume & 0x03 {
            0x01 => 0x100, // Volume 1
            0x03 => 0x200, // Volume 2
            _ => 0x000,    // Volume 0
        };

        for i in 0..4 {
            let bank_idx = volume_offset + i;
            let bank_offset = bank_idx * BANK_SIZE;
            if bank_offset + BANK_SIZE <= rom.len() {
                self.bbs_pages[i * 4].copy_from_slice(&rom[bank_offset..bank_offset + PAGE_SIZE]);
                self.bbs_pages[i * 4 + 1].copy_from_slice(&rom[bank_offset + PAGE_SIZE..bank_offset + 2 * PAGE_SIZE]);
                self.bbs_pages[i * 4 + 2].copy_from_slice(&rom[bank_offset + 2 * PAGE_SIZE..bank_offset + 3 * PAGE_SIZE]);
                self.bbs_pages[i * 4 + 3].copy_from_slice(&rom[bank_offset + 3 * PAGE_SIZE..bank_offset + 4 * PAGE_SIZE]);
            }
        }
        // Page 1 of BBS is RAM page 3
        self.bbs_pages[1].copy_from_slice(&self.ram[0x6000..0x8000]);

        // Set up memmap[7] = volume[0] + 0x2000
        // This is the fixed ROM page at 0xE000-0xFFFF
        // memmap[1] depends on register 0x0A bit2
        // memmap[6] depends on register 0x0A & 0x0F
    }

    /// Handle register 0x0D (volume select) write
    pub fn write_volume(&mut self, value: u8, rom: &[u8]) {
        let old_value = self.current_volume;
        self.io[0x0D] = value;
        self.current_volume = value;
        if value != old_value {
            self.update_bbs_pages_from_volume(rom);
        }
    }

    /// Update BBS pages when volume changes
    fn update_bbs_pages_from_volume(&mut self, rom: &[u8]) {
        let volume_offset = match self.current_volume & 0x03 {
            0x01 => 0x100,
            0x03 => 0x200,
            _ => 0x000,
        };

        for i in 0..4 {
            let bank_idx = volume_offset + i;
            let bank_offset = bank_idx * BANK_SIZE;
            if bank_offset + BANK_SIZE <= rom.len() {
                self.bbs_pages[i * 4].copy_from_slice(&rom[bank_offset..bank_offset + PAGE_SIZE]);
                self.bbs_pages[i * 4 + 1].copy_from_slice(&rom[bank_offset + PAGE_SIZE..bank_offset + 2 * PAGE_SIZE]);
                self.bbs_pages[i * 4 + 2].copy_from_slice(&rom[bank_offset + 2 * PAGE_SIZE..bank_offset + 3 * PAGE_SIZE]);
                self.bbs_pages[i * 4 + 3].copy_from_slice(&rom[bank_offset + 3 * PAGE_SIZE..bank_offset + 4 * PAGE_SIZE]);
            }
        }
        // BBS page 1 is always RAM page 3
        self.bbs_pages[1].copy_from_slice(&self.ram[0x6000..0x8000]);
    }

    /// Handle register 0x0A (ROA/BBS) write
    pub fn write_roa_bbs(&mut self, value: u8) {
        self.io[0x0A] = value;
        self.current_roa_bbs = value;
    }

    /// Handle register 0x00 (bank switch) write
    pub fn write_bank_switch(&mut self, value: u8) {
        self.io[0x00] = value;
        self.current_bank = value;
        self.bank_writable = value < 0x20; // NOR banks are writable
    }

    /// Get the current bank index
    pub fn current_bank(&self) -> u8 {
        self.current_bank
    }

    /// Check if current bank is NOR (writable)
    pub fn is_nor_bank(&self) -> bool {
        self.current_bank < 0x20
    }

    /// Get the effective memory page for a given address
    /// Returns (storage_type, offset) where storage_type indicates
    /// RAM, ROM, NOR, or BBS
    pub fn map_address(&self, addr: u16) -> (MemRegion, usize) {
        let page = (addr >> 13) as usize;
        let offset = (addr & 0x1FFF) as usize;

        match page {
            0 => {
                // 0x0000-0x1FFF: Always RAM page 0
                (MemRegion::Ram, offset)
            }
            1 => {
                // 0x2000-0x3FFF: RAM page 1 (or page 2 based on register 0x0A bit2)
                if self.current_roa_bbs & 0x04 != 0 {
                    (MemRegion::Ram, 0x4000 + offset) // RAM page 2
                } else {
                    (MemRegion::Ram, 0x2000 + offset) // RAM page 1
                }
            }
            2 => {
                // 0x4000-0x5FFF: Bank window page 0
                if self.current_bank < 0x20 {
                    // NOR bank
                    let bank_offset = self.current_bank as usize * BANK_SIZE;
                    (MemRegion::Nor, bank_offset + offset)
                } else if self.current_bank >= 0x80 {
                    // ROM bank
                    let volume_offset = match self.current_volume & 0x03 {
                        0x01 => 0x100,
                        0x03 => 0x200,
                        _ => 0x000,
                    };
                    let bank_idx = volume_offset + (self.current_bank - 0x80) as usize;
                    let bank_offset = bank_idx * BANK_SIZE;
                    (MemRegion::Rom, bank_offset + offset)
                } else {
                    // Invalid bank, return zeros
                    (MemRegion::Invalid, 0)
                }
            }
            3 => {
                // 0x6000-0x7FFF: Bank window page 1
                if self.current_bank < 0x20 {
                    let bank_offset = self.current_bank as usize * BANK_SIZE;
                    (MemRegion::Nor, bank_offset + PAGE_SIZE + offset)
                } else if self.current_bank >= 0x80 {
                    let volume_offset = match self.current_volume & 0x03 {
                        0x01 => 0x100,
                        0x03 => 0x200,
                        _ => 0x000,
                    };
                    let bank_idx = volume_offset + (self.current_bank - 0x80) as usize;
                    let bank_offset = bank_idx * BANK_SIZE;
                    (MemRegion::Rom, bank_offset + PAGE_SIZE + offset)
                } else {
                    (MemRegion::Invalid, 0)
                }
            }
            4 => {
                // BBS page based on register 0x0A
                let bbs_idx = (self.current_roa_bbs & 0x0F) as usize;
                if bbs_idx < 16 {
                    (MemRegion::Bbs, bbs_idx * PAGE_SIZE + offset)
                } else {
                    (MemRegion::Invalid, 0)
                }
            }
            5 => {
                // Fixed ROM page (volume[0] + 0x2000)
                // This is the interrupt vector area at 0xE000-0xFFFF
                let volume_offset = match self.current_volume & 0x03 {
                    0x01 => 0x100,
                    0x03 => 0x200,
                    _ => 0x000,
                };
                let bank_offset = volume_offset * BANK_SIZE;
                (MemRegion::Rom, bank_offset + 0x2000 + offset)
            }
            _ => (MemRegion::Invalid, 0),
        }
    }
}

/// Memory region type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemRegion {
    /// RAM
    Ram,
    /// ROM
    Rom,
    /// NOR Flash
    Nor,
    /// BBS (banked ROM page)
    Bbs,
    /// Invalid/unmapped
    Invalid,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
