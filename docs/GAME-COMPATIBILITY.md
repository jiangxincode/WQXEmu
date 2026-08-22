# Game Compatibility

This document tracks the compatibility status of Wenquxing (文曲星) games and applications with WQXEmu.

## Compatibility Status

| Status | Description |
|--------|-------------|
| ✅ Working | Game runs correctly without issues |
| ⚠️ Partial | Game runs but has minor issues |
| ❌ Not Working | Game does not run or crashes |
| 🔍 Untested | Game has not been tested yet |

## NC1020 Games

| Game | Status | Notes |
|------|--------|-------|
| — | 🔍 Untested | — |

## PC1000 Games

| Game | Status | Notes |
|------|--------|-------|
| — | 🔍 Untested | — |

## CC800 Games

| Game | Status | Notes |
|------|--------|-------|
| — | 🔍 Untested | — |

## NC2000 Games

| Game | Status | Notes |
|------|--------|-------|
| — | 🔍 Untested | — |

## NC3000 Games

| Game | Status | Notes |
|------|--------|-------|
| — | 🔍 Untested | — |

## Testing Games

### How to Test

1. **Load the game** in WQXEmu
2. **Run for at least 300 frames** (about 5 seconds)
3. **Check for issues**:
   - Does the game load?
   - Does the title screen appear?
   - Does the game respond to input?
   - Are there any crashes?
4. **Report your findings** in the [Game Compatibility](https://github.com/AloysHF/WQXEmu/issues) issue tracker

### Testing Checklist

- [ ] Game loads without errors
- [ ] Title screen appears
- [ ] Game responds to input
- [ ] Game runs without crashes
- [ ] Audio works (if applicable)
- [ ] Save/load works (if applicable)

## Reporting Issues

When reporting game compatibility issues, please include:

1. **Game name** — exact name of the game
2. **Model** — which Wenquxing model (NC1020, PC1000, etc.)
3. **Firmware version** — firmware version used
4. **Steps to reproduce** — how to reproduce the issue
5. **Expected behavior** — what should happen
6. **Actual behavior** — what actually happens
7. **Screenshots** — if applicable

## Known Issues

### General

- Some games may require specific firmware versions
- Save states may not be compatible between different firmware versions
- Audio may have minor glitches in some games

### Model-Specific

- **NC1020**: Some older games may not be compatible
- **PC1000**: Game compatibility depends on firmware version
- **CC800**: Limited game library
- **NC2000**: Some games may require specific NAND configurations
- **NC3000**: Game compatibility is still being tested

## Game Resources

Game resources can be downloaded from:
- [Baidu Netdisk](https://pan.baidu.com/s/xxx?pwd=xxx)

**Note**: Game files are not distributed with the emulator. You must obtain them separately.

## Contributing

Help us improve game compatibility! You can:

1. **Test games** and report compatibility status
2. **Report bugs** you encounter
3. **Suggest improvements** for better compatibility
4. **Share game resources** (if you have permission)

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Compatibility Database

We maintain a compatibility database to track game status. You can help by:

1. **Testing games** you own
2. **Reporting results** in the issue tracker
3. **Updating this document** with your findings

## Future Plans

- [ ] Test all available NC1020 games
- [ ] Test all available PC1000 games
- [ ] Test all available CC800 games
- [ ] Test all available NC2000 games
- [ ] Test all available NC3000 games
- [ ] Create automated compatibility testing
- [ ] Improve audio compatibility
- [ ] Improve save state compatibility
