// IO register handler for NC1020.
//
// IO registers are at addresses 0x0000-0x003F (64 bytes).
// Each register can have custom read/write behavior with side effects.
//
// Key registers:
//   0x00: Bank switch
//   0x01: Interrupt flag 1
//   0x02: Interrupt flag 2
//   0x05: Sleep control (bit3 = sleep enable)
//   0x06: LCD address low
//   0x08: Keypad data read
//   0x09: Keypad scan write
//   0x0A: ROA/BBS control
//   0x0B: Keypad status
//   0x0C: LCD address high (bit[0:1])
//   0x0D: Volume select
//   0x0F: Zero page switch
//   0x20: JG WAV control
//   0x22: JG WAV data
//   0x23: JG WAV command
//   0x3B: Clock data read (special)
//   0x3D: Clock flags
//   0x3E: Clock index
//   0x3F: Clock data read/write

use crate::input::Input;
use crate::lcd::Lcd;
use crate::timer::Timer;
use crate::audio::Audio;
use crate::memory::Memory;

/// IO register handler
pub struct IoHandler;

impl IoHandler {
    /// Read from an IO register
    pub fn read(
        addr: u8,
        io: &[u8],
        input: &Input,
        timer: &Timer,
        audio: &Audio,
    ) -> u8 {
        match addr {
            // Register 0x06: LCD address low
            0x06 => io[0x06],

            // Register 0x08: Keypad data (set by scan register 0x09)
            0x08 => io[0x08],

            // Register 0x0B: Keypad status
            0x0B => {
                let mut status = io[0x0B];
                // Bit 0: keypad status
                if input.keypad_status() & 0x01 != 0 {
                    status |= 0x01;
                } else {
                    status &= 0xFE;
                }
                status
            }

            // Register 0x3B: Clock data read (special handling)
            0x3B => {
                if io[0x3D] & 0x03 == 0 {
                    // Return clock data with bit 0 cleared
                    timer.read_clock(0x3B) & 0xFE
                } else {
                    io[0x3B]
                }
            }

            // Register 0x3F: Clock data read (via index register 0x3E)
            0x3F => {
                let idx = io[0x3E];
                if idx < 80 {
                    timer.read_clock(idx)
                } else {
                    0
                }
            }

            // All other registers: return stored value
            _ => io[addr as usize],
        }
    }

    /// Write to an IO register
    /// Returns the side effects that occurred
    pub fn write(
        addr: u8,
        value: u8,
        io: &mut [u8],
        input: &mut Input,
        timer: &mut Timer,
        audio: &mut Audio,
        lcd: &mut Lcd,
        memory: &mut Memory,
    ) -> IoSideEffects {
        let mut effects = IoSideEffects::default();

        match addr {
            // Register 0x00: Bank switch
            0x00 => {
                let old_value = io[0x00];
                io[0x00] = value;
                if value != old_value {
                    memory.write_bank_switch(value);
                    effects.bank_changed = true;
                }
            }

            // Register 0x05: Sleep control
            0x05 => {
                let old_value = io[0x05];
                io[0x05] = value;
                if (old_value ^ value) & 0x08 != 0 {
                    input.slept = value & 0x08 == 0;
                    effects.sleep_changed = true;
                }
            }

            // Register 0x06: LCD address low
            0x06 => {
                io[0x06] = value;
                if lcd.lcd_addr == 0 {
                    lcd.update_address(io);
                }
                // Clear interrupt flag 1 bit 0
                io[0x09] &= 0xFE;
            }

            // Register 0x08: Clear keypad interrupt
            0x08 => {
                io[0x08] = value;
                // Clear interrupt flag 1 bit 0
                io[0x0B] &= 0xFE;
            }

            // Register 0x09: Keypad scan
            0x09 => {
                io[0x09] = value;
                // Read keypad matrix for the selected row
                io[0x08] = input.read_keypad(value);
                // Update keypad status
                match value {
                    0x00 => {
                        // Check row 7 for power key
                        if input.matrix[7] & 0xFE == 0 {
                            io[0x0B] |= 0x01; // No key pressed
                        } else {
                            io[0x0B] &= 0xFE; // Key pressed
                        }
                    }
                    _ => {}
                }
            }

            // Register 0x0A: ROA/BBS control
            0x0A => {
                let old_value = io[0x0A];
                io[0x0A] = value;
                if value != old_value {
                    memory.write_roa_bbs(value);
                    effects.bbs_changed = true;
                }
            }

            // Register 0x0D: Volume select
            0x0D => {
                let old_value = io[0x0D];
                io[0x0D] = value;
                if value != old_value {
                    // Volume change triggers full memory remap
                    effects.volume_changed = true;
                }
            }

            // Register 0x0F: Zero page switch
            0x0F => {
                let old_value = io[0x0F];
                io[0x0F] = value;
                let new_idx = value & 0x07;
                let old_idx = old_value & 0x07;
                if new_idx != old_idx {
                    memory.switch_zp40(value);
                    effects.zp_changed = true;
                }
            }

            // Register 0x20: JG WAV control
            0x20 => {
                io[0x20] = value;
                audio.write_control(value);
            }

            // Register 0x22: JG WAV data
            0x22 => {
                io[0x22] = value;
            }

            // Register 0x23: JG WAV command
            0x23 => {
                io[0x23] = value;
                let data = io[0x22];
                audio.write_command(data, value);
                // Update status register
                if value == 0x80 {
                    io[0x20] = 0x80;
                }
            }

            // Register 0x3E: Clock index
            0x3E => {
                io[0x3E] = value;
            }

            // Register 0x3F: Clock data write
            0x3F => {
                io[0x3F] = value;
                let idx = io[0x3E];
                timer.write_clock(idx, value);
            }

            // All other registers: simple write
            _ => {
                io[addr as usize] = value;
            }
        }

        effects
    }
}

/// Side effects from IO register writes
#[derive(Default)]
pub struct IoSideEffects {
    /// Bank switch register changed
    pub bank_changed: bool,
    /// Sleep state changed
    pub sleep_changed: bool,
    /// BBS (banked page) changed
    pub bbs_changed: bool,
    /// Volume changed
    pub volume_changed: bool,
    /// Zero page switch changed
    pub zp_changed: bool,
}
