// Audio system for NC1020.
//
// The NC1020 has a simple JG WAV audio system:
//   - Register 0x20: Control (0x80 = stop, 0x40 = reset)
//   - Register 0x22: Data write
//   - Register 0x23: Command (0xC2 = write single, 0xC4 = write buffer,
//                              0x80 = play)
//
// The audio system has a 32-byte waveform buffer.
// When play is triggered, the waveform is played as a simple tone.

use serde::{Deserialize, Serialize};

/// Audio sample rate (standard CD quality)
pub const SAMPLE_RATE: u32 = 44100;
/// Waveform buffer size
pub const WAVE_BUFFER_SIZE: usize = 0x20;

/// Audio system state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Audio {
    /// Waveform buffer (32 bytes)
    pub wave_buffer: [u8; WAVE_BUFFER_SIZE],
    /// Waveform flags
    pub wave_flags: u8,
    /// Current write index in wave buffer
    pub wave_index: u8,
    /// Whether audio is currently playing
    pub playing: bool,
    /// Current playback position (in samples)
    pub playback_pos: u32,
    /// Audio output buffer (samples)
    output_buffer: Vec<i16>,
}

impl Audio {
    /// Create a new audio system
    pub fn new() -> Self {
        Self {
            wave_buffer: [0; WAVE_BUFFER_SIZE],
            wave_flags: 0,
            wave_index: 0,
            playing: false,
            playback_pos: 0,
            output_buffer: Vec::new(),
        }
    }

    /// Reset audio state
    pub fn reset(&mut self) {
        self.wave_buffer.fill(0);
        self.wave_flags = 0;
        self.wave_index = 0;
        self.playing = false;
        self.playback_pos = 0;
        self.output_buffer.clear();
    }

    /// Handle register 0x20 write (control)
    pub fn write_control(&mut self, value: u8) {
        if value == 0x80 || value == 0x40 {
            // Stop or reset
            self.wave_buffer.fill(0);
            self.wave_flags = 1;
            self.wave_index = 0;
            self.playing = false;
        }
    }

    /// Handle register 0x23 write (command)
    pub fn write_command(&mut self, data: u8, command: u8) {
        match command {
            0xC2 if (self.wave_index as usize) < WAVE_BUFFER_SIZE => {
                // Write single byte
                self.wave_buffer[self.wave_index as usize] = data;
            }
            0xC4 if (self.wave_index as usize) < WAVE_BUFFER_SIZE => {
                // Write to buffer and advance
                self.wave_buffer[self.wave_index as usize] = data;
                self.wave_index += 1;
            }
            0x80 => {
                // Play
                self.wave_flags = 0;
                if self.wave_index > 0 {
                    self.playing = true;
                    self.playback_pos = 0;
                    self.generate_tone();
                    self.wave_index = 0;
                }
            }
            _ => {}
        }
    }

    /// Generate audio tone from waveform buffer
    fn generate_tone(&mut self) {
        self.output_buffer.clear();
        if !self.playing || self.wave_index == 0 {
            return;
        }

        // Generate a simple square wave from the waveform data
        // Each byte in the wave buffer represents a half-period
        let samples_per_byte = SAMPLE_RATE / 1000; // Approximate
        for i in 0..self.wave_index as usize {
            let amplitude = if self.wave_buffer[i] & 0x80 != 0 {
                4000i16 // Positive half
            } else {
                -4000i16 // Negative half
            };
            for _ in 0..samples_per_byte {
                self.output_buffer.push(amplitude);
            }
        }
    }

    /// Drain audio samples from the output buffer
    pub fn drain_audio(&mut self, output: &mut Vec<i16>) {
        output.extend_from_slice(&self.output_buffer);
        self.output_buffer.clear();
    }

    /// Generate audio samples for one frame (called at 30fps)
    /// Returns the number of samples generated
    pub fn generate_frame_samples(&mut self) -> usize {
        if !self.playing {
            return 0;
        }

        // Calculate how many samples we need for this frame
        let samples_needed = SAMPLE_RATE / 30; // ~1470 samples per frame at 30fps

        // Generate simple tone based on wave buffer
        let tone_freq = 440; // Default tone frequency (A4)
        let period_samples = SAMPLE_RATE / tone_freq;

        for i in 0..samples_needed {
            let pos = (self.playback_pos + i) % period_samples;
            let sample = if pos < period_samples / 2 {
                3000i16
            } else {
                -3000i16
            };
            self.output_buffer.push(sample);
        }

        self.playback_pos += samples_needed;

        // Auto-stop after a while
        if self.playback_pos > SAMPLE_RATE * 2 {
            self.playing = false;
        }

        samples_needed as usize
    }

    /// Get the current playing state
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Get the wave flags for register 0x20
    pub fn wave_flags_reg(&self) -> u8 {
        self.wave_flags
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
