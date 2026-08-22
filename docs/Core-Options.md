# Core Options

This document lists all available core options for the WQXEmu libretro core.

## Accessing Core Options

Core options can be configured from RetroArch's **Quick Menu → Core Options**.

## Display Options

### LCD Scale

- **Description**: Scale factor for the LCD display
- **Values**: 1x, 2x, 3x, 4x (default)
- **Effect**: Changes the size of the LCD display

### Show Grid Lines

- **Description**: Show grid lines on the LCD
- **Values**: Off, On (default)
- **Effect**: Shows or hides grid lines on the LCD

### Ghosting Effect

- **Description**: Enable/disable LCD ghosting effect
- **Values**: Off, On (default)
- **Effect**: Enables or disables the LCD ghosting effect

## Audio Options

### Audio Volume

- **Description**: Master audio volume
- **Values**: 0-100 (default: 100)
- **Effect**: Adjusts the overall audio volume

### Audio Sample Rate

- **Description**: Audio sample rate
- **Values**: 22050, 44100, 48000 (default)
- **Effect**: Changes the audio sample rate

## Input Options

### Key Repeat Delay

- **Description**: Delay before key repeat starts
- **Values**: 100-1000 ms (default: 500)
- **Effect**: Adjusts the delay before key repeat starts

### Key Repeat Interval

- **Description**: Interval between key repeats
- **Values**: 50-500 ms (default: 100)
- **Effect**: Adjusts the interval between key repeats

## Emulation Options

### CPU Speed

- **Description**: CPU speed multiplier
- **Values**: 0.5x, 1x (default), 2x, 4x
- **Effect**: Adjusts the CPU speed

### Timer Speed

- **Description**: Timer speed multiplier
- **Values**: 0.5x, 1x (default), 2x, 4x
- **Effect**: Adjusts the timer speed

## Debug Options

### CPU Trace

- **Description**: Enable CPU instruction tracing
- **Values**: Off (default), On
- **Effect**: Shows each CPU instruction executed in the log

### IO Trace

- **Description**: Enable IO register tracing
- **Values**: Off (default), On
- **Effect**: Shows IO register reads and writes in the log

### Bank Trace

- **Description**: Enable bank switching tracing
- **Values**: Off (default), On
- **Effect**: Shows bank switch operations in the log

## Notes

1. **Changes require restart** — Most core options require restarting the core to take effect
2. **Performance impact** — Some options may affect performance
3. **Debug options are verbose** — Use with caution in production
4. **Default values are recommended** — Change only if you know what you're doing
