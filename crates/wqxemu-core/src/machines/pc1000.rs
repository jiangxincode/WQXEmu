// PC1000 hardware machine implementation (skeleton).
//
// The PC1000 shares the SPDC1024-era 6502 SoC family but uses a different
// memory bank layout and IO register semantics than the NC1020 (see the
// internal reference for the PC1000 bus definition). This module provides
// the machine scaffolding and IO register map; full emulation of the
// PC1000 IO/DSP behaviour is not implemented yet.

use anyhow::{Context, Result};

use crate::audio::Audio;
use crate::cpu::Cpu;
use crate::flash::Flash;
use crate::input::Input;
use crate::lcd::Lcd;
use crate::machine::{Machine, MachineModel, RomFiles};
use crate::save::SaveState;
use crate::timer::Timer;

/// PC1000 IO register map (from the PC1000 bus definition).
#[allow(dead_code)]
mod io_map {
    pub const IO_BANK_SWITCH: u8 = 0x00;
    pub const IO_INT_ENABLE: u8 = 0x01;
    pub const IO_INT_STATUS: u8 = 0x01;
    pub const IO_TIMER0_VAL: u8 = 0x02;
    pub const IO_TIMER1_VAL: u8 = 0x03;
    pub const IO_STOP_TIMER0: u8 = 0x04;
    pub const IO_GENERAL_CTRL: u8 = 0x04;
    pub const IO_START_TIMER0: u8 = 0x05;
    pub const IO_CLOCK_CTRL: u8 = 0x05;
    pub const IO_STOP_TIMER1: u8 = 0x06;
    pub const IO_LCD_CONFIG: u8 = 0x06;
    pub const IO_START_TIMER1: u8 = 0x07;
    pub const IO_PORT_CONFIG: u8 = 0x07;
    pub const IO_PORT0: u8 = 0x08;
    pub const IO_PORT1: u8 = 0x09;
    pub const IO_BIOS_BSW: u8 = 0x0A;
    pub const IO_PORT3: u8 = 0x0B;
    pub const IO_GENERAL_STATUS: u8 = 0x0C;
    pub const IO_LCD_SEGMENT: u8 = 0x0D;
    pub const IO_DAC_DATA: u8 = 0x0E;
    pub const IO_ZP_BSW: u8 = 0x0F;
    pub const IO_TIMERA_VAL_L: u8 = 0x10;
    pub const IO_TIMERB_VAL_L: u8 = 0x12;
    pub const IO_TIMERAB_CTRL: u8 = 0x14;
    pub const IO_PORT2: u8 = 0x17;
    pub const IO_PORT4: u8 = 0x18;
    pub const IO_CTV_SELECT: u8 = 0x19;
    pub const IO_VOLUME_SET: u8 = 0x1A;
    pub const IO_DSP_STAT: u8 = 0x20;
    pub const IO_DSP_RET_DATA: u8 = 0x21;
    pub const IO_DSP_DATA_LOW: u8 = 0x22;
    pub const IO_DSP_DATA_HI: u8 = 0x23;
}

/// PC1000 machine.
pub struct Pc1000Machine {
    // TODO: PC1000 memory layout (internal/extended RAM banking, ROM/NOR
    // window mapping) and IO semantics are not implemented yet.
    io: [u8; 0x40],
    input: Input,
    timer: Timer,
    audio: Audio,
    lcd: Lcd,
    flash: Flash,
    rom: Vec<u8>,
    nor: Vec<u8>,
}

impl Pc1000Machine {
    /// Create the machine and load its ROM files.
    pub fn new(files: &RomFiles) -> Result<Self> {
        let mut machine = Self {
            io: [0; 0x40],
            input: Input::new(),
            timer: Timer::new(),
            audio: Audio::new(),
            lcd: Lcd::new(),
            flash: Flash::new(),
            rom: Vec::new(),
            nor: Vec::new(),
        };

        if let Some(rom_path) = &files.rom {
            machine.rom = std::fs::read(rom_path)
                .with_context(|| format!("Failed to read PC1000 ROM: {}", rom_path.display()))?;
        }
        if let Some(nor_path) = &files.nor {
            machine.nor = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read PC1000 NOR: {}", nor_path.display()))?;
        }

        Ok(machine)
    }
}

impl Machine for Pc1000Machine {
    fn model(&self) -> MachineModel {
        MachineModel::Pc1000
    }

    fn reset(&mut self) {
        self.io.fill(0);
        self.input.reset();
        self.timer.reset();
        self.audio.reset();
        self.lcd.reset();
        self.flash.reset();
        log::warn!("PC1000 machine is a skeleton: boot will not work yet");
    }

    fn load_rom(&mut self, files: &RomFiles) -> Result<()> {
        if let Some(rom_path) = &files.rom {
            self.rom = std::fs::read(rom_path)
                .with_context(|| format!("Failed to read PC1000 ROM: {}", rom_path.display()))?;
        }
        if let Some(nor_path) = &files.nor {
            self.nor = std::fs::read(nor_path)
                .with_context(|| format!("Failed to read PC1000 NOR: {}", nor_path.display()))?;
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
        // TODO: implement PC1000 bus / IO semantics.
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
            &[],
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
        self.timer = state.timer.clone();
        self.input = state.input.clone();
        self.audio = state.audio.clone();
        self.flash = state.flash.clone();
        self.lcd.lcd_addr = state.lcd_addr;
    }
}
