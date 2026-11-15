//! SAN.GRF atlas and DOS graphic-character code conversion.
//! The byte ranges and fixed-code conversion come from VGA640.ASM and HANCONV.C.

use encoding_rs::EUC_KR;
use std::{collections::HashMap, sync::LazyLock};

const ATLAS: &[u8] = include_bytes!("../binary/SAN.GRF");
const HANCONV: &[u8] = include_bytes!("../src-c/HANCONV.C");
const PRIVATE_BASE: u32 = 0xe000;
const STARTS: [usize; 6] = [125, 287, 475, 637, 825, 1002];
const COUNTS: [usize; 6] = [162, 188, 162, 188, 177, 175];

// LINE.C's eight 11-glyph line styles. The first four use DOS page D4;
// the last four use page DB. Order: corners, tees, cross, horizontals, verticals.
const LINE_CODES: [[u8; 11]; 8] = [
    [
        0xda, 0xc2, 0xbf, 0xc3, 0xc5, 0xb4, 0xc0, 0xc1, 0xd9, 0xc4, 0xb3,
    ],
    [
        0xd5, 0xd1, 0xb8, 0xc6, 0xd8, 0xb5, 0xd4, 0xcf, 0xbe, 0xcd, 0xb3,
    ],
    [
        0xd6, 0xd2, 0xb7, 0xc7, 0xd7, 0xb6, 0xd3, 0xd0, 0xbd, 0xc4, 0xba,
    ],
    [
        0xc9, 0xcb, 0xbb, 0xcc, 0xce, 0xb9, 0xc8, 0xca, 0xbc, 0xcd, 0xba,
    ],
    [
        0xa3, 0xa8, 0xa4, 0xa7, 0xab, 0xa9, 0xa6, 0xaa, 0xa5, 0xa1, 0xa2,
    ],
    [
        0xc8, 0xb8, 0xc2, 0xbc, 0xbb, 0xbe, 0xc6, 0xba, 0xc4, 0xac, 0xa2,
    ],
    [
        0xc7, 0xbd, 0xc1, 0xb7, 0xc0, 0xb9, 0xc5, 0xbf, 0xc3, 0xa1, 0xad,
    ],
    [
        0xae, 0xb3, 0xaf, 0xb2, 0xb6, 0xb4, 0xb1, 0xb5, 0xb0, 0xac, 0xad,
    ],
];
const BOX_UNICODE: [[char; 11]; 4] = [
    ['┌', '┬', '┐', '├', '┼', '┤', '└', '┴', '┘', '─', '│'],
    ['╒', '╤', '╕', '╞', '╪', '╡', '╘', '╧', '╛', '═', '│'],
    ['╓', '╥', '╖', '╟', '╫', '╢', '╙', '╨', '╜', '─', '║'],
    ['╔', '╦', '╗', '╠', '╬', '╣', '╚', '╩', '╝', '═', '║'],
];

struct Maps {
    decoded: Vec<Option<char>>,
    by_char: HashMap<char, usize>,
    fixed: [u8; 256],
}

static MAPS: LazyLock<Maps> = LazyLock::new(|| {
    let marker = b"FixedGrfTable[]=";
    let begin = HANCONV
        .windows(marker.len())
        .position(|w| w == marker)
        .expect("HANCONV.C table")
        + marker.len();
    let mut fixed_vec = Vec::with_capacity(256);
    let mut in_quote = false;
    for &b in &HANCONV[begin..] {
        if b == b';' && !in_quote {
            break;
        }
        if b == b'"' {
            in_quote = !in_quote;
        } else if in_quote {
            fixed_vec.push(b);
        }
    }
    let fixed: [u8; 256] = fixed_vec.try_into().expect("128 original graphic mappings");
    let mut candidates = Vec::with_capacity(ATLAS.len() / 32);
    let mut frequencies = HashMap::<char, usize>::new();
    for index in 0..ATLAS.len() / 32 {
        let pair = index_to_bytes(index).unwrap();
        let pair = fixed_pair_for_raw(pair, &fixed);
        let (text, _, errors) = EUC_KR.decode(&pair);
        let ch = if !errors && text.chars().count() == 1 {
            text.chars().next()
        } else {
            None
        };
        if let Some(ch) = ch {
            *frequencies.entry(ch).or_default() += 1;
        }
        candidates.push(ch);
    }
    let mut decoded = vec![None; candidates.len()];
    let mut by_char = HashMap::new();
    for (index, maybe_ch) in candidates.into_iter().enumerate() {
        if let Some(ch) = maybe_ch {
            if ch.is_ascii() || ('가'..='힣').contains(&ch) || ('ㄱ'..='ㅣ').contains(&ch) {
                continue;
            }
            by_char.entry(ch).or_insert(index);
            if frequencies[&ch] == 1 {
                decoded[index] = Some(ch);
            }
        }
    }
    // The original DOS line table is authoritative for these glyphs, even
    // where a legacy converter offers a different Unicode approximation.
    let mut line_unicode = std::collections::HashSet::new();
    for style in 0..4 {
        for shape in 0..11 {
            if let Some(index) = raw_index(0xd4, LINE_CODES[style][shape]) {
                let ch = BOX_UNICODE[style][shape];
                if line_unicode.insert(ch) {
                    decoded[index] = Some(ch);
                    by_char.insert(ch, index);
                } else if by_char.get(&ch) != Some(&index) {
                    decoded[index] = None;
                }
            }
        }
    }
    for (index, maybe_ch) in decoded.iter_mut().enumerate() {
        if maybe_ch.is_some_and(|ch| by_char.get(&ch) != Some(&index)) {
            *maybe_ch = None;
        }
    }
    Maps {
        decoded,
        by_char,
        fixed,
    }
});

fn raw_index(lead: u8, trail: u8) -> Option<usize> {
    if lead == 0xd4 {
        return (0x81..=0xfd)
            .contains(&trail)
            .then_some((trail - 0x81) as usize);
    }
    if !(0xd9..=0xde).contains(&lead) {
        return None;
    }
    let mut offset = if (0x31..=0x7e).contains(&trail) {
        (trail - 0x31) as usize
    } else if (0x91..=0xfe).contains(&trail) {
        (trail - 0x43) as usize
    } else {
        return None;
    };
    let page = (lead - 0xd9) as usize;
    if page == 0 {
        offset = offset.checked_sub(1)?;
    }
    (offset < COUNTS[page]).then_some(STARTS[page] + offset)
}

fn index_to_bytes(index: usize) -> Option<[u8; 2]> {
    if index < 125 {
        return Some([0xd4, 0x81 + index as u8]);
    }
    for page in 0..6 {
        if (STARTS[page]..STARTS[page] + COUNTS[page]).contains(&index) {
            let offset = index - STARTS[page] + usize::from(page == 0);
            let trail = if offset <= 0x7e - 0x31 {
                offset + 0x31
            } else {
                offset + 0x43
            };
            return Some([0xd9 + page as u8, trail as u8]);
        }
    }
    None
}

fn fixed_pair_for_raw([lead, trail]: [u8; 2], table: &[u8; 256]) -> [u8; 2] {
    if lead == 0xd4 {
        let index = (trail & 0x7f) as usize * 2;
        [table[index], table[index + 1]]
    } else {
        let page = lead - 0xd9;
        if trail > 0xa0 {
            [0xa2 + page * 2, trail]
        } else {
            [
                0xa1 + page * 2,
                if trail > 0x90 {
                    trail - 0x12 + 0x70
                } else {
                    trail + 0x70
                },
            ]
        }
    }
}

pub fn decode_pair(lead: u8, trail: u8) -> Option<char> {
    let index = raw_index(lead, trail)?;
    Some(
        MAPS.decoded[index].unwrap_or_else(|| char::from_u32(PRIVATE_BASE + index as u32).unwrap()),
    )
}

pub fn encode_pair(ch: char) -> Option<[u8; 2]> {
    if let Some(index) = private_index(ch) {
        return index_to_bytes(index);
    }
    MAPS.by_char
        .get(&ch)
        .and_then(|&index| index_to_bytes(index))
}

pub fn fixed_pair(ch: char) -> Option<[u8; 2]> {
    encode_pair(ch).map(|pair| fixed_pair_for_raw(pair, &MAPS.fixed))
}

fn private_index(ch: char) -> Option<usize> {
    let index = (ch as u32).checked_sub(PRIVATE_BASE)? as usize;
    (index < ATLAS.len() / 32).then_some(index)
}

pub fn is_private(ch: char) -> bool {
    private_index(ch).is_some()
}

pub fn glyph_index(ch: char) -> Option<usize> {
    private_index(ch).or_else(|| MAPS.by_char.get(&ch).copied())
}

pub fn glyph_row(index: usize, row: usize) -> u16 {
    let offset = index * 32 + row * 2;
    u16::from_be_bytes([ATLAS[offset], ATLAS[offset + 1]])
}

pub fn page_cell(page: usize, row: usize, col: usize) -> Option<char> {
    if col >= 16 || page > 6 {
        return None;
    }
    if page == 0 {
        if row >= 8 {
            return None;
        }
        return decode_pair(0xd4, 0x80 + (row * 16 + col) as u8);
    }
    if row >= 12 {
        return None;
    }
    let ordinal = row * 16 + col;
    let blanks = [0, 95, 96, 191];
    if blanks.contains(&ordinal) {
        return None;
    }
    let earlier = blanks.iter().filter(|&&n| n < ordinal).count();
    let count = ordinal - earlier;
    let trail = if count + 0x31 < 0x7f {
        count + 0x31
    } else {
        count + 0x43
    };
    decode_pair(0xd8 + page as u8, trail as u8)
}

pub fn line_char(style: usize, shape: usize) -> char {
    let lead = if style < 4 { 0xd4 } else { 0xdb };
    decode_pair(lead, LINE_CODES[style][shape]).unwrap()
}

pub fn line_shape(ch: char) -> Option<(usize, usize)> {
    let pair = encode_pair(ch)?;
    for (style, codes) in LINE_CODES.iter().enumerate() {
        let lead = if style < 4 { 0xd4 } else { 0xdb };
        for (shape, &trail) in codes.iter().enumerate() {
            if pair == [lead, trail] {
                return Some((style, shape));
            }
        }
    }
    None
}

pub fn is_graphic(ch: char) -> bool {
    glyph_index(ch).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_atlas_and_line_codes() {
        assert_eq!(ATLAS.len(), 1177 * 32);
        assert_eq!(raw_index(0xd4, 0xda), Some(0x59));
        assert_eq!(decode_pair(0xd4, 0xda), Some('┌'));
        assert_eq!(encode_pair('┌'), Some([0xd4, 0xda]));
        assert_eq!(line_char(0, 9), '─');
        assert_eq!(line_shape('┼'), Some((0, 4)));
        for index in 0..ATLAS.len() / 32 {
            let pair = index_to_bytes(index).unwrap();
            assert_eq!(raw_index(pair[0], pair[1]), Some(index));
            let ch = decode_pair(pair[0], pair[1]).unwrap();
            assert_eq!(encode_pair(ch), Some(pair), "atlas glyph {index}");
        }
    }
}
