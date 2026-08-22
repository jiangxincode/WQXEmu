# WQXEmu — Wenquxing Emulator

<p align="center">
  <a href="https://AloysHF.github.io/WQXEmu/"><img src="https://img.shields.io/badge/Website-WQXEmu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/AloysHF/WQXEmu/actions/workflows/ci.yml"><img src="https://github.com/AloysHF/WQXEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/AloysHF/WQXEmu/releases/latest"><img src="https://img.shields.io/github/v/release/AloysHF/WQXEmu" alt="Release"></a>
  <a href="https://github.com/AloysHF/WQXEmu/releases"><img src="https://img.shields.io/github/downloads/AloysHF/WQXEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=AloysHF_WQXEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=AloysHF_WQXEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPLv3%2B-blue.svg" alt="License: GPLv3 or later"></a>
</p>

A Wenquxing (文曲星) electronic dictionary emulator written in Rust, using Low-Level Emulation (LLE) to run real firmware. Supports NC1020, PC1000, CC800, NC2000, and NC3000 models.

## Features

- **6502/W65C02 CPU emulation** — cycle-accurate instruction execution with BCD support
- **Bank-switched memory** — full memory map with NOR/NAND flash, RAM, and IO registers
- **Multi-model architecture** — shared `Machine` trait with NC1020 / PC1000 / CC800 / NC2000 / NC3000 backends (PC1000 and CC800 boot to the main menu; NC2000/NC3000 boot to the clock screen with standby/wake)
- **LCD display** — 160×80 pixel display with model-correct framebuffer placement, 4 grayscale levels, and ghosting effects
- **Keyboard input** — complete QWERTY keyboard matrix emulation
- **Device skins and virtual keypad** — the desktop frontend embeds the live LCD in a model-specific device image; the pictured keys can be clicked with the mouse or pressed on the PC keyboard, and pressed keys are highlighted
- **Audio system** — SPDS104A DSP emulation with tone generation
- **Timer system** — multiple timer sources with interrupt generation
- **Persistent sessions** — optional compressed state files resume any supported model without modifying source dumps
- **RetroArch integration** — libretro core for use with RetroArch frontend
- **Cross-platform** — Windows, macOS, Linux, Android, iOS, and webOS

## Usage

### Standalone Mode

Download the latest binary from the [Releases](https://github.com/AloysHF/WQXEmu/releases) page and run:

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls
```

Firmware dumps are passed with options named after the storage device: `--rom`, `--nor`, `--nand`, and `--nand0`. The required combination depends on the selected model.

| Model | Required firmware | Optional firmware |
|-------|-------------------|-------------------|
| NC1020 | ROM + NOR | — |
| PC1000 | ROM + NOR | — |
| CC800 | ROM + NOR | — |
| NC2000 | NOR + NAND + NAND0 | — |
| NC3000 | NOR + NAND | NAND0 |

See the [Standalone Emulator](docs/Standalone-Emulator.md) guide for installation, keyboard controls, headless mode, screenshots, and all command-line options.

### RetroArch Mode

Install the core and load a game through RetroArch's **Load Content** menu.

See the [RetroArch Core](docs/RetroArch-Core.md) guide for installation, supported platforms, RetroPad mapping, and features.

## Building

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

### Standalone Mode (Default)

```bash
cargo build --release
cargo run --release -- --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls
```

The binary is produced at `target/release/wqxemu` (or `wqxemu.exe` on Windows).

### Libretro Core (for RetroArch)

```bash
cargo build -p wqxemu-libretro --release
```

The compiled core (`wqxemu_libretro.dll` / `libwqxemu_libretro.so` / `libwqxemu_libretro.dylib`) can be loaded in RetroArch.

For Android cross-compilation, see [Android Libretro Core](docs/Android-Libretro-Core.md).
For iOS, see [iOS Libretro Core](docs/iOS-Libretro-Core.md).

## Testing

Run the unit tests:

```bash
cargo test --workspace
```

There is also a smoke test that loads every available game, runs it for a number of frames, and checks that the emulator neither panics nor produces a blank frame. It needs the (non-distributed) game assets, so it is `#[ignore]`d by default and only runs on demand:

```bash
# Uses <repo>/tmp/games by default, or set WQX_GAME_DIR
cargo test -p wqxemu-core --test smoke -- --ignored --nocapture
```

## Architecture

```
crates/
├── wqxemu-core/               # Platform-independent emulator engine (library)
│   └── src/
│       ├── lib.rs             # Crate root
│       ├── emulator.rs        # Main emulator orchestrator
│       ├── cpu.rs             # 6502/W65C02 CPU emulation
│       ├── memory.rs          # Memory bus with bank switching
│       ├── lcd.rs             # LCD framebuffer (160×80, 4 grayscale)
│       ├── input.rs           # Keyboard input handling
│       ├── audio.rs           # Audio tone generation
│       ├── timer.rs           # Timer system with interrupts
│       ├── flash.rs           # NOR/NAND flash controller
│       ├── io.rs              # IO register handling
│       ├── save.rs            # Save state serialization
│       └── machines/          # Per-model implementations
│           ├── nc1020.rs      # NC1020 model
│           ├── pc1000.rs      # PC1000 model
│           ├── cc800.rs       # CC800 model
│           ├── nc2000.rs      # NC2000 model
│           └── nc3000.rs      # NC3000 model
├── wqxemu/                    # Standalone binary (→ wqxemu)
│   └── src/
│       └── main.rs            # Window loop and CLI frontend
└── wqxemu-libretro/           # libretro cdylib (→ wqxemu_libretro.{dll,so,dylib})
    └── src/
        └── lib.rs             # libretro API implementation
```

For detailed architecture information, see [ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Game Compatibility

For detailed game list with screenshots and compatibility status, see:
- [Game Compatibility](docs/GAME-COMPATIBILITY.md) — Game compatibility status
- [Standalone Emulator](docs/Standalone-Emulator.md) — Standalone emulator guide with all command-line options
- [RetroArch Core](docs/RetroArch-Core.md) — RetroArch integration guide

## Contributing

Contributions are welcome! Whether you're interested in fixing bugs, adding features, improving documentation, or testing game compatibility, we'd love your help. See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for details.

## Acknowledgments

- [wangyu-/NC2000](https://github.com/wangyu-/NC2000) — NC2000/NC2600/NC1020 emulator
- [Wang-Yue/NC1020](https://github.com/Wang-Yue/NC1020) — NC1020 emulator
- [hackwaly/jswqx](https://github.com/hackwaly/jswqx) — JavaScript NC1020 emulator
- [banxian/Sim800](https://github.com/banxian/Sim800) — CC800/PC1000 emulator

## License

This project is licensed under the [GNU General Public License v3.0 or later](LICENSE).
