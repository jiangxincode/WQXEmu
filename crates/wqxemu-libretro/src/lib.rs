// WQXEmu libretro core - RetroArch integration for Wenquxing emulators.
//
// This crate implements the libretro C API, allowing WQXEmu to be loaded
// as a core in RetroArch. It wraps the platform-independent wqxemu-core
// emulator and bridges it to the libretro callbacks.

#![allow(clippy::upper_case_acronyms)]
#![allow(static_mut_refs)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(private_interfaces)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::manual_range_contains)]

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::panic;
use std::path::Path;
use std::ptr;

use wqxemu_core::input::key_ids;
use wqxemu_core::{Emulator, LCD_HEIGHT, LCD_WIDTH};

// ============================================================
// libretro constants
// ============================================================

const RETRO_API_VERSION: u32 = 1;
const RETRO_REGION_NTSC: u32 = 0;

// Environment commands
const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: u32 = 1;
const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: u32 = 11;
const RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK: u32 = 12;
const RETRO_ENVIRONMENT_SET_VARIABLES: u32 = 16;
const RETRO_ENVIRONMENT_GET_VARIABLE: u32 = 17;
const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: u32 = 18;
const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: u32 = 9;
const RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY: u32 = 31;

// Pixel format
const RETRO_PIXEL_FORMAT_XRGB8888: u32 = 0;
const RETRO_PIXEL_FORMAT_RGB565: u32 = 1;

// Memory types
const RETRO_MEMORY_SAVE_RAM: u32 = 0;
const RETRO_MEMORY_SYSTEM_RAM: u32 = 2;

// Joypad constants
const RETRO_DEVICE_JOYPAD: u32 = 1;
const RETRO_DEVICE_ID_JOYPAD_B: u32 = 0;
const RETRO_DEVICE_ID_JOYPAD_Y: u32 = 1;
const RETRO_DEVICE_ID_JOYPAD_SELECT: u32 = 2;
const RETRO_DEVICE_ID_JOYPAD_START: u32 = 3;
const RETRO_DEVICE_ID_JOYPAD_UP: u32 = 4;
const RETRO_DEVICE_ID_JOYPAD_DOWN: u32 = 5;
const RETRO_DEVICE_ID_JOYPAD_LEFT: u32 = 6;
const RETRO_DEVICE_ID_JOYPAD_RIGHT: u32 = 7;
const RETRO_DEVICE_ID_JOYPAD_A: u32 = 8;
const RETRO_DEVICE_ID_JOYPAD_X: u32 = 9;
const RETRO_DEVICE_ID_JOYPAD_L: u32 = 10;
const RETRO_DEVICE_ID_JOYPAD_R: u32 = 11;

// Keyboard constants
const RETROK_RETURN: u32 = 13;
const RETROK_ESCAPE: u32 = 27;
const RETROK_SPACE: u32 = 32;
const RETROK_LEFT: u32 = 0x250;
const RETROK_UP: u32 = 0x251;
const RETROK_RIGHT: u32 = 0x252;
const RETROK_DOWN: u32 = 0x253;
const RETROK_A: u32 = 97;
const RETROK_Z: u32 = 122;
const RETROK_0: u32 = 48;
const RETROK_9: u32 = 57;
const RETROK_F1: u32 = 282;
const RETROK_F12: u32 = 293;
const RETROK_BACKSPACE: u32 = 8;
const RETROK_DELETE: u32 = 127;
const RETROK_PAGEUP: u32 = 0x254;
const RETROK_PAGEDOWN: u32 = 0x255;

// ============================================================
// libretro types
// ============================================================

type RetroEnvironmentT = Option<unsafe extern "C" fn(cmd: u32, data: *mut c_void) -> bool>;
type RetroVideoRefreshT =
    Option<unsafe extern "C" fn(data: *const c_void, width: u32, height: u32, pitch: usize)>;
type RetroAudioSampleT = Option<unsafe extern "C" fn(left: i16, right: i16)>;
type RetroAudioSampleBatchT =
    Option<unsafe extern "C" fn(data: *const i16, frames: usize) -> usize>;
type RetroInputPollT = Option<unsafe extern "C" fn()>;
type RetroInputStateT =
    Option<unsafe extern "C" fn(port: u32, device: u32, index: u32, id: u32) -> i16>;
type RetroKeyboardCallbackT =
    Option<unsafe extern "C" fn(down: bool, keycode: u32, character: u32, key_modifiers: u16)>;

#[repr(C)]
struct RetroSystemInfo {
    library_name: *const c_char,
    library_version: *const c_char,
    valid_extensions: *const c_char,
    need_fullpath: bool,
    block_extract: bool,
}

#[repr(C)]
struct RetroGameGeometry {
    base_width: u32,
    base_height: u32,
    max_width: u32,
    max_height: u32,
    aspect_ratio: f32,
}

#[repr(C)]
struct RetroSystemTiming {
    fps: f64,
    sample_rate: f64,
}

#[repr(C)]
struct RetroSystemAvInfo {
    geometry: RetroGameGeometry,
    timing: RetroSystemTiming,
}

#[repr(C)]
struct RetroGameInfo {
    path: *const c_char,
    data: *const c_void,
    size: usize,
    meta: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetroVariable {
    key: *const c_char,
    value: *const c_char,
}

#[repr(C)]
struct RetroInputDescriptor {
    port: u32,
    device: u32,
    index: u32,
    id: u32,
    description: *const c_char,
}

// ============================================================
// Global state
// ============================================================

static mut EMULATOR: Option<Emulator> = None;
static mut ENV_CB: RetroEnvironmentT = None;
static mut VIDEO_CB: RetroVideoRefreshT = None;
static mut AUDIO_CB: RetroAudioSampleT = None;
static mut AUDIO_BATCH_CB: RetroAudioSampleBatchT = None;
static mut INPUT_POLL_CB: RetroInputPollT = None;
static mut INPUT_STATE_CB: RetroInputStateT = None;
static mut KEYBOARD_CB: RetroKeyboardCallbackT = None;
static mut SYSTEM_DIR: Option<String> = None;
static mut SAVE_DIR: Option<String> = None;

// ============================================================
// Helper functions
// ============================================================

unsafe fn get_emulator() -> &'static Emulator {
    EMULATOR.as_ref().expect("Emulator not initialized")
}

unsafe fn get_emulator_mut() -> &'static mut Emulator {
    EMULATOR.as_mut().expect("Emulator not initialized")
}

unsafe fn environment(cmd: u32, data: *mut c_void) -> bool {
    ENV_CB.map(|cb| cb(cmd, data)).unwrap_or(false)
}

/// Map RetroArch keyboard keycode to NC1020 key ID
fn map_keyboard_key(keycode: u32) -> Option<u8> {
    match keycode {
        RETROK_RETURN => Some(key_ids::ENTER),
        RETROK_ESCAPE => Some(key_ids::ESC),
        RETROK_SPACE => Some(key_ids::SPACE),
        RETROK_BACKSPACE => Some(key_ids::BACKSPACE),
        RETROK_UP => Some(key_ids::UP),
        RETROK_DOWN => Some(key_ids::DOWN),
        RETROK_LEFT => Some(key_ids::LEFT),
        RETROK_RIGHT => Some(key_ids::RIGHT),
        RETROK_PAGEUP => Some(key_ids::PAGE_UP),
        RETROK_PAGEDOWN => Some(key_ids::PAGE_DOWN),
        RETROK_DELETE => Some(key_ids::POWER),
        // F keys
        282 => Some(key_ids::F1),  // F1
        283 => Some(key_ids::F2),  // F2
        284 => Some(key_ids::F3),  // F3
        285 => Some(key_ids::F4),  // F4
        286 => Some(key_ids::F5),  // F5
        287 => Some(key_ids::F6),  // F6
        288 => Some(key_ids::F7),  // F7
        289 => Some(key_ids::F8),  // F8
        290 => Some(key_ids::F9),  // F9
        291 => Some(key_ids::F10), // F10
        292 => Some(key_ids::F11), // F11
        // Letters
        k if k >= RETROK_A && k <= RETROK_Z => {
            let letter_idx = (k - RETROK_A) as u8;
            Some(0x10 + letter_idx) // Map to NC1020 key matrix
        }
        // Numbers
        k if k >= RETROK_0 && k <= RETROK_9 => {
            let num_idx = (k - RETROK_0) as u8;
            Some(0x20 + num_idx) // Map to NC1020 key matrix
        }
        _ => None,
    }
}

/// Map RetroPad button to NC1020 key ID
fn map_joypad_button(button: u32) -> Option<u8> {
    match button {
        RETRO_DEVICE_ID_JOYPAD_UP => Some(key_ids::UP),
        RETRO_DEVICE_ID_JOYPAD_DOWN => Some(key_ids::DOWN),
        RETRO_DEVICE_ID_JOYPAD_LEFT => Some(key_ids::LEFT),
        RETRO_DEVICE_ID_JOYPAD_RIGHT => Some(key_ids::RIGHT),
        RETRO_DEVICE_ID_JOYPAD_A => Some(key_ids::ENTER),
        RETRO_DEVICE_ID_JOYPAD_B => Some(key_ids::ESC),
        RETRO_DEVICE_ID_JOYPAD_X => Some(key_ids::F1),
        RETRO_DEVICE_ID_JOYPAD_Y => Some(key_ids::F4),
        RETRO_DEVICE_ID_JOYPAD_L => Some(key_ids::PAGE_UP),
        RETRO_DEVICE_ID_JOYPAD_R => Some(key_ids::PAGE_DOWN),
        RETRO_DEVICE_ID_JOYPAD_START => Some(key_ids::F10),
        RETRO_DEVICE_ID_JOYPAD_SELECT => Some(key_ids::F11),
        _ => None,
    }
}

// ============================================================
// libretro API implementation
// ============================================================

/// Set environment callback
#[no_mangle]
pub extern "C" fn retro_set_environment(cb: RetroEnvironmentT) {
    unsafe {
        ENV_CB = cb;
    }
    // Set input descriptors
    set_input_descriptors();
    // Set core variables
    set_core_variables();
}

/// Set video refresh callback
#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: RetroVideoRefreshT) {
    unsafe {
        VIDEO_CB = cb;
    }
}

/// Set audio sample callback
#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: RetroAudioSampleT) {
    unsafe {
        AUDIO_CB = cb;
    }
}

/// Set audio sample batch callback
#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: RetroAudioSampleBatchT) {
    unsafe {
        AUDIO_BATCH_CB = cb;
    }
}

/// Set input poll callback
#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: RetroInputPollT) {
    unsafe {
        INPUT_POLL_CB = cb;
    }
}

/// Set input state callback
#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: RetroInputStateT) {
    unsafe {
        INPUT_STATE_CB = cb;
    }
}

/// Set keyboard callback
#[no_mangle]
pub extern "C" fn retro_set_keyboard_callback(cb: RetroKeyboardCallbackT) {
    unsafe {
        KEYBOARD_CB = cb;
    }
}

/// Return API version
#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

/// Initialize the core
#[no_mangle]
pub extern "C" fn retro_init() {
    unsafe {
        // Get system directory
        let mut sys_dir: *const c_char = ptr::null();
        if environment(
            RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,
            &mut sys_dir as *mut *const c_char as *mut c_void,
        ) && !sys_dir.is_null()
        {
            SYSTEM_DIR = Some(CStr::from_ptr(sys_dir).to_string_lossy().into_owned());
        }

        // Get save directory
        let mut save_dir: *const c_char = ptr::null();
        if environment(
            RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY,
            &mut save_dir as *mut *const c_char as *mut c_void,
        ) && !save_dir.is_null()
        {
            SAVE_DIR = Some(CStr::from_ptr(save_dir).to_string_lossy().into_owned());
        }

        // Set pixel format to XRGB8888
        let mut pixel_format = RETRO_PIXEL_FORMAT_XRGB8888;
        environment(
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
            &mut pixel_format as *mut u32 as *mut c_void,
        );
    }
    log::info!("WQXEmu libretro core initialized");
}

/// Deinitialize the core
#[no_mangle]
pub extern "C" fn retro_deinit() {
    unsafe {
        EMULATOR = None;
    }
    log::info!("WQXEmu libretro core deinitialized");
}

/// Get system information
#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    unsafe {
        (*info) = RetroSystemInfo {
            library_name: c"WQXEmu".as_ptr(),
            library_version: c"0.1.0".as_ptr(),
            valid_extensions: c"bin|rom|fls".as_ptr(),
            need_fullpath: true,
            block_extract: true,
        };
    }
}

/// Get system AV information
#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    unsafe {
        let fps = EMULATOR
            .as_ref()
            .map(|emulator| emulator.frame_rate() as f64)
            .unwrap_or(30.0);
        (*info) = RetroSystemAvInfo {
            geometry: RetroGameGeometry {
                base_width: LCD_WIDTH as u32,
                base_height: LCD_HEIGHT as u32,
                max_width: LCD_WIDTH as u32,
                max_height: LCD_HEIGHT as u32,
                aspect_ratio: LCD_WIDTH as f32 / LCD_HEIGHT as f32,
            },
            timing: RetroSystemTiming {
                fps,
                sample_rate: 44100.0,
            },
        };
    }
}

/// Set controller port device
#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {
    // NC1020 only supports basic input
}

/// Load a game
#[no_mangle]
pub extern "C" fn retro_load_game(info: *const RetroGameInfo) -> bool {
    unsafe {
        let game_info = &*info;

        // Check if path is valid
        if game_info.path.is_null() {
            log::error!("Game path is null");
            return false;
        }

        let path = match CStr::from_ptr(game_info.path).to_str() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Invalid game path: {}", e);
                return false;
            }
        };

        // Assemble ROM / Flash files. The loaded file is classified by
        // extension; sibling files with the same stem are picked up too
        // (e.g. loading `nc2000.nand` finds `nc2000.nor`/`nc2000.nand0`).
        let game_path = Path::new(path);
        let stem = game_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned());
        let ext = game_path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let parent = game_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        let mut files = wqxemu_core::RomFiles::new(None, None, None, None);
        match ext.as_deref() {
            Some("nand") => files.nand = Some(game_path.to_path_buf()),
            Some("nand0") => files.nand0 = Some(game_path.to_path_buf()),
            Some("fls") | Some("nor") => files.nor = Some(game_path.to_path_buf()),
            _ => files.rom = Some(game_path.to_path_buf()),
        }

        if files.nor.is_none() {
            if let Some(stem) = &stem {
                for ext in ["fls", "nor"] {
                    let candidate = parent.join(format!("{}.{}", stem, ext));
                    if candidate.exists() {
                        files.nor = Some(candidate);
                        break;
                    }
                }
            }
        }
        if files.nand.is_none() {
            if let Some(stem) = &stem {
                let candidate = parent.join(format!("{}.nand", stem));
                if candidate.exists() {
                    files.nand = Some(candidate);
                }
            }
        }
        if files.nand0.is_none() {
            if let Some(stem) = &stem {
                let candidate = parent.join(format!("{}.nand0", stem));
                if candidate.exists() {
                    files.nand0 = Some(candidate);
                }
            }
        }

        let model = wqxemu_core::detect_model(&files);
        log::info!("Detected model: {}", model.name());

        let mut emu = match Emulator::new(model, &files) {
            Ok(e) => e,
            Err(e) => {
                log::error!("Failed to create emulator: {}", e);
                return false;
            }
        };

        emu.reset();
        EMULATOR = Some(emu);

        log::info!("Game loaded: {}", path);
        true
    }
}

/// Unload a game
#[no_mangle]
pub extern "C" fn retro_unload_game() {
    unsafe {
        // Save NOR before unloading
        if let Some(ref emu) = EMULATOR {
            if let Some(ref save_dir) = SAVE_DIR {
                let nor_path = std::path::Path::new(save_dir).join("nc1020.fls");
                if let Err(e) = emu.save_nor(&nor_path.to_string_lossy()) {
                    log::warn!("Failed to save NOR: {}", e);
                }
            }
        }
        EMULATOR = None;
    }
}

/// Run one frame
#[no_mangle]
pub extern "C" fn retro_run() {
    unsafe {
        // Poll input
        if let Some(poll) = INPUT_POLL_CB {
            poll();
        }

        // Process joypad input
        if let (Some(get_state), Some(emu)) = (INPUT_STATE_CB, EMULATOR.as_mut()) {
            // Check all joypad buttons
            for button in 0..12 {
                let state = get_state(0, RETRO_DEVICE_JOYPAD, 0, button);
                if let Some(key_id) = map_joypad_button(button) {
                    emu.set_key(key_id, state != 0);
                }
            }
        }

        // Process keyboard input
        if let Some(ref mut emu) = EMULATOR {
            // Note: Keyboard callback handling would go here
            // For now, we rely on joypad mapping
        }

        // Run one frame
        if let Some(ref mut emu) = EMULATOR {
            emu.run_frame();

            // Video output
            if let Some(video_cb) = VIDEO_CB {
                let pixels = emu.framebuffer();
                video_cb(
                    pixels.as_ptr() as *const c_void,
                    LCD_WIDTH as u32,
                    LCD_HEIGHT as u32,
                    LCD_WIDTH * 4, // pitch in bytes
                );
            }

            // Audio output
            if let Some(audio_batch_cb) = AUDIO_BATCH_CB {
                let mut audio_samples = Vec::new();
                emu.drain_audio(&mut audio_samples);
                if !audio_samples.is_empty() {
                    audio_batch_cb(audio_samples.as_ptr(), audio_samples.len() / 2);
                }
            }
        }
    }
}

/// Serialize save state
#[no_mangle]
pub extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    unsafe {
        let emu = match EMULATOR.as_ref() {
            Some(e) => e,
            None => return false,
        };

        let state = emu.save_state();
        let bytes = match state.serialize() {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to serialize: {}", e);
                return false;
            }
        };

        if bytes.len() > size {
            log::error!(
                "Save state too large: {} bytes needed, {} available",
                bytes.len(),
                size
            );
            return false;
        }

        ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, bytes.len());
        true
    }
}

/// Deserialize save state
#[no_mangle]
pub extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    unsafe {
        let emu = match EMULATOR.as_mut() {
            Some(e) => e,
            None => return false,
        };

        let bytes = std::slice::from_raw_parts(data as *const u8, size);
        let state = match wqxemu_core::save::SaveState::deserialize(bytes) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to deserialize: {}", e);
                return false;
            }
        };

        if let Err(e) = emu.load_state(&state) {
            log::error!("Failed to load state: {}", e);
            return false;
        }

        true
    }
}

/// Get save state size
#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    // Return a generous size estimate
    1024 * 1024 // 1MB should be more than enough
}

/// Get memory data
#[no_mangle]
pub extern "C" fn retro_get_memory_data(id: u32) -> *mut c_void {
    unsafe {
        match id {
            RETRO_MEMORY_SAVE_RAM => {
                // Return NOR Flash as save RAM
                if let Some(ref emu) = EMULATOR {
                    // NOR data is private, we'd need to expose it
                    ptr::null_mut()
                } else {
                    ptr::null_mut()
                }
            }
            RETRO_MEMORY_SYSTEM_RAM => {
                // Return system RAM
                if let Some(ref mut emu) = EMULATOR {
                    // RAM is private, we'd need to expose it
                    ptr::null_mut()
                } else {
                    ptr::null_mut()
                }
            }
            _ => ptr::null_mut(),
        }
    }
}

/// Get memory size
#[no_mangle]
pub extern "C" fn retro_get_memory_size(id: u32) -> usize {
    match id {
        RETRO_MEMORY_SAVE_RAM => 1024 * 1024, // NOR Flash size
        RETRO_MEMORY_SYSTEM_RAM => 32 * 1024, // RAM size
        _ => 0,
    }
}

/// Reset the core
#[no_mangle]
pub extern "C" fn retro_reset() {
    unsafe {
        if let Some(ref mut emu) = EMULATOR {
            emu.reset();
        }
    }
}

/// Get region
#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    RETRO_REGION_NTSC
}

// ============================================================
// Helper functions for setup
// ============================================================

/// Set input descriptors for RetroArch
fn set_input_descriptors() {
    // Input descriptors are optional but help RetroArch show proper labels
    // We'll skip the full implementation for now
}

/// Set core variables for RetroArch
fn set_core_variables() {
    // Core variables allow users to configure the emulator through RetroArch UI
    // We'll skip the full implementation for now
}
