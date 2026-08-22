// WQXEmu - Standalone desktop frontend for Wenquxing emulators.
//
// This is a simple desktop application that runs the emulator in a
// window with keyboard input support. The target model is selected with
// `--model` or auto-detected from the ROM files.

use anyhow::{Context, Result};
use clap::Parser;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use minifb::{Key, Window, WindowOptions};

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use wqxemu_core::{
    detect_model, key_ids, layout_for, Emulator, MachineModel, RomFiles, LCD_HEIGHT, LCD_WIDTH,
};

mod keypad;
use keypad::{DeviceSkin, SkinInput};

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

    /// Load this persistent session if it exists and atomically update it on exit
    #[arg(
        long,
        value_name = "PATH",
        help = "Load/save a persistent session without modifying ROM or Flash dump files"
    )]
    state_file: Option<PathBuf>,

    /// Hardware model: nc1020, pc1000, cc800, nc2000 or nc3000 (default: auto-detect)
    #[arg(long, help = "Hardware model (nc1020, pc1000, cc800, nc2000, nc3000)")]
    model: Option<String>,

    /// Scale factor for the device skin (4 = 600 pixels high)
    #[arg(
        short = 's',
        long,
        default_value = "4",
        help = "Device skin scale factor (4 = 600 pixels high)"
    )]
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
        Key::F5 => Some(key_ids::F5),
        Key::F6 => Some(key_ids::F6),
        Key::F7 => Some(key_ids::F7),
        Key::F8 => Some(key_ids::F8),
        Key::F9 => Some(key_ids::F9),
        Key::F10 => Some(key_ids::F10),
        Key::F11 => Some(key_ids::F11),

        // Page Up/Down
        Key::PageUp => Some(key_ids::PAGE_UP),
        Key::PageDown => Some(key_ids::PAGE_DOWN),

        // Power button (mapped to Delete)
        Key::Delete => Some(key_ids::POWER),

        // Letter keys
        Key::A => Some(0x28),
        Key::B => Some(0x34),
        Key::C => Some(0x32),
        Key::D => Some(0x2A),
        Key::E => Some(0x22),
        Key::F => Some(0x2B),
        Key::G => Some(0x2C),
        Key::H => Some(0x2D),
        Key::I => Some(0x27),
        Key::J => Some(0x2E),
        Key::K => Some(0x2F),
        Key::L => Some(0x19),
        Key::M => Some(0x36),
        Key::N => Some(0x35),
        Key::O => Some(0x18),
        Key::P => Some(0x1C),
        Key::Q => Some(0x20),
        Key::R => Some(0x23),
        Key::S => Some(0x29),
        Key::T => Some(0x24),
        Key::U => Some(0x26),
        Key::V => Some(0x33),
        Key::W => Some(0x21),
        Key::X => Some(0x31),
        Key::Y => Some(0x25),
        Key::Z => Some(0x30),

        // Number keys
        Key::Key0 => Some(0x3C),
        Key::Key1 => Some(0x34),
        Key::Key2 => Some(0x35),
        Key::Key3 => Some(0x36),
        Key::Key4 => Some(0x2C),
        Key::Key5 => Some(0x2D),
        Key::Key6 => Some(0x2E),
        Key::Key7 => Some(0x24),
        Key::Key8 => Some(0x25),
        Key::Key9 => Some(0x26),

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

/// Map a resized window coordinate through the aspect-ratio letterbox.
fn window_to_skin_pos(
    mouse: (f32, f32),
    window_size: (usize, usize),
    skin_size: (usize, usize),
) -> Option<(usize, usize)> {
    let (window_width, window_height) = (window_size.0 as f32, window_size.1 as f32);
    let (skin_width, skin_height) = (skin_size.0 as f32, skin_size.1 as f32);
    if window_width <= 0.0 || window_height <= 0.0 {
        return None;
    }

    let scale = (window_width / skin_width).min(window_height / skin_height);
    let drawn_width = skin_width * scale;
    let drawn_height = skin_height * scale;
    let offset_x = (window_width - drawn_width) / 2.0;
    let offset_y = (window_height - drawn_height) / 2.0;
    if mouse.0 < offset_x
        || mouse.0 >= offset_x + drawn_width
        || mouse.1 < offset_y
        || mouse.1 >= offset_y + drawn_height
    {
        return None;
    }

    Some((
        ((mouse.0 - offset_x) / scale) as usize,
        ((mouse.1 - offset_y) / scale) as usize,
    ))
}

fn load_persistent_state_if_present(emu: &mut Emulator, path: Option<&Path>) -> Result<bool> {
    if let Some(path) = path.filter(|path| path.exists()) {
        let state = read_persistent_state_file(path)?;
        emu.load_persistent_state(&state)
            .with_context(|| format!("Failed to load persistent state: {}", path.display()))?;
        log::info!("Persistent state loaded from {}", path.display());
        return Ok(true);
    }
    Ok(false)
}

fn save_persistent_state_if_requested(emu: &Emulator, path: Option<&Path>) -> Result<()> {
    if let Some(path) = path {
        let state = emu.save_persistent_state()?;
        write_persistent_state_file(path, &state)?;
        log::info!("Persistent state saved to {}", path.display());
    }
    Ok(())
}

fn read_persistent_state_file(path: &Path) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open state file: {}", path.display()))?;
    let mut decoder = GzDecoder::new(BufReader::new(file));
    let mut state = Vec::new();
    decoder
        .read_to_end(&mut state)
        .with_context(|| format!("Failed to decompress state file: {}", path.display()))?;
    Ok(state)
}

fn write_persistent_state_file(path: &Path, state: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to create temporary state file for {}",
            path.display()
        )
    })?;
    {
        let writer = BufWriter::new(temporary.as_file_mut());
        let mut encoder = GzEncoder::new(writer, Compression::fast());
        encoder
            .write_all(state)
            .context("Failed to compress persistent state")?;
        let mut writer = encoder
            .finish()
            .context("Failed to finish persistent state compression")?;
        writer.flush().context("Failed to flush persistent state")?;
    }
    temporary
        .as_file()
        .sync_all()
        .context("Failed to sync persistent state")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace state file: {}", path.display()))?;
    Ok(())
}

fn normalized_output_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path)
            .with_context(|| format!("Failed to resolve path: {}", path.display()));
    }
    let file_name = path
        .file_name()
        .context("State file path must include a file name")?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = std::fs::canonicalize(parent)
        .with_context(|| format!("Failed to resolve directory: {}", parent.display()))?;
    Ok(parent.join(file_name))
}

fn validate_state_file_path(path: Option<&Path>, files: &RomFiles) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let state_path = normalized_output_path(path)?;
    for source in [&files.rom, &files.nor, &files.nand, &files.nand0]
        .into_iter()
        .flatten()
    {
        if normalized_output_path(source)? == state_path {
            anyhow::bail!(
                "State file must not overwrite a ROM or Flash dump: {}",
                path.display()
            );
        }
    }
    Ok(())
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
    validate_state_file_path(args.state_file.as_deref(), &files)?;

    // Create emulator
    let mut emu = Emulator::new(model, &files)?;
    emu.reset();
    load_persistent_state_if_present(&mut emu, args.state_file.as_deref())?;

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
        save_persistent_state_if_requested(&emu, args.state_file.as_deref())?;
        return Ok(());
    }

    // Create a normal resizable window whose client area is the device skin.
    let skin = DeviceSkin::load(model, args.scale)?;
    let window_width = skin.width();
    let window_height = skin.height();

    let mut window = Window::new(
        &format!("WQXEmu - {}", model.name().to_uppercase()),
        window_width,
        window_height,
        WindowOptions {
            resize: true,
            scale_mode: minifb::ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window");

    window.set_target_fps(emu.frame_rate() as usize);

    // Main event loop
    let layout = layout_for(model);
    let mut pressed = [false; 64];
    let mut mouse_input: Option<SkinInput> = None;

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
        let mouse_pos = window
            .get_unscaled_mouse_pos(minifb::MouseMode::Discard)
            .and_then(|mouse| {
                window_to_skin_pos(mouse, window.get_size(), (window_width, window_height))
            });
        if mouse_down {
            let next_input = mouse_pos.and_then(|(x, y)| skin.hit_test(x, y, layout));
            if mouse_input != next_input {
                if let Some(SkinInput::Key(old)) = mouse_input {
                    emu.set_key(old, false);
                    pressed[old as usize] = false;
                }
                match next_input {
                    Some(SkinInput::Key(key_id)) => {
                        emu.set_key(key_id, true);
                        pressed[key_id as usize] = true;
                    }
                    Some(SkinInput::Reset) => {
                        emu.reset();
                        pressed.fill(false);
                    }
                    None => {}
                }
                mouse_input = next_input;
            }
        } else {
            if let Some(SkinInput::Key(old)) = mouse_input {
                emu.set_key(old, false);
                pressed[old as usize] = false;
            }
            mouse_input = None;
        }

        // Run one frame
        emu.run_frame();

        // Get framebuffer and render
        let pixels = emu.framebuffer();

        let buffer = skin.render(&pixels, layout, &pressed);

        window
            .update_with_buffer(&buffer, window_width, window_height)
            .expect("Failed to update window");
    }

    save_persistent_state_if_requested(&emu, args.state_file.as_deref())?;

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

#[cfg(test)]
mod tests {
    use super::{
        map_key_nc1020, read_persistent_state_file, validate_state_file_path, window_to_skin_pos,
        write_persistent_state_file, Args,
    };
    use clap::Parser;
    use minifb::Key;
    use std::path::{Path, PathBuf};
    use wqxemu_core::{key_ids, RomFiles};

    #[test]
    fn persistent_state_file_is_opt_in_and_accepts_any_path() {
        let default_args = Args::try_parse_from(["wqxemu"]).unwrap();
        assert!(default_args.state_file.is_none());

        let enabled_args = Args::try_parse_from(["wqxemu", "--state-file", "device.wqxs"]).unwrap();
        assert_eq!(enabled_args.state_file, Some(PathBuf::from("device.wqxs")));
    }

    #[test]
    fn persistent_state_file_must_not_alias_a_source_dump() {
        let files = RomFiles::new(None, Some(PathBuf::from("device.nor")), None, None);
        assert!(validate_state_file_path(Some(Path::new("device.wqxs")), &files).is_ok());
        assert!(validate_state_file_path(Some(Path::new("device.nor")), &files).is_err());
    }

    #[test]
    fn persistent_state_file_is_compressed_and_atomically_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("device.wqxs");

        write_persistent_state_file(&path, b"first state").unwrap();
        write_persistent_state_file(&path, b"replacement state").unwrap();

        assert_eq!(
            read_persistent_state_file(&path).unwrap(),
            b"replacement state"
        );
    }

    #[test]
    fn resized_window_coordinates_follow_the_letterboxed_skin() {
        assert_eq!(
            window_to_skin_pos((400.0, 300.0), (800, 600), (400, 600)),
            Some((200, 300))
        );
        assert_eq!(
            window_to_skin_pos((100.0, 300.0), (800, 600), (400, 600)),
            None
        );
    }

    #[test]
    fn nc1020_pc_keyboard_uses_reference_matrix_ids() {
        assert_eq!(map_key_nc1020(Key::F5), Some(key_ids::F5));
        assert_eq!(map_key_nc1020(Key::F9), Some(key_ids::F9));
        assert_eq!(map_key_nc1020(Key::Q), Some(0x20));
        assert_eq!(map_key_nc1020(Key::A), Some(0x28));
        assert_eq!(map_key_nc1020(Key::Space), Some(0x3E));
        assert_eq!(map_key_nc1020(Key::Key1), Some(0x34));
    }
}
