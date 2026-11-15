//! Bitmap renderer for the original SAN.ENG, SAN.LIN, and SAN.KOR assets.
//! SAN.KOR is the 400-glyph 10:4:4 component format described in HANDRV.C.

use crate::{
    graphics,
    hangul::{INITIAL, VOWEL},
};
use std::sync::LazyLock;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const COLS: usize = 80;
pub const ROWS: usize = 30;
const ENG: &[u8] = include_bytes!("../font-eng/SAN.ENG");
const LIN: &[u8] = include_bytes!("../binary/SAN.LIN");
const TITLE: &[u8] = include_bytes!("../binary/SAN.TIL");
const KOR_RAW: &[u8] = include_bytes!("../font-kor/SAN.KOR");
static KOR: LazyLock<[u8; 12800]> = LazyLock::new(|| {
    // SAN.KOR is the 360-glyph DKB2 variant. SetDKB2Han in HANDRV.C
    // expands it to 400 components before Draw10_4_4 composes syllables.
    let mut font = [0u8; 12800];
    font[..KOR_RAW.len()].copy_from_slice(KOR_RAW);
    font.copy_within(160 * 32..360 * 32, 200 * 32);
    font.copy_within(140 * 32..160 * 32, 160 * 32);
    font.copy_within(140 * 32..160 * 32, 180 * 32);
    font.copy_within(120 * 32..140 * 32, 140 * 32);
    font
});

const CHO_GROUP: [usize; 22] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 3, 3, 3, 1, 2, 4, 4, 4, 2, 1, 3, 0,
];
const MID_GROUP: [usize; 20] = [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1];
const TAIL_GROUP: [usize; 22] = [
    0, 0, 2, 0, 2, 1, 2, 1, 2, 3, 0, 2, 1, 3, 3, 1, 2, 1, 2, 3, 1, 1,
];

pub const BG: u32 = 0x10121a;
pub const FG: u32 = 0xd8d8c8;
pub const HI: u32 = 0xf4db83;
pub const DIM: u32 = 0x8797a8;

pub struct Canvas {
    pub pixels: Vec<u32>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            pixels: vec![BG; WIDTH * HEIGHT],
        }
    }
    pub fn clear(&mut self) {
        self.pixels.fill(BG);
    }

    pub fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for yy in y.min(HEIGHT)..y.saturating_add(h).min(HEIGHT) {
            let start = yy * WIDTH + x.min(WIDTH);
            let end = yy * WIDTH + x.saturating_add(w).min(WIDTH);
            if start <= end {
                self.pixels[start..end].fill(color);
            }
        }
    }

    pub fn cell_bg(&mut self, col: usize, row: usize, width: usize, color: u32) {
        self.rect(col * 8, row * 16, width * 8, 16, color);
    }

    pub fn glyph(&mut self, col: usize, row: usize, ch: char, color: u32) -> usize {
        if col >= COLS || row >= ROWS {
            return char_width(ch);
        }
        let mut rows = [0u16; 16];
        let width = if ch.is_ascii() {
            let code = ch as usize;
            for y in 0..16 {
                rows[y] = ((ENG[code * 16 + y] as u16) << 8) & 0xff00;
            }
            8
        } else if ('가'..='힣').contains(&ch) {
            let s = ch as usize - 0xac00;
            let first = s / 588 + 1;
            let mid = (s % 588) / 28 + 1;
            let tail = s % 28;
            let has_tail = tail != 0;
            let cho_index = (CHO_GROUP[mid] + if has_tail { 5 } else { 0 }) * 20 + first;
            let mid_index = 200 + (MID_GROUP[first] + if has_tail { 2 } else { 0 }) * 22 + mid;
            for (y, row_bits) in rows.iter_mut().enumerate() {
                let a = u16::from_be_bytes([
                    KOR[cho_index * 32 + y * 2],
                    KOR[cho_index * 32 + y * 2 + 1],
                ]);
                let b = u16::from_be_bytes([
                    KOR[mid_index * 32 + y * 2],
                    KOR[mid_index * 32 + y * 2 + 1],
                ]);
                *row_bits = a | b;
                if has_tail && y >= 8 {
                    let t = 288 + TAIL_GROUP[mid] * 28 + tail;
                    *row_bits |= u16::from_be_bytes([KOR[t * 32 + y * 2], KOR[t * 32 + y * 2 + 1]]);
                }
            }
            16
        } else if let Some(index) = INITIAL
            .iter()
            .position(|&c| c == ch)
            .map(|i| i + 1)
            .or_else(|| VOWEL.iter().position(|&c| c == ch).map(|i| 201 + i))
        {
            for (y, row_bits) in rows.iter_mut().enumerate() {
                *row_bits =
                    u16::from_be_bytes([KOR[index * 32 + y * 2], KOR[index * 32 + y * 2 + 1]]);
            }
            16
        } else if let Some(index) = graphics::glyph_index(ch) {
            for (y, row_bits) in rows.iter_mut().enumerate() {
                *row_bits = graphics::glyph_row(index, y);
            }
            16
        } else {
            // SAN's English font contains the DOS box characters at 128..147.
            let code = match ch {
                '─' | '━' | '―' => Some(137),
                '│' | '┃' => Some(131),
                '┌' | '┏' => Some(128),
                '┐' | '┓' => Some(130),
                '└' | '┗' => Some(134),
                '┘' | '┛' => Some(136),
                _ => None,
            };
            if let Some(code) = code {
                let offset = (code - 128) * 16;
                if offset + 16 <= LIN.len() {
                    for y in 0..16 {
                        rows[y] = (LIN[offset + y] as u16) << 8;
                    }
                }
                8
            } else {
                for y in 0..16 {
                    rows[y] = (ENG[b'?' as usize * 16 + y] as u16) << 8;
                }
                if char_width(ch) == 2 {
                    16
                } else {
                    8
                }
            }
        };
        for (yy, bits) in rows.iter().enumerate() {
            let y = row * 16 + yy;
            for xx in 0..width {
                if bits & (0x8000 >> xx) != 0 {
                    let x = col * 8 + xx;
                    if x < WIDTH {
                        self.pixels[y * WIDTH + x] = color;
                    }
                }
            }
        }
        width / 8
    }

    pub fn text(&mut self, mut col: usize, row: usize, text: &str, color: u32) {
        for ch in text.chars() {
            if col >= COLS {
                break;
            }
            col += self.glyph(col, row, ch, color);
        }
    }

    pub fn text_inverse(&mut self, col: usize, row: usize, width: usize, text: &str) {
        self.cell_bg(col, row, width, FG);
        self.text(col, row, text, BG);
    }

    pub fn box_outline(&mut self, x: usize, y: usize, w: usize, h: usize) {
        let px = x * 8;
        let py = y * 16;
        let pw = w * 8;
        let ph = h * 16;
        self.rect(px, py, pw, ph, BG);
        self.rect(px, py, pw, 1, FG);
        self.rect(px, py + ph - 1, pw, 1, FG);
        self.rect(px, py, 1, ph, FG);
        self.rect(px + pw - 1, py, 1, ph, FG);
        self.rect(px + 4, py + ph, pw, 2, 0x05060a);
        self.rect(px + pw, py + 5, 2, ph - 3, 0x05060a);
    }

    pub fn title_bitmap(&mut self, col: usize, row: usize) {
        // The original SaveScreen format is 35 bytes per scanline, 8 rows of
        // 16 scanlines. TITLE.C restores it at columns 22..56, rows 8..15.
        for y in 0..128 {
            for byte_x in 0..35 {
                let bits = TITLE[y * 35 + byte_x];
                for bit in 0..8 {
                    let x = col * 8 + byte_x * 8 + bit;
                    let py = row * 16 + y;
                    if x < WIDTH && py < HEIGHT {
                        self.pixels[py * WIDTH + x] =
                            if bits & (0x80 >> bit) != 0 { FG } else { BG };
                    }
                }
            }
        }
    }

    pub fn separator(&mut self, row: usize, code: u8) {
        let offset = (code as usize - 128) * 16;
        for y in 0..16 {
            let bits = LIN[offset + y];
            for col in 0..COLS {
                for bit in 0..8 {
                    self.pixels[(row * 16 + y) * WIDTH + col * 8 + bit] =
                        if bits & (0x80 >> bit) != 0 { BG } else { FG };
                }
            }
        }
    }
}

pub fn char_width(ch: char) -> usize {
    if ch.is_ascii() {
        1
    } else {
        2
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fonts_have_expected_original_sizes() {
        assert_eq!(ENG.len(), 2048);
        assert_eq!(LIN.len(), 320);
        assert_eq!(KOR_RAW.len(), 11520);
        assert_eq!(TITLE.len(), 4480);
        assert_eq!(KOR.len(), 12800);
        for c in ['가', '한', '힣'] {
            let mut canvas = Canvas::new();
            canvas.glyph(0, 0, c, FG);
            assert!(canvas.pixels.contains(&FG));
        }
    }
}
