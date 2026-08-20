// Virtual keypad rendering for the standalone frontend.
//
// Draws an on-screen Wenquxing keypad below the LCD, using the shared
// keyboard layout tables from `wqxemu_core`. Keys can be clicked with
// the mouse; pressed keys are highlighted. The PC keyboard mapping in
// `main.rs` remains available and updates the same highlight state.

use wqxemu_core::{key_id_for, KeyDef, MachineModel};

/// 8x8 monochrome font (public domain, derived from IBM VGA fonts).
/// Each entry is one glyph: 8 bytes, one byte per row (bit 7 = leftmost).
const FONT8X8: [[u8; 8]; 95] = [
    // 0x20 ' '
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x21 '!'
    [0x18, 0x3C, 0x3C, 0x18, 0x18, 0x00, 0x18, 0x00],
    // 0x22 '"'
    [0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x23 '#'
    [0x36, 0x36, 0x7F, 0x36, 0x7F, 0x36, 0x36, 0x00],
    // 0x24 '$'
    [0x0C, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x0C, 0x00],
    // 0x25 '%'
    [0x00, 0x63, 0x33, 0x18, 0x0C, 0x66, 0x63, 0x00],
    // 0x26 '&'
    [0x1C, 0x36, 0x1C, 0x6E, 0x3B, 0x33, 0x6E, 0x00],
    // 0x27 ' '
    [0x06, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x28 '('
    [0x18, 0x0C, 0x06, 0x06, 0x06, 0x0C, 0x18, 0x00],
    // 0x29 ')'
    [0x06, 0x0C, 0x18, 0x18, 0x18, 0x0C, 0x06, 0x00],
    // 0x2A '*'
    [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
    // 0x2B '+'
    [0x00, 0x0C, 0x0C, 0x3F, 0x0C, 0x0C, 0x00, 0x00],
    // 0x2C ','
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06],
    // 0x2D '-'
    [0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00],
    // 0x2E '.'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00],
    // 0x2F '/'
    [0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01, 0x00],
    // 0x30 '0'
    [0x3E, 0x63, 0x73, 0x7B, 0x6F, 0x67, 0x3E, 0x00],
    // 0x31 '1'
    [0x0C, 0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x3F, 0x00],
    // 0x32 '2'
    [0x1E, 0x33, 0x30, 0x1C, 0x06, 0x33, 0x3F, 0x00],
    // 0x33 '3'
    [0x1E, 0x33, 0x30, 0x1C, 0x30, 0x33, 0x1E, 0x00],
    // 0x34 '4'
    [0x38, 0x3C, 0x36, 0x33, 0x7F, 0x30, 0x78, 0x00],
    // 0x35 '5'
    [0x3F, 0x03, 0x1F, 0x30, 0x30, 0x33, 0x1E, 0x00],
    // 0x36 '6'
    [0x1C, 0x06, 0x03, 0x1F, 0x33, 0x33, 0x1E, 0x00],
    // 0x37 '7'
    [0x3F, 0x33, 0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x00],
    // 0x38 '8'
    [0x1E, 0x33, 0x33, 0x1E, 0x33, 0x33, 0x1E, 0x00],
    // 0x39 '9'
    [0x1E, 0x33, 0x33, 0x3E, 0x30, 0x18, 0x0E, 0x00],
    // 0x3A ':'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x00],
    // 0x3B ';'
    [0x00, 0x0C, 0x0C, 0x00, 0x00, 0x0C, 0x0C, 0x06],
    // 0x3C '<'
    [0x18, 0x0C, 0x06, 0x03, 0x06, 0x0C, 0x18, 0x00],
    // 0x3D '='
    [0x00, 0x00, 0x3F, 0x00, 0x00, 0x3F, 0x00, 0x00],
    // 0x3E '>'
    [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00],
    // 0x3F '?'
    [0x1E, 0x33, 0x30, 0x18, 0x0C, 0x00, 0x0C, 0x00],
    // 0x40 '@'
    [0x3E, 0x63, 0x7B, 0x7B, 0x7B, 0x03, 0x1E, 0x00],
    // 0x41 'A'
    [0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00],
    // 0x42 'B'
    [0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00],
    // 0x43 'C'
    [0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C, 0x00],
    // 0x44 'D'
    [0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00],
    // 0x45 'E'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F, 0x00],
    // 0x46 'F'
    [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00],
    // 0x47 'G'
    [0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C, 0x00],
    // 0x48 'H'
    [0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00],
    // 0x49 'I'
    [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    // 0x4A 'J'
    [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00],
    // 0x4B 'K'
    [0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67, 0x00],
    // 0x4C 'L'
    [0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00],
    // 0x4D 'M'
    [0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63, 0x00],
    // 0x4E 'N'
    [0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00],
    // 0x4F 'O'
    [0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C, 0x00],
    // 0x50 'P'
    [0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00],
    // 0x51 'Q'
    [0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38, 0x00],
    // 0x52 'R'
    [0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00],
    // 0x53 'S'
    [0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E, 0x00],
    // 0x54 'T'
    [0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    // 0x55 'U'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x00],
    // 0x56 'V'
    [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],
    // 0x57 'W'
    [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],
    // 0x58 'X'
    [0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00],
    // 0x59 'Y'
    [0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E, 0x00],
    // 0x5A 'Z'
    [0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00],
    // 0x5B '['
    [0x1E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x1E, 0x00],
    // 0x5C ' '
    [0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00],
    // 0x5D ']'
    [0x1E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1E, 0x00],
    // 0x5E '^'
    [0x08, 0x1C, 0x36, 0x63, 0x00, 0x00, 0x00, 0x00],
    // 0x5F '_'
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
    // 0x60 '`'
    [0x0C, 0x0C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],
    // 0x61 'a'
    [0x00, 0x00, 0x1E, 0x30, 0x3E, 0x33, 0x6E, 0x00],
    // 0x62 'b'
    [0x07, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3B, 0x00],
    // 0x63 'c'
    [0x00, 0x00, 0x1E, 0x33, 0x03, 0x33, 0x1E, 0x00],
    // 0x64 'd'
    [0x38, 0x30, 0x30, 0x3E, 0x33, 0x33, 0x6E, 0x00],
    // 0x65 'e'
    [0x00, 0x00, 0x1E, 0x33, 0x3F, 0x03, 0x1E, 0x00],
    // 0x66 'f'
    [0x1C, 0x36, 0x06, 0x0F, 0x06, 0x06, 0x0F, 0x00],
    // 0x67 'g'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x1F],
    // 0x68 'h'
    [0x07, 0x06, 0x36, 0x6E, 0x66, 0x66, 0x67, 0x00],
    // 0x69 'i'
    [0x0C, 0x00, 0x0E, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    // 0x6A 'j'
    [0x30, 0x00, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E],
    // 0x6B 'k'
    [0x07, 0x06, 0x66, 0x36, 0x1E, 0x36, 0x67, 0x00],
    // 0x6C 'l'
    [0x0E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    // 0x6D 'm'
    [0x00, 0x00, 0x33, 0x7F, 0x7F, 0x6B, 0x63, 0x00],
    // 0x6E 'n'
    [0x00, 0x00, 0x1F, 0x33, 0x33, 0x33, 0x33, 0x00],
    // 0x6F 'o'
    [0x00, 0x00, 0x1E, 0x33, 0x33, 0x33, 0x1E, 0x00],
    // 0x70 'p'
    [0x00, 0x00, 0x3B, 0x66, 0x66, 0x3E, 0x06, 0x0F],
    // 0x71 'q'
    [0x00, 0x00, 0x6E, 0x33, 0x33, 0x3E, 0x30, 0x78],
    // 0x72 'r'
    [0x00, 0x00, 0x3B, 0x6E, 0x66, 0x06, 0x0F, 0x00],
    // 0x73 's'
    [0x00, 0x00, 0x3E, 0x03, 0x1E, 0x30, 0x1F, 0x00],
    // 0x74 't'
    [0x08, 0x0C, 0x3E, 0x0C, 0x0C, 0x2C, 0x18, 0x00],
    // 0x75 'u'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x33, 0x6E, 0x00],
    // 0x76 'v'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],
    // 0x77 'w'
    [0x00, 0x00, 0x63, 0x6B, 0x7F, 0x7F, 0x36, 0x00],
    // 0x78 'x'
    [0x00, 0x00, 0x63, 0x36, 0x1C, 0x36, 0x63, 0x00],
    // 0x79 'y'
    [0x00, 0x00, 0x33, 0x33, 0x33, 0x3E, 0x30, 0x1F],
    // 0x7A 'z'
    [0x00, 0x00, 0x3F, 0x19, 0x0C, 0x26, 0x3F, 0x00],
    // 0x7B '{'
    [0x38, 0x0C, 0x0C, 0x07, 0x0C, 0x0C, 0x38, 0x00],
    // 0x7C '|'
    [0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x18, 0x00],
    // 0x7D '}'
    [0x07, 0x0C, 0x0C, 0x38, 0x0C, 0x0C, 0x07, 0x00],
    // 0x7E '~'
    [0x6E, 0x3B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
];

/// Keypad geometry (independent of the LCD scale so it stays readable).
///
/// The physical Wenquxing keypad is a 6-row x 10-column layout; the
/// matrix coordinates (used for input) differ from the display
/// coordinates, so keys carry both.
pub const KEYPAD_COLS: usize = 10;
pub const KEYPAD_ROWS: usize = 6;
const CELL_W: usize = 60;
const CELL_H: usize = 36;
const TITLE_H: usize = 20;
const PAD: usize = 4;

/// Total keypad panel width (including padding).
pub const KEYPAD_WIDTH: usize = KEYPAD_COLS * CELL_W + PAD * 2;
/// Total keypad panel height.
pub const KEYPAD_HEIGHT: usize = TITLE_H + KEYPAD_ROWS * CELL_H + PAD;

const COLOR_BG: u32 = 0xFF1E_1E24;
const COLOR_PANEL: u32 = 0xFF2A_2A32;
const COLOR_KEY: u32 = 0xFF38_3844;
const COLOR_KEY_ACTIVE: u32 = 0xFF3A_5BC8;
const COLOR_KEY_BORDER: u32 = 0xFF58_5868;
const COLOR_TEXT: u32 = 0xFFEA_EAEA;
const COLOR_HINT: u32 = 0xFF9A_9AAA;

/// Draw a character at (x, y) into the framebuffer.
fn draw_char(buf: &mut [u32], width: usize, x: usize, y: usize, c: u8, color: u32) {
    if !(0x20..=0x7E).contains(&c) {
        return;
    }
    let glyph = &FONT8X8[(c - 0x20) as usize];
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            // font8x8 stores each row with bit 0 as the leftmost pixel.
            if bits & (1 << col) != 0 {
                let px = x + col;
                let py = y + row;
                if px < width && py * width + px < buf.len() {
                    buf[py * width + px] = color;
                }
            }
        }
    }
}

/// Draw text starting at (x, y).
fn draw_text(buf: &mut [u32], width: usize, mut x: usize, y: usize, text: &str, color: u32) {
    for &b in text.as_bytes() {
        draw_char(buf, width, x, y, b, color);
        x += 8;
    }
}

/// Center a text line horizontally in [x0, x1).
fn draw_text_centered(
    buf: &mut [u32],
    width: usize,
    x0: usize,
    x1: usize,
    y: usize,
    text: &str,
    color: u32,
) {
    let text_w = text.len() * 8;
    let x = x0 + x1.saturating_sub(x0).saturating_sub(text_w) / 2;
    draw_text(buf, width, x, y, text, color);
}

/// Fill a rectangle.
fn fill_rect(
    buf: &mut [u32],
    width: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    color: u32,
) {
    for y in y0..y1 {
        if y * width + x0 >= buf.len() {
            break;
        }
        for x in x0..x1 {
            let idx = y * width + x;
            if idx < buf.len() {
                buf[idx] = color;
            }
        }
    }
}

/// Render the keypad panel into the framebuffer below the LCD.
///
/// `panel_x` / `panel_y` is the top-left of the panel; the panel is
/// centered horizontally by the caller.
pub fn render_keypad(
    buf: &mut [u32],
    width: usize,
    panel_x: usize,
    panel_y: usize,
    model: MachineModel,
    layout: &[KeyDef],
    pressed: &[bool; 64],
) {
    // Outer background + grid panel + title.
    fill_rect(
        buf,
        width,
        panel_x,
        panel_y,
        panel_x + KEYPAD_WIDTH,
        panel_y + KEYPAD_HEIGHT,
        COLOR_BG,
    );
    fill_rect(
        buf,
        width,
        panel_x + PAD,
        panel_y + TITLE_H,
        panel_x + KEYPAD_WIDTH - PAD,
        panel_y + KEYPAD_HEIGHT - PAD,
        COLOR_PANEL,
    );
    draw_text_centered(
        buf,
        width,
        panel_x,
        panel_x + KEYPAD_WIDTH,
        panel_y + 6,
        "KEYPAD: CLICK A KEY OR USE THE PC KEYBOARD",
        COLOR_HINT,
    );

    let grid_y = panel_y + TITLE_H;
    for def in layout {
        let x0 = panel_x + PAD + def.dcol as usize * CELL_W;
        let y0 = grid_y + def.drow as usize * CELL_H;
        let x1 = x0 + CELL_W;
        let y1 = y0 + CELL_H;
        let active = pressed[key_id_for(model, def.row, def.col) as usize];
        let bg = if active { COLOR_KEY_ACTIVE } else { COLOR_KEY };
        fill_rect(buf, width, x0, y0, x1, y1, bg);
        // Border.
        fill_rect(buf, width, x0, y0, x1, y0 + 1, COLOR_KEY_BORDER);
        fill_rect(buf, width, x0, y1 - 1, x1, y1, COLOR_KEY_BORDER);
        fill_rect(buf, width, x0, y0, x0 + 1, y1, COLOR_KEY_BORDER);
        fill_rect(buf, width, x1 - 1, y0, x1, y1, COLOR_KEY_BORDER);

        // Label centered in the upper half of the key.
        draw_text_centered(buf, width, x0 + 2, x1 - 2, y0 + 6, def.label, COLOR_TEXT);
        if !def.hint.is_empty() {
            draw_text_centered(buf, width, x0 + 2, x1 - 2, y0 + 20, def.hint, COLOR_HINT);
        }
    }
}

/// Hit test a window coordinate against the keypad. Returns the key ID
/// when the point lands on a key that exists in `layout`.
pub fn hit_test(
    x: usize,
    y: usize,
    panel_x: usize,
    panel_y: usize,
    model: MachineModel,
    layout: &[KeyDef],
) -> Option<u8> {
    if y < panel_y + TITLE_H || x < panel_x + PAD {
        return None;
    }
    let col = (x - panel_x - PAD) / CELL_W;
    let row = (y - panel_y - TITLE_H) / CELL_H;
    if row >= KEYPAD_ROWS || col >= KEYPAD_COLS {
        return None;
    }
    layout
        .iter()
        .find(|d| d.drow as usize == row && d.dcol as usize == col)
        .map(|d| key_id_for(model, d.row, d.col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wqxemu_core::{layout_for, MachineModel};

    #[test]
    fn hit_test_finds_keys() {
        let layout = layout_for(MachineModel::Pc1000);
        // A known key: PC1000 DICT at (1,0) -> key id 8.
        let key = layout.iter().find(|d| d.label == "DICT").unwrap();
        let x = key.dcol as usize * CELL_W + CELL_W / 2;
        let y = TITLE_H + key.drow as usize * CELL_H + CELL_H / 2;
        let expect = key_id_for(MachineModel::Pc1000, key.row, key.col);
        assert_eq!(
            hit_test(x, y, 0, 0, MachineModel::Pc1000, layout),
            Some(expect)
        );
    }

    #[test]
    fn hit_test_misses_empty_cells() {
        let layout = layout_for(MachineModel::Pc1000);
        // PC1000 has no key at display (1,0).
        let x = CELL_W / 2;
        let y = TITLE_H + CELL_H + CELL_H / 2;
        assert_eq!(hit_test(x, y, 0, 0, MachineModel::Pc1000, layout), None);
    }

    #[test]
    fn render_writes_pixels() {
        let layout = layout_for(MachineModel::Nc2000);
        let width = KEYPAD_WIDTH;
        let mut buf = vec![0u32; width * KEYPAD_HEIGHT];
        let pressed = [false; 64];
        render_keypad(
            &mut buf,
            width,
            0,
            0,
            MachineModel::Nc2000,
            layout,
            &pressed,
        );
        // The panel should draw borders/text somewhere.
        let non_bg = buf.iter().filter(|&&p| p != 0).count();
        assert!(non_bg > 100, "keypad should render content, got {}", non_bg);
    }

    #[test]
    fn font_glyphs_are_not_mirrored() {
        // font8x8 stores each row with bit 0 as the leftmost pixel; the
        // glyphs must render with the correct (non-mirrored) shape.

        // 'C' must open to the right: the left side is filled and the
        // right side is open. A mirrored C opens to the left.
        let mut buf = [0u32; 8 * 8];
        draw_char(&mut buf, 8, 0, 0, b'C', 1);
        let row = |r: usize| (0..8).filter(|&c| buf[r * 8 + c] != 0).collect::<Vec<_>>();
        let r1 = row(1); // 0x66: left leg + right leg
        assert!(
            r1.contains(&1) && r1.contains(&2),
            "C left side must be filled"
        );
        assert!(
            r1.contains(&5) && r1.contains(&6),
            "C right side must be filled"
        );
        let r2 = row(2); // 0x03: only the left side remains -> right is open
        assert!(r2.contains(&0) && r2.contains(&1), "C opening");
        assert!(
            !r2.contains(&6) && !r2.contains(&7),
            "C must open to the right"
        );

        // 'A' apex near the center, legs at both sides.
        let mut buf_a = [0u32; 8 * 8];
        draw_char(&mut buf_a, 8, 0, 0, b'A', 1);
        let top = (0..8).filter(|&c| buf_a[c] != 0).collect::<Vec<_>>();
        assert!(top.contains(&2) && top.contains(&3), "A apex");
        let bottom = (0..8).filter(|&c| buf_a[48 + c] != 0).collect::<Vec<_>>();
        assert!(bottom.contains(&0) && bottom.contains(&1), "A left leg");
        assert!(bottom.contains(&4) && bottom.contains(&5), "A right leg");

        // '/' must slant from top-right to bottom-left.
        let mut buf2 = [0u32; 8 * 8];
        draw_char(&mut buf2, 8, 0, 0, b'/', 1);
        let r0 = (0..8).filter(|&c| buf2[c] != 0).collect::<Vec<_>>();
        let r6 = (0..8).filter(|&c| buf2[48 + c] != 0).collect::<Vec<_>>();
        assert!(
            r0.iter().any(|&c| c >= 6),
            "slash top should be on the right: {:?}",
            r0
        );
        assert!(
            r6.iter().any(|&c| c <= 1),
            "slash bottom should be on the left: {:?}",
            r6
        );

        // 'Z' diagonal runs from top-right to bottom-left.
        let mut buf3 = [0u32; 8 * 8];
        draw_char(&mut buf3, 8, 0, 0, b'Z', 1);
        let row1 = (0..8).filter(|&c| buf3[8 + c] != 0).collect::<Vec<_>>();
        let row6 = (0..8).filter(|&c| buf3[40 + c] != 0).collect::<Vec<_>>();
        assert!(
            row1.iter().any(|&c| c >= 6),
            "Z diagonal should start at the top-right: {:?}",
            row1
        );
        assert!(
            row6.iter().any(|&c| c <= 1),
            "Z diagonal should end at the bottom-left: {:?}",
            row6
        );
    }

    #[test]
    fn all_layouts_are_unique_positions() {
        for model in [
            MachineModel::Nc1020,
            MachineModel::Nc2000,
            MachineModel::Nc3000,
            MachineModel::Pc1000,
            MachineModel::Cc800,
        ] {
            let layout = layout_for(model);
            let mut seen = std::collections::HashSet::new();
            for d in layout {
                assert!(
                    seen.insert((d.drow, d.dcol)),
                    "{}: duplicate key at ({},{})",
                    model.name(),
                    d.drow,
                    d.dcol
                );
            }
        }
    }

    #[test]
    fn nc1020_key_id_roundtrip() {
        // NC1020 matrix (0,2) must decode back to key id 0x10 (F1),
        // matching the NC1020 reference encoding (row = key_id % 8).
        assert_eq!(key_id_for(MachineModel::Nc1020, 0, 2), 0x10);
        // The other models use row << 3 | col.
        assert_eq!(key_id_for(MachineModel::Nc2000, 0, 2), 0x02);
        assert_eq!(key_id_for(MachineModel::Pc1000, 3, 0), 0x18);
    }
}
