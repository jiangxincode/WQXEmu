// WQXEmu Core - Platform-independent Wenquxing hardware emulation.
//
// This crate provides hardware emulation cores for multiple Wenquxing
// models (NC1020, PC1000, NC2000) that can be used by any frontend
// (standalone, libretro, WASM, etc.).
//
// # Architecture
//
// Shared components:
// - `cpu`: 6502 CPU emulation with full instruction set
// - `lcd`: LCD controller (160x80, 1-bit)
// - `input`: Keyboard matrix (8x8)
// - `timer`: Timer system (Timer0: 2Hz, Timer1: 256Hz)
// - `audio`: JG WAV audio
// - `flash`: NOR Flash controller
// - `save`: Save state serialization
//
// Model-specific components:
// - `machine`: `Machine` trait, `MachineModel` and `RomFiles`
// - `machines`: per-model implementations (NC1020 complete, PC1000/NC2000
//   scaffolding)
// - `emulator`: model-agnostic shell owning the shared CPU and frame loop

pub mod audio;
pub mod cpu;
pub mod emulator;
pub mod flash;
pub mod input;
pub mod io;
pub mod keyboard;
pub mod lcd;
pub mod machine;
pub mod machines;
pub mod memory;
pub mod save;
pub mod timer;

// Re-export main types for convenience
pub use emulator::Emulator;
pub use input::key_ids;
pub use keyboard::{key_id_for, layout_for, KeyDef};
pub use lcd::{LCD_HEIGHT, LCD_WIDTH};
pub use machine::{Machine, MachineModel, RomFiles};
pub use machines::detect_model;
