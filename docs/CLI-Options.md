# Command-Line Options

This document lists all available command-line options for the WQXEmu standalone emulator.

## Usage

```bash
wqxemu [OPTIONS] [COMMAND]
```

## Options

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

### Headless Mode

```bash
# Run for 300 frames and exit
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --headless --frames 300

# Take a screenshot
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --screenshot screenshot.png --frames 300

# Run headless without audio
wqxemu --model nc1020 --rom roms/nc1020/obj_lu.bin --nor roms/nc1020/nc1020.fls --headless --no-audio
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

## Notes

1. **Firmware files are required** — The emulator cannot run without firmware files
2. **State files are optional** — You can run without a state file
3. **Headless mode is for testing** — Use `--headless` for automated testing
4. **Debug options are verbose** — Use with caution in production
5. **Scale factor affects performance** — Higher scale factors may be slower
