# RetroArch Core

This guide covers using WQXEmu as a libretro core with RetroArch, including installation, supported platforms, RetroPad mapping, and features.

## Installation

### Online Updater (Recommended)

1. Open RetroArch
2. Go to **Main Menu → Online Updater → Core Downloader**
3. Select **WQXEmu** from the list

### Manual Installation

1. Download the core from the [Releases](https://github.com/AloysHF/WQXEmu/releases) page
2. Copy the core file to RetroArch's `cores/` directory:
   - Windows: `wqxemu_libretro.dll`
   - Linux: `libwqxemu_libretro.so`
   - macOS: `libwqxemu_libretro.dylib`
3. Copy the info file to RetroArch's `info/` directory:
   - `wqxemu_libretro.info`

### Build from Source

```bash
cargo build -p wqxemu-libretro --release
```

The compiled core will be at:
- Windows: `target/release/wqxemu_libretro.dll`
- Linux: `target/release/libwqxemu_libretro.so`
- macOS: `target/release/libwqxemu_libretro.dylib`

## Supported Platforms

- Windows (x86_64)
- Linux (x86_64)
- macOS (x86_64, aarch64)
- Android (arm64-v8a, armeabi-v7a)
- iOS (arm64)
- webOS

## Loading Content

### Automatic Model Detection

The core automatically detects the model based on the loaded firmware files:

1. Open RetroArch
2. Select **Load Core** → **WQXEmu**
3. Select **Load Content** and choose a firmware file
4. The core will automatically detect the model and load required firmware files

### Firmware File Requirements

| Model | Required firmware | File types |
|-------|-------------------|------------|
| NC1020 | ROM + NOR | `.bin`, `.fls` |
| PC1000 | ROM + NOR | `.rom`, `.fls` |
| CC800 | ROM + NOR | `.bin`, `.fls` |
| NC2000 | NOR + NAND + NAND0 | `.nor`, `.nand`, `.nand0` |
| NC3000 | NOR + NAND | `.nor`, `.nand` |

### Firmware File Placement

Place firmware files in RetroArch's `system/` directory:

```
system/
└── WQXEmu/
    ├── nc1020/
    │   ├── obj_lu.bin
    │   └── nc1020.fls
    ├── pc1000/
    │   ├── pc1000.rom
    │   └── pc1000.fls
    ├── cc800/
    │   ├── obj.bin
    │   └── cc800.fls
    ├── nc2000/
    │   ├── nc2000.nor
    │   ├── nc2000.nand
    │   └── nc2000.nand0
    └── nc3000/
        ├── nc3000.nor
        └── nc3000.nand
```

## RetroPad Button Mapping

| RetroPad Button | WQX Key | Action |
|-----------------|---------|--------|
| D-Pad Up | Up | Navigate up |
| D-Pad Down | Down | Navigate down |
| D-Pad Left | Left | Navigate left |
| D-Pad Right | Right | Navigate right |
| A | Enter | Confirm |
| B | Escape | Back / Cancel |
| X | — | — |
| Y | — | — |
| L1 | — | — |
| R1 | — | — |
| L2 | — | — |
| R2 | — | — |
| Select | — | — |
| Start | — | — |

## Core Options

Core options can be configured from RetroArch's **Quick Menu → Core Options**.

### Display Options

- **LCD Scale** — Scale factor for the LCD display (1x, 2x, 3x, 4x)
- **Show Grid Lines** — Show grid lines on the LCD
- **Ghosting Effect** — Enable/disable LCD ghosting effect

### Audio Options

- **Audio Volume** — Master audio volume (0-100)
- **Audio Sample Rate** — Audio sample rate (22050, 44100, 48000)

### Input Options

- **Key Repeat Delay** — Delay before key repeat starts (ms)
- **Key Repeat Interval** — Interval between key repeats (ms)

### Emulation Options

- **CPU Speed** — CPU speed multiplier (0.5x, 1x, 2x, 4x)
- **Timer Speed** — Timer speed multiplier (0.5x, 1x, 2x, 4x)

## Save States

Save states are supported through RetroArch's save state system.

### Save State

- Press **F2** or use **Quick Menu → Save State**

### Load State

- Press **F4** or use **Quick Menu → Load State**

### Important Notes

- Save states are tied to the specific model and firmware version
- Save states from different models are not compatible
- Save states are separate from persistent sessions

## Screenshots

Screenshots can be taken through RetroArch:

- Press **F8** or use **Quick Menu → Take Screenshot**

## Debug Features

### CPU Tracing

Enable CPU tracing through core options:

1. Open **Quick Menu → Core Options**
2. Enable **CPU Trace**
3. Restart the core

### IO Tracing

Enable IO register tracing:

1. Open **Quick Menu → Core Options**
2. Enable **IO Trace**
3. Restart the core

## Troubleshooting

### Common Issues

1. **"No firmware found"** — Ensure firmware files are in the correct location
2. **"Model detection failed"** — Manually select the model in core options
3. **"Black screen"** — Check firmware file integrity

### Debug Logging

Enable debug logging in RetroArch:

1. Go to **Settings → Logging**
2. Set **Logging Verbosity** to **Debug**
3. Restart RetroArch

### Core Information

View core information:

1. Go to **Quick Menu → Information**
2. Check **Core Name**, **Core Version**, and **System Name**

## Android

For Android-specific instructions, see [Android Libretro Core](Android-Libretro-Core.md).

## iOS

For iOS-specific instructions, see [iOS Libretro Core](iOS-Libretro-Core.md).
