// NC2000 hardware machine implementation (skeleton).
//
// The NC2000 boots from NOR Flash plus NAND Flash (no big ROM dump), and
// uses a different bank model: banks 0x00-0x1F select NOR pages while
// banks 0x80+ select extended RAM. This module provides the machine
// scaffolding and file loading; NAND controller emulation and the NC2000
// IO semantics are not implemented yet.

use anyhow::{Context, Result};

use crate::audio::Audio;
use crate::cpu::Cpu;
use crate::flash::Flash;
use crate::input::Input;
use crate::lcd::Lcd;
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::save::SaveState;
use crate::timer::Timer;

/// Internal RAM (24K) + external RAM (32K) layout used by NC2000 banking.
const INTERNAL_RAM_SIZE: usize = 24 * 1024;
const EXTERNAL_RAM_SIZE: usize = 32 * 1024;

/// NC2000 machine.
pub struct Nc2000Machine {
    // TODO: NAND controller, NC2000 IO semantics and the
    // `ramb`/`ramb2` bank window behaviour are not implemented yet.
    io: [u8; 0x40],
    internal_ram: Vec<u8>,
    external_ram: Vec<u8>,
    input: Input,
    timer: Timer,
    audio: Audio,
    lcd: Lcd,
    flash: Flash,
    nor: Vec<u8>,
    nand: Vec<u8>,
    nand0: Vec<u8>,
}

impl Nc2000Machine {
    /// Create the machine and load its NOR / NAND files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            io: [0; 0x40],
            internal_ram: vec![0; INTERNAL_RAM_SIZE],
            external_ram: vec![0; EXTERNAL_RAM_SIZE],
            input: Input::new(),
            timer: Timer::new(),
            audio: Audio::new(),
            lcd: Lcd::new(),
            flash: Flash::new(),
            nor: Vec::new(),
            nand: Vec::new(),
            nand0: Vec::new(),
        };

        if let Some(nor_path) = &files.nor {
            machine.nor = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read NC2000 NOR: {}", nor_path.display()))?;
        }
        if let Some(nand_path) = &files.nand {
            machine.nand = std::fs::read(nand_path)
                .with_context(|| format!("Failed to read NC2000 NAND: {}", nand_path.display()))?;
        }
        if let Some(nand0_path) = &files.nand0 {
            machine.nand0 = std::fs::read(nand0_path).with_context(|| {
                format!("Failed to read NC2000 NAND0: {}", nand0_path.display())
            })?;
        }

        Ok(machine)
    }
}

impl Machine for Nc2000Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Nc2000
    }

    fn reset(&mut self) {
        self.io.fill(0);
        self.internal_ram.fill(0);
        self.external_ram.fill(0);
        self.input.reset();
        self.timer.reset();
        self.audio.reset();
        self.lcd.reset();
        self.flash.reset();
        log::warn!("NC2000 machine is a skeleton: boot will not work yet");
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        if let Some(nor_path) = &files.nor {
            self.nor = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read NC2000 NOR: {}", nor_path.display()))?;
        }
        if let Some(nand_path) = &files.nand {
            self.nand = std::fs::read(nand_path)
                .with_context(|| format!("Failed to read NC2000 NAND: {}", nand_path.display()))?;
        }
        if let Some(nand0_path) = &files.nand0 {
            self.nand0 = std::fs::read(nand0_path).with_context(|| {
                format!("Failed to read NC2000 NAND0: {}", nand0_path.display())
            })?;
        }
        Ok(())
    }

    fn set_key(&mut self, key_id: u8, pressed: bool) {
        self.input.set_key(key_id, pressed);
    }

    fn is_sleeping(&self) -> bool {
        self.input.slept
    }

    fn step(&mut self, _cpu: &mut Cpu) -> u64 {
        // TODO: implement NC2000 bank windows and IO semantics.
        2
    }

    fn end_of_frame(&mut self, _cpu: &mut Cpu) {
        // TODO: copy LCD framebuffer once the memory map is implemented.
    }

    fn peek(&self, addr: u16) -> u8 {
        if (addr as usize) < self.io.len() {
            self.io[addr as usize]
        } else {
            0
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

    fn save_state(&self, cpu: &Cpu) -> SaveState {
        SaveState::new(
            cpu,
            &self.internal_ram,
            &self.io,
            &self.timer,
            &self.input,
            &self.audio,
            &self.flash,
            self.lcd.lcd_addr,
        )
    }

    fn load_state(&mut self, cpu: &mut Cpu, state: &SaveState) {
        *cpu = state.cpu.clone();
        if state.io.len() == 0x40 {
            self.io.copy_from_slice(&state.io);
        }
        if state.ram.len() >= INTERNAL_RAM_SIZE {
            self.internal_ram
                .copy_from_slice(&state.ram[..INTERNAL_RAM_SIZE]);
        }
        self.timer = state.timer.clone();
        self.input = state.input.clone();
        self.audio = state.audio.clone();
        self.flash = state.flash.clone();
        self.lcd.lcd_addr = state.lcd_addr;
    }
}
