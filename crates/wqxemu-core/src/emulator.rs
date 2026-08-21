// Generic emulator shell around a hardware `Machine`.
//
// The shell owns the shared 6502 CPU and the frame loop, while the
// model-specific hardware lives in the `Machine` implementation. This is
// the type frontends interact with; it stays model-agnostic.

use std::path::Path;

use anyhow::{Context, Result};

use crate::audio::SAMPLE_RATE;
use crate::cpu::{Cpu, RESET_VECTOR};
use crate::lcd::{CYCLES_PER_FRAME, LCD_HEIGHT, LCD_WIDTH};
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::machines;
use crate::save::{PersistentState, SaveState, PERSISTENT_STATE_VERSION};

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

    /// Serialize a complete cross-process session.
    pub fn save_persistent_state(&self) -> Result<Vec<u8>> {
        let state = PersistentState {
            version: PERSISTENT_STATE_VERSION,
            model: self.model(),
            machine_identity: self.machine.persistent_state_identity(),
            cpu: self.cpu.clone(),
            frame_count: self.frame_count,
            speed_up: self.speed_up,
            machine: self.machine.save_persistent_state()?,
        };
        bincode::serialize(&state).context("Failed to serialize persistent state")
    }

    /// Restore a complete cross-process session.
    pub fn load_persistent_state(&mut self, data: &[u8]) -> Result<()> {
        let state: PersistentState =
            bincode::deserialize(data).context("Failed to deserialize persistent state")?;
        if state.version != PERSISTENT_STATE_VERSION {
            anyhow::bail!(
                "Persistent state version mismatch: expected {}, got {}",
                PERSISTENT_STATE_VERSION,
                state.version
            );
        }
        if state.model != self.model() {
            anyhow::bail!(
                "Persistent state model mismatch: expected {}, got {}",
                self.model().name(),
                state.model.name()
            );
        }
        let machine_identity = self.machine.persistent_state_identity();
        if state.machine_identity != machine_identity {
            anyhow::bail!("Persistent state firmware mismatch");
        }

        self.machine.load_persistent_state(&state.machine)?;
        self.cpu = state.cpu;
        self.frame_count = state.frame_count;
        self.speed_up = state.speed_up;
        self.machine.set_speed_up(state.speed_up);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_state_round_trips_without_rom_or_flash_paths() {
        let mut emulator = Emulator::new(MachineModel::Nc1020, &RomFiles::default()).unwrap();
        emulator.cpu.pc = 0x1234;
        emulator.frame_count = 42;

        let state = emulator.save_persistent_state().unwrap();
        emulator.cpu.pc = 0x5678;
        emulator.frame_count = 99;
        emulator.load_persistent_state(&state).unwrap();

        assert_eq!(emulator.pc(), 0x1234);
        assert_eq!(emulator.frame_count(), 42);
    }

    #[test]
    fn persistent_state_rejects_a_different_machine_model() {
        let source = Emulator::new(MachineModel::Nc1020, &RomFiles::default()).unwrap();
        let state = source.save_persistent_state().unwrap();

        let mut target = Emulator::new(MachineModel::Pc1000, &RomFiles::default()).unwrap();
        let error = target.load_persistent_state(&state).unwrap_err();
        assert!(error.to_string().contains("model mismatch"));
    }
}
