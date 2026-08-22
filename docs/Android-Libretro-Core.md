# Android Libretro Core

This guide covers building and using the WQXEmu libretro core on Android.

## Building for Android

### Prerequisites

1. **Android NDK** — Install via Android Studio or download directly
2. **Rust** — Install via [rustup](https://rustup.rs/)
3. **Android targets** — Add Rust targets for Android

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
```

### Configure Cargo

Create or edit `~/.cargo/config.toml`:

```toml
[target.aarch64-linux-android]
linker = "/path/to/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android30-clang"

[target.armv7-linux-androideabi]
linker = "/path/to/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi30-clang"

[target.x86_64-linux-android]
linker = "/path/to/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android30-clang"
```

Replace `/path/to/android-ndk` with your actual NDK path.

### Build the Core

```bash
# For arm64-v8a (most modern devices)
cargo build -p wqxemu-libretro --target aarch64-linux-android --release

# For armeabi-v7a (older devices)
cargo build -p wqxemu-libretro --target armv7-linux-androideabi --release

# For x86_64 (emulators)
cargo build -p wqxemu-libretro --target x86_64-linux-android --release
```

### Output Files

The compiled core will be at:
- `target/aarch64-linux-android/release/libwqxemu_libretro.so`
- `target/armv7-linux-androideabi/release/libwqxemu_libretro.so`
- `target/x86_64-linux-android/release/libwqxemu_libretro.so`

## Installation

### Using RetroArch on Android

1. **Install RetroArch** from Google Play Store or F-Droid
2. **Copy the core** to RetroArch's cores directory:
   - Internal storage: `RetroArch/cores/`
   - Or use RetroArch's built-in core updater
3. **Copy firmware files** to RetroArch's system directory:
   - Internal storage: `RetroArch/system/WQXEmu/`
4. **Launch RetroArch** and load the core

### Manual Installation

1. Connect your Android device to your computer
2. Copy the core file to your device:
   ```bash
   adb push target/aarch64-linux-android/release/libwqxemu_libretro.so /sdcard/RetroArch/cores/
   ```
3. Copy firmware files:
   ```bash
   adb push roms/nc1020/ /sdcard/RetroArch/system/WQXEmu/nc1020/
   ```

## Supported Android Architectures

| Architecture | Rust Target | Status |
|--------------|-------------|--------|
| arm64-v8a | aarch64-linux-android | ✅ Supported |
| armeabi-v7a | armv7-linux-androideabi | ✅ Supported |
| x86_64 | x86_64-linux-android | ✅ Supported |
| x86 | i686-linux-android | ⚠️ Experimental |

## Configuration

### Firmware Placement

Place firmware files in RetroArch's system directory:

```
/sdcard/RetroArch/system/WQXEmu/
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

1. **Use arm64-v8a** — Best performance for modern devices
2. **Close background apps** — Free up memory and CPU
3. **Use a gamepad** — Better control than touchscreen
4. **Lower audio quality** — Reduce CPU usage if needed

## Troubleshooting

### Common Issues

1. **"Core failed to load"** — Ensure the core file is in the correct location
2. **"No firmware found"** — Check firmware file paths and permissions
3. **"Black screen"** — Try a different firmware version
4. **"Audio crackling"** — Lower audio quality in core options

### Debug Logging

Enable debug logging in RetroArch:

1. Go to **Settings → Logging**
2. Set **Logging Verbosity** to **Debug**
3. Check logs at `/sdcard/RetroArch/logs/`

### Performance Issues

If you experience performance issues:

1. Check CPU usage in RetroArch's **Quick Menu → Information**
2. Try lowering the audio sample rate
3. Disable unnecessary core options
4. Close other apps running in the background

## Building with Android Studio

If you prefer using Android Studio:

1. Open the project in Android Studio
2. Build the core using Gradle
3. Copy the built core to RetroArch

## Resources

- [RetroArch Android Guide](https://docs.libretro.com/guides/install-android/)
- [Rust Android Guide](https://mozilla.github.io/book/ch20-05-rust-on-android.html)
- [Android NDK Documentation](https://developer.android.com/ndk)
