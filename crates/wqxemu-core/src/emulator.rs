// Main emulator orchestrator for NC1020.
//
// Combines all hardware components (CPU, memory, LCD, input, timer,
// audio, flash) into a single coherent emulator.
//
// The emulator runs at 5.12 MHz and produces frames at 30 fps.
// Each frame is approximately 170,666 CPU cycles.

use anyhow::{Context, Result};

use crate::audio::{Audio, SAMPLE_RATE};
use crate::cpu::{Cpu, CpuBus, RESET_VECTOR};
use crate::flash::Flash;
use crate::input::Input;
use crate::io::IoHandler;
use crate::lcd::{Lcd, CYCLES_PER_FRAME, LCD_HEIGHT, LCD_WIDTH};
use crate::memory::{Memory, MemRegion, BANK_SIZE, IO_LIMIT, NOR_SIZE, RAM_SIZE, ROM_SIZE};
use crate::save::SaveState;
use crate::timer::Timer;

/// CPU cycles per millisecond at 5.12 MHz
const CYCLES_PER_MS: u32 = 5120;

/// Main emulator struct
pub struct Emulator {
    /// CPU state
    pub cpu: Cpu,
    /// Memory system
    pub memory: Memory,
    /// LCD controller
    pub lcd: Lcd,
    /// Input system
    pub input: Input,
    /// Timer system
    pub timer: Timer,
    /// Audio system
    pub audio: Audio,
    /// Flash controller
    pub flash: Flash,
    /// ROM data (1.5MB)
    rom: Vec<u8>,
    /// NOR Flash data (1MB)
    nor: Vec<u8>,
    /// Frame counter
    frame_count: u64,
    /// Whether the emulator is running
    running: bool,
    /// Speed-up mode
    speed_up: bool,
}

impl Emulator {
    /// Create a new emulator instance
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            memory: Memory::new(),
            lcd: Lcd::new(),
            input: Input::new(),
            timer: Timer::new(),
            audio: Audio::new(),
            flash: Flash::new(),
            rom: vec![0; ROM_SIZE],
            nor: vec![0; NOR_SIZE],
            frame_count: 0,
            running: false,
            speed_up: false,
        }
    }

    /// Create emulator from ROM and NOR files
    pub fn from_rom(rom_path: &str, nor_path: Option<&str>) -> Result<Self> {
        let mut emu = Self::new();
        emu.load_rom(rom_path)?;

        if let Some(nor_path) = nor_path {
            emu.load_nor(nor_path)?;
        }

        Ok(emu)
    }

    /// Load ROM data from file
    pub fn load_rom(&mut self, path: &str) -> Result<()> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read ROM file: {}", path))?;

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

        // Debug: check ROM data at key offsets
        log::debug!("ROM[0x0000]: {:02X} {:02X}", self.rom[0], self.rom[1]);
        log::debug!("ROM[0x2000]: {:02X} {:02X}", self.rom[0x2000], self.rom[0x2001]);
        log::debug!("ROM[0x3FFC]: {:02X} {:02X} (reset vector)", self.rom[0x3FFC], self.rom[0x3FFD]);
        let rv = self.rom[0x3FFC] as u16 | (self.rom[0x3FFD] as u16) << 8;
        log::debug!("Reset vector: 0x{:04X}", rv);
        if (rv as usize) < self.rom.len() {
            log::debug!("Code at reset vec: {:02X} {:02X} {:02X} {:02X}",
                self.rom[rv as usize], self.rom[rv as usize + 1],
                self.rom[rv as usize + 2], self.rom[rv as usize + 3]);
        }

        // Initialize BBS pages from ROM
        self.memory.update_bbs_pages(&self.rom, &self.nor);

        log::info!("ROM loaded: {} bytes", ROM_SIZE);
        Ok(())
    }

    /// Load NOR Flash data from file
    pub fn load_nor(&mut self, path: &str) -> Result<()> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read NOR file: {}", path))?;

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

    /// Save NOR Flash to file
    pub fn save_nor(&self, path: &str) -> Result<()> {
        // Reverse the swap for saving
        let mut output = vec![0u8; NOR_SIZE];
        for offset in (0..NOR_SIZE).step_by(BANK_SIZE) {
            output[offset + 0x4000..offset + 0x8000]
                .copy_from_slice(&self.nor[offset..offset + 0x4000]);
            output[offset..offset + 0x4000]
                .copy_from_slice(&self.nor[offset + 0x4000..offset + 0x8000]);
        }
        std::fs::write(path, &output)
            .with_context(|| format!("Failed to write NOR file: {}", path))?;
        log::info!("NOR Flash saved: {}", path);
        Ok(())
    }

    /// Reset the emulator
    pub fn reset(&mut self) {
        self.cpu.reset(0);
        self.memory.reset();
        self.lcd.reset();
        self.input.reset();
        self.timer.reset();
        self.audio.reset();
        self.flash.reset();
        self.frame_count = 0;

        // Set up initial memory mapping
        self.memory.update_bbs_pages(&self.rom, &self.nor);

        // Read reset vector from ROM
        let reset_vec = self.peek_u16(RESET_VECTOR);
        self.cpu.pc = reset_vec;

        log::info!("Emulator reset, PC=0x{:04X}", reset_vec);
    }

    /// Load save state
    pub fn load_state(&mut self, state: &SaveState) -> Result<()> {
        self.cpu = state.cpu.clone();
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

        Ok(())
    }

    /// Create save state
    pub fn save_state(&self) -> SaveState {
        SaveState::new(
            &self.cpu,
            &self.memory.ram,
            &self.memory.io,
            &self.timer,
            &self.input,
            &self.audio,
            &self.flash,
            self.lcd.lcd_addr,
        )
    }

    /// Set a key state
    pub fn set_key(&mut self, key_id: u8, pressed: bool) {
        self.input.set_key(key_id, pressed);
    }

    /// Set speed-up mode
    pub fn set_speed_up(&mut self, speed_up: bool) {
        self.speed_up = speed_up;
        self.timer.set_speed_up(speed_up);
    }

    /// Run one frame (approximately 1/30 second)
    pub fn run_frame(&mut self) {
        let target_cycles = CYCLES_PER_FRAME;
        let mut cycles_this_frame = 0u64;

        while cycles_this_frame < target_cycles as u64 {
            let cycles = self.step();
            cycles_this_frame += cycles;
        }

        // Update LCD from RAM
        self.lcd.copy_from_ram(&self.memory.ram);

        // Handle wake-up if needed
        if self.input.should_wake_up {
            self.input.should_wake_up = false;
            self.memory.io[0x01] |= 0x01;
            self.memory.io[0x02] |= 0x01;
            self.cpu.pc = self.peek_u16(RESET_VECTOR);
        }

        self.frame_count += 1;
    }

    /// Execute one CPU step
    fn step(&mut self) -> u64 {
        // Create a bus adapter for the CPU
        let mut bus = EmulatorBus {
            memory: &mut self.memory,
            input: &mut self.input,
            timer: &mut self.timer,
            audio: &mut self.audio,
            lcd: &mut self.lcd,
            flash: &mut self.flash,
            rom: &self.rom,
            nor: &mut self.nor,
        };

        let cycles = self.cpu.step(&mut bus);

        // Update timer
        let fired = self.timer.tick(cycles);
        if fired.any() {
            self.cpu.irq_pending = true;
            if fired.timer1 {
                // Timer1 (256 Hz) marks its interrupt flag in io[0x01],
                // matching the reference implementation the firmware
                // polls to identify the interrupt source.
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

    /// Peek at a memory address (for debugging)
    pub fn peek(&self, addr: u16) -> u8 {
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
                // BBS pages are in memory.bbs_pages
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

    /// Peek at a 16-bit word (little-endian)
    pub fn peek_u16(&self, addr: u16) -> u16 {
        let lo = self.peek(addr) as u16;
        let hi = self.peek(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    /// Get the LCD framebuffer as XRGB8888 pixels
    pub fn framebuffer(&self) -> Vec<u32> {
        self.lcd.framebuffer_xrgb8888()
    }

    /// Get the LCD framebuffer as raw bytes
    pub fn framebuffer_raw(&self) -> &[u8] {
        self.lcd.framebuffer_raw()
    }

    /// Get the frame counter
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the LCD width
    pub fn lcd_width(&self) -> usize {
        LCD_WIDTH
    }

    /// Get the LCD height
    pub fn lcd_height(&self) -> usize {
        LCD_HEIGHT
    }

    /// Get the sample rate for audio
    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    /// Drain audio samples
    pub fn drain_audio(&mut self, output: &mut Vec<i16>) {
        self.audio.drain_audio(output);
    }

    /// Check if the emulator is in sleep mode
    pub fn is_sleeping(&self) -> bool {
        self.input.slept
    }

    /// Get current CPU PC
    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }
}

impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Bus adapter for CPU memory access
struct EmulatorBus<'a> {
    memory: &'a mut Memory,
    input: &'a mut Input,
    timer: &'a mut Timer,
    audio: &'a mut Audio,
    lcd: &'a mut Lcd,
    flash: &'a mut Flash,
    rom: &'a [u8],
    nor: &'a mut [u8],
}

impl<'a> CpuBus for EmulatorBus<'a> {
    fn read(&self, addr: u16) -> u8 {
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
                MemRegion::Ram => {
                    if offset < RAM_SIZE {
                        self.memory.ram[offset] = value;
                    }
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
            self.flash.step = crate::flash::FlashStep::Idle;
        }
    }
}
