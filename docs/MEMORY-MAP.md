# Memory Map

This document describes the memory map for the Wenquxing (文曲星) emulated hardware.

## Overview

All Wenquxing models use a 6502 CPU with a 64KB address space (0x0000-0xFFFF). The memory is bank-switched, with different regions mapped to different hardware components.

## Common Memory Regions

| Address Range | Size | Description |
|---------------|------|-------------|
| 0x0000-0x0FFF | 4KB | Internal RAM (zero page, stack, etc.) |
| 0x1000-0x1FFF | 4KB | Internal RAM |
| 0x2000-0x3FFF | 8KB | Internal RAM (some models) |
| 0x4000-0xBFFF | 32KB | Bank-switched window (ROM/RAM) |
| 0xC000-0xDFFF | 8KB | BBS (Bank-Banked Storage) page |
| 0xE000-0xFFFF | 8KB | Fixed BIOS page |

## Bank-Switched Window (0x4000-0xBFFF)

The bank-switched window is the main area where ROM and RAM are mapped. The specific mapping depends on the model and bank register settings.

### Bank Register 0x00

- **NC1020**: Selects ROM bank (0x80-0xFF indexes file banks directly)
- **PC1000**: Selects ROM bank with page order `{+0x4000, +0x6000, +0x0000, +0x2000}`
- **CC800**: Selects ROM bank with page order `{+0x0000, +0x2000, +0x4000, +0x6000}`
- **NC2000**: Selects NOR bank (0x00-0x0F)
- **NC3000**: Selects NOR bank (0x00-0x1F)

### Bank Register 0x0A

- **Bits 0-3**: BBS page selector
- **Bit 7**: ROA (ROM/RAM Access) flag
  - 0 = ROM access
  - 1 = RAM access

### Bank Register 0x0D

- **Bit 0**: Volume selector (for models with multiple ROM volumes)
- **Bit 2**: RAMB selector

## BBS Page (0xC000-0xDFFF)

The BBS (Bank-Banked Storage) page is an 8KB window that can be mapped to different memory regions.

### BBS Pages

| Page | Description |
|------|-------------|
| 0 | Default ROM/RAM |
| 1 | Internal RAM |
| 2-15 | Model-specific |

## Fixed BIOS Page (0xE000-0xFFFF)

The fixed BIOS page is always mapped to the same location. It contains:

- **Reset vector**: 0xFFFC-0xFFFD
- **IRQ vector**: 0xFFFE-0xFFFF
- **NMI vector**: 0xFFFA-0xFFFB
- **BIOS code**: System initialization and interrupt handlers

## Model-Specific Memory Maps

### NC1020

- **ROM**: 24MB split into three 8MB volumes
- **NOR**: 1MB
- **RAM**: 24KB internal + 32KB external + 4KB SPR4096
- **Bank 0x80-0xFF**: Direct file bank indexing

### PC1000

- **ROM**: 12MB (obj1 + obj2 + obj3)
- **NOR**: 512KB
- **RAM**: 24KB internal + 32KB external + 4KB SPR4096
- **Bank 0x00-0x3F**: ROM banks
- **Bank 0x40-0x7F**: NOR banks (with ROA set)

### CC800

- **ROM**: 16MB
- **NOR**: 512KB
- **RAM**: 24KB internal + 32KB external + 4KB SPR4096
- **Bank 0x00-0x3F**: ROM banks
- **Bank 0x40-0x7F**: NOR banks (with ROA set)

### NC2000

- **NOR**: 512KB (16 x 32KB banks)
- **NAND**: 32MB
- **RAM**: 24KB internal + 32KB external + 4KB SPR4096
- **Bank 0x00-0x0F**: NOR banks
- **Bank 0x80-0xFF**: Extended RAM

### NC3000

- **NOR**: 1MB (32 x 32KB banks)
- **NAND**: ~66MB
- **RAM**: 24KB internal + 32KB external + 4KB SPR4096
- **Bank 0x00-0x1F**: NOR banks
- **Bank 0x80-0xFF**: Extended RAM

## Memory Banking Details

### NC1020 Memory Banking

The NC1020 uses a unique memory banking scheme:

1. **ROM Volumes**: Three 8MB volumes (0, 1, 2)
2. **Bank Selection**: Register 0x00 selects bank (0x80-0xFF)
3. **Volume Selection**: Register 0x0D bit 0 selects volume
4. **16KB Half-Swap**: ROM and NOR dumps are swapped on load

### PC1000 Memory Banking

The PC1000 uses a more complex banking scheme:

1. **Bank Window**: 0x4000-0xBFFF (32KB)
2. **Page Order**: `{+0x4000, +0x6000, +0x0000, +0x2000}`
3. **ROA Flag**: Register 0x0A bit 7
4. **Volume Selection**: Register 0x0D bit 0

### CC800 Memory Banking

The CC800 uses a simpler banking scheme:

1. **Bank Window**: 0x4000-0xBFFF (32KB)
2. **Page Order**: `{+0x0000, +0x2000, +0x4000, +0x6000}`
3. **ROA Flag**: Register 0x0A bit 7
4. **Volume Selection**: Register 0x0D bit 0

### NC2000 Memory Banking

The NC2000 uses NOR + NAND banking:

1. **NOR Banks**: 16 x 32KB banks (0x00-0x0F)
2. **Extended RAM**: Banks 0x80+ select extended RAM
3. **Fixed BIOS**: 0xE000-0xFFFF maps to NOR bank 0

### NC3000 Memory Banking

The NC3000 uses NOR + NAND banking:

1. **NOR Banks**: 32 x 32KB banks (0x00-0x1F)
2. **Extended RAM**: Banks 0x80+ select extended RAM
3. **Fixed BIOS**: 0xE000-0xFFFF maps to NOR bank 0 + 0x6000

## Memory Access Modes

### ROM Access

When ROA flag (0x0A bit 7) is clear:
- Bank window maps to ROM banks
- BBS page maps to ROM/RAM based on BBS page selector

### RAM Access

When ROA flag (0x0A bit 7) is set:
- Bank window maps to RAM banks
- BBS page maps to RAM

## Notes

1. **Bank switching is model-specific** — Each model has its own banking scheme
2. **ROA flag affects memory mapping** — Changes how bank window and BBS page are mapped
3. **Fixed BIOS page is always present** — Contains reset and interrupt vectors
4. **Memory sizes vary by model** — Different models have different ROM/NOR/RAM sizes
