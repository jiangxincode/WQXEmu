// WQXEmu Core - Platform-independent Wenquxing NC1020 hardware emulation.
//
// This crate provides a complete NC1020 hardware emulation core that can be
// used by any frontend (standalone, libretro, WASM, etc.).
//
// # Architecture
//
// The emulator is organized into these main components:
// - `cpu`: 6502 CPU emulation with full instruction set
// - `memory`: Memory mapping with bank switching
// - `io`: IO register handling (0x00-0x3F)
// - `lcd`: LCD controller (160x80, 1-bit)
// - `input`: Keyboard matrix (8x8)
// - `timer`: Timer system (Timer0: 2Hz, Timer1: 256Hz)
// - `audio`: JG WAV audio
// - `flash`: NOR Flash controller
// - `emulator`: Main orchestrator combining all components
// - `save`: Save state serialization

pub mod audio;
pub mod cpu;
pub mod emulator;
pub mod flash;
pub mod input;
pub mod io;
pub mod lcd;
pub mod memory;
pub mod save;
pub mod timer;

// Re-export main types for convenience
pub use emulator::Emulator;
pub use input::key_ids;
pub use lcd::{LCD_HEIGHT, LCD_WIDTH};
