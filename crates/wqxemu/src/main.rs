// WQXEmu - Standalone desktop frontend for NC1020 emulator.
//
// This is a simple desktop application that runs the NC1020 emulator
// in a window with keyboard input support.

use anyhow::Result;
use clap::Parser;
use minifb::{Key, Window, WindowOptions};

use wqxemu_core::{key_ids, Emulator, LCD_HEIGHT, LCD_WIDTH};

/// WQXEmu - Wenquxing NC1020 Emulator
#[derive(Parser)]
#[command(name = "wqxemu", version, about)]
struct Args {
    /// Path to ROM file (obj_lu.bin)
    #[arg(help = "Path to the NC1020 ROM file")]
    rom_path: String,

    /// Path to NOR Flash file (nc1020.fls)
    #[arg(short = 'n', long, help = "Path to the NOR Flash file")]
    nor_path: Option<String>,

    /// Scale factor for the display
    #[arg(short = 's', long, default_value = "4", help = "Display scale factor")]
    scale: u32,

    /// Take screenshot after N frames and exit
    #[arg(long, help = "Take screenshot after N frames and exit")]
    screenshot_frames: Option<u64>,
}

/// Map minifb key to NC1020 key ID
fn map_key(key: Key) -> Option<u8> {
    match key {
        // Arrow keys
        Key::Up => Some(key_ids::UP),
        Key::Down => Some(key_ids::DOWN),
        Key::Left => Some(key_ids::LEFT),
        Key::Right => Some(key_ids::RIGHT),

        // Enter
        Key::Enter => Some(key_ids::ENTER),

        // Escape -> ESC
        Key::Escape => Some(key_ids::ESC),

        // Space
        Key::Space => Some(key_ids::SPACE),

        // Backspace
        Key::Backspace => Some(key_ids::BACKSPACE),

        // F keys
        Key::F1 => Some(key_ids::F1),
        Key::F2 => Some(key_ids::F2),
        Key::F3 => Some(key_ids::F3),
        Key::F4 => Some(key_ids::F4),
        Key::F10 => Some(key_ids::F10),
        Key::F11 => Some(key_ids::F11),

        // Page Up/Down
        Key::PageUp => Some(key_ids::PAGE_UP),
        Key::PageDown => Some(key_ids::PAGE_DOWN),

        // Power button (mapped to Delete)
        Key::Delete => Some(key_ids::POWER),

        // Letter keys
        Key::A => Some(0x30),
        Key::B => Some(0x31),
        Key::C => Some(0x32),
        Key::D => Some(0x33),
        Key::E => Some(0x14),
        Key::F => Some(0x24),
        Key::G => Some(0x15),
        Key::H => Some(0x25),
        Key::I => Some(0x16),
        Key::J => Some(0x26),
        Key::K => Some(0x36),
        Key::L => Some(0x17),
        Key::M => Some(0x27),
        Key::N => Some(0x37),
        Key::O => Some(0x38),
        Key::P => Some(0x28),
        Key::Q => Some(0x11),
        Key::R => Some(0x12),
        Key::S => Some(0x21),
        Key::T => Some(0x22),
        Key::U => Some(0x13),
        Key::V => Some(0x23),
        Key::W => Some(0x31),
        Key::X => Some(0x32),
        Key::Y => Some(0x33),
        Key::Z => Some(0x34),

        // Number keys
        Key::Key0 => Some(0x20),
        Key::Key1 => Some(0x10),
        Key::Key2 => Some(0x11),
        Key::Key3 => Some(0x12),
        Key::Key4 => Some(0x13),
        Key::Key5 => Some(0x14),
        Key::Key6 => Some(0x15),
        Key::Key7 => Some(0x16),
        Key::Key8 => Some(0x17),
        Key::Key9 => Some(0x18),

        _ => None,
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    // Create emulator
    log::info!("Loading ROM: {}", args.rom_path);
    let mut emu = Emulator::from_rom(&args.rom_path, args.nor_path.as_deref())?;
    emu.reset();

    log::info!("Emulator initialized, PC=0x{:04X}", emu.pc());

    // If screenshot mode, run N frames and save screenshot
    if let Some(frames) = args.screenshot_frames {
        for _ in 0..frames {
            emu.run_frame();
        }
        let pixels = emu.framebuffer();
        save_screenshot(&pixels, "screenshot.png")?;
        log::info!("Screenshot saved to screenshot.png");
        return Ok(());
    }

    // Create window
    let scale = args.scale as usize;
    let window_width = LCD_WIDTH * scale;
    let window_height = LCD_HEIGHT * scale;

    let mut window = Window::new(
        "WQXEmu - NC1020",
        window_width,
        window_height,
        WindowOptions {
            resize: false,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window");

    // Limit to ~30 fps
    window.set_target_fps(30);

    // Main event loop
    let mut last_key_state: std::collections::HashMap<u8, bool> = std::collections::HashMap::new();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Process input
        window
            .get_keys_pressed(minifb::KeyRepeat::No)
            .iter()
            .for_each(|key| {
                if let Some(key_id) = map_key(*key) {
                    let was_pressed = last_key_state.get(&key_id).copied().unwrap_or(false);
                    if !was_pressed {
                        emu.set_key(key_id, true);
                        last_key_state.insert(key_id, true);
                    }
                }
            });

        window.get_keys_released().iter().for_each(|key| {
            if let Some(key_id) = map_key(*key) {
                emu.set_key(key_id, false);
                last_key_state.insert(key_id, false);
            }
        });

        // Run one frame
        emu.run_frame();

        // Get framebuffer and render
        let pixels = emu.framebuffer();

        // Scale the framebuffer
        let mut buffer = vec![0u32; window_width * window_height];
        for y in 0..window_height {
            for x in 0..window_width {
                let src_x = x / scale;
                let src_y = y / scale;
                let pixel = pixels[src_y * LCD_WIDTH + src_x];
                buffer[y * window_width + x] = pixel;
            }
        }

        window
            .update_with_buffer(&buffer, window_width, window_height)
            .expect("Failed to update window");
    }

    Ok(())
}

/// Save framebuffer as PNG screenshot
fn save_screenshot(pixels: &[u32], path: &str) -> Result<()> {
    use image::{ImageBuffer, Rgba};

    let width = LCD_WIDTH as u32;
    let height = LCD_HEIGHT as u32;

    let img = ImageBuffer::from_fn(width, height, |x, y| {
        let idx = (y * width + x) as usize;
        let pixel = pixels[idx];
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        let a = ((pixel >> 24) & 0xFF) as u8;
        Rgba([r, g, b, a])
    });

    img.save(path)?;
    Ok(())
}
