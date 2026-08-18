// Generic emulator shell around a hardware `Machine`.
//
// The shell owns the shared 6502 CPU and the frame loop, while the
// model-specific hardware lives in the `Machine` implementation. This is
// the type frontends interact with; it stays model-agnostic.

use std::path::Path;

use anyhow::Result;

use crate::audio::SAMPLE_RATE;
use crate::cpu::{Cpu, RESET_VECTOR};
use crate::lcd::{CYCLES_PER_FRAME, LCD_HEIGHT, LCD_WIDTH};
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::machines;
use crate::save::SaveState;

/// Main emulator struct.
pub struct Emulator {
    /// CPU state
    pub cpu: Cpu,
    /// Model-specific hardware
    machine: Box<dyn Machine>,
    /// Frame counter
    frame_count: u64,
    /// Speed-up mode
    speed_up: bool,
}

impl Emulator {
    /// Create an emulator for the given model, loading its ROM files.
    pub fn new(model: MachineModel, files: &RomFiles) -> Result<Self> {
        let mut machine = machines::create_machine(model, files)?;
        machine.reset();

        let mut emu = Self {
            cpu: Cpu::new(),
            machine,
            frame_count: 0,
            speed_up: false,
        };

        // Read the reset vector from the machine's memory
        let reset_vec = emu.machine.peek_u16(RESET_VECTOR);
        emu.cpu.pc = reset_vec;
        log::info!(
            "Emulator initialized: model={}, PC=0x{:04X}",
            model.name(),
            reset_vec
        );

        Ok(emu)
    }

    /// Convenience constructor for the NC1020 model (ROM + optional NOR).
    pub fn from_rom(rom_path: &str, nor_path: Option<&str>) -> Result<Self> {
        let files = RomFiles::new(
            Some(Path::new(rom_path).to_path_buf()),
            nor_path.map(|p| Path::new(p).to_path_buf()),
            None,
            None,
        );
        Self::new(MachineModel::Nc1020, &files)
    }

    /// Reset the emulator to its initial power-on state.
    pub fn reset(&mut self) {
        self.machine.reset();
        self.cpu.reset(0);
        let reset_vec = self.machine.peek_u16(RESET_VECTOR);
        self.cpu.pc = reset_vec;
        self.frame_count = 0;
        log::info!("Emulator reset, PC=0x{:04X}", reset_vec);
    }

    /// Load a NOR Flash file (supported by machines that use NOR).
    pub fn load_nor(&mut self, path: &str) -> Result<()> {
        self.machine.load_nor(Path::new(path))
    }

    /// Save the NOR Flash contents to a file.
    pub fn save_nor(&self, path: &str) -> Result<()> {
        self.machine.save_nor(Path::new(path))
    }

    /// Set a key state.
    pub fn set_key(&mut self, key_id: u8, pressed: bool) {
        self.machine.set_key(key_id, pressed);
    }

    /// Set speed-up mode.
    pub fn set_speed_up(&mut self, speed_up: bool) {
        self.speed_up = speed_up;
        self.machine.set_speed_up(speed_up);
    }

    /// Run one frame (approximately 1/30 second).
    pub fn run_frame(&mut self) {
        let target_cycles = CYCLES_PER_FRAME;
        let mut cycles_this_frame = 0u64;

        while cycles_this_frame < target_cycles {
            let cycles = self.machine.step(&mut self.cpu);
            cycles_this_frame += cycles;
        }

        self.machine.end_of_frame(&mut self.cpu);
        self.frame_count += 1;
    }

    /// Read a memory/IO address for debugging.
    pub fn peek(&self, addr: u16) -> u8 {
        self.machine.peek(addr)
    }

    /// Read a little-endian 16-bit word for debugging.
    pub fn peek_u16(&self, addr: u16) -> u16 {
        self.machine.peek_u16(addr)
    }

    /// Get the LCD framebuffer as XRGB8888 pixels.
    pub fn framebuffer(&self) -> Vec<u32> {
        self.machine.lcd().framebuffer_xrgb8888()
    }

    /// Get the LCD framebuffer as raw bytes.
    pub fn framebuffer_raw(&self) -> &[u8] {
        self.machine.lcd().framebuffer_raw()
    }

    /// Get the frame counter.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the LCD width.
    pub fn lcd_width(&self) -> usize {
        LCD_WIDTH
    }

    /// Get the LCD height.
    pub fn lcd_height(&self) -> usize {
        LCD_HEIGHT
    }

    /// Get the sample rate for audio.
    pub fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    /// Drain audio samples.
    pub fn drain_audio(&mut self, output: &mut Vec<i16>) {
        self.machine.drain_audio(output);
    }

    /// Check if the emulator is in sleep mode.
    pub fn is_sleeping(&self) -> bool {
        self.machine.is_sleeping()
    }

    /// Get current CPU PC.
    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    /// Get the active machine model.
    pub fn model(&self) -> MachineModel {
        self.machine.model()
    }

    /// Create a save state.
    pub fn save_state(&self) -> SaveState {
        self.machine.save_state(&self.cpu)
    }

    /// Load a save state.
    pub fn load_state(&mut self, state: &SaveState) -> Result<()> {
        self.machine.load_state(&mut self.cpu, state);
        Ok(())
    }
}
