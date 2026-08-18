# WQXEmu —— Wenquxing NC1020 Emulator

<p align="center">
  <a href="https://jiangxincode.github.io/WQXEmu/"><img src="https://img.shields.io/badge/Website-WQXEmu-E8553A?logo=githubpages&logoColor=white" alt="Website"></a>
  <a href="https://github.com/jiangxincode/WQXEmu/actions/workflows/ci.yml"><img src="https://github.com/jiangxincode/WQXEmu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/jiangxincode/WQXEmu/releases/latest"><img src="https://img.shields.io/github/v/release/jiangxincode/WQXEmu" alt="Release"></a>
  <a href="https://github.com/jiangxincode/WQXEmu/releases"><img src="https://img.shields.io/github/downloads/jiangxincode/WQXEmu/total" alt="Downloads"></a>
  <a href="https://sonarcloud.io/dashboard?id=jiangxincode_WQXEmu"><img src="https://sonarcloud.io/api/project_badges/measure?project=jiangxincode_WQXEmu&metric=alert_status" alt="Quality Gate Status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPLv3%2B-blue.svg" alt="License: GPLv3 or later"></a>
</p>

文曲星 NC1020 电子辞典模拟器，使用 Rust 编写，采用低级模拟 (LLE) 方式运行真实固件。

A Wenquxing NC1020 electronic dictionary emulator written in Rust, using Low-Level Emulation (LLE) to run real firmware.

## Status

> **Under development.** Core emulation is functional but the project is still a work in progress.

> **开发中。** 核心模拟功能已可用，但项目仍在积极开发中。

## Features

- **6502/W65C02 CPU emulation** — cycle-accurate instruction execution with BCD support
- **Bank-switched memory** — full memory map with NOR/NAND flash, RAM, and IO registers
- **LCD display** — 160×80 pixel display with 4 grayscale levels and ghosting effects
- **Keyboard input** — complete NC1020 QWERTY keyboard matrix emulation
- **Audio system** — SPDS104A DSP emulation with tone generation
- **Timer system** — multiple timer sources with interrupt generation
- **RetroArch integration** — libretro core for use with RetroArch frontend
- **Cross-platform** — Windows, macOS, Linux, Android, iOS, and webOS

## Quick Start

```bash
# Build and run the standalone emulator
cargo run --release -- path/to/nc1020.rom

# Build the libretro core for RetroArch
cargo build -p wqxemu-libretro --release
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
