use crate::font::char_width;
use crate::graphics;
use crate::hangul::{INITIAL, VOWEL};
use encoding_rs::EUC_KR;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Utf8,
    EucKr,
    Johab,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::EucKr => "EUC-KR",
            Self::Johab => "JOHAB",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "utf8" | "utf-8" => Some(Self::Utf8),
            "euc-kr" | "euckr" | "cp949" => Some(Self::EucKr),
            "johab" => Some(Self::Johab),
            _ => None,
        }
    }
}

const VOWEL_CODE: [u8; 21] = [
    3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 26, 27, 28, 29,
];
const TAIL_CODE: [u8; 28] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29,
];

fn decode_johab(bytes: &[u8]) -> (String, usize) {
    let mut result = String::new();
    let mut errors = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            result.push(b as char);
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            result.push('\u{fffd}');
            errors += 1;
            break;
        }
        let n = bytes[i + 1];
        if let Some(ch) = graphics::decode_pair(b, n) {
            result.push(ch);
            i += 2;
            continue;
        }
        let first = ((b.saturating_sub(0x80)) >> 2) as usize;
        let vowel = (((b & 3) as usize) << 3) | ((n >> 5) as usize);
        let tail = n & 31;
        let v_index = VOWEL_CODE.iter().position(|&x| x as usize == vowel);
        let t_index = TAIL_CODE.iter().position(|&x| x == tail);
        if (2..=20).contains(&first) && vowel == 2 && tail == 1 {
            result.push(INITIAL[first - 2]);
            i += 2;
            continue;
        }
        if first == 1 && tail == 1 {
            if let Some(v) = v_index {
                result.push(VOWEL[v]);
                i += 2;
                continue;
            }
        }
        if (2..=20).contains(&first) {
            if let (Some(v), Some(t)) = (v_index, t_index) {
                let code = 0xac00 + ((first - 2) * 21 + v) * 28 + t;
                if let Some(ch) = char::from_u32(code as u32) {
                    result.push(ch);
                    i += 2;
                    continue;
                }
            }
        }
        result.push('\u{fffd}');
        errors += 1;
        i += 2;
    }
    (result, errors)
}

fn encode_johab(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii() {
            out.push(ch as u8);
            continue;
        }
        if let Some(pair) = graphics::encode_pair(ch) {
            out.extend(pair);
            continue;
        }
        if ('가'..='힣').contains(&ch) {
            let s = ch as usize - 0xac00;
            let first = s / 588 + 2;
            let vowel = VOWEL_CODE[(s % 588) / 28] as usize;
            let tail = TAIL_CODE[s % 28];
            out.push(0x80 + (first * 4) as u8 + (vowel >> 3) as u8);
            out.push(((vowel & 7) * 32) as u8 + tail);
        } else if let Some(l) = INITIAL.iter().position(|&c| c == ch) {
            out.push(0x80 + ((l + 2) * 4) as u8);
            out.push(0x41);
        } else if let Some(v) = VOWEL.iter().position(|&c| c == ch) {
            let vowel = VOWEL_CODE[v] as usize;
            out.push(0x80 + 4 + (vowel >> 3) as u8);
            out.push(((vowel & 7) * 32) as u8 + 1);
        } else {
            return Err(format!("JOHAB로 저장할 수 없는 문자: {ch}"));
        }
    }
    Ok(out)
}

fn decode(bytes: &[u8], forced: Option<Encoding>) -> Result<(String, Encoding), String> {
    if let Some(encoding) = forced {
        return match encoding {
            Encoding::Utf8 => String::from_utf8(bytes.to_vec())
                .map(|s| (s, encoding))
                .map_err(|e| e.to_string()),
            Encoding::EucKr => {
                let (s, _, errors) = EUC_KR.decode(bytes);
                if errors {
                    Err("EUC-KR 디코딩 오류".into())
                } else {
                    Ok((s.into_owned(), encoding))
                }
            }
            Encoding::Johab => {
                let (s, errors) = decode_johab(bytes);
                if errors > 0 {
                    Err(format!("JOHAB 디코딩 오류: {errors}개"))
                } else {
                    Ok((s, encoding))
                }
            }
        };
    }
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        return Ok((s, Encoding::Utf8));
    }
    let (johab, johab_errors) = decode_johab(bytes);
    let (euc, _, euc_errors) = EUC_KR.decode(bytes);
    if johab_errors == 0 && johab.chars().any(|c| ('가'..='힣').contains(&c)) {
        return Ok((johab, Encoding::Johab));
    }
    if !euc_errors {
        return Ok((euc.into_owned(), Encoding::EucKr));
    }
    Err("문자 인코딩을 판별할 수 없습니다. --encoding 옵션을 사용하세요.".into())
}

fn encode(text: &str, encoding: Encoding) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Utf8 => Ok(text.as_bytes().to_vec()),
        Encoding::EucKr => {
            let mut out = Vec::with_capacity(text.len());
            for ch in text.chars() {
                if graphics::is_private(ch) {
                    out.extend(graphics::fixed_pair(ch).ok_or("그림 문자 변환 오류")?);
                } else {
                    let character = ch.to_string();
                    let (bytes, _, errors) = EUC_KR.encode(&character);
                    if errors {
                        return Err(format!("EUC-KR로 저장할 수 없는 문자: {ch}"));
                    }
                    out.extend(bytes.as_ref());
                }
            }
            Ok(out)
        }
        Encoding::Johab => encode_johab(text),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone)]
struct Snapshot {
    lines: Vec<Vec<char>>,
    cursor: Pos,
}

pub struct Document {
    pub lines: Vec<Vec<char>>,
    pub cursor: Pos,
    pub top: usize,
    pub left: usize,
    pub path: Option<PathBuf>,
    pub encoding: Encoding,
    pub crlf: bool,
    pub dirty: bool,
    pub insert: bool,
    pub indent: bool,
    pub mark: Option<Pos>,
    pub block: Option<(Pos, Pos)>,
    pub kill_line: Option<Vec<char>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            lines: vec![vec![]],
            cursor: Pos::default(),
            top: 0,
            left: 0,
            path: None,
            encoding: Encoding::Utf8,
            crlf: false,
            dirty: false,
            insert: true,
            indent: true,
            mark: None,
            block: None,
            kill_line: None,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn open(path: &Path, forced: Option<Encoding>) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let (text, encoding) = decode(&bytes, forced)?;
        let crlf = text.contains("\r\n");
        let text = text.replace("\r\n", "\n");
        let mut doc = Self::empty();
        doc.lines = text
            .split('\n')
            .map(|line| line.chars().collect())
            .collect();
        doc.path = Some(path.to_path_buf());
        doc.encoding = encoding;
        doc.crlf = crlf;
        Ok(doc)
    }

    pub fn save(&mut self, path: Option<&Path>) -> Result<(), String> {
        let path = path
            .or(self.path.as_deref())
            .ok_or("파일 이름이 필요합니다.")?
            .to_path_buf();
        let sep = if self.crlf { "\r\n" } else { "\n" };
        let text = self
            .lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(sep);
        let bytes = encode(&text, self.encoding)?;
        if path.exists() {
            let backup = if path
                .extension()
                .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("bak"))
            {
                path.with_file_name(format!(
                    "{}.BAK",
                    path.file_name().unwrap().to_string_lossy()
                ))
            } else {
                path.with_extension("BAK")
            };
            fs::copy(&path, backup).map_err(|e| format!("백업 실패: {e}"))?;
        }
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    pub fn write_recovery(&self, task: usize) -> Result<PathBuf, String> {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "SAN".into());
        let file_name = format!("{name}.san-recovery-{}-{task}.txt", std::process::id());
        let path = self
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .join(file_name);
        let text = self
            .lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(if self.crlf { "\r\n" } else { "\n" });
        fs::write(&path, text.as_bytes()).map_err(|e| e.to_string())?;
        Ok(path)
    }

    fn snapshot(&mut self) {
        self.undo.push(Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        self.mark = None;
        self.block = None;
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = previous.lines;
            self.cursor = previous.cursor;
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = next.lines;
            self.cursor = next.cursor;
            self.dirty = true;
        }
    }

    pub fn column(&self) -> usize {
        self.lines[self.cursor.line][..self.cursor.col]
            .iter()
            .map(|&c| char_width(c))
            .sum()
    }
    pub fn set_visual_col(&mut self, target: usize) {
        let mut width = 0;
        let mut col = 0;
        for &ch in &self.lines[self.cursor.line] {
            if width + char_width(ch) > target {
                break;
            }
            width += char_width(ch);
            col += 1;
        }
        self.cursor.col = col;
    }

    pub fn char_at_visual(&self, line: usize, target: usize) -> Option<char> {
        let mut width = 0;
        for &ch in self.lines.get(line)? {
            if width == target {
                return Some(ch);
            }
            if width > target {
                return None;
            }
            width += char_width(ch);
        }
        None
    }

    /// Replace a two-column cell without shifting the rest of its line.
    pub fn put_visual(&mut self, line: usize, target: usize, graphic: Option<char>) {
        if target > 252 {
            return;
        }
        if self.char_at_visual(line, target) == graphic && graphic.is_some() {
            return;
        }
        self.snapshot();
        while self.lines.len() <= line {
            self.lines.push(vec![]);
        }
        let row = &mut self.lines[line];
        let mut index = 0;
        let mut width = 0;
        while index < row.len() && width + char_width(row[index]) <= target {
            width += char_width(row[index]);
            index += 1;
        }
        if width < target && index < row.len() {
            // The requested position is in the second half of an old glyph.
            row[index] = ' ';
            row.insert(index + 1, ' ');
            index += 1;
            width += 1;
        }
        while width < target {
            row.insert(index, ' ');
            index += 1;
            width += 1;
        }
        let mut removed = 0;
        while removed < 2 && index < row.len() {
            removed += char_width(row.remove(index));
        }
        if let Some(ch) = graphic {
            row.insert(index, ch);
        } else {
            row.insert(index, ' ');
            row.insert(index + 1, ' ');
        }
        if removed > 2 {
            row.insert(index + if graphic.is_some() { 1 } else { 2 }, ' ');
        }
        while row.last() == Some(&' ') {
            row.pop();
        }
    }

    pub fn move_vertical(&mut self, delta: isize) {
        let visual = self.column();
        self.cursor.line = self
            .cursor
            .line
            .saturating_add_signed(delta)
            .min(self.lines.len() - 1);
        self.set_visual_col(visual);
    }
    pub fn move_horizontal(&mut self, delta: isize) {
        if delta < 0 {
            if self.cursor.col > 0 {
                self.cursor.col -= 1;
            } else if self.cursor.line > 0 {
                self.cursor.line -= 1;
                self.cursor.col = self.lines[self.cursor.line].len();
            }
        } else if self.cursor.col < self.lines[self.cursor.line].len() {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\r' || ch == '\n' {
            self.newline();
            return;
        }
        if ch.is_control() {
            return;
        }
        self.snapshot();
        let line = &mut self.lines[self.cursor.line];
        if self.insert || self.cursor.col == line.len() {
            line.insert(self.cursor.col, ch);
        } else {
            line[self.cursor.col] = ch;
        }
        self.cursor.col += 1;
    }

    pub fn replace_previous(&mut self, ch: char) {
        if self.cursor.col == 0 {
            self.insert_char(ch);
            return;
        }
        self.snapshot();
        self.lines[self.cursor.line][self.cursor.col - 1] = ch;
    }

    pub fn newline(&mut self) {
        self.snapshot();
        let indent = if self.indent {
            self.lines[self.cursor.line]
                .iter()
                .take_while(|&&c| c == ' ')
                .count()
                .min(self.cursor.col)
        } else {
            0
        };
        let tail = self.lines[self.cursor.line].split_off(self.cursor.col);
        self.lines.insert(
            self.cursor.line + 1,
            std::iter::repeat_n(' ', indent).chain(tail).collect(),
        );
        self.cursor.line += 1;
        self.cursor.col = indent;
    }

    pub fn backspace(&mut self) {
        if self.cursor == Pos::default() {
            return;
        }
        self.snapshot();
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            self.lines[self.cursor.line].remove(self.cursor.col);
        } else {
            let tail = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.col = self.lines[self.cursor.line].len();
            self.lines[self.cursor.line].extend(tail);
        }
    }
    pub fn delete(&mut self) {
        if self.cursor.col == self.lines[self.cursor.line].len()
            && self.cursor.line + 1 == self.lines.len()
        {
            return;
        }
        self.snapshot();
        if self.cursor.col < self.lines[self.cursor.line].len() {
            self.lines[self.cursor.line].remove(self.cursor.col);
        } else {
            let tail = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].extend(tail);
        }
    }
    pub fn delete_line(&mut self) {
        self.snapshot();
        let old = self.lines.remove(self.cursor.line);
        self.kill_line = Some(old);
        if self.lines.is_empty() {
            self.lines.push(vec![]);
        }
        self.cursor.line = self.cursor.line.min(self.lines.len() - 1);
        self.cursor.col = self.cursor.col.min(self.lines[self.cursor.line].len());
    }
    pub fn restore_line(&mut self) {
        if let Some(line) = self.kill_line.clone() {
            self.snapshot();
            self.lines.insert(self.cursor.line, line);
            self.cursor.col = 0;
        }
    }
    pub fn delete_to_end(&mut self) {
        self.snapshot();
        self.lines[self.cursor.line].truncate(self.cursor.col);
    }

    pub fn selection(&self) -> Option<(Pos, Pos)> {
        self.block
    }
    pub fn finish_block(&mut self) {
        if let Some(mark) = self.mark.take() {
            if mark != self.cursor {
                self.block = Some(if mark < self.cursor {
                    (mark, self.cursor)
                } else {
                    (self.cursor, mark)
                });
            }
        }
    }
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        let mut parts = vec![];
        for line in start.line..=end.line {
            let from = if line == start.line { start.col } else { 0 };
            let to = if line == end.line {
                end.col
            } else {
                self.lines[line].len()
            };
            parts.push(self.lines[line][from..to].iter().collect::<String>());
        }
        Some(parts.join("\n"))
    }
    pub fn replace_selection(&mut self, text: &str) {
        let (start, end) = self.selection().unwrap_or((self.cursor, self.cursor));
        self.snapshot();
        let mut replacement: Vec<Vec<char>> =
            text.split('\n').map(|s| s.chars().collect()).collect();
        let prefix = self.lines[start.line][..start.col].to_vec();
        let suffix = self.lines[end.line][end.col..].to_vec();
        replacement[0].splice(0..0, prefix);
        let end_line = replacement.len() - 1;
        let end_col = replacement[end_line].len();
        replacement[end_line].extend(suffix);
        self.lines.splice(start.line..=end.line, replacement);
        self.cursor = Pos {
            line: start.line + end_line,
            col: end_col,
        };
        self.mark = None;
    }

    pub fn insert_text(&mut self, text: &str) {
        self.snapshot();
        let start = self.cursor;
        let mut replacement: Vec<Vec<char>> =
            text.split('\n').map(|s| s.chars().collect()).collect();
        let prefix = self.lines[start.line][..start.col].to_vec();
        let suffix = self.lines[start.line][start.col..].to_vec();
        replacement[0].splice(0..0, prefix);
        let last = replacement.len() - 1;
        let end_col = replacement[last].len();
        replacement[last].extend(suffix);
        self.lines.splice(start.line..=start.line, replacement);
        self.cursor = Pos {
            line: start.line + last,
            col: end_col,
        };
    }

    pub fn find_next(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let needle: Vec<char> = needle.chars().collect();
        let mut scan = self.cursor;
        scan.col = (scan.col + 1).min(self.lines[scan.line].len());
        for pass in 0..2 {
            let range = if pass == 0 {
                scan.line..self.lines.len()
            } else {
                0..self.cursor.line + 1
            };
            for line in range {
                let start = if pass == 0 && line == scan.line {
                    scan.col
                } else {
                    0
                };
                for col in start..=self.lines[line].len().saturating_sub(needle.len()) {
                    if self.lines[line].get(col..col + needle.len()) == Some(needle.as_slice()) {
                        self.cursor = Pos { line, col };
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn replace_next(&mut self, needle: &str, replacement: &str) -> bool {
        if !self.find_next(needle) {
            return false;
        }
        let start = self.cursor;
        self.block = Some((
            start,
            Pos {
                line: start.line,
                col: start.col + needle.chars().count(),
            },
        ));
        self.replace_selection(replacement);
        true
    }

    pub fn ensure_visible(&mut self) {
        let line = self.cursor.line;
        let col = self.column();
        self.ensure_visible_at(line, col);
    }

    pub fn ensure_visible_at(&mut self, line: usize, col: usize) {
        const FIRST_ROW: usize = 3;
        const LAST_ROW: usize = 27;
        let height = LAST_ROW - FIRST_ROW + 1;
        if line < self.top {
            self.top = line;
        }
        if line >= self.top + height {
            self.top = line + 1 - height;
        }
        if col < self.left {
            self.left = col;
        }
        if col >= self.left + 79 {
            self.left = col - 78;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn johab_roundtrip_and_known_bytes() {
        assert_eq!(encode_johab("가").unwrap(), vec![0x88, 0x61]);
        let text = "산 한글 편집기\r\n둘째 줄 ㄱ ㅏ";
        let bytes = encode_johab(text).unwrap();
        assert_eq!(decode_johab(&bytes), (text.into(), 0));
    }
    #[test]
    fn edits_and_undo() {
        let mut d = Document::empty();
        d.insert_char('산');
        d.insert_char('A');
        d.newline();
        d.insert_char('글');
        assert_eq!(d.lines.len(), 2);
        d.backspace();
        d.undo();
        assert_eq!(d.lines[1], vec!['글']);
        d.mark = Some(Pos { line: 0, col: 0 });
        d.finish_block();
        assert_eq!(d.selected_text().as_deref(), Some("산A\n글"));
        d.replace_selection("새 문서");
        assert_eq!(d.lines[0].iter().collect::<String>(), "새 문서");
    }

    #[test]
    fn detects_original_distribution_document() {
        let original = include_bytes!("../doc-euc-kr/README.TXT");
        let (text, encoding) = decode(original, None).unwrap();
        assert_eq!(encoding, Encoding::EucKr);
        assert!(text.contains("산"));
    }

    #[test]
    fn finds_and_replaces_next_occurrence() {
        let mut d = Document::empty();
        d.insert_text("산과 산");
        d.cursor = Pos::default();
        assert!(d.replace_next("산", "SAN"));
        assert_eq!(d.lines[0].iter().collect::<String>(), "산과 SAN");
    }

    #[test]
    fn preserves_legacy_encoding_and_line_endings_on_save() {
        let path = std::env::temp_dir().join(format!("san-rs-test-{}.txt", std::process::id()));
        let original = encode("산\r\n둘째", Encoding::EucKr).unwrap();
        fs::write(&path, &original).unwrap();
        let mut d = Document::open(&path, None).unwrap();
        assert_eq!(d.encoding, Encoding::EucKr);
        assert!(d.crlf);
        d.cursor = Pos { line: 1, col: 2 };
        d.insert_char('!');
        d.save(None).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            encode("산\r\n둘째!", Encoding::EucKr).unwrap()
        );
        assert_eq!(fs::read(path.with_extension("BAK")).unwrap(), original);
        fs::remove_file(&path).unwrap();
        fs::remove_file(path.with_extension("BAK")).unwrap();
    }

    #[test]
    fn original_graphics_survive_johab_and_fixed_code_save() {
        let raw = [0xd4, 0xda, 0xd9, 0x32, 0xda, 0x31, 0xdb, 0xa3, 0xde, 0xf1];
        let (text, errors) = decode_johab(&raw);
        assert_eq!(errors, 0);
        assert_eq!(encode_johab(&text).unwrap(), raw);
        let fixed = encode(&text, Encoding::EucKr).unwrap();
        assert_eq!(fixed.len(), raw.len());
    }

    #[test]
    fn drawing_overwrites_two_columns_without_moving_following_text() {
        let mut doc = Document::empty();
        doc.insert_text("ABCD");
        doc.put_visual(0, 0, Some('┌'));
        assert_eq!(doc.lines[0].iter().collect::<String>(), "┌CD");
        doc.put_visual(0, 2, Some('─'));
        assert_eq!(doc.lines[0].iter().collect::<String>(), "┌─");
        doc.put_visual(0, 0, None);
        assert_eq!(doc.lines[0].iter().collect::<String>(), "  ─");
    }
}
