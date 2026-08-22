# Standalone Emulator

This guide covers the standalone WQXEmu desktop application, including installation, keyboard controls, headless mode, screenshots, and all command-line options.

## Installation

### Download Pre-built Binaries

Download the latest release from the [Releases](https://github.com/AloysHF/WQXEmu/releases) page. Binaries are available for:

- Windows (x86_64)
- macOS (x86_64, aarch64)
- Linux (x86_64)

### Build from Source

Requires [Rust](https://www.rust-lang.org/tools/install) (stable).

```bash
cargo build --release
```

The binary will be at `target/release/wqxemu` (or `wqxemu.exe` on Windows).

## Quick Start

### Basic Usage

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls
```

### Supported Models

| Model | Required firmware | Optional firmware |
|-------|-------------------|-------------------|
| NC1020 | ROM + NOR | — |
| PC1000 | ROM + NOR | — |
| CC800 | ROM + NOR | — |
| NC2000 | NOR + NAND + NAND0 | — |
| NC3000 | NOR + NAND | NAND0 |

### Firmware Files

Firmware dumps are passed with options named after the storage device:

- `--rom` — System ROM (NC1020, PC1000, CC800)
- `--nor` — NOR Flash dump
- `--nand` — NAND Flash dump (NC2000, NC3000)
- `--nand0` — First NAND plane dump (NC2000, NC3000)

## Command-Line Options

### Model Selection

- `--model <MODEL>` — Select the model to emulate
  - `nc1020` (default)
  - `pc1000`
  - `cc800`
  - `nc2000`
  - `nc3000`

### Firmware Files

- `--rom <PATH>` — Path to system ROM file
- `--nor <PATH>` — Path to NOR Flash dump
- `--nand <PATH>` — Path to NAND Flash dump
- `--nand0 <PATH>` — Path to first NAND plane dump

### Display Options

- `--scale <N>` — Window scale factor (default: 4)
- `--fullscreen` — Start in fullscreen mode

### State Management

- `--state-file <PATH>` — Path to save/load compressed session state

### Headless Mode

- `--headless` — Run without a window
- `--frames <N>` — Run for N frames and exit
- `--screenshot <PATH>` — Save screenshot to file

### Audio Options

- `--no-audio` — Disable audio output

### Debug Options

- `--debug` — Enable debug logging
- `--trace-cpu` — Enable CPU instruction tracing
- `--trace-io` — Enable IO register tracing

## Keyboard Controls

### Navigation

| Key | Action |
|-----|--------|
| Arrow Up | Navigate up |
| Arrow Down | Navigate down |
| Arrow Left | Navigate left |
| Arrow Right | Navigate right |
| Enter | Confirm / Select |
| Escape | Back / Cancel / Exit |

### Emulator Controls

| Key | Action |
|-----|--------|
| F5 | Save state |
| F8 | Load state |
| F12 | Take screenshot |
| Pause | Pause / Resume emulation |

## Headless Mode

Headless mode runs the emulator without a window, useful for testing and batch processing.

### Basic Headless Usage

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --headless --frames 300
```

### Taking Screenshots

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --screenshot screenshot.png --frames 300
```

## Persistent Sessions

The `--state-file` option enables persistent sessions that save and restore emulator state.

### First Run

```bash
wqxemu --model nc2000 --nor roms/nc2000/nc2000.nor --nand roms/nc2000/nc2000.nand --nand0 roms/nc2000/nc2000.nand0 --state-file nc2000.wqxs
```

On exit, the emulator saves a compressed session state to `nc2000.wqxs`.

### Subsequent Runs

```bash
wqxemu --model nc2000 --state-file nc2000.wqxs
```

The emulator restores the saved state without re-running first-boot recovery.

### Important Notes

- Use a separate state file for each machine and firmware configuration
- States from another model are rejected
- ROM, NOR, NAND, and NAND0 source dumps remain read-only

## Examples

### NC1020

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls
```

### PC1000

```bash
wqxemu --model pc1000 --rom roms/pc1000/pc1000.rom --nor roms/pc1000/pc1000.fls
```

### CC800

```bash
wqxemu --model cc800 --rom roms/cc800/obj.bin --nor roms/cc800/cc800.fls
```

### NC2000

```bash
wqxemu --model nc2000 --nor roms/nc2000/nc2000.nor --nand roms/nc2000/nc2000.nand --nand0 roms/nc2000/nc2000.nand0
```

### NC3000

```bash
wqxemu --model nc3000 --nor roms/nc3000/nc3000.nor --nand roms/nc3000/nc3000.nand
```

## Troubleshooting

### Common Issues

1. **"ROM file not found"** — Ensure the firmware file paths are correct
2. **"Invalid model"** — Check the `--model` parameter
3. **"State file mismatch"** — Use a state file created with the same model and firmware

### Debug Logging

Enable debug logging to see detailed information:

```bash
RUST_LOG=wqxemu=debug wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls
```

### CPU Tracing

Trace CPU instructions for debugging:

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --trace-cpu
```
