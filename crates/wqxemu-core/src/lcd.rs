// LCD controller emulation for NC1020.
//
// The NC1020 uses an SPLD803A-compatible LCD controller.
// Display: 160x80 pixels, 1-bit (black and white).
// LCD framebuffer is stored in RAM at a configurable address.
//
// LCD address calculation:
//   High byte from register 0x0C bits[0:1]
//   Low byte from register 0x06 (shifted left by 4)
//   lcd_addr = ((reg_0x0C & 0x03) << 12) | (reg_0x06 << 4)

use serde::{Deserialize, Serialize};

/// LCD width in pixels
pub const LCD_WIDTH: usize = 160;
/// LCD height in pixels
pub const LCD_HEIGHT: usize = 80;
/// LCD framebuffer size in bytes (160*80/8 = 1600 bytes)
pub const LCD_BUFFER_SIZE: usize = LCD_WIDTH * LCD_HEIGHT / 8;
/// Frame rate (30 fps for NC1020)
pub const FRAME_RATE: u32 = 30;
/// CPU cycles per frame at 5.12MHz
pub const CYCLES_PER_FRAME: u64 = 5_120_000 / FRAME_RATE as u64;

fn default_pixel_brightness() -> Vec<u8> {
    vec![0xFF; LCD_WIDTH * LCD_HEIGHT]
}

/// LCD color for pixel rendering
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LcdColor {
    /// Black pixel (bit set)
    On,
    /// White pixel (bit clear)
    Off,
}

/// LCD controller state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lcd {
    /// LCD framebuffer (160x80 pixels, 1 bit per pixel, packed bytes)
    /// Stored as raw bytes from RAM, converted to RGBA on output
    framebuffer: Vec<u8>,
    /// Per-pixel LCD persistence brightness (0 = black, 255 = white).
    #[serde(skip, default = "default_pixel_brightness")]
    pixel_brightness: Vec<u8>,
    /// Current LCD address in RAM
    pub lcd_addr: u32,
    /// Whether LCD has been updated this frame
    dirty: bool,
}

impl Lcd {
    /// Create a new LCD controller
    pub fn new() -> Self {
        Self {
            framebuffer: vec![0; LCD_BUFFER_SIZE],
            pixel_brightness: default_pixel_brightness(),
            lcd_addr: 0,
            dirty: false,
        }
    }

    /// Reset LCD state
    pub fn reset(&mut self) {
        self.framebuffer.fill(0);
        if self.pixel_brightness.len() != LCD_WIDTH * LCD_HEIGHT {
            self.pixel_brightness = default_pixel_brightness();
        } else {
            self.pixel_brightness.fill(0xFF);
        }
        self.lcd_addr = 0;
        self.dirty = false;
    }

    /// Update LCD address from IO registers
    /// Called when register 0x06 or 0x0C is written
    pub fn update_address(&mut self, io: &[u8]) {
        if io.len() > 0x0C {
            self.lcd_addr = (((io[0x0C] & 0x03) as u32) << 12) | ((io[0x06] as u32) << 4);
        }
    }

    /// Copy LCD framebuffer from RAM
    /// Returns true if the framebuffer was successfully copied
    pub fn copy_from_ram(&mut self, ram: &[u8]) -> bool {
        if self.lcd_addr == 0 {
            return false;
        }
        let addr = self.lcd_addr as usize;
        self.copy_from(ram, addr)
    }

    /// Copy the framebuffer from a RAM region at the given address.
    pub fn copy_from(&mut self, ram: &[u8], addr: usize) -> bool {
        if addr + LCD_BUFFER_SIZE <= ram.len() {
            self.framebuffer
                .copy_from_slice(&ram[addr..addr + LCD_BUFFER_SIZE]);
            self.update_pixel_brightness(false);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Copy a framebuffer while simulating the persistence of a passive LCD.
    pub fn copy_from_with_persistence(&mut self, ram: &[u8], addr: usize) -> bool {
        if addr + LCD_BUFFER_SIZE <= ram.len() {
            self.framebuffer
                .copy_from_slice(&ram[addr..addr + LCD_BUFFER_SIZE]);
            self.update_pixel_brightness(true);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    fn update_pixel_brightness(&mut self, persistence: bool) {
        if self.pixel_brightness.len() != LCD_WIDTH * LCD_HEIGHT {
            self.pixel_brightness = vec![0xFF; LCD_WIDTH * LCD_HEIGHT];
        }
        for (index, brightness) in self.pixel_brightness.iter_mut().enumerate() {
            let byte = self.framebuffer[index / 8];
            let pixel_on = byte & (1 << (7 - index % 8)) != 0;
            if !persistence {
                *brightness = if pixel_on { 0 } else { 0xFF };
            } else if pixel_on {
                let delta = (*brightness / 6).max(1);
                *brightness = brightness.saturating_sub(delta);
            } else {
                let delta = ((0xFF - *brightness) / 8).max(1);
                *brightness = brightness.saturating_add(delta);
            }
        }
    }

    /// Get the raw framebuffer data (160x80, 1 bit per pixel)
    pub fn framebuffer_raw(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Get the framebuffer as XRGB8888 pixels (for libretro)
    /// Returns 160*80 = 12800 u32 values
    pub fn framebuffer_xrgb8888(&self) -> Vec<u32> {
        let mut pixels = Vec::with_capacity(LCD_WIDTH * LCD_HEIGHT);
        for &brightness in &self.pixel_brightness {
            let value = brightness as u32;
            pixels.push(0xFF00_0000 | value << 16 | value << 8 | value);
        }
        pixels
    }

    /// Get the framebuffer as RGBA8888 bytes (for standalone frontend)
    /// Returns 160*80*4 = 51200 bytes
    pub fn framebuffer_rgba8888(&self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(LCD_WIDTH * LCD_HEIGHT * 4);
        for &brightness in &self.pixel_brightness {
            pixels.extend_from_slice(&[brightness, brightness, brightness, 0xFF]);
        }
        pixels
    }

    /// Check if LCD was updated this frame
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

impl Default for Lcd {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_blends_alternating_subframes() {
        let mut lcd = Lcd::new();
        let off = vec![0; LCD_BUFFER_SIZE];
        let on = vec![0xFF; LCD_BUFFER_SIZE];

        for frame in 0..120 {
            let source = if frame % 2 == 0 { &on } else { &off };
            assert!(lcd.copy_from_with_persistence(source, 0));
        }
        let first = lcd.framebuffer_xrgb8888()[0] & 0xFF;
        assert!(lcd.copy_from_with_persistence(&on, 0));
        let second = lcd.framebuffer_xrgb8888()[0] & 0xFF;

        assert!((64..224).contains(&first));
        assert!(first.abs_diff(second) < 32);
    }
}
