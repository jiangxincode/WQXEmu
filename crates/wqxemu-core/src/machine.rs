// Hardware machine abstraction for WQXEmu.
//
// All supported Wenquxing models share the same 6502 CPU, 160x80 LCD and
// SPR4096 NOR Flash controller, but differ in memory banking, IO register
// semantics and ROM file layout (NC1020/PC1000 use ROM + NOR, NC2000 adds
// NAND). The `Machine` trait abstracts those differences so the frontend
// and the generic `Emulator` shell stay model-agnostic.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::cpu::Cpu;
use crate::lcd::Lcd;
use crate::save::SaveState;

/// Supported hardware models.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineModel {
    /// NC1020 (SPDC1024 SoC, ROM + NOR Flash)
    Nc1020,
    /// PC1000 (different bank layout and IO semantics)
    Pc1000,
    /// CC800 (older SPDC1016 SoC, half-swapped ROM/NOR dumps)
    Cc800,
    /// NC2000 (ROM is replaced by NAND Flash)
    Nc2000,
    /// NC3000 (10.24 MHz variant, 1MB NOR + two-plane NAND)
    Nc3000,
}

impl MachineModel {
    /// Stable model name used by frontends and CLI.
    pub fn name(self) -> &'static str {
        match self {
            MachineModel::Nc1020 => "nc1020",
            MachineModel::Pc1000 => "pc1000",
            MachineModel::Cc800 => "cc800",
            MachineModel::Nc2000 => "nc2000",
            MachineModel::Nc3000 => "nc3000",
        }
    }

    /// Parse a model name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "nc1020" => Some(MachineModel::Nc1020),
            "pc1000" => Some(MachineModel::Pc1000),
            "cc800" => Some(MachineModel::Cc800),
            "nc2000" => Some(MachineModel::Nc2000),
            "nc3000" => Some(MachineModel::Nc3000),
            _ => None,
        }
    }
}

/// ROM / Flash files for a machine.
///
/// - `rom`: system ROM dump (NC1020 `obj_lu.bin` / `.rom`, PC1000 `.rom`)
/// - `nor`: NOR Flash dump (`.fls` / `.nor`)
/// - `nand`: NAND Flash dump (NC2000 `.nand`)
/// - `nand0`: first NAND plane (NC2000 `.nand0`, optional)
#[derive(Clone, Debug, Default)]
pub struct RomFiles {
    pub rom: Option<PathBuf>,
    pub nor: Option<PathBuf>,
    pub nand: Option<PathBuf>,
    pub nand0: Option<PathBuf>,
}

impl RomFiles {
    /// Build from loose file paths (any may be `None`).
    pub fn new(
        rom: Option<PathBuf>,
        nor: Option<PathBuf>,
        nand: Option<PathBuf>,
        nand0: Option<PathBuf>,
    ) -> Self {
        Self {
            rom,
            nor,
            nand,
            nand0,
        }
    }
}

/// A single hardware model implementation.
///
/// The machine owns every hardware component whose behavior differs
/// between models (memory map, IO registers, ROM/Flash storage, keyboard,
/// timers, audio) and exposes a uniform interface to the generic
/// `Emulator` shell, which owns the shared CPU and the frame loop.
pub trait Machine: Send {
    /// Which model this machine implements.
    fn model(&self) -> MachineModel;

    /// Reset the machine to its initial power-on state.
    fn reset(&mut self);

    /// Load system ROM / Flash files.
    fn load_rom(&mut self, files: &RomFiles) -> Result<()>;

    /// Set a keyboard key state (pressed or released).
    fn set_key(&mut self, key_id: u8, pressed: bool);

    /// Whether the device is currently in sleep mode.
    fn is_sleeping(&self) -> bool;

    /// Execute one CPU instruction. Returns the number of cycles consumed.
    fn step(&mut self, cpu: &mut Cpu) -> u64;

    /// Called once per frame after the CPU has run. Machines update their
    /// LCD framebuffer and handle wake-up here.
    fn end_of_frame(&mut self, cpu: &mut Cpu);

    /// Read a memory/IO address for debugging (no side effects).
    fn peek(&self, addr: u16) -> u8;

    /// Read a little-endian 16-bit word for debugging.
    fn peek_u16(&self, addr: u16) -> u16;

    /// Access the LCD controller.
    fn lcd(&self) -> &Lcd;

    /// Mutable access to the LCD controller.
    fn lcd_mut(&mut self) -> &mut Lcd;

    /// Drain pending audio samples.
    fn drain_audio(&mut self, out: &mut Vec<i16>);

    /// Load a NOR Flash dump from disk (models without NOR can leave the
    /// default implementation).
    fn load_nor(&mut self, _path: &Path) -> Result<()> {
        anyhow::bail!("NOR Flash loading is not supported for this model")
    }

    /// Persist NOR Flash contents to disk.
    fn save_nor(&self, _path: &Path) -> Result<()> {
        anyhow::bail!("NOR Flash saving is not supported for this model")
    }

    /// Enable/disable speed-up mode.
    fn set_speed_up(&mut self, _speed_up: bool) {}

    /// Capture a save state.
    fn save_state(&self, cpu: &Cpu) -> SaveState;

    /// Restore a save state.
    fn load_state(&mut self, cpu: &mut Cpu, state: &SaveState);

    /// Serialize the complete model-specific state for persistent sessions.
    fn save_persistent_state(&self) -> Result<Vec<u8>>;

    /// Restore model-specific state from a persistent session.
    fn load_persistent_state(&mut self, data: &[u8]) -> Result<()>;

    /// Identify immutable firmware that is intentionally omitted from sessions.
    fn persistent_state_identity(&self) -> u64 {
        0
    }
}

/// Shared hardware component types that machines embed.
///
/// Re-exported so machine implementations can construct them without
/// depending on the internal module paths.
pub use crate::audio::Audio as SharedAudio;
pub use crate::flash::Flash as SharedFlash;
pub use crate::input::Input as SharedInput;
pub use crate::timer::Timer as SharedTimer;
