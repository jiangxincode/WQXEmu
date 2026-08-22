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

### Usage

```bash
wqxemu [OPTIONS] [COMMAND]
```

### Model Selection

- `--model <MODEL>` — Select the model to emulate
  - `nc1020` (default)
  - `pc1000`
  - `cc800`
  - `nc2000`
  - `nc3000`

### Firmware Files

- `--rom <PATH>` — Path to system ROM file
  - Used by: NC1020, PC1000, CC800
- `--nor <PATH>` — Path to NOR Flash dump
  - Used by: All models
- `--nand <PATH>` — Path to NAND Flash dump
  - Used by: NC2000, NC3000
- `--nand0 <PATH>` — Path to first NAND plane dump
  - Used by: NC2000, NC3000

### Display Options

- `--scale <N>` — Window scale factor (default: 4)
  - Range: 1-8
- `--fullscreen` — Start in fullscreen mode

### State Management

- `--state-file <PATH>` — Path to save/load compressed session state
  - Creates a gzip-compressed session state file
  - Restores state on subsequent runs

### Headless Mode

- `--headless` — Run without a window
  - Useful for testing and batch processing
- `--frames <N>` — Run for N frames and exit
  - Only works with `--headless`
- `--screenshot <PATH>` — Save screenshot to file
  - Only works with `--headless`

### Audio Options

- `--no-audio` — Disable audio output
  - Useful for headless mode or testing

### Debug Options

- `--debug` — Enable debug logging
  - Shows detailed information about emulator operation
- `--trace-cpu` — Enable CPU instruction tracing
  - Shows each CPU instruction executed
- `--trace-io` — Enable IO register tracing
  - Shows IO register reads and writes
- `--trace-bank` — Enable bank switching tracing
  - Shows bank switch operations

### Help

- `-h, --help` — Print help information
- `-V, --version` — Print version information

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

### Run Headless Without Audio

```bash
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --headless --no-audio
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

### Basic Usage

```bash
# NC1020 with ROM and NOR
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls

# PC1000 with ROM and NOR
wqxemu --model pc1000 --rom roms/pc1000/pc1000.rom --nor roms/pc1000/pc1000.fls

# CC800 with ROM and NOR
wqxemu --model cc800 --rom roms/cc800/obj.bin --nor roms/cc800/cc800.fls

# NC2000 with NOR, NAND, and NAND0
wqxemu --model nc2000 --nor roms/nc2000/nc2000.nor --nand roms/nc2000/nc2000.nand --nand0 roms/nc2000/nc2000.nand0

# NC3000 with NOR and NAND
wqxemu --model nc3000 --nor roms/nc3000/nc3000.nor --nand roms/nc3000/nc3000.nand
```

### Display Options

```bash
# 2x scale
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --scale 2

# Fullscreen mode
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --fullscreen
```

### State Management

```bash
# First run with state file
wqxemu --model nc2000 --nor roms/nc2000/nc2000.nor --nand roms/nc2000/nc2000.nand --nand0 roms/nc2000/nc2000.nand0 --state-file nc2000.wqxs

# Subsequent runs (firmware files are not needed)
wqxemu --model nc2000 --state-file nc2000.wqxs
```

### Debug Options

```bash
# Enable debug logging
RUST_LOG=wqxemu=debug wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls

# Enable CPU tracing
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --trace-cpu

# Enable IO tracing
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --trace-io

# Enable bank switching tracing
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --trace-bank
```

## Environment Variables

- `RUST_LOG` — Set logging level
  - `wqxemu=debug` — Enable debug logging
  - `wqxemu=trace` — Enable trace logging
  - `wqxemu=info` — Enable info logging (default)
  - `wqxemu=warn` — Enable warning logging
  - `wqxemu=error` — Enable error logging

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

## Notes

1. **Firmware files are required** — The emulator cannot run without firmware files
2. **State files are optional** — You can run without a state file
3. **Headless mode is for testing** — Use `--headless` for automated testing
4. **Debug options are verbose** — Use with caution in production
5. **Scale factor affects performance** — Higher scale factors may be slower
