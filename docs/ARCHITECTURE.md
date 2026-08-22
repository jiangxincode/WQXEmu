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
- `save_persistent_state()` / `load_persistent_state()` — complete
  model-specific state used for cross-process sessions
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
| `nc3000.rs` | 1MB NOR + ~66MB NAND (`*.nor` / `*.nand`) | Boots to clock screen, standby/wake works |

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

### NC3000

- The NC3000 is the faster sibling of the NC2000 (10.24 MHz CPU) and
  shares its NOR + NAND boot model. The NOR is 1MB (32 x 32KB banks) and
  the NAND covers two planes (~66MB plus an optional 64-page first-plane
  dump). The main NAND follows those 64 pages; when `nand0` is absent, the
  emulator initializes the NC3000 marker required by the firmware.
- NC3000 overrides the generic frame loop with its native 64 Hz schedule. Each
  frame runs 1202 CPU slices of 128 cycles and places the four 256 Hz timebase
  interrupts at fixed slice boundaries. The RTC subsecond register is derived
  from the current frame and slice, with 2 Hz IRQ/NMI events at phases 0 and 32.
- Memory map: 0x2000-0x3FFF is always RAM (no RAMB); banks 0x00-0x1F
  select NOR pages; banks 0x80+ are invalid (no extended RAM). Bank 0/1
  with ROA set map the whole 0x4000-0xBFFF window onto internal RAM
  pages (bank 0 -> ram00/02/04/06, bank 1 -> ram08/0A/0C/0E); bank 0
  without ROA maps 0x4000-0x7FFF onto RAM 0x4000/0x6000.
- 0xC000-0xDFFF is the BBS page (page 1 is RAM 0x6000-0x7FFF);
  0xE000-0xFFFF is the fixed BIOS page (NOR bank 0 + 0x6000). The reset
  vector of the official firmware is 0xFFF4.
- IO follows the SPDC1016 model shared with the NC2000, with two
  differences: the NAND data register is 0x39 (not 0x29) and the NAND
  control bits in port 4 are CLE=bit5, ALE=bit4, CE=bit2. The keypad has
  a port-6 extension at 0x1E.
- NOR block erase is 4KB. NAND commands (read/program/erase/status/ID)
  are shared with the NC2000.
- The one-bit LCD framebuffer is read from its fixed internal RAM window at
  0x19C0; the register-derived address is not used as the host buffer start.
- Standby/wake: the emulated 256 Hz RTC advances the firmware-visible `TR_MS`
  register. The firmware inactivity counter is reset at frame phase zero once
  per second, while an explicit ON/OFF press can still switch the clock off
  (CKS=7). The frontend's logical power key (0,0) triggers a warm reset and is
  translated to the NC3000 hardware scan position (4,0), allowing the firmware
  to observe the held key during startup.
- RTC/UART register 0x3D exposes the prioritized 2 Hz, sample, and alarm
  interrupt vectors. RCR1 writes clear the selected vectors instead of leaving
  a permanently pending IRQ.
- The one-bit LCD framebuffer is published after every completed machine frame,
  matching the native display loop without cross-frame sampling or blending.
- Verified with the official NC3000 firmware: both first-boot choices remain
  visible without frame alternation, and an explicit ON/OFF power cycle wakes
  to a persistently visible screen.

### NC2000

- No large ROM dump; firmware lives on NOR + NAND. Banks 0x00-0x0F select
  NOR pages (16 x 32KB), banks 0x80+ select extended RAM, and the fixed
  BIOS page at 0xE000-0xFFFF is NOR bank 0.
- NAND controller: 528-byte pages (512 main + 16 spare). Physical pages 0-63
  come from the `nand0` first-plane dump, followed by the main `nand` dump.
  Command sequencing uses IO 0x18 (CLE/ALE/CE) and data IO 0x29. Supports read
  (0x00/0x01/0x50), program (0x80/0x10), erase (0x60/0xD0), status (0x70)
  and ID (0x90).
- NOR controller: SPR4096 command sequences (software ID, byte program,
  block/mass erase, info block).
- IO: SPDC1016 register model — timers (0x04-0x07, 0x0C, 0x10-0x14),
  keypad ports (0x08/0x09/0x18), LCD address (0x06/0x0B/0x0C), RTC
  (0x3E/0x3F), banked UART/interrupt-vector registers (0x3A-0x3D), DSP
  (0x30-0x33), battery (0x1C). RTC interrupt acknowledge bits clear their
  matching pending vectors.
- The one-bit LCD framebuffer is read from its fixed internal RAM window at
  0x19C0, preventing the bottom 20 rows from being displaced off-screen.
- Keypad: 8x8 matrix with port conduction emulation driven by port
  direction registers.
- Standby/wake: firmware switches the clock off (io[0x05] CKS=7) to enter
  standby; the CPU is suspended and a key press on matrix columns 0/1
  triggers a warm reset that restores the clock and restarts the CPU.
- Persistent sessions include NC2000 NOR, NAND, RAM, registers, timers and
  peripheral state, so a restored process resumes without rerunning first-boot
  recovery. Source Flash dumps remain unchanged.
- UART data transfer / infrared and the NC2000-specific NAND file system are
  not emulated yet; verified with the official NC2000 3.5 dump: the firmware
  completes first-boot recovery, reaches the menu, enters standby and wakes
  on key press.

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
kept as a convenience for the NC1020 case. The frame loop calls the machine's
`run_frame()` implementation; the default uses `cycles_per_frame()`, while
models with hardware-specific scheduling can override the whole frame.

The standalone frontend's optional `--state-file` path stores a gzip-compressed,
versioned persistent session. The envelope binds the session to its hardware
model and immutable firmware identity, while each `Machine` implementation
serializes all mutable model-specific storage and peripheral state. Writes use
a temporary file in the destination directory followed by an atomic replace.
This path is separate from the compact Save State API used by libretro, so
libretro serialization size and compatibility are unchanged.

## Model selection

- Standalone frontend: `--model nc1020|pc1000|cc800|nc2000|nc3000`, or
  auto-detection via `detect_model(&files)` (NAND > 60MB or 1MB NOR →
  NC3000; NAND present → NC2000; 24MB ROM → NC1020; 12MB ROM → PC1000;
  16MB ROM with a volume-1 boot page at 8MB → CC800; otherwise NC1020
  default).
- libretro core: classifies the loaded file by extension
  (`.nand`/`.nand0`/`.fls`/`.nor`/other) and picks up sibling files with
  the same stem, then runs the same auto-detection.

## Keyboard layouts (`keyboard.rs`)

The frontend embeds the live LCD in the matching model image from
`res/`. `keyboard.rs` is the single source of truth for the model-specific
key matrices: each `KeyDef` carries the matrix position (`row << 3 | col`),
the key-face label and the PC key hint. The desktop frontend (`wqxemu`)
maps those keys to their positions on the device skin, highlights pressed
keys, and accepts mouse clicks; the PC keyboard mapping in `main.rs`
updates the same highlight state.

## Adding a new model

1. Add a variant to `MachineModel` in `machine.rs`.
2. Create `machines/<model>.rs` implementing `Machine`.
3. Register it in `machines::create_machine`.
4. Extend `detect_model` if the ROM files allow auto-detection.
5. Update the frontend CLI help and this document.
