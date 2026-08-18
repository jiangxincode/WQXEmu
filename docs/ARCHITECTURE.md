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
| `nc2000.rs` | NOR + NAND (`*.nor` / `*.nand`) | Skeleton (NAND/bank TODO) |

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

- No large ROM dump; firmware lives on NOR + NAND. Banks 0x00-0x1F select
  NOR pages, banks 0x80+ select extended RAM. Scaffolding only: NAND
  controller and NC2000 IO semantics are not implemented yet.

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
