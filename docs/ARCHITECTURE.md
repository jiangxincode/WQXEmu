# WQXEmu Architecture

## Overview

WQXEmu emulates multiple Wenquxing (文曲星) models. All supported models
are built around the same SPDC1024-era 6502 SoC family and share the CPU,
LCD, keyboard matrix, timers, audio and NOR Flash controller. The models
differ in memory banking, IO register semantics and ROM file layout.

To keep the frontends model-agnostic, the core is split into:

- **Shared components** — used by every machine.
- **`Machine` trait** — abstracts model-specific hardware.
- **Per-model implementations** under `machines/`.
- **`Emulator` shell** — owns the shared CPU and the frame loop, and
  delegates everything else to the active `Machine`.

## Shared components (`crates/wqxemu-core/src/`)

| Module | Description |
|--------|-------------|
| `cpu.rs` | 6502/W65C02 CPU with cycle-accurate timing |
| `lcd.rs` | 160x80 LCD controller |
| `input.rs` | 8x8 keyboard matrix + sleep/wake logic |
| `timer.rs` | Timer0 (2 Hz) / Timer1 (256 Hz) + RTC |
| `audio.rs` | JG WAV audio |
| `flash.rs` | SPR4096 NOR Flash controller |
| `save.rs` | Save state serialization |
| `io.rs` | IO handler helpers used by machines |
| `memory.rs` | NC1020 memory banking (used by the NC1020 machine) |

## Machine abstraction (`machine.rs`)

`Machine` is the trait a model must implement:

- `model()` / `MachineModel` — stable model identifier
  (`nc1020`, `pc1000`, `nc2000`)
- `load_rom(&RomFiles)` — load ROM / NOR / NAND dumps
- `reset()` / `step(&mut Cpu)` — reset and execute one instruction
- `end_of_frame(&mut Cpu)` — per-frame LCD copy and wake-up handling
- `peek()` / `peek_u16()` — debug reads
- `lcd()` / `drain_audio()` — framebuffer and audio access
- `save_state()` / `load_state()` — save/load support
- `load_nor()` / `save_nor()` / `set_speed_up()` — optional overrides

`RomFiles` carries the file paths for each storage device:

```rust
pub struct RomFiles {
    pub rom: Option<PathBuf>,   // NC1020/PC1000 system ROM
    pub nor: Option<PathBuf>,   // NOR Flash dump
    pub nand: Option<PathBuf>,  // NC2000 NAND dump
    pub nand0: Option<PathBuf>, // NC2000 first NAND plane
}
```

## Model implementations (`machines/`)

| Model | ROM files | Status |
|-------|-----------|--------|
| `nc1020.rs` | 24MB ROM (`obj_lu.bin`) + 1MB NOR | Complete, boots to menu |
| `pc1000.rs` | ROM + NOR | Skeleton (IO/bank semantics TODO) |
| `nc2000.rs` | NOR + NAND (`*.nor` / `*.nand`) | Boots to clock screen, standby/wake works |

### NC1020

- 24MB ROM split into three 8MB "volumes"; bank 0x80-0xFF indexes file
  banks directly (no 0x80 offset).
- 16KB halves of each 32KB bank are swapped in the dump; both ROM and NOR
  are swapped on load.
- BBS pages selectable via IO register 0x0A; volume via 0x0D.

### PC1000

- Different bank window layout and IO semantics (interrupt enable/status,
  timers, ports). Scaffolding only: the register map is defined in
  `machines/pc1000.rs` under `io_map`, the bus behaviour is not
  implemented yet.

### NC2000

- No large ROM dump; firmware lives on NOR + NAND. Banks 0x00-0x0F select
  NOR pages (16 x 32KB), banks 0x80+ select extended RAM, and the fixed
  BIOS page at 0xE000-0xFFFF is NOR bank 0.
- NAND controller: 528-byte pages (512 main + 16 spare), command sequence
  via IO 0x18 (CLE/ALE/CE) and data via IO 0x29. Supports read
  (0x00/0x01/0x50), program (0x80/0x10), erase (0x60/0xD0), status (0x70)
  and ID (0x90).
- NOR controller: SPR4096 command sequences (software ID, byte program,
  block/mass erase, info block).
- IO: SPDC1016 register model — timers (0x04-0x07, 0x0C, 0x10-0x14),
  keypad ports (0x08/0x09/0x18), LCD address (0x06/0x0B/0x0C), RTC
  (0x3A-0x3F), DSP (0x30-0x33), battery (0x1C).
- Keypad: 8x8 matrix with port conduction emulation driven by port
  direction registers.
- Standby/wake: firmware switches the clock off (io[0x05] CKS=7) to enter
  standby; the CPU is suspended and a key press on matrix columns 0/1
  triggers a warm reset that restores the clock and restarts the CPU.
- UART / infrared and the NC2000-specific NAND file system are not
  emulated yet; verified with the official NC2000 3.5 dump: the firmware
  boots, draws the clock screen, enters standby and wakes on key press.

## `Emulator` shell (`emulator.rs`)

The shell exposes the frontend API and keeps the CPU:

```rust
pub struct Emulator {
    pub cpu: Cpu,
    machine: Box<dyn Machine>,
    frame_count: u64,
    speed_up: bool,
}
```

`Emulator::new(model, &files)` instantiates the machine, loads its ROMs
and initializes the CPU from the machine's reset vector. `from_rom` is
kept as a convenience for the NC1020 case. The frame loop calls
`machine.step()` until `CYCLES_PER_FRAME` and then
`machine.end_of_frame()`.

## Model selection

- Standalone frontend: `--model nc1020|pc1000|nc2000`, or auto-detection
  via `detect_model(&files)` (NAND present → NC2000; 24MB ROM → NC1020;
  otherwise NC1020 default).
- libretro core: classifies the loaded file by extension
  (`.nand`/`.nand0`/`.fls`/`.nor`/other) and picks up sibling files with
  the same stem, then runs the same auto-detection.

## Adding a new model

1. Add a variant to `MachineModel` in `machine.rs`.
2. Create `machines/<model>.rs` implementing `Machine`.
3. Register it in `machines::create_machine`.
4. Extend `detect_model` if the ROM files allow auto-detection.
5. Update the frontend CLI help and this document.
