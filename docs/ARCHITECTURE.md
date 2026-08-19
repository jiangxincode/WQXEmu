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
| `pc1000.rs` | 12MB ROM (`pc1000.rom` = obj1+obj2+obj3) + 512KB NOR | Complete, boots to menu, keyboard + NOR work |
| `cc800.rs` | 16MB ROM (`obj.bin`) + 512KB NOR | Complete, boots to menu, keyboard + NOR work |
| `nc2000.rs` | NOR + NAND (`*.nor` / `*.nand`) | Boots to clock screen, standby/wake works |

### NC1020

- 24MB ROM split into three 8MB "volumes"; bank 0x80-0xFF indexes file
  banks directly (no 0x80 offset).
- 16KB halves of each 32KB bank are swapped in the dump; both ROM and NOR
  are swapped on load.
- BBS pages selectable via IO register 0x0A; volume via 0x0D.

### PC1000

- The ROM dump is stored linearly (no 16KB half swap, unlike the NC1020).
  12MB = obj1 + obj2 + obj3; volume 0 covers obj1+obj2 (8MB) and volume 1
  covers obj1 + obj3. The 16MB Android/PC1000EMUX buffer layout is also
  accepted.
- Bank window (0x4000-0xBFFF) with register 0x00; page order is
  `{+0x4000, +0x6000, +0x0000, +0x2000}` inside each 32KB bank. With
  ROA (0x0A bit7) set the window selects one of the 16 NOR banks; with
  ROA clear it selects a BROM bank (0x0D bit0 picks volume 1).
- Bank 0 with ROA clear maps 0x4000-0x7FFF onto internal RAM (0x6000 is
  a mirror of 0x4000) so the firmware can copy itself into RAM at boot.
- 0xC000-0xDFFF is the BBS page (0x0A bits 0-3); page 1 is internal RAM.
  0xE000-0xFFFF is the fixed BIOS page (volume bank 0 + 0x6000); the
  reset vector for the official 3.9/3.5 firmware is 0xFFF4.
- IO follows the PC1000EMUX register model: timer start/stop via reads of
  0x04-0x07, interrupt status at 0x01 (clear-on-read), timer A/B at
  0x10-0x14, keypad ports 0x08/0x09/0x0B/0x0F/0x15, LCD address from
  register 0x06 (address << 4), DSP at 0x20-0x23, beeper at 0x18.
- Periodic sources: timer0/1 and timer A at 576*50 Hz ticks, time base at
  ~250 Hz (every 115 ticks), NMI at 2 Hz. Interrupt enable is the shadow
  register 0x40; status register 0x01 keeps its two high bits on read.
- NOR controller: SPR4096 command sequences (software ID 0xBF/0xD7,
  byte program, 4KB block erase, mass erase, info block). NOR dumps may
  be stored linear or with the half-swapped NC1020 convention (detected
  automatically and unswapped on load; `save_nor` restores the original
  layout).
- Keypad: 8x8 matrix; rows 0-5 are scanned through port 1 writes, rows
  6-7 (hotkeys and power) are read through port 3 with the port
  direction registers. Verified with the official PC1000 3.9 firmware:
  cold boot draws the main menu, arrows change the selection, and NOR
  user data persists.

### CC800

- The CC800 is the older sibling of the PC1000 and shares the SPDC1016
  SoC. Its 16MB `obj.bin` ROM and 512KB NOR are stored with the
  half-swapped 16KB bank convention (like the NC1020) and are unswapped
  on load.
- Bank window (0x4000-0xBFFF) page order is `{+0x0000, +0x2000, +0x4000,
  +0x6000}` inside each 32KB bank — different from the PC1000. With ROA
  set the window selects one of the 16 NOR banks; with ROA clear it
  selects a BROM bank (0x0D bit0 picks volume 1, the second 8MB).
- Bank 0 with ROA clear maps 0x4000-0x7FFF onto internal RAM (0x6000
  mirrors 0x4000) so the firmware can copy itself into RAM at boot;
  volume 1 bank 0 maps 0x4000-0x7FFF onto NOR page 0 instead.
- 0xC000-0xDFFF is the BBS page (0x0A bits 0-3); page 1 is internal RAM
  (or NOR + 0x2000 with volume 1). 0xE000-0xFFFF is the fixed BIOS page
  (volume bank 0 + 0x2000); the reset vector is 0xFFF4.
- IO follows the Sim800 register model: timer start/stop via reads of
  0x04-0x07 (returning the corresponding IO register), interrupt status
  at 0x01 (clear-on-read), timer A/B at 0x10-0x14, keypad ports
  0x08/0x09/0x0F/0x15 with an 8x8 port-conduction matrix, LCD address
  from register 0x06 (address << 4), JG WAV control at 0x20, beeper at
  0x18.
- Periodic sources match the shared SPDC1016 model: timer0/1 and timer A
  at 576*50 Hz ticks, time base at ~250 Hz, NMI at 2 Hz.
- Standby/wake: when the firmware clears the LCD clock (0x05 low
  nibble) the device enters standby; pressing the power/hotkey rows
  triggers a watchdog warm reset.
- Keypad layout is identical to the PC1000 (same matrix positions), so
  the desktop frontend reuses the PC1000 key mapping. Verified with the
  official CC800 2.x firmware: cold boot draws the main menu and arrows
  change the selection.

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
  12MB ROM → PC1000; 16MB ROM with a volume-1 boot page at 8MB → CC800;
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
