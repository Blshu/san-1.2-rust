//! Two-beolsik Hangul composition. SAN's original toggle is Shift+Space.

pub enum Edit {
    Insert(char),
    Replace(char),
    Split(char, char),
    Delete,
}

#[derive(Clone, Copy)]
enum Pending {
    Consonant { l: usize },
    Vowel { v: usize },
    Syllable { l: usize, v: usize, t: usize },
}

pub struct Composer {
    pending: Option<Pending>,
}

pub const INITIAL: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];
pub const VOWEL: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

fn syllable(l: usize, v: usize, t: usize) -> char {
    char::from_u32((0xac00 + ((l * 21 + v) * 28 + t)) as u32).unwrap()
}

fn consonant(ch: char) -> Option<(usize, Option<usize>, char)> {
    let mapped = if matches!(ch, 'R' | 'E' | 'Q' | 'T' | 'W') {
        ch
    } else {
        ch.to_ascii_lowercase()
    };
    let (l, t) = match mapped {
        'r' => (0, Some(1)),
        'R' => (1, Some(2)),
        's' => (2, Some(4)),
        'e' => (3, Some(7)),
        'E' => (4, None),
        'f' => (5, Some(8)),
        'a' => (6, Some(16)),
        'q' => (7, Some(17)),
        'Q' => (8, None),
        't' => (9, Some(19)),
        'T' => (10, Some(20)),
        'd' => (11, Some(21)),
        'w' => (12, Some(22)),
        'W' => (13, None),
        'c' => (14, Some(23)),
        'z' => (15, Some(24)),
        'x' => (16, Some(25)),
        'v' => (17, Some(26)),
        'g' => (18, Some(27)),
        _ => return None,
    };
    Some((l, t, INITIAL[l]))
}
fn vowel(ch: char) -> Option<usize> {
    let mapped = if matches!(ch, 'O' | 'P') {
        ch
    } else {
        ch.to_ascii_lowercase()
    };
    Some(match mapped {
        'k' => 0,
        'o' => 1,
        'i' => 2,
        'O' => 3,
        'j' => 4,
        'p' => 5,
        'u' => 6,
        'P' => 7,
        'h' => 8,
        'y' => 12,
        'n' => 13,
        'b' => 17,
        'm' => 18,
        'l' => 20,
        _ => return None,
    })
}
fn combine_vowel(a: usize, b: usize) -> Option<usize> {
    match (a, b) {
        (8, 0) => Some(9),
        (8, 1) => Some(10),
        (8, 20) => Some(11),
        (13, 4) => Some(14),
        (13, 5) => Some(15),
        (13, 20) => Some(16),
        (18, 20) => Some(19),
        (9, 20) => Some(10),
        (14, 20) => Some(15),
        _ => None,
    }
}
fn decompose_vowel(v: usize) -> Option<usize> {
    match v {
        9..=11 => Some(8),
        14..=16 => Some(13),
        19 => Some(18),
        _ => None,
    }
}
fn combine_tail(a: usize, b: usize) -> Option<usize> {
    match (a, b) {
        (1, 19) => Some(3),
        (4, 22) => Some(5),
        (4, 27) => Some(6),
        (8, 1) => Some(9),
        (8, 16) => Some(10),
        (8, 17) => Some(11),
        (8, 19) => Some(12),
        (8, 25) => Some(13),
        (8, 26) => Some(14),
        (8, 27) => Some(15),
        (17, 19) => Some(18),
        _ => None,
    }
}
fn split_tail(t: usize) -> (usize, usize) {
    match t {
        3 => (1, 9),
        5 => (4, 12),
        6 => (4, 18),
        9 => (8, 0),
        10 => (8, 6),
        11 => (8, 7),
        12 => (8, 9),
        13 => (8, 16),
        14 => (8, 17),
        15 => (8, 18),
        18 => (17, 9),
        _ => (0, tail_to_initial(t).unwrap_or(11)),
    }
}
fn tail_to_initial(t: usize) -> Option<usize> {
    Some(match t {
        1 => 0,
        2 => 1,
        4 => 2,
        7 => 3,
        8 => 5,
        16 => 6,
        17 => 7,
        19 => 9,
        20 => 10,
        21 => 11,
        22 => 12,
        23 => 14,
        24 => 15,
        25 => 16,
        26 => 17,
        27 => 18,
        _ => return None,
    })
}

impl Composer {
    pub fn new() -> Self {
        Self { pending: None }
    }
    pub fn reset(&mut self) {
        self.pending = None;
    }

    pub fn feed(&mut self, ch: char) -> Option<Edit> {
        let c = consonant(ch);
        let v = vowel(ch);
        if c.is_none() && v.is_none() {
            self.reset();
            return None;
        }
        let old = self.pending.take();
        match (old, c, v) {
            (None, Some((l, _, display)), _) => {
                self.pending = Some(Pending::Consonant { l });
                Some(Edit::Insert(display))
            }
            (None, _, Some(v)) => {
                self.pending = Some(Pending::Vowel { v });
                Some(Edit::Insert(VOWEL[v]))
            }
            (Some(Pending::Consonant { l }), _, Some(v)) => {
                self.pending = Some(Pending::Syllable { l, v, t: 0 });
                Some(Edit::Replace(syllable(l, v, 0)))
            }
            (Some(Pending::Consonant { .. }), Some((l, _, display)), _) => {
                self.pending = Some(Pending::Consonant { l });
                Some(Edit::Insert(display))
            }
            (Some(Pending::Vowel { v: a }), _, Some(b)) => {
                if let Some(v) = combine_vowel(a, b) {
                    self.pending = Some(Pending::Vowel { v });
                    Some(Edit::Replace(VOWEL[v]))
                } else {
                    self.pending = Some(Pending::Vowel { v: b });
                    Some(Edit::Insert(VOWEL[b]))
                }
            }
            (Some(Pending::Vowel { .. }), Some((l, _, display)), _) => {
                self.pending = Some(Pending::Consonant { l });
                Some(Edit::Insert(display))
            }
            (Some(Pending::Syllable { l, v, t: 0 }), _, Some(b)) => {
                if let Some(v2) = combine_vowel(v, b) {
                    self.pending = Some(Pending::Syllable { l, v: v2, t: 0 });
                    Some(Edit::Replace(syllable(l, v2, 0)))
                } else {
                    self.pending = Some(Pending::Vowel { v: b });
                    Some(Edit::Insert(VOWEL[b]))
                }
            }
            (Some(Pending::Syllable { l, v, t }), _, Some(b)) => {
                let (remain, next_l) = split_tail(t);
                self.pending = Some(Pending::Syllable {
                    l: next_l,
                    v: b,
                    t: 0,
                });
                Some(Edit::Split(syllable(l, v, remain), syllable(next_l, b, 0)))
            }
            (Some(Pending::Syllable { l, v, t: 0 }), Some((new_l, new_t, display)), _) => {
                if let Some(t) = new_t {
                    self.pending = Some(Pending::Syllable { l, v, t });
                    Some(Edit::Replace(syllable(l, v, t)))
                } else {
                    self.pending = Some(Pending::Consonant { l: new_l });
                    Some(Edit::Insert(display))
                }
            }
            (Some(Pending::Syllable { l, v, t }), Some((new_l, new_t, display)), _) => {
                if let Some(t2) = new_t.and_then(|t2| combine_tail(t, t2)) {
                    self.pending = Some(Pending::Syllable { l, v, t: t2 });
                    Some(Edit::Replace(syllable(l, v, t2)))
                } else {
                    self.pending = Some(Pending::Consonant { l: new_l });
                    Some(Edit::Insert(display))
                }
            }
            _ => None,
        }
    }

    pub fn backspace(&mut self) -> Option<Edit> {
        let current = self.pending.take()?;
        match current {
            Pending::Consonant { .. } | Pending::Vowel { .. } => Some(Edit::Delete),
            Pending::Syllable { l, v, t } => {
                if t != 0 {
                    let remaining = split_tail(t).0;
                    self.pending = Some(Pending::Syllable { l, v, t: remaining });
                    Some(Edit::Replace(syllable(l, v, remaining)))
                } else if let Some(v2) = decompose_vowel(v) {
                    self.pending = Some(Pending::Syllable { l, v: v2, t: 0 });
                    Some(Edit::Replace(syllable(l, v2, 0)))
                } else {
                    self.pending = Some(Pending::Consonant { l });
                    Some(Edit::Replace(INITIAL[l]))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composes_and_splits_korean() {
        let mut c = Composer::new();
        assert!(matches!(c.feed('g'), Some(Edit::Insert('ㅎ'))));
        assert!(matches!(c.feed('k'), Some(Edit::Replace('하'))));
        assert!(matches!(c.feed('s'), Some(Edit::Replace('한'))));
        assert!(matches!(c.feed('r'), Some(Edit::Insert('ㄱ'))));
        assert!(matches!(c.feed('m'), Some(Edit::Replace('그'))));
        assert!(matches!(c.feed('f'), Some(Edit::Replace('글'))));
        c.reset();
        assert!(matches!(c.feed('r'), Some(Edit::Insert('ㄱ'))));
        assert!(matches!(c.feed('k'), Some(Edit::Replace('가'))));
        assert!(matches!(c.feed('s'), Some(Edit::Replace('간'))));
        assert!(matches!(c.feed('k'), Some(Edit::Split('가', '나'))));
        c.reset();
        assert!(matches!(c.feed('G'), Some(Edit::Insert('ㅎ'))));
        assert!(matches!(c.feed('K'), Some(Edit::Replace('하'))));
    }
}
