// Keyboard matrix input for NC1020.
//
// The NC1020 has an 8x8 keyboard matrix.
// Key IDs map to (row, column) pairs.
// Register 0x09 selects the row to scan.
// Register 0x08 returns the column state for the selected row.
//
// Key matrix layout:
//   Row 0: key_id 0x00-0x07
//   Row 1: key_id 0x08-0x0F
//   Row 2: key_id 0x10-0x17
//   ...
//   Row 7: key_id 0x38-0x3F
//
// Key ID encoding: row = key_id % 8, col = key_id / 8
// Special: key_id 0x0F (Power) sets all bits in row 7

use serde::{Deserialize, Serialize};

/// Total number of keys (8x8 matrix)
pub const KEY_COUNT: usize = 64;

/// Key ID constants for common keys
pub mod key_ids {
    /// Power button (special: sets all bits in row 7)
    pub const POWER: u8 = 0x0F;
    /// F1 key
    pub const F1: u8 = 0x10;
    /// F2 key
    pub const F2: u8 = 0x11;
    /// F3 key
    pub const F3: u8 = 0x12;
    /// F4 key
    pub const F4: u8 = 0x13;
    /// Up arrow
    pub const UP: u8 = 0x1A;
    /// Down arrow
    pub const DOWN: u8 = 0x1B;
    /// Left arrow
    pub const LEFT: u8 = 0x3F;
    /// Right arrow
    pub const RIGHT: u8 = 0x1F;
    /// Enter
    pub const ENTER: u8 = 0x1D;
    /// ESC
    pub const ESC: u8 = 0x3B;
    /// Space
    pub const SPACE: u8 = 0x35;
    /// Backspace
    pub const BACKSPACE: u8 = 0x36;
    /// Page Up
    pub const PAGE_UP: u8 = 0x37;
    /// Page Down
    pub const PAGE_DOWN: u8 = 0x1E;
    /// F10 (main screen)
    pub const F10: u8 = 0x08;
    /// F11 (game menu)
    pub const F11: u8 = 0x0E;
}

/// Keyboard matrix state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    /// Keyboard matrix (8 rows, each byte has 8 column bits)
    pub matrix: [u8; 8],
    /// Whether the device is in sleep mode
    pub slept: bool,
    /// Whether a wake-up is pending
    pub should_wake_up: bool,
    /// Wake-up pending flag
    pub wake_up_pending: bool,
    /// Wake-up key code
    pub wake_up_key: u8,
}

impl Input {
    /// Create a new input system
    pub fn new() -> Self {
        Self {
            matrix: [0; 8],
            slept: false,
            should_wake_up: false,
            wake_up_pending: false,
            wake_up_key: 0,
        }
    }

    /// Reset input state
    pub fn reset(&mut self) {
        self.matrix.fill(0);
        self.slept = false;
        self.should_wake_up = false;
        self.wake_up_pending = false;
        self.wake_up_key = 0;
    }

    /// Set a key state (pressed or released)
    /// key_id: 0x00-0x3F (row = key_id % 8, col = key_id / 8)
    pub fn set_key(&mut self, key_id: u8, pressed: bool) {
        let row = (key_id % 8) as usize;
        let col = key_id / 8;
        let bits = if key_id == 0x0F { 0xFE } else { 1 << col };

        if pressed {
            self.matrix[row] |= bits;
        } else {
            self.matrix[row] &= !bits;
        }

        // Handle sleep/wake
        if pressed {
            if self.slept {
                // Wake up on specific keys (F1-F9, not F10)
                if key_id >= 0x08 && key_id <= 0x0F && key_id != 0x0E {
                    self.wake_up_key = match key_id {
                        0x08 => 0x00,
                        0x09 => 0x0A,
                        0x0A => 0x08,
                        0x0B => 0x06,
                        0x0C => 0x04,
                        0x0D => 0x02,
                        0x0E => 0x0C,
                        0x0F => 0x00,
                        _ => 0x00,
                    };
                    self.should_wake_up = true;
                    self.wake_up_pending = true;
                    self.slept = false;
                }
            } else {
                // Power button puts device to sleep
                if key_id == 0x0F {
                    self.slept = true;
                }
            }
        }
    }

    /// Read keypad data for a given scan row
    /// Register 0x09 selects the row, this returns the column bits
    pub fn read_keypad(&self, scan_value: u8) -> u8 {
        match scan_value {
            0x01 => self.matrix[0],
            0x02 => self.matrix[1],
            0x04 => self.matrix[2],
            0x08 => self.matrix[3],
            0x10 => self.matrix[4],
            0x20 => self.matrix[5],
            0x40 => self.matrix[6],
            0x80 => self.matrix[7],
            0x00 => {
                // Special: check if power key (row 7, bit 1) is pressed
                if self.matrix[7] & 0xFE != 0 {
                    0 // Return 0 if any key in row 7 is pressed
                } else {
                    1 // Return 1 if no key in row 7
                }
            }
            0x7F => {
                // Scan all rows at once
                self.matrix[0] | self.matrix[1] | self.matrix[2] | self.matrix[3] |
                self.matrix[4] | self.matrix[5] | self.matrix[6] | self.matrix[7]
            }
            _ => 0xFF, // No key pressed
        }
    }

    /// Check if any key is pressed in row 7 (for interrupt generation)
    pub fn is_key_pressed_row7(&self) -> bool {
        self.matrix[7] & 0xFE != 0
    }

    /// Get the keypad status for register 0x0B
    /// Bit 0: 1 if no key in row 7 is pressed, 0 if any key is pressed
    pub fn keypad_status(&self) -> u8 {
        if self.matrix[7] & 0xFE != 0 {
            0x00
        } else {
            0x01
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
