// WQXEmu - Standalone desktop frontend for Wenquxing emulators.
//
// This is a simple desktop application that runs the emulator in a
// window with keyboard input support. The target model is selected with
// `--model` or auto-detected from the ROM files.

use anyhow::Result;
use clap::Parser;
use minifb::{Key, Window, WindowOptions};

use std::path::PathBuf;

use wqxemu_core::{
    detect_model, key_ids, layout_for, Emulator, MachineModel, RomFiles, LCD_HEIGHT, LCD_WIDTH,
};

mod keypad;
use keypad::{hit_test, render_keypad, KEYPAD_HEIGHT, KEYPAD_WIDTH};

/// WQXEmu - Wenquxing Emulator
#[derive(Parser)]
#[command(name = "wqxemu", version, about)]
struct Args {
    /// Path to the system ROM dump (obj_lu.bin / *.rom)
    #[arg(help = "Path to the ROM file (not needed for NC2000)")]
    rom_path: Option<String>,

    /// Path to the NOR Flash file (nc1020.fls / *.nor)
    #[arg(short = 'n', long, help = "Path to the NOR Flash file")]
    nor_path: Option<String>,

    /// Path to the NAND Flash file (NC2000)
    #[arg(long, help = "Path to the NAND Flash file")]
    nand_path: Option<String>,

    /// Path to the first NAND plane (NC2000, optional)
    #[arg(long, help = "Path to the first NAND plane file")]
    nand0_path: Option<String>,

    /// Hardware model: nc1020, pc1000, cc800, nc2000 or nc3000 (default: auto-detect)
    #[arg(long, help = "Hardware model (nc1020, pc1000, cc800, nc2000, nc3000)")]
    model: Option<String>,

    /// Scale factor for the display
    #[arg(short = 's', long, default_value = "4", help = "Display scale factor")]
    scale: u32,

    /// Take a screenshot after N frames and exit (saves as PNG)
    #[arg(short = 'S', long = "screenshot", value_name = "PATH")]
    screenshot: Option<String>,

    /// Number of frames to run before taking the screenshot (default: 30)
    #[arg(long = "screenshot-frames", default_value = "30")]
    screenshot_frames: u32,
}

/// Map minifb key to a key ID for the given model.
///
/// Key IDs encode the 8x8 matrix position: row = key_id >> 3,
/// col = key_id & 7. The PC1000/CC800 use a different physical matrix
/// than the NC1020/NC2000, so the mapping is model-specific.
fn map_key(model: MachineModel, key: Key) -> Option<u8> {
    match model {
        MachineModel::Pc1000 | MachineModel::Cc800 => map_key_pc1000(key),
        MachineModel::Nc2000 => map_key_nc2000(key),
        MachineModel::Nc3000 => map_key_nc3000(key),
        _ => map_key_nc1020(key),
    }
}

/// Map minifb key to NC2000 key ID (matrix position).
///
/// The NC2000 shares the NC2000-era QWERTY matrix with the NC3000 but
/// keeps its hotkeys in matrix column 1 (英汉/名片/计算/行程/资料/时间/
/// 网络) and the power key at (0,0).
fn map_key_nc2000(key: Key) -> Option<u8> {
    let id = |row: u8, col: u8| row << 3 | col;
    map_qwerty_2000(key).or_else(|| match key {
        Key::F5 => Some(id(3, 1)),                // 英汉
        Key::F6 => Some(id(4, 1)),                // 名片
        Key::F7 => Some(id(5, 1)),                // 计算
        Key::F8 => Some(id(2, 1)),                // 行程
        Key::F9 => Some(id(1, 1)),                // 资料
        Key::F10 => Some(id(0, 1)),               // 时间
        Key::F11 => Some(id(6, 1)),               // 网络
        Key::F12 | Key::Delete => Some(id(0, 0)), // ON/OFF
        _ => None,
    })
}

/// QWERTY block shared by the NC2000/NC3000 keypads.
fn map_qwerty_2000(key: Key) -> Option<u8> {
    let id = |row: u8, col: u8| row << 3 | col;
    match key {
        Key::Up => Some(id(2, 3)),
        Key::Down => Some(id(3, 3)),
        Key::Left => Some(id(7, 7)),
        Key::Right => Some(id(7, 3)),
        Key::Enter => Some(id(5, 3)),
        Key::Escape => Some(id(3, 7)),
        Key::Space => Some(id(6, 7)),
        Key::Backspace => Some(id(1, 2)), // F2 = 删除
        Key::F1 => Some(id(0, 2)),
        Key::F2 => Some(id(1, 2)),
        Key::F3 => Some(id(2, 2)),
        Key::F4 => Some(id(3, 2)),
        Key::A => Some(id(0, 5)),
        Key::B => Some(id(4, 6)),
        Key::C => Some(id(2, 6)),
        Key::D => Some(id(2, 5)),
        Key::E => Some(id(2, 4)),
        Key::F => Some(id(3, 5)),
        Key::G => Some(id(4, 5)),
        Key::H => Some(id(5, 5)),
        Key::I => Some(id(7, 4)),
        Key::J => Some(id(6, 5)),
        Key::K => Some(id(7, 5)),
        Key::L => Some(id(1, 3)),
        Key::M => Some(id(6, 6)),
        Key::N => Some(id(5, 6)),
        Key::O => Some(id(0, 3)),
        Key::P => Some(id(4, 3)),
        Key::Q => Some(id(0, 4)),
        Key::R => Some(id(3, 4)),
        Key::S => Some(id(1, 5)),
        Key::T => Some(id(4, 4)),
        Key::U => Some(id(6, 4)),
        Key::V => Some(id(3, 6)),
        Key::W => Some(id(1, 4)),
        Key::X => Some(id(1, 6)),
        Key::Y => Some(id(5, 4)),
        Key::Z => Some(id(0, 6)),
        Key::Key0 => Some(id(4, 7)),
        Key::Key1 => Some(id(4, 6)),
        Key::Key2 => Some(id(5, 6)),
        Key::Key3 => Some(id(6, 6)),
        Key::Key4 => Some(id(4, 5)),
        Key::Key5 => Some(id(5, 5)),
        Key::Key6 => Some(id(6, 5)),
        Key::Key7 => Some(id(4, 4)),
        Key::Key8 => Some(id(5, 4)),
        Key::Key9 => Some(id(6, 4)),
        _ => None,
    }
}

/// Map minifb key to NC1020/NC2000 key ID.
fn map_key_nc1020(key: Key) -> Option<u8> {
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

/// Map minifb key to PC1000 key ID (matrix position).
///
/// PC1000 matrix layout (from the PC1000 reference keymap):
///   row 0: ON/OFF        row 4: A S D F G H J K
///   row 1: 英汉 名片 计算 行程 资料 时间 网络
///   row 2: 求助 中英数 输入法 跳出 0 . 空格 ←
///   row 3: Z X C V B N M ⇞
///   row 5: Q W E R T Y U I
///   row 6: O L ▲ ▼ P 输入 ⇟ →
///   row 7: F1 F2 F3 F4
fn map_key_pc1000(key: Key) -> Option<u8> {
    let id = |row: u8, col: u8| row << 3 | col;
    match key {
        // Arrows / navigation
        Key::Up => Some(id(6, 2)),
        Key::Down => Some(id(6, 3)),
        Key::Left => Some(id(2, 7)),
        Key::Right => Some(id(6, 7)),
        Key::Enter => Some(id(6, 5)),
        Key::Escape => Some(id(2, 3)),
        Key::Space => Some(id(2, 6)),
        Key::Backspace => Some(id(7, 3)),

        // Function keys / hotkeys
        Key::F1 => Some(id(7, 2)),
        Key::F2 => Some(id(7, 3)),
        Key::F3 => Some(id(7, 4)),
        Key::F4 => Some(id(7, 5)),
        Key::F5 => Some(id(1, 0)),  // 英汉
        Key::F6 => Some(id(1, 1)),  // 名片
        Key::F7 => Some(id(1, 2)),  // 计算
        Key::F8 => Some(id(1, 3)),  // 行程
        Key::F9 => Some(id(1, 4)),  // 资料
        Key::F10 => Some(id(1, 5)), // 时间
        Key::F11 => Some(id(1, 6)), // 网络

        // Power button (Delete)
        Key::Delete => Some(id(0, 0)),

        // Letters
        Key::A => Some(id(4, 0)),
        Key::B => Some(id(3, 4)),
        Key::C => Some(id(3, 2)),
        Key::D => Some(id(4, 2)),
        Key::E => Some(id(5, 2)),
        Key::F => Some(id(4, 3)),
        Key::G => Some(id(4, 4)),
        Key::H => Some(id(4, 5)),
        Key::I => Some(id(5, 7)),
        Key::J => Some(id(4, 6)),
        Key::K => Some(id(4, 7)),
        Key::L => Some(id(6, 1)),
        Key::M => Some(id(3, 6)),
        Key::N => Some(id(3, 5)),
        Key::O => Some(id(6, 0)),
        Key::P => Some(id(6, 4)),
        Key::Q => Some(id(5, 0)),
        Key::R => Some(id(5, 3)),
        Key::S => Some(id(4, 1)),
        Key::T => Some(id(5, 4)),
        Key::U => Some(id(5, 6)),
        Key::V => Some(id(3, 3)),
        Key::W => Some(id(5, 1)),
        Key::X => Some(id(3, 1)),
        Key::Y => Some(id(5, 5)),
        Key::Z => Some(id(3, 0)),

        // Numbers
        Key::Key0 => Some(id(2, 4)),
        Key::Key1 => Some(id(3, 4)),
        Key::Key2 => Some(id(3, 5)),
        Key::Key3 => Some(id(3, 6)),
        Key::Key4 => Some(id(4, 4)),
        Key::Key5 => Some(id(4, 5)),
        Key::Key6 => Some(id(4, 6)),
        Key::Key7 => Some(id(5, 4)),
        Key::Key8 => Some(id(5, 5)),
        Key::Key9 => Some(id(5, 6)),

        _ => None,
    }
}

/// Map minifb key to NC3000 key ID (matrix position).
///
/// The NC3000 shares the standard NC2000-era QWERTY matrix plus its own
/// hotkey column (col 0): 网络/电源 (0,0), 游戏 (1,0), 计算 (2,0),
/// 时间 (3,0), 英汉 (5,0), 词库 (6,0), 学习 (7,0).
fn map_key_nc3000(key: Key) -> Option<u8> {
    let id = |row: u8, col: u8| row << 3 | col;
    map_qwerty_2000(key).or_else(|| match key {
        // Hotkey column 0 (from the NC3000 keymap):
        // 网络/电源 (0,0), 游戏 (1,0), 计算 (2,0), 时间 (3,0),
        // 英汉 (5,0), 词库 (6,0), 学习 (7,0).
        Key::F5 => Some(id(1, 0)),                // 游戏
        Key::F6 => Some(id(2, 0)),                // 计算
        Key::F7 => Some(id(3, 0)),                // 时间
        Key::F9 => Some(id(5, 0)),                // 英汉
        Key::F10 => Some(id(6, 0)),               // 词库
        Key::F11 => Some(id(7, 0)),               // 学习
        Key::F12 | Key::Delete => Some(id(0, 0)), // 网络/电源
        _ => None,
    })
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    // Assemble ROM files and pick the model
    let files = RomFiles::new(
        args.rom_path.as_ref().map(PathBuf::from),
        args.nor_path.as_ref().map(PathBuf::from),
        args.nand_path.as_ref().map(PathBuf::from),
        args.nand0_path.as_ref().map(PathBuf::from),
    );
    let model = match &args.model {
        Some(name) => MachineModel::from_name(name)
            .ok_or_else(|| anyhow::anyhow!("unknown model: {}", name))?,
        None => detect_model(&files),
    };
    log::info!("Selected model: {}", model.name());

    // Create emulator
    let mut emu = Emulator::new(model, &files)?;
    emu.reset();

    log::info!("Emulator initialized, PC=0x{:04X}", emu.pc());

    // If screenshot mode, run N frames and save screenshot to the
    // user-provided path, then exit.
    if let Some(ref screenshot_path) = args.screenshot {
        log::info!(
            "Running {} frames before taking screenshot...",
            args.screenshot_frames
        );
        for _ in 0..args.screenshot_frames {
            emu.run_frame();
        }
        let pixels = emu.framebuffer();
        save_screenshot(&pixels, screenshot_path)?;
        log::info!("Screenshot saved to {}", screenshot_path);
        return Ok(());
    }

    // Create window
    let scale = args.scale as usize;
    let lcd_width = LCD_WIDTH * scale;
    let lcd_height = LCD_HEIGHT * scale;
    // The keypad panel sits below the LCD; the window is widened when
    // the LCD is narrower than the panel so the keys stay readable.
    let window_width = lcd_width.max(KEYPAD_WIDTH);
    let window_height = lcd_height + KEYPAD_HEIGHT;
    let lcd_x = (window_width - lcd_width) / 2;
    let panel_x = (window_width - KEYPAD_WIDTH) / 2;
    let panel_y = lcd_height;

    let mut window = Window::new(
        &format!("WQXEmu - {}", model.name().to_uppercase()),
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
    let layout = layout_for(model);
    let mut pressed = [false; 64];
    let mut mouse_key: Option<u8> = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Process input
        window
            .get_keys_pressed(minifb::KeyRepeat::No)
            .iter()
            .for_each(|key| {
                if let Some(key_id) = map_key(model, *key) {
                    if !pressed[key_id as usize] {
                        emu.set_key(key_id, true);
                        pressed[key_id as usize] = true;
                    }
                }
            });

        window.get_keys_released().iter().for_each(|key| {
            if let Some(key_id) = map_key(model, *key) {
                emu.set_key(key_id, false);
                pressed[key_id as usize] = false;
            }
        });

        // Mouse: click the virtual keypad.
        let mouse_down = window.get_mouse_down(minifb::MouseButton::Left);
        let mouse_pos = window.get_mouse_pos(minifb::MouseMode::Clamp);
        let (mx, my) = mouse_pos.map_or((0, 0), |(x, y)| (x as usize, y as usize));
        if mouse_down {
            if let Some(key_id) = hit_test(mx, my, panel_x, panel_y, model, layout) {
                if mouse_key != Some(key_id) {
                    if let Some(old) = mouse_key {
                        emu.set_key(old, false);
                        pressed[old as usize] = false;
                    }
                    emu.set_key(key_id, true);
                    pressed[key_id as usize] = true;
                    mouse_key = Some(key_id);
                }
            } else if let Some(old) = mouse_key {
                emu.set_key(old, false);
                pressed[old as usize] = false;
                mouse_key = None;
            }
        } else if let Some(old) = mouse_key {
            emu.set_key(old, false);
            pressed[old as usize] = false;
            mouse_key = None;
        }

        // Run one frame
        emu.run_frame();

        // Get framebuffer and render
        let pixels = emu.framebuffer();

        // Scale the LCD framebuffer (centered) + draw the keypad below.
        let mut buffer = vec![0u32; window_width * window_height];
        for y in 0..lcd_height {
            for x in 0..lcd_width {
                let src_x = x / scale;
                let src_y = y / scale;
                let pixel = pixels[src_y * LCD_WIDTH + src_x];
                buffer[y * window_width + lcd_x + x] = pixel;
            }
        }
        render_keypad(
            &mut buffer,
            window_width,
            panel_x,
            panel_y,
            model,
            layout,
            &pressed,
        );

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
