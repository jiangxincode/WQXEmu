// Device skin and virtual keypad support for the standalone frontend.

use anyhow::{Context, Result};
use image::imageops::FilterType;
use wqxemu_core::{key_id_for, key_ids, KeyDef, MachineModel, LCD_HEIGHT, LCD_WIDTH};

const SOURCE_WIDTH: usize = 1086;
const SOURCE_HEIGHT: usize = 1448;
const HEIGHT_PER_SCALE: usize = 150;

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl Rect {
    fn centered(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x: x.saturating_sub(width / 2),
            y: y.saturating_sub(height / 2),
            width,
            height,
        }
    }

    fn contains(self, x: usize, y: usize) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Copy)]
struct SkinSpec {
    screen: Rect,
    lcd_background: u32,
    lcd_foreground: u32,
}

/// An input action exposed by a clickable part of the device skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinInput {
    Key(u8),
    Reset,
}

/// A model-specific device image with LCD and clickable key geometry.
pub struct DeviceSkin {
    model: MachineModel,
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    spec: SkinSpec,
}

impl DeviceSkin {
    /// Decode the embedded model image and prepare a desktop-sized skin.
    pub fn load(model: MachineModel, display_scale: u32) -> Result<Self> {
        let bytes: &[u8] = match model {
            MachineModel::Nc1020 => include_bytes!("../../../res/NC1020.png"),
            MachineModel::Nc2000 => include_bytes!("../../../res/NC2000.png"),
            MachineModel::Nc3000 => include_bytes!("../../../res/NC3000.png"),
            MachineModel::Pc1000 => include_bytes!("../../../res/PC1000.png"),
            MachineModel::Cc800 => include_bytes!("../../../res/CC800.png"),
        };
        let image = image::load_from_memory(bytes)
            .with_context(|| format!("failed to decode the {} device skin", model.name()))?
            .to_rgba8();
        anyhow::ensure!(
            image.width() as usize == SOURCE_WIDTH && image.height() as usize == SOURCE_HEIGHT,
            "unexpected {} skin size: {}x{}",
            model.name(),
            image.width(),
            image.height()
        );

        anyhow::ensure!(
            display_scale > 0,
            "device skin scale must be greater than zero"
        );
        let display_scale = display_scale as usize;
        let height = display_scale
            .checked_mul(HEIGHT_PER_SCALE)
            .context("device skin height overflow")?;
        let width = SOURCE_WIDTH
            .checked_mul(height)
            .context("device skin width overflow")?
            / SOURCE_HEIGHT;
        let image =
            image::imageops::resize(&image, width as u32, height as u32, FilterType::Lanczos3);
        let mut pixels = Vec::with_capacity(width * height);
        for pixel in image.pixels() {
            let [r, g, b, a] = pixel.0;
            // The source PNG is transparent outside the device outline.
            // Composite it over the normal light window background.
            let alpha = a as u32;
            let blend = |channel: u8| (channel as u32 * alpha + 0xF0 * (255 - alpha) + 127) / 255;
            pixels.push((blend(r) << 16) | (blend(g) << 8) | blend(b));
        }

        Ok(Self {
            model,
            width,
            height,
            pixels,
            spec: spec_for(model),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Draw the skin, the live LCD framebuffer, and pressed-key feedback.
    pub fn render(&self, lcd: &[u32], layout: &[KeyDef], pressed: &[bool; 64]) -> Vec<u32> {
        let mut output = self.pixels.clone();
        self.render_lcd(&mut output, lcd);

        for def in layout {
            let key_id = key_id_for(self.model, def.row, def.col);
            if pressed[key_id as usize] {
                if let Some(region) = key_region(self.model, def) {
                    self.darken(&mut output, region);
                }
            }
        }
        if self.model == MachineModel::Nc1020 {
            if pressed[key_ids::F10 as usize] {
                self.darken(&mut output, Rect::centered(105, 950, 82, 52));
            }
            if pressed[key_ids::F11 as usize] {
                self.darken(&mut output, Rect::centered(203, 950, 82, 52));
            }
        }
        output
    }

    /// Translate a window-buffer coordinate into a device input action.
    pub fn hit_test(&self, x: usize, y: usize, layout: &[KeyDef]) -> Option<SkinInput> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let source_x = x * SOURCE_WIDTH / self.width;
        let source_y = y * SOURCE_HEIGHT / self.height;

        if self.model == MachineModel::Nc1020 {
            if Rect::centered(295, 970, 48, 48).contains(source_x, source_y) {
                return Some(SkinInput::Reset);
            }
            if Rect::centered(105, 950, 82, 52).contains(source_x, source_y) {
                return Some(SkinInput::Key(key_ids::F10));
            }
            if Rect::centered(203, 950, 82, 52).contains(source_x, source_y) {
                return Some(SkinInput::Key(key_ids::F11));
            }
        }

        let hit = layout.iter().find(|def| {
            key_region(self.model, def).is_some_and(|region| region.contains(source_x, source_y))
        });
        if let Some(def) = hit {
            return Some(SkinInput::Key(key_id_for(self.model, def.row, def.col)));
        }

        // NC3000 exposes the same matrix key as both ON/OFF and NETWORK.
        if self.model == MachineModel::Nc3000
            && Rect::centered(132, 964, 62, 58).contains(source_x, source_y)
        {
            return layout
                .iter()
                .find(|def| def.row == 0 && def.col == 0)
                .map(|def| SkinInput::Key(key_id_for(self.model, def.row, def.col)));
        }
        None
    }

    fn render_lcd(&self, output: &mut [u32], lcd: &[u32]) {
        debug_assert_eq!(lcd.len(), LCD_WIDTH * LCD_HEIGHT);
        let screen = self.spec.screen;
        let x0 = self.scale_x(screen.x);
        let y0 = self.scale_y(screen.y);
        let x1 = self.scale_x(screen.x + screen.width);
        let y1 = self.scale_y(screen.y + screen.height);

        for y in y0..y1 {
            let source_y = (y - y0) * LCD_HEIGHT / (y1 - y0);
            for x in x0..x1 {
                let source_x = (x - x0) * LCD_WIDTH / (x1 - x0);
                let lcd_pixel = lcd[source_y * LCD_WIDTH + source_x] & 0x00FF_FFFF;
                let brightness = lcd_pixel & 0xFF;
                let darkness = 0xFF - brightness;
                let blend = |shift: u32| {
                    let background = (self.spec.lcd_background >> shift) & 0xFFu32;
                    let foreground = (self.spec.lcd_foreground >> shift) & 0xFFu32;
                    (background * brightness + foreground * darkness + 0x7F) / 0xFF
                };
                output[y * self.width + x] = blend(16) << 16 | blend(8) << 8 | blend(0);
            }
        }
    }

    fn darken(&self, output: &mut [u32], region: Rect) {
        let x0 = self.scale_x(region.x);
        let y0 = self.scale_y(region.y);
        let x1 = self.scale_x(region.x + region.width).min(self.width);
        let y1 = self.scale_y(region.y + region.height).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let pixel = output[y * self.width + x];
                let r = ((pixel >> 16) & 0xFF) * 3 / 4;
                let g = ((pixel >> 8) & 0xFF) * 3 / 4;
                let b = (pixel & 0xFF) * 3 / 4;
                output[y * self.width + x] = (r << 16) | (g << 8) | b;
            }
        }
    }

    fn scale_x(&self, x: usize) -> usize {
        x * self.width / SOURCE_WIDTH
    }

    fn scale_y(&self, y: usize) -> usize {
        y * self.height / SOURCE_HEIGHT
    }
}

fn spec_for(model: MachineModel) -> SkinSpec {
    match model {
        MachineModel::Pc1000 => SkinSpec {
            screen: Rect {
                x: 128,
                y: 105,
                width: 648,
                height: 317,
            },
            lcd_background: 0x0099_AD9C,
            lcd_foreground: 0x0015_2118,
        },
        MachineModel::Cc800 => SkinSpec {
            screen: Rect {
                x: 105,
                y: 96,
                width: 575,
                height: 299,
            },
            lcd_background: 0x0071_9A61,
            lcd_foreground: 0x0008_241A,
        },
        MachineModel::Nc1020 => SkinSpec {
            screen: Rect {
                x: 95,
                y: 113,
                width: 695,
                height: 340,
            },
            lcd_background: 0x0090_A872,
            lcd_foreground: 0x0010_220B,
        },
        MachineModel::Nc2000 => SkinSpec {
            screen: Rect {
                x: 107,
                y: 90,
                width: 861,
                height: 376,
            },
            lcd_background: 0x00A7_BA76,
            lcd_foreground: 0x0009_1604,
        },
        MachineModel::Nc3000 => SkinSpec {
            screen: Rect {
                x: 136,
                y: 122,
                width: 708,
                height: 348,
            },
            lcd_background: 0x008D_AE78,
            lcd_foreground: 0x0013_2715,
        },
    }
}

fn key_region(model: MachineModel, def: &KeyDef) -> Option<Rect> {
    if def.drow >= 2 {
        return keyboard_region(model, def.drow, def.dcol);
    }
    match model {
        MachineModel::Pc1000 => pc1000_special_region(def),
        MachineModel::Cc800 => cc800_special_region(def),
        MachineModel::Nc1020 => nc1020_special_region(def),
        MachineModel::Nc2000 => nc2000_special_region(def),
        MachineModel::Nc3000 => nc3000_special_region(def),
    }
}

fn keyboard_region(model: MachineModel, row: u8, col: u8) -> Option<Rect> {
    if !(2..=5).contains(&row) || col > 9 {
        return None;
    }
    let (columns, rows): (&[usize; 10], &[usize; 4]) = match model {
        MachineModel::Pc1000 => (
            &[108, 205, 302, 399, 496, 593, 690, 787, 883, 980],
            &[1032, 1115, 1197, 1280],
        ),
        MachineModel::Cc800 => (
            &[116, 216, 315, 413, 510, 608, 705, 801, 894, 980],
            &[999, 1082, 1166, 1252],
        ),
        MachineModel::Nc1020 => (
            &[105, 202, 300, 397, 494, 591, 688, 786, 883, 980],
            &[1033, 1116, 1199, 1290],
        ),
        MachineModel::Nc2000 => (
            &[108, 207, 304, 401, 499, 596, 693, 789, 886, 981],
            &[1029, 1112, 1195, 1277],
        ),
        MachineModel::Nc3000 => (
            &[121, 219, 316, 413, 510, 607, 704, 800, 895, 980],
            &[1040, 1121, 1204, 1286],
        ),
    };
    Some(Rect::centered(
        columns[col as usize],
        rows[(row - 2) as usize],
        82,
        61,
    ))
}

fn pc1000_special_region(def: &KeyDef) -> Option<Rect> {
    match (def.drow, def.dcol) {
        (0, col @ 0..=5) => Some(Rect::centered(402 + col as usize * 96, 838, 82, 52)),
        (0, 6) => Some(Rect::centered(575, 933, 82, 50)),
        (1, col @ 2..=5) => Some(Rect::centered(675 + (col as usize - 2) * 97, 933, 82, 50)),
        (1, 8) => Some(Rect::centered(430, 739, 70, 58)),
        _ => None,
    }
}

fn cc800_special_region(def: &KeyDef) -> Option<Rect> {
    match (def.drow, def.dcol) {
        (0, col @ 0..=5) => Some(Rect::centered(202 + col as usize * 136, 809, 104, 58)),
        (0, 6) => Some(Rect::centered(312, 934, 64, 64)),
        (1, col @ 2..=5) => Some(Rect::centered(440 + (col as usize - 2) * 103, 934, 62, 62)),
        (1, 8) => Some(Rect::centered(885, 902, 68, 68)),
        _ => None,
    }
}

fn nc1020_special_region(def: &KeyDef) -> Option<Rect> {
    match (def.drow, def.dcol) {
        (0, col @ 0..=6) => Some(Rect::centered(392 + col as usize * 98, 953, 82, 52)),
        (1, col @ 2..=5) => Some(Rect::centered(
            973,
            [135, 216, 300, 383][col as usize - 2],
            68,
            42,
        )),
        (1, 8) => Some(Rect::centered(966, 560, 80, 80)),
        _ => None,
    }
}

fn nc2000_special_region(def: &KeyDef) -> Option<Rect> {
    match (def.drow, def.dcol) {
        (0, col @ 0..=6) => Some(Rect::centered(404 + col as usize * 96, 954, 78, 50)),
        (1, col @ 2..=5) => Some(Rect::centered(570 + (col as usize - 2) * 97, 591, 70, 54)),
        (1, 8) => Some(Rect::centered(179, 591, 72, 60)),
        _ => None,
    }
}

fn nc3000_special_region(def: &KeyDef) -> Option<Rect> {
    match (def.drow, def.dcol) {
        (0, col @ 0..=2) => Some(Rect::centered(518 + col as usize * 111, 897, 80, 54)),
        (0, col @ 3..=5) => Some(Rect::centered(518 + (col as usize - 3) * 111, 968, 80, 54)),
        (1, col @ 2..=5) => Some(Rect::centered(518 + (col as usize - 2) * 84, 828, 58, 58)),
        (1, 8) => Some(Rect::centered(140, 828, 68, 68)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wqxemu_core::layout_for;

    #[test]
    fn all_skins_load_at_desktop_size() {
        for model in [
            MachineModel::Nc1020,
            MachineModel::Nc2000,
            MachineModel::Nc3000,
            MachineModel::Pc1000,
            MachineModel::Cc800,
        ] {
            let skin = DeviceSkin::load(model, 4).unwrap();
            assert_eq!(skin.width(), 450);
            assert_eq!(skin.height(), 600);
        }
    }

    #[test]
    fn every_visible_layout_key_can_be_clicked() {
        for model in [
            MachineModel::Nc1020,
            MachineModel::Nc2000,
            MachineModel::Nc3000,
            MachineModel::Pc1000,
            MachineModel::Cc800,
        ] {
            let skin = DeviceSkin::load(model, 4).unwrap();
            let layout = layout_for(model);
            for def in layout {
                let region = key_region(model, def).unwrap_or_else(|| {
                    panic!(
                        "{} has no image region for display key ({}, {})",
                        model.name(),
                        def.drow,
                        def.dcol
                    )
                });
                let x = (region.x + region.width / 2) * skin.width() / SOURCE_WIDTH;
                let y = (region.y + region.height / 2) * skin.height() / SOURCE_HEIGHT;
                assert_eq!(
                    skin.hit_test(x, y, layout),
                    Some(SkinInput::Key(key_id_for(model, def.row, def.col))),
                    "{} key {}",
                    model.name(),
                    def.label
                );
            }
        }
    }

    #[test]
    fn lcd_replaces_the_static_screen_image() {
        let skin = DeviceSkin::load(MachineModel::Pc1000, 4).unwrap();
        let lcd = vec![0xFFFF_FFFF; LCD_WIDTH * LCD_HEIGHT];
        let output = skin.render(&lcd, layout_for(MachineModel::Pc1000), &[false; 64]);
        let screen_x = skin.scale_x(skin.spec.screen.x + skin.spec.screen.width / 2);
        let screen_y = skin.scale_y(skin.spec.screen.y + skin.spec.screen.height / 2);
        assert_eq!(
            output[screen_y * skin.width() + screen_x],
            skin.spec.lcd_background
        );
    }

    #[test]
    fn lcd_blends_persistence_brightness_into_the_skin() {
        let skin = DeviceSkin::load(MachineModel::Nc3000, 4).unwrap();
        let lcd = vec![0xFF80_8080; LCD_WIDTH * LCD_HEIGHT];
        let output = skin.render(&lcd, layout_for(MachineModel::Nc3000), &[false; 64]);
        let screen_x = skin.scale_x(skin.spec.screen.x + skin.spec.screen.width / 2);
        let screen_y = skin.scale_y(skin.spec.screen.y + skin.spec.screen.height / 2);
        let pixel = output[screen_y * skin.width() + screen_x];

        assert_ne!(pixel, skin.spec.lcd_background);
        assert_ne!(pixel, skin.spec.lcd_foreground);
    }

    #[test]
    fn nc1020_auxiliary_buttons_have_actions() {
        let skin = DeviceSkin::load(MachineModel::Nc1020, 4).unwrap();
        let layout = layout_for(MachineModel::Nc1020);
        let point = |source_x, source_y| {
            (
                source_x * skin.width() / SOURCE_WIDTH,
                source_y * skin.height() / SOURCE_HEIGHT,
            )
        };

        let (x, y) = point(105, 950);
        assert_eq!(
            skin.hit_test(x, y, layout),
            Some(SkinInput::Key(key_ids::F10))
        );
        let (x, y) = point(203, 950);
        assert_eq!(
            skin.hit_test(x, y, layout),
            Some(SkinInput::Key(key_ids::F11))
        );
        let (x, y) = point(295, 970);
        assert_eq!(skin.hit_test(x, y, layout), Some(SkinInput::Reset));
    }
}
