# IO Registers

This document describes the IO registers for the Wenquxing (文曲星) emulated hardware.

## Overview

The Wenquxing models use memory-mapped IO registers in the address range 0x00-0x3F. These registers control various hardware components including timers, interrupts, LCD, keyboard, audio, and bank switching.

## Common IO Registers

| Address | Name | Description |
|---------|------|-------------|
| 0x00 | BANK | Bank switch register |
| 0x01 | ISR | Interrupt status register (clear-on-read) |
| 0x02 | IER | Interrupt enable register |
| 0x03 | — | Reserved |
| 0x04 | TM0 | Timer 0 control |
| 0x05 | TM1 | Timer 1 control |
| 0x06 | LCD_ADDR | LCD address register |
| 0x07 | — | Reserved |
| 0x08 | KB_ROW | Keyboard row register |
| 0x09 | KB_COL | Keyboard column register |
| 0x0A | BBS | BBS page selector + ROA flag |
| 0x0B | — | Reserved |
| 0x0C | — | Reserved |
| 0x0D | VOL | Volume/RAMB select register |
| 0x0E | — | Reserved |
| 0x0F | KB_EXT | Keyboard extension register |
| 0x10 | TM_A | Timer A control |
| 0x11 | TM_A_L | Timer A low byte |
| 0x12 | TM_A_H | Timer A high byte |
| 0x13 | TM_B | Timer B control |
| 0x14 | TM_B_L | Timer B low byte |
| 0x15 | KB_EXT2 | Keyboard extension 2 |
| 0x16 | — | Reserved |
| 0x17 | — | Reserved |
| 0x18 | BEEP | Beeper control |
| 0x19 | — | Reserved |
| 0x1A | — | Reserved |
| 0x1B | — | Reserved |
| 0x1C | BATT | Battery status register |
| 0x1D | — | Reserved |
| 0x1E | KB_EXT3 | Keyboard extension 3 (NC3000) |
| 0x1F | — | Reserved |
| 0x20 | DSP | DSP control register |
| 0x21 | DSP_L | DSP low byte |
| 0x22 | DSP_H | DSP high byte |
| 0x23 | DSP_F | DSP flags |
| 0x24-0x2F | — | Reserved |
| 0x30 | UART | UART data register |
| 0x31 | UART_STAT | UART status register |
| 0x32-0x39 | — | Reserved |
| 0x3A | RTC_SEC | RTC seconds register |
| 0x3B | RTC_MIN | RTC minutes register |
| 0x3C | RTC_HR | RTC hours register |
| 0x3D | RTC_DAY | RTC days register |
| 0x3E | RTC_CTL | RTC control register |
| 0x3F | RTC_INT | RTC interrupt register |

## Register Details

### Bank Register (0x00)

Controls memory bank switching.

**NC1020**: Selects ROM bank (0x80-0xFF indexes file banks directly)
**PC1000**: Selects ROM bank with page order `{+0x4000, +0x6000, +0x0000, +0x2000}`
**CC800**: Selects ROM bank with page order `{+0x0000, +0x2000, +0x4000, +0x6000}`
**NC2000**: Selects NOR bank (0x00-0x0F)
**NC3000**: Selects NOR bank (0x00-0x1F)

### Interrupt Status Register (0x01)

Shows pending interrupts. Reading clears the register.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM0 | Timer 0 interrupt |
| 1 | TM1 | Timer 1 interrupt |
| 2 | TM_A | Timer A interrupt |
| 3 | TM_B | Timer B interrupt |
| 4 | KB | Keyboard interrupt |
| 5 | RTC | RTC interrupt |
| 6 | UART | UART interrupt |
| 7 | NMI | NMI interrupt |

### Interrupt Enable Register (0x02)

Controls which interrupts are enabled.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM0_EN | Timer 0 interrupt enable |
| 1 | TM1_EN | Timer 1 interrupt enable |
| 2 | TM_A_EN | Timer A interrupt enable |
| 3 | TM_B_EN | Timer B interrupt enable |
| 4 | KB_EN | Keyboard interrupt enable |
| 5 | RTC_EN | RTC interrupt enable |
| 6 | UART_EN | UART interrupt enable |
| 7 | NMI_EN | NMI interrupt enable |

### Timer 0 Control (0x04)

Controls Timer 0 operation.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM0_EN | Timer 0 enable |
| 1 | TM0_INT | Timer 0 interrupt enable |
| 2-7 | — | Reserved |

### Timer 1 Control (0x05)

Controls Timer 1 operation.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM1_EN | Timer 1 enable |
| 1 | TM1_INT | Timer 1 interrupt enable |
| 2-7 | — | Reserved |

### LCD Address Register (0x06)

Sets the LCD framebuffer address.

**Formula**: `address = register_value << 4`

### BBS Register (0x0A)

Controls BBS page selection and ROM/RAM access.

| Bit | Name | Description |
|-----|------|-------------|
| 0-3 | BBS_PAGE | BBS page selector (0-15) |
| 4-6 | — | Reserved |
| 7 | ROA | ROM/RAM Access flag |

**ROA Flag**:
- 0 = ROM access (bank window maps to ROM)
- 1 = RAM access (bank window maps to RAM)

### Volume Register (0x0D)

Controls volume and RAM bank selection.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | VOL | Volume selector (for models with multiple ROM volumes) |
| 1 | — | Reserved |
| 2 | RAMB | RAM bank selector |
| 3-7 | — | Reserved |

### Timer A Control (0x10)

Controls Timer A operation.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM_A_EN | Timer A enable |
| 1 | TM_A_INT | Timer A interrupt enable |
| 2 | TM_A_MODE | Timer A mode (0 = free-running, 1 = one-shot) |
| 3-7 | — | Reserved |

### Timer A Low/High (0x11-0x12)

Timer A counter value (16-bit).

### Timer B Control (0x13)

Controls Timer B operation.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | TM_B_EN | Timer B enable |
| 1 | TM_B_INT | Timer B interrupt enable |
| 2 | TM_B_MODE | Timer B mode (0 = free-running, 1 = one-shot) |
| 3-7 | — | Reserved |

### Timer B Low/High (0x14)

Timer B counter value (16-bit).

### Beeper Control (0x18)

Controls the beeper output.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | BEEP_EN | Beeper enable |
| 1 | BEEP_FREQ | Beeper frequency select |
| 2-7 | — | Reserved |

### Battery Status Register (0x1C)

Shows battery status.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | BATT_LOW | Battery low flag |
| 1 | BATT_CHG | Battery charging flag |
| 2-7 | — | Reserved |

### DSP Control Register (0x20)

Controls the DSP (Digital Signal Processor).

| Bit | Name | Description |
|-----|------|-------------|
| 0 | DSP_EN | DSP enable |
| 1 | DSP_PLAY | DSP play command |
| 2 | DSP_STOP | DSP stop command |
| 3-7 | — | Reserved |

### DSP Low/High (0x21-0x22)

DSP data registers (16-bit).

### DSP Flags (0x23)

DSP status flags.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | DSP_BUSY | DSP busy flag |
| 1 | DSP_INT | DSP interrupt flag |
| 2-7 | — | Reserved |

### UART Data Register (0x30)

UART data register for serial communication.

### UART Status Register (0x31)

UART status flags.

| Bit | Name | Description |
|-----|------|-------------|
| 0 | UART_TX_RDY | UART transmit ready |
| 1 | UART_RX_RDY | UART receive ready |
| 2 | UART_TX_EMPTY | UART transmit empty |
| 3 | UART_OVERRUN | UART overrun error |
| 4 | UART_FRAME | UART frame error |
| 5-7 | — | Reserved |

### RTC Registers (0x3A-0x3F)

Real-Time Clock registers.

| Address | Name | Description |
|---------|------|-------------|
| 0x3A | RTC_SEC | Seconds (0-59) |
| 0x3B | RTC_MIN | Minutes (0-59) |
| 0x3C | RTC_HR | Hours (0-23) |
| 0x3D | RTC_DAY | Days (0-255) |
| 0x3E | RTC_CTL | RTC control register |
| 0x3F | RTC_INT | RTC interrupt register |

#### RTC Control Register (0x3E)

| Bit | Name | Description |
|-----|------|-------------|
| 0 | RTC_EN | RTC enable |
| 1 | RTC_INT_EN | RTC interrupt enable |
| 2-7 | — | Reserved |

#### RTC Interrupt Register (0x3F)

| Bit | Name | Description |
|-----|------|-------------|
| 0 | RTC_SEC_INT | Second interrupt flag |
| 1 | RTC_MIN_INT | Minute interrupt flag |
| 2 | RTC_HR_INT | Hour interrupt flag |
| 3 | RTC_DAY_INT | Day interrupt flag |
| 4-7 | — | Reserved |

## Model-Specific Registers

### NC1020-Specific

- **0x0A**: BBS page selector + ROA flag
- **0x0D**: Volume selector

### PC1000-Specific

- **0x0A**: BBS page selector + ROA flag
- **0x0D**: Volume selector
- **0x15**: Keyboard extension 2

### CC800-Specific

- **0x0A**: BBS page selector + ROA flag
- **0x0D**: Volume selector
- **0x15**: Keyboard extension 2

### NC2000-Specific

- **0x18**: NAND control (CLE/ALE/CE)
- **0x29**: NAND data register
- **0x3A-0x3D**: Banked UART/interrupt-vector registers

### NC3000-Specific

- **0x18**: NAND control (CLE/ALE/CE)
- **0x1E**: Keyboard extension 3
- **0x39**: NAND data register
- **0x3D**: RTC/UART interrupt vectors

## Notes

1. **Registers are model-specific** — Not all registers are available on all models
2. **Clear-on-read registers** — Some registers clear when read (e.g., ISR)
3. **Write-only registers** — Some registers can only be written to
4. **Reserved bits** — Should be written as 0
5. **Register behavior varies** — Some registers behave differently on different models
