# WQXEmu —— Wenquxing NC1020 Emulator

<p align="center">
  <a href="https://AloysHF.github.io/WQXEmu/"><img src="https://img.shields.io/badge/Website-WQXEmu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/AloysHF/WQXEmu/actions/workflows/ci.yml"><img src="https://github.com/AloysHF/WQXEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/AloysHF/WQXEmu/releases/latest"><img src="https://img.shields.io/github/v/release/AloysHF/WQXEmu" alt="Release"></a>
  <a href="https://github.com/AloysHF/WQXEmu/releases"><img src="https://img.shields.io/github/downloads/AloysHF/WQXEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=AloysHF_WQXEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=AloysHF_WQXEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPLv3%2B-blue.svg" alt="License: GPLv3 or later"></a>
</p>

文曲星电子辞典模拟器，使用 Rust 编写，采用低级模拟 (LLE) 方式运行真实固件。目前支持 NC1020、PC1000、CC800、NC2000 和 NC3000。

A Wenquxing electronic dictionary emulator written in Rust, using Low-Level Emulation (LLE) to run real firmware. NC1020, PC1000, CC800, NC2000 and NC3000 are all supported.

## Status

> **Under development.** Core emulation is functional but the project is still a work in progress.

> **开发中。** 核心模拟功能已可用，但项目仍在积极开发中。

## Features

- **6502/W65C02 CPU emulation** — cycle-accurate instruction execution with BCD support
- **Bank-switched memory** — full memory map with NOR/NAND flash, RAM, and IO registers
- **Multi-model architecture** — shared `Machine` trait with NC1020 / PC1000 / CC800 / NC2000 / NC3000 backends (PC1000 and CC800 boot to the main menu; NC2000/NC3000 boot to the clock screen with standby/wake)
- **LCD display** — 160×80 pixel display with model-correct framebuffer placement, 4 grayscale levels, and ghosting effects
- **Keyboard input** — complete QWERTY keyboard matrix emulation
- **Device skins and virtual keypad** — the desktop frontend embeds the
  live LCD in a model-specific device image; the pictured keys can be
  clicked with the mouse or pressed on the PC keyboard, and pressed keys
  are highlighted
- **Audio system** — SPDS104A DSP emulation with tone generation
- **Timer system** — multiple timer sources with interrupt generation
- **Persistent sessions** — optional compressed state files resume any supported model without modifying source dumps
- **RetroArch integration** — libretro core for use with RetroArch frontend
- **Cross-platform** — Windows, macOS, Linux, Android, iOS, and webOS

## Quick Start

```bash
# The commands below assume a roms/ directory at the repository root
# (the test dumps are already there locally; ROMs are not distributed):

# NC1020 (24MB ROM + 1MB NOR)
cargo run --release -- roms/nc1020/obj_lu.bin -n roms/nc1020/nc1020.fls

# NC2000 boots from NOR + NAND (the first-plane dump is required).
cargo run --release -- --model nc2000 -n roms/nc2000/nc2000.nor --nand-path roms/nc2000/nc2000.nand --nand0-path roms/nc2000/nc2000.nand0

# PC1000 (12MB ROM: obj1 + obj2 + obj3, plus 512KB NOR)
cargo run --release -- --model pc1000 roms/pc1000.rom -n roms/pc1000.nor

# CC800 (16MB ROM: obj.bin, plus 512KB NOR)
cargo run --release -- --model cc800 roms/cc800/obj.bin -n roms/cc800/cc800.fls

# NC3000 boots from NOR + NAND (1MB NOR + ~66MB NAND)
cargo run --release -- --model nc3000 -n roms/nc3000/nc3000.nor --nand-path roms/nc3000/nc3000.nand

# Run headless for 300 frames and save a screenshot to the given path
cargo run --release -- roms/nc1020/obj_lu.bin -n roms/nc1020/nc1020.fls --screenshot screenshot.png --screenshot-frames 300

# Build the libretro core for RetroArch
cargo build -p wqxemu-libretro --release
```

ROM dumps are not distributed with the repository (the local `roms/`
directory is git-ignored); prepare your own dumps using the layout above.
`--state-file PATH` works with every supported model. If the file exists, the
desktop frontend restores it after loading the configured dumps; on a normal
exit it atomically replaces only that compressed state file. ROM, NOR, NAND,
and NAND0 source dumps remain read-only. Use a separate state file for each
machine and firmware configuration; states from another model are rejected.

```bash
# Pass --state-file on the first run to create a compressed session state on exit.
# Reuse the same state file on later runs to skip first-boot recovery.
cargo run --release -- --model nc2000 -n roms/nc2000/nc2000.nor --nand-path roms/nc2000/nc2000.nand --nand0-path roms/nc2000/nc2000.nand0 --state-file nc2000.wqxs
```

## Building

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))

### Desktop

```bash
cargo build --release
```

### RetroArch Core

```bash
cargo build -p wqxemu-libretro --release
```

The compiled core (`wqxemu_libretro.dll` / `libwqxemu_libretro.so` / `libwqxemu_libretro.dylib`) can be loaded in RetroArch.

## Architecture

| Component | Specification |
|-----------|---------------|
| **CPU** | 6502/W65C02 @ 5 MHz (SPDC1024 SoC) |
| **RAM** | 24K internal + 32K external + 4K SPR4096 |
| **NOR Flash** | 512K × 8-bit (SPR4096) |
| **NAND Flash** | 32M × 8-bit |
| **Display** | 160×80 LCD, 4 grayscale levels |
| **Audio** | SPDS104A DSP |
| **Input** | QWERTY keyboard matrix |

## License

This project is licensed under the [GNU General Public License v3.0 or later](LICENSE).
