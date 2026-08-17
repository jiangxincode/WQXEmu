// NOR Flash emulation for NC1020.
//
// The NC1020 has 1MB NOR Flash (32 banks x 32KB each).
// Flash operations use a command sequence:
//   Step 0: Write 0xAA to address 0x5555
//   Step 1: Write 0x55 to address 0xAAAA
//   Step 2: Write command to address 0x5555
//     0x90 = Enter ID mode
//     0xA0 = Byte program
//     0x80 = Erase
//     0xA8 = Buffer program
//     0x88 = Chip erase
//     0x78 = Sector erase
//
// Flash writes are only possible when the current bank is NOR (0x00-0x1F).

use serde::{Deserialize, Serialize};

/// Flash command state machine steps
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashStep {
    /// Waiting for 0xAA at 0x5555
    Idle,
    /// Received 0xAA, waiting for 0x55 at 0xAAAA
    Step1,
    /// Received 0x55, waiting for command at 0x5555
    Step2,
    /// Command received, ready for operation
    Command,
    /// Byte program in progress
    Program,
    /// Sector erase in progress
    EraseSector,
    /// Chip erase in progress
    EraseChip,
}

/// Flash operation type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashType {
    /// No operation
    None,
    /// Enter ID mode
    IdMode,
    /// Byte program
    ByteProgram,
    /// Erase (sector or chip)
    Erase,
    /// Buffer program
    BufferProgram,
    /// Chip erase
    ChipErase,
    /// Sector erase
    SectorErase,
}

/// NOR Flash state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flash {
    /// Current flash command step
    pub step: FlashStep,
    /// Current flash operation type
    pub flash_type: FlashType,
    /// Current bank being operated on
    pub bank_idx: u8,
    /// Backup bytes for ID mode
    pub bak1: u8,
    pub bak2: u8,
    /// Flash buffer for buffer programming
    pub buffer: Vec<u8>,
}

impl Flash {
    /// Create a new flash controller
    pub fn new() -> Self {
        Self {
            step: FlashStep::Idle,
            flash_type: FlashType::None,
            bank_idx: 0,
            bak1: 0,
            bak2: 0,
            buffer: vec![0; 0x100],
        }
    }

    /// Reset flash state
    pub fn reset(&mut self) {
        self.step = FlashStep::Idle;
        self.flash_type = FlashType::None;
        self.bank_idx = 0;
        self.bak1 = 0;
        self.bak2 = 0;
        self.buffer.fill(0);
    }

    /// Process a write to the NOR Flash address space
    /// Returns true if the write was handled by the flash controller
    pub fn write(&mut self, addr: u16, value: u8, bank_idx: u8, nor: &mut [u8]) -> bool {
        // Flash writes only work on NOR banks
        if bank_idx >= 0x20 {
            return false;
        }

        let bank_offset = bank_idx as usize * 0x8000;

        match self.step {
            FlashStep::Idle => {
                if addr == 0x5555 && value == 0xAA {
                    self.step = FlashStep::Step1;
                    return true;
                }
            }
            FlashStep::Step1 => {
                if addr == 0xAAAA && value == 0x55 {
                    self.step = FlashStep::Step2;
                    return true;
                }
                // Wrong sequence, reset
                self.step = FlashStep::Idle;
            }
            FlashStep::Step2 => {
                if addr == 0x5555 {
                    self.flash_type = match value {
                        0x90 => FlashType::IdMode,
                        0xA0 => FlashType::ByteProgram,
                        0x80 => FlashType::Erase,
                        0xA8 => FlashType::BufferProgram,
                        0x88 => FlashType::ChipErase,
                        0x78 => FlashType::SectorErase,
                        _ => FlashType::None,
                    };
                    if self.flash_type != FlashType::None {
                        self.step = FlashStep::Command;
                        if self.flash_type == FlashType::IdMode {
                            self.bank_idx = bank_idx;
                            let bank_start = bank_offset + 0x4000;
                            if bank_start + 2 <= nor.len() {
                                self.bak1 = nor[bank_start];
                                self.bak2 = nor[bank_start + 1];
                            }
                        }
                        return true;
                    }
                }
                // Wrong sequence, reset
                self.step = FlashStep::Idle;
            }
            FlashStep::Command => {
                match self.flash_type {
                    FlashType::IdMode => {
                        if value == 0xF0 {
                            // Exit ID mode
                            let bank_start = bank_offset + 0x4000;
                            if bank_start + 2 <= nor.len() {
                                nor[bank_start] = self.bak1;
                                nor[bank_start + 1] = self.bak2;
                            }
                            self.step = FlashStep::Idle;
                            return true;
                        }
                    }
                    FlashType::ByteProgram => {
                        // Byte program: AND the value with existing data
                        let offset = bank_offset + (addr as usize - 0x4000);
                        if offset < nor.len() {
                            nor[offset] &= value;
                        }
                        self.step = FlashStep::Idle;
                        return true;
                    }
                    FlashType::BufferProgram => {
                        // Buffer program
                        let offset = addr as usize & 0xFF;
                        if offset < 0x100 {
                            self.buffer[offset] &= value;
                        }
                        self.step = FlashStep::Program;
                        return true;
                    }
                    FlashType::Erase | FlashType::SectorErase | FlashType::ChipErase => {
                        if addr == 0x5555 && value == 0xAA {
                            self.step = FlashStep::Program;
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            FlashStep::Program => {
                match self.flash_type {
                    FlashType::Erase | FlashType::SectorErase | FlashType::ChipErase => {
                        if addr == 0xAAAA && value == 0x55 {
                            self.step = FlashStep::EraseSector;
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            FlashStep::EraseSector => {
                if addr == 0x5555 && value == 0x10 {
                    // Chip erase
                    if self.flash_type == FlashType::ChipErase {
                        for i in 0..0x20 {
                            let start = i * 0x8000;
                            let end = start + 0x8000;
                            if end <= nor.len() {
                                nor[start..end].fill(0xFF);
                            }
                        }
                        self.buffer.fill(0xFF);
                        self.step = FlashStep::Idle;
                        return true;
                    }
                    // Sector erase
                    if self.flash_type == FlashType::SectorErase && value == 0x30 {
                        let sector_start = bank_offset + (addr & !0x7FF) as usize - 0x4000;
                        let sector_end = sector_start + 0x800;
                        if sector_end <= nor.len() {
                            nor[sector_start..sector_end].fill(0xFF);
                        }
                        self.step = FlashStep::Idle;
                        return true;
                    }
                }
                // Buffer program complete
                if self.flash_type == FlashType::BufferProgram {
                    self.step = FlashStep::Idle;
                    return true;
                }
            }
            _ => {}
        }

        // Reset on 0xF0 at 0x8000
        if addr == 0x8000 && value == 0xF0 {
            self.step = FlashStep::Idle;
            return true;
        }

        false
    }
}

impl Default for Flash {
    fn default() -> Self {
        Self::new()
    }
}
