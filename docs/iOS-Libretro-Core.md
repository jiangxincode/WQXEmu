# iOS Libretro Core

This guide covers building and using the WQXEmu libretro core on iOS.

## Building for iOS

### Prerequisites

1. **Xcode** — Install from Mac App Store
2. **Rust** — Install via [rustup](https://rustup.rs/)
3. **iOS targets** — Add Rust targets for iOS

```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
```

### Build the Core

```bash
# For real devices (arm64)
cargo build -p wqxemu-libretro --target aarch64-apple-ios --release

# For simulator (x86_64)
cargo build -p wqxemu-libretro --target x86_64-apple-ios --release

# For simulator (arm64 - Apple Silicon)
cargo build -p wqxemu-libretro --target aarch64-apple-ios-sim --release
```

### Output Files

The compiled core will be at:
- `target/aarch64-apple-ios/release/libwqxemu_libretro.dylib`
- `target/x86_64-apple-ios/release/libwqxemu_libretro.dylib`
- `target/aarch64-apple-ios-sim/release/libwqxemu_libretro.dylib`

## Installation

### Using RetroArch on iOS

RetroArch on iOS requires manual core injection due to Apple's restrictions.

#### Method 1: Using AltStore

1. **Install AltStore** on your iOS device
2. **Install RetroArch** through AltStore
3. **Copy the core** to RetroArch's cores directory:
   - Use AltStore's file sharing feature
   - Or use iCloud Drive

#### Method 2: Using Sideloadly

1. **Install Sideloadly** on your computer
2. **Sideload RetroArch** to your iOS device
3. **Copy the core** using Sideloadly's file sharing

#### Method 3: Using TestFlight

1. **Join the RetroArch TestFlight** beta
2. **Install RetroArch** through TestFlight
3. **Copy the core** using iTunes File Sharing

### Manual Installation

1. Connect your iOS device to your computer
2. Open **Finder** (macOS Catalina or later) or **iTunes**
3. Select your device and go to **Files** tab
4. Find **RetroArch** in the app list
5. Copy the core file to RetroArch's documents

## Supported iOS Architectures

| Architecture | Rust Target | Status |
|--------------|-------------|--------|
| arm64 (real devices) | aarch64-apple-ios | ✅ Supported |
| x86_64 (Intel simulator) | x86_64-apple-ios | ✅ Supported |
| arm64 (Apple Silicon simulator) | aarch64-apple-ios-sim | ✅ Supported |

## Configuration

### Firmware Placement

Place firmware files in RetroArch's system directory:

```
RetroArch/
└── system/
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

### Core Options

Core options can be configured through RetroArch's **Quick Menu → Core Options**.

## Performance Tips

1. **Use real device** — Better performance than simulator
2. **Close background apps** — Free up memory and CPU
3. **Use a gamepad** — Better control than touchscreen
4. **Lower audio quality** — Reduce CPU usage if needed

## Troubleshooting

### Common Issues

1. **"Core failed to load"** — Ensure the core file is in the correct location
2. **"No firmware found"** — Check firmware file paths
3. **"Black screen"** — Try a different firmware version
4. **"Audio crackling"** — Lower audio quality in core options
5. **"App crashes on startup"** — Check iOS version compatibility

### Debug Logging

Enable debug logging in RetroArch:

1. Go to **Settings → Logging**
2. Set **Logging Verbosity** to **Debug**
3. Check logs in RetroArch's logging section

### Performance Issues

If you experience performance issues:

1. Check CPU usage in RetroArch's **Quick Menu → Information**
2. Try lowering the audio sample rate
3. Disable unnecessary core options
4. Close other apps running in the background

## Building with Xcode

If you prefer using Xcode:

1. Create a new Xcode project
2. Add the Rust library as a dependency
3. Build and run on your device

## Resources

- [RetroArch iOS Guide](https://docs.libretro.com/guides/install-ios/)
- [Rust iOS Guide](https://mozilla.github.io/book/ch20-05-rust-on-ios.html)
- [Xcode Documentation](https://developer.apple.com/xcode/)
