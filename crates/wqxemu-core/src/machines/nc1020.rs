// NC1020 hardware machine implementation.
//
// The NC1020 (SPDC1024 SoC) boots from a 24MB ROM dump (three 8MB
// "volumes") plus a 1MB NOR Flash. This machine owns the NC1020-specific
// memory banking, IO registers and ROM layout, and drives the shared 6502
// CPU through the generic `Machine` trait.

use anyhow::{Context, Result};

use crate::audio::Audio;
use crate::cpu::{Cpu, CpuBus, RESET_VECTOR};
use crate::flash::{Flash, FlashStep};
use crate::input::Input;
use crate::io::IoHandler;
use crate::lcd::Lcd;
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::memory::{MemRegion, Memory, BANK_SIZE, IO_LIMIT, NOR_SIZE, RAM_SIZE, ROM_SIZE};
use crate::save::SaveState;
use crate::timer::Timer;

/// NC1020 machine.
pub struct Nc1020Machine {
    memory: Memory,
    input: Input,
    timer: Timer,
    audio: Audio,
    lcd: Lcd,
    flash: Flash,
    rom: Vec<u8>,
    nor: Vec<u8>,
}

impl Nc1020Machine {
    /// Create the machine and load its ROM / NOR files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            memory: Memory::new(),
            input: Input::new(),
            timer: Timer::new(),
            audio: Audio::new(),
            lcd: Lcd::new(),
            flash: Flash::new(),
            rom: vec![0; ROM_SIZE],
            nor: vec![0; NOR_SIZE],
        };

        if let Some(rom_path) = &files.rom {
            machine.load_rom_path(rom_path)?;
        }
        if let Some(nor_path) = &files.nor {
            machine.load_nor_path(nor_path)?;
        }

        Ok(machine)
    }

    /// Load ROM data from file.
    fn load_rom_path(&mut self, path: &std::path::Path) -> Result<()> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read ROM file: {}", path.display()))?;

        if data.len() < ROM_SIZE {
            anyhow::bail!(
                "ROM file too small: expected at least {} bytes, got {}",
                ROM_SIZE,
                data.len()
            );
        }

        // WQX binary dumps store each 32KB bank with its 16KB halves
        // swapped. Swap them back so bank windows map linearly, matching
        // how the NOR flash file is handled below.
        let mut processed = vec![0u8; data.len()];
        for offset in (0..data.len()).step_by(BANK_SIZE) {
            if offset + BANK_SIZE > data.len() {
                // Copy any trailing bytes beyond complete banks unchanged.
                processed[offset..].copy_from_slice(&data[offset..]);
                break;
            }
            let src = &data[offset..offset + BANK_SIZE];
            processed[offset + 0x4000..offset + 0x8000].copy_from_slice(&src[0..0x4000]);
            processed[offset..offset + 0x4000].copy_from_slice(&src[0x4000..0x8000]);
        }
        self.rom = processed;

        log::debug!("ROM[0x0000]: {:02X} {:02X}", self.rom[0], self.rom[1]);
        let rv = self.rom[0x3FFC] as u16 | (self.rom[0x3FFD] as u16) << 8;
        log::debug!("Reset vector: 0x{:04X}", rv);

        // Initialize BBS pages from ROM
        self.memory.update_bbs_pages(&self.rom, &self.nor);

        log::info!("ROM loaded: {} bytes", ROM_SIZE);
        Ok(())
    }

    /// Load NOR Flash data from file.
    fn load_nor_path(&mut self, path: &std::path::Path) -> Result<()> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read NOR file: {}", path.display()))?;

        if data.len() < NOR_SIZE {
            anyhow::bail!(
                "NOR file too small: expected at least {} bytes, got {}",
                NOR_SIZE,
                data.len()
            );
        }

        // Process NOR: same swap as ROM
        let mut processed = vec![0u8; NOR_SIZE];
        for offset in (0..NOR_SIZE).step_by(BANK_SIZE) {
            let src = &data[offset..offset + BANK_SIZE];
            processed[offset + 0x4000..offset + 0x8000].copy_from_slice(&src[0..0x4000]);
            processed[offset..offset + 0x4000].copy_from_slice(&src[0x4000..0x8000]);
        }
        self.nor = processed;

        log::info!("NOR Flash loaded: {} bytes", NOR_SIZE);
        Ok(())
    }

    /// Save NOR Flash to file (reverse the swap).
    fn save_nor_path(&self, path: &std::path::Path) -> Result<()> {
        let mut output = vec![0u8; NOR_SIZE];
        for offset in (0..NOR_SIZE).step_by(BANK_SIZE) {
            output[offset + 0x4000..offset + 0x8000]
                .copy_from_slice(&self.nor[offset..offset + 0x4000]);
            output[offset..offset + 0x4000]
                .copy_from_slice(&self.nor[offset + 0x4000..offset + 0x8000]);
        }
        std::fs::write(path, &output)
            .with_context(|| format!("Failed to write NOR file: {}", path.display()))?;
        log::info!("NOR Flash saved: {}", path.display());
        Ok(())
    }
}

impl Machine for Nc1020Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Nc1020
    }

    fn reset(&mut self) {
        self.memory.reset();
        self.lcd.reset();
        self.input.reset();
        self.timer.reset();
        self.audio.reset();
        self.flash.reset();

        // Set up initial memory mapping
        self.memory.update_bbs_pages(&self.rom, &self.nor);

        let reset_vec = self.peek_u16(RESET_VECTOR);
        log::info!("Emulator reset, PC=0x{:04X}", reset_vec);
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        if let Some(rom_path) = &files.rom {
            self.load_rom_path(rom_path)?;
        }
        if let Some(nor_path) = &files.nor {
            self.load_nor_path(nor_path)?;
        }
        Ok(())
    }

    fn set_key(&mut self, key_id: u8, pressed: bool) {
        self.input.set_key(key_id, pressed);
    }

    fn is_sleeping(&self) -> bool {
        self.input.slept
    }

    fn step(&mut self, cpu: &mut Cpu) -> u64 {
        let mut bus = Nc1020Bus {
            memory: &mut self.memory,
            input: &mut self.input,
            timer: &mut self.timer,
            audio: &mut self.audio,
            lcd: &mut self.lcd,
            flash: &mut self.flash,
            rom: &self.rom,
            nor: &mut self.nor,
        };

        let cycles = cpu.step(&mut bus);

        // Update timer
        let fired = self.timer.tick(cycles);
        if fired.any() {
            cpu.irq_pending = true;
            if fired.timer1 {
                // Timer1 (256 Hz) marks its interrupt flag in io[0x01],
                // which the firmware polls to identify the interrupt source.
                self.memory.io[0x01] |= 0x08;
            }
            if fired.timer0 {
                // Timer0 (2 Hz) updates the clock flags register.
                self.memory.io[0x3D] = self.timer.io_3d;
            }
        }

        // Handle wake-up pending
        if self.input.wake_up_pending {
            self.input.wake_up_pending = false;
            self.memory.ram[0x45F] = self.input.wake_up_key;
        }

        cycles
    }

    fn end_of_frame(&mut self, cpu: &mut Cpu) {
        // Update LCD from RAM
        self.lcd.copy_from_ram(&self.memory.ram);

        // Handle wake-up if needed
        if self.input.should_wake_up {
            self.input.should_wake_up = false;
            self.memory.io[0x01] |= 0x01;
            self.memory.io[0x02] |= 0x01;
            cpu.pc = self.peek_u16(RESET_VECTOR);
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        if addr < IO_LIMIT {
            return self.memory.io[addr as usize];
        }
        let (region, offset) = self.memory.map_address(addr);
        match region {
            MemRegion::Ram => self.memory.ram[offset],
            MemRegion::Rom => {
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0
                }
            }
            MemRegion::Nor => {
                if offset < self.nor.len() {
                    self.nor[offset]
                } else {
                    0
                }
            }
            MemRegion::Bbs => {
                let page_idx = offset / 0x2000;
                let page_offset = offset % 0x2000;
                if page_idx < 16 && page_offset < 0x2000 {
                    self.memory.bbs_pages[page_idx][page_offset]
                } else {
                    0
                }
            }
            MemRegion::Invalid => 0,
        }
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
        self.load_nor_path(path)
    }

    fn save_nor(&self, path: &std::path::Path) -> Result<()> {
        self.save_nor_path(path)
    }

    fn set_speed_up(&mut self, speed_up: bool) {
        self.timer.set_speed_up(speed_up);
    }

    fn save_state(&self, cpu: &Cpu) -> SaveState {
        SaveState::new(
            cpu,
            &self.memory.ram,
            &self.memory.io,
            &self.timer,
            &self.input,
            &self.audio,
            &self.flash,
            self.lcd.lcd_addr,
        )
    }

    fn load_state(&mut self, cpu: &mut Cpu, state: &SaveState) {
        *cpu = state.cpu.clone();
        if state.ram.len() == RAM_SIZE {
            self.memory.ram.copy_from_slice(&state.ram);
        }
        if state.io.len() == 0x40 {
            self.memory.io.copy_from_slice(&state.io);
        }
        self.timer = state.timer.clone();
        self.input = state.input.clone();
        self.audio = state.audio.clone();
        self.flash = state.flash.clone();
        self.lcd.lcd_addr = state.lcd_addr;

        // Rebuild memory mapping
        self.memory.update_bbs_pages(&self.rom, &self.nor);
    }
}

/// Bus adapter connecting the shared CPU to NC1020 hardware.
struct Nc1020Bus<'a> {
    memory: &'a mut Memory,
    input: &'a mut Input,
    timer: &'a mut Timer,
    audio: &'a mut Audio,
    lcd: &'a mut Lcd,
    flash: &'a mut Flash,
    rom: &'a [u8],
    nor: &'a mut [u8],
}

impl<'a> CpuBus for Nc1020Bus<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < IO_LIMIT {
            return IoHandler::read(
                addr as u8,
                &self.memory.io,
                self.input,
                self.timer,
                self.audio,
            );
        }

        let (region, offset) = self.memory.map_address(addr);
        match region {
            MemRegion::Ram => self.memory.ram[offset],
            MemRegion::Rom => {
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0
                }
            }
            MemRegion::Nor => {
                if offset < self.nor.len() {
                    self.nor[offset]
                } else {
                    0
                }
            }
            MemRegion::Bbs => {
                let page_idx = offset / 0x2000;
                let page_offset = offset % 0x2000;
                if page_idx < 16 && page_offset < 0x2000 {
                    self.memory.bbs_pages[page_idx][page_offset]
                } else {
                    0
                }
            }
            MemRegion::Invalid => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        if addr < IO_LIMIT {
            // Handle IO writes directly to avoid borrow checker issues
            match addr as u8 {
                0x00 => {
                    let old_value = self.memory.io[0x00];
                    self.memory.io[0x00] = value;
                    if value != old_value {
                        self.memory.write_bank_switch(value);
                    }
                }
                0x05 => {
                    let old_value = self.memory.io[0x05];
                    self.memory.io[0x05] = value;
                    if (old_value ^ value) & 0x08 != 0 {
                        self.input.slept = value & 0x08 == 0;
                    }
                }
                0x06 => {
                    self.memory.io[0x06] = value;
                    if self.lcd.lcd_addr == 0 {
                        self.lcd.update_address(&self.memory.io);
                    }
                    self.memory.io[0x09] &= 0xFE;
                }
                0x08 => {
                    self.memory.io[0x08] = value;
                    self.memory.io[0x0B] &= 0xFE;
                }
                0x09 => {
                    self.memory.io[0x09] = value;
                    self.memory.io[0x08] = self.input.read_keypad(value);
                }
                0x0A => {
                    let old_value = self.memory.io[0x0A];
                    self.memory.io[0x0A] = value;
                    if value != old_value {
                        self.memory.write_roa_bbs(value);
                    }
                }
                0x0D => {
                    let old_value = self.memory.io[0x0D];
                    self.memory.io[0x0D] = value;
                    if value != old_value {
                        self.memory.write_volume(value, self.rom);
                    }
                }
                0x0F => {
                    let old_value = self.memory.io[0x0F];
                    self.memory.io[0x0F] = value;
                    if (value & 0x07) != (old_value & 0x07) {
                        self.memory.switch_zp40(value);
                    }
                }
                0x20 => {
                    self.memory.io[0x20] = value;
                    self.audio.write_control(value);
                    // Hardware clears the register after a stop/reset
                    // command; the firmware polls it for completion.
                    if value == 0x80 || value == 0x40 {
                        self.memory.io[0x20] = 0;
                    }
                }
                0x22 => {
                    self.memory.io[0x22] = value;
                }
                0x23 => {
                    self.memory.io[0x23] = value;
                    let data = self.memory.io[0x22];
                    self.audio.write_command(data, value);
                    if value == 0x80 {
                        self.memory.io[0x20] = 0x80;
                    }
                }
                0x3E => {
                    self.memory.io[0x3E] = value;
                }
                0x3F => {
                    self.memory.io[0x3F] = value;
                    let idx = self.memory.io[0x3E];
                    self.timer.write_clock(idx, value);
                    // Writing clock control index 0x0B latches 0xF8 into
                    // the clock flags register (matching reference).
                    if idx == 0x0B {
                        self.memory.io[0x3D] = 0xF8;
                    }
                }
                _ => {
                    self.memory.io[addr as usize] = value;
                }
            }
            return;
        }

        // RAM writes (0x0040-0x3FFF)
        if addr < 0x4000 {
            let offset = addr as usize;
            if offset < RAM_SIZE {
                self.memory.ram[offset] = value;
            }
            return;
        }

        // Banked region writes (0x4000-0x7FFF)
        if addr < 0x8000 {
            let (region, offset) = self.memory.map_address(addr);
            match region {
                MemRegion::Ram if offset < RAM_SIZE => {
                    self.memory.ram[offset] = value;
                }
                MemRegion::Nor => {
                    // NOR Flash writes go through the flash controller
                    let bank_idx = self.memory.current_bank();
                    self.flash.write(addr, value, bank_idx, self.nor);
                }
                _ => {} // ROM/BBS are read-only
            }
            return;
        }

        // 0x8000+: Read-only (ROM/BBS)
        // But flash reset command can be sent here
        if addr == 0x8000 && value == 0xF0 {
            self.flash.step = FlashStep::Idle;
        }
    }
}
