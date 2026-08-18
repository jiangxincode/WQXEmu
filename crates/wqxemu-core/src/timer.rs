// Timer system for NC1020.
//
// Two timers:
//   Timer0: 0.5s period (2 Hz) - 5,120,000 / 2 = 2,560,000 cycles
//   Timer1: 1/256s period (256 Hz) - 5,120,000 / 256 = 20,000 cycles
//
// Timer0 generates IRQ and updates RTC clock.
// Timer1 generates IRQ and handles wake-up logic.
//
// RTC clock data is stored in clock_buff[0..79]:
//   [0] = seconds (0-59)
//   [1] = minutes (0-59)
//   [2] = hours (0-23, bits 6-7 are AM/PM)
//   [3] = day of month (1-31)
//   [4] = timer1 counter (increments at 256Hz)
//   [5] = alarm seconds
//   [6] = alarm minutes
//   [7] = alarm hours
//   [8-9] = reserved
//   [10] = alarm control (bit1 = alarm enable)
//   [11] = clock control (bit7 = clock halt)

use serde::{Deserialize, Serialize};

/// CPU frequency (5.12 MHz)
pub const CPU_FREQ: u32 = 5_120_000;
/// Timer0 frequency (2 Hz)
pub const TIMER0_FREQ: u32 = 2;
/// Timer1 frequency (256 Hz)
pub const TIMER1_FREQ: u32 = 256;
/// CPU cycles per Timer0 period
pub const CYCLES_TIMER0: u32 = CPU_FREQ / TIMER0_FREQ;
/// CPU cycles per Timer1 period
pub const CYCLES_TIMER1: u32 = CPU_FREQ / TIMER1_FREQ;
/// Speed-up factor for Timer1
pub const SPEED_UP_FACTOR: u32 = 20;
/// CPU cycles per Timer1 period with speed-up
pub const CYCLES_TIMER1_SPEED_UP: u32 = CYCLES_TIMER1 / SPEED_UP_FACTOR;
/// CPU cycles per millisecond
pub const CYCLES_PER_MS: u32 = CPU_FREQ / 1000;
/// RTC clock data size
pub const CLOCK_DATA_SIZE: usize = 80;

/// Which timer(s) fired during a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimerIrq {
    /// Timer0 (2 Hz) fired.
    pub timer0: bool,
    /// Timer1 (256 Hz) fired.
    pub timer1: bool,
}

impl TimerIrq {
    /// True if any timer fired.
    pub fn any(self) -> bool {
        self.timer0 || self.timer1
    }
}

/// Timer system state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timer {
    /// RTC clock data (80 bytes)
    pub clock_data: Vec<u8>,
    /// Clock flags
    pub clock_flags: u8,
    /// Timer0 cycle counter
    pub timer0_cycles: u32,
    /// Timer1 cycle counter
    pub timer1_cycles: u32,
    /// Timer0 toggle (for 0.5s alternating)
    pub timer0_toggle: bool,
    /// Whether IRQ should be generated
    pub should_irq: bool,
    /// Value written to IO register 0x3D when Timer0 fires
    /// (0 normally, 0x20 when a countdown alarm triggers)
    pub io_3d: u8,
    /// Accumulated CPU cycles for timing
    pub cycles: u32,
    /// Whether speed-up mode is active
    speed_up: bool,
}

impl Timer {
    /// Create a new timer system
    pub fn new() -> Self {
        Self {
            clock_data: vec![0; CLOCK_DATA_SIZE],
            clock_flags: 0,
            timer0_cycles: CYCLES_TIMER0,
            timer1_cycles: CYCLES_TIMER1,
            timer0_toggle: false,
            should_irq: false,
            io_3d: 0,
            cycles: 0,
            speed_up: false,
        }
    }

    /// Reset timer state
    pub fn reset(&mut self) {
        self.clock_data.fill(0);
        self.clock_flags = 0;
        self.timer0_cycles = CYCLES_TIMER0;
        self.timer1_cycles = CYCLES_TIMER1;
        self.timer0_toggle = false;
        self.should_irq = false;
        self.io_3d = 0;
        self.cycles = 0;
    }

    /// Set speed-up mode
    pub fn set_speed_up(&mut self, speed_up: bool) {
        self.speed_up = speed_up;
    }

    /// Advance timers by the given number of CPU cycles
    /// Returns which timer(s) fired and should generate an IRQ
    pub fn tick(&mut self, cpu_cycles: u64) -> TimerIrq {
        self.cycles += cpu_cycles as u32;
        let mut fired = TimerIrq::default();

        // Check Timer0 (0.5s period)
        if self.cycles >= self.timer0_cycles {
            self.timer0_cycles = self.timer0_cycles.wrapping_add(CYCLES_TIMER0);
            self.timer0_toggle = !self.timer0_toggle;

            if !self.timer0_toggle {
                self.adjust_time();
            }

            if !self.is_countdown() || self.timer0_toggle {
                // Normal: clear clock flags
                self.clock_flags = 0;
                self.io_3d = 0;
            } else {
                // Countdown alarm triggered
                self.clock_flags &= 0xFD;
                self.io_3d = 0x20;
            }

            fired.timer0 = true;
        }

        // Check Timer1 (1/256s period)
        let timer1_period = if self.speed_up {
            CYCLES_TIMER1_SPEED_UP
        } else {
            CYCLES_TIMER1
        };
        if self.cycles >= self.timer1_cycles {
            self.timer1_cycles = self.timer1_cycles.wrapping_add(timer1_period);
            self.clock_data[4] = self.clock_data[4].wrapping_add(1);
            fired.timer1 = true;
        }

        self.should_irq = fired.any();
        fired
    }

    /// Adjust RTC time (called every 0.5s)
    fn adjust_time(&mut self) {
        // Increment seconds
        self.clock_data[0] += 1;
        if self.clock_data[0] >= 60 {
            self.clock_data[0] = 0;
            // Increment minutes
            self.clock_data[1] += 1;
            if self.clock_data[1] >= 60 {
                self.clock_data[1] = 0;
                // Increment hours
                self.clock_data[2] =
                    (self.clock_data[2] & 0xC0) | ((self.clock_data[2] & 0x1F) + 1);
                if (self.clock_data[2] & 0x1F) >= 24 {
                    self.clock_data[2] &= 0xC0;
                    // Increment day
                    self.clock_data[3] += 1;
                }
            }
        }
    }

    /// Check if countdown alarm is triggered
    fn is_countdown(&self) -> bool {
        if self.clock_data[10] & 0x02 == 0 || self.clock_flags & 0x02 == 0 {
            return false;
        }

        // Check hour alarm
        if self.clock_data[7] & 0x80 != 0 && (self.clock_data[7] ^ self.clock_data[2]) & 0x1F == 0 {
            return true;
        }
        // Check minute alarm
        if self.clock_data[6] & 0x80 != 0 && (self.clock_data[6] ^ self.clock_data[1]) & 0x3F == 0 {
            return true;
        }
        // Check second alarm
        if self.clock_data[5] & 0x80 != 0 && (self.clock_data[5] ^ self.clock_data[0]) & 0x3F == 0 {
            return true;
        }

        false
    }

    /// Read clock data at index
    pub fn read_clock(&self, index: u8) -> u8 {
        if (index as usize) < CLOCK_DATA_SIZE {
            self.clock_data[index as usize]
        } else {
            0
        }
    }

    /// Write clock data at index
    pub fn write_clock(&mut self, index: u8, value: u8) {
        if (index as usize) >= CLOCK_DATA_SIZE {
            return;
        }

        let idx = index as usize;
        if idx >= 7 {
            if idx == 0x0B {
                // Clock control register
                self.clock_flags |= value & 0x07;
                self.clock_data[0x0B] = value ^ ((self.clock_data[0x0B] ^ value) & 0x7F);
            } else if idx == 0x0A {
                // Alarm control register
                self.clock_flags |= value & 0x07;
                self.clock_data[0x0A] = value;
            } else {
                self.clock_data[idx % CLOCK_DATA_SIZE] = value;
            }
        } else {
            // Time registers - only writable if clock is not halted
            if self.clock_data[0x0B] & 0x80 == 0 {
                self.clock_data[idx] = value;
            }
        }
    }

    /// Get the current clock flags for register 0x3D
    pub fn clock_flags_reg(&self) -> u8 {
        self.clock_flags
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
