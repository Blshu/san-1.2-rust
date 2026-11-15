use crate::{
    document::{Document, Encoding, Pos},
    drawing::{Direction, DrawAction, DrawState},
    font::{self, Canvas, BG, DIM, FG, HEIGHT, HI, WIDTH},
    graphics,
    hangul::{Composer, Edit as HangulEdit},
    macros::{self, Event, Recorder},
};
use minifb::{InputCallback, Key, KeyRepeat, MouseButton, MouseMode, Scale, Window, WindowOptions};
use std::{cell::RefCell, env, path::PathBuf, rc::Rc, time::Instant};

type InputQueue = Rc<RefCell<Vec<char>>>;
struct TextInput(InputQueue);
impl InputCallback for TextInput {
    fn add_char(&mut self, code: u32) {
        if let Some(ch) = char::from_u32(code) {
            self.0.borrow_mut().push(ch);
        }
    }
}

#[derive(Clone, Copy)]
enum Prompt {
    Open,
    SaveAs,
    Find,
    ReplaceFind,
    ReplaceWith,
}
#[derive(Clone, Copy)]
enum MenuKind {
    Main,
    File,
    Edit,
    Block,
    Find,
    Task,
}
enum Mode {
    Normal,
    Splash,
    Menu(MenuKind, usize),
    Prompt(Prompt, String),
    Help(usize),
    ConfirmQuit,
    GraphicPicker(GraphicPicker),
    Drawing(DrawState),
    StylePicker(DrawState, usize),
}
struct GraphicPicker {
    page: usize,
    row: usize,
    col: usize,
    sequence: Vec<char>,
    drawing: Option<DrawState>,
}
impl GraphicPicker {
    fn new(drawing: Option<DrawState>) -> Self {
        Self {
            page: 0,
            row: 0,
            col: 1,
            sequence: Vec::new(),
            drawing,
        }
    }
    fn selected(&self) -> Option<char> {
        graphics::page_cell(self.page, self.row, self.col)
    }
    fn advance(&mut self) {
        self.col += 1;
        if self.col == 16 {
            self.col = 0;
            self.row = (self.row + 1) % self.rows();
        }
    }
    fn rows(&self) -> usize {
        if self.page == 0 {
            8
        } else {
            12
        }
    }
}
#[derive(Clone, Copy)]
enum Prefix {
    K,
    Q,
    O,
}

const MAIN: &[(&str, &str)] = &[
    ("F", "문서"),
    ("C", "실행"),
    ("O", "선택"),
    ("Y", "마당"),
    ("B", "블럭"),
    ("I", "삽입"),
    ("E", "입력"),
    ("H", "찾기"),
    ("P", "특수"),
    ("Q", "종료"),
];
const FILE: &[(&str, &str)] = &[
    ("N", "새 문서"),
    ("O", "문서 읽기  F3"),
    ("S", "문서 저장  F2"),
    ("A", "새 이름 저장  Alt-W"),
    ("Q", "작업 종료"),
];
const EDIT: &[(&str, &str)] = &[
    ("U", "실행 취소"),
    ("R", "다시 실행"),
    ("I", "삽입/수정 전환"),
    ("D", "들임 전환"),
    ("Y", "현재 줄 지움"),
    ("L", "지운 줄 복원"),
];
const BLOCK: &[(&str, &str)] = &[
    ("B", "블록 처음 표시"),
    ("K", "블록 끝 표시"),
    ("C", "블록 복사"),
    ("V", "블록 붙이기"),
    ("Y", "블록 지움"),
];
const FIND: &[(&str, &str)] = &[
    ("F", "단어 검색  Ctrl-QF"),
    ("N", "다음 찾기  Ctrl-L"),
    ("A", "단어 치환  Ctrl-QA"),
];
const TASK: &[(&str, &str)] = &[
    ("1", "작업 마당 1"),
    ("2", "작업 마당 2"),
    ("3", "작업 마당 3"),
];
const HELP: &[&str] = &[
    "산 1.2  -  현대 운영체제용 Rust 포트",
    "",
    "원본 사용법의 주요 글쇠를 유지했습니다.",
    "F2 저장     F3 문서 읽기    F4 그림 문자",
    "Alt-G 그림 문자   Alt-D 선 그리기",
    "Ctrl-L 다음 찾기",
    "Ctrl+Shift+A~Z 매크로 기록/종료",
    "Alt+Shift+A~Z 매크로 재생  Shift+Esc 빈 기록 취소",
    "F10 차림    Alt-W 새 이름 저장",
    "화살표 / Home / End / PageUp / PageDown 이동",
    "Ctrl-S/D/E/X  왼쪽/오른쪽/위/아래",
    "Ctrl-R/C  한 화면 위/아래",
    "Ctrl-N    행 분할       Ctrl-Y 줄 지움",
    "Ctrl-U    지운 줄 복원  Ctrl-QY 줄 끝 지움",
    "Ctrl-KB   블록 시작     Ctrl-KK 블록 끝",
    "Ctrl-KC   블록 복사     Ctrl-KV 블록 붙이기",
    "Ctrl-KY   블록 지움     Ctrl-QF 단어 검색",
    "Ctrl-QA   다음 단어 치환",
    "Ctrl-QS/D 행 처음/끝  Ctrl-QR/C 문서 처음/끝",
    "Insert    삽입/수정 전환",
    "",
    "원본 SAN.KOR, SAN.ENG 자형을 사용합니다.",
    "저장할 때 기존 파일은 .BAK로 백업됩니다.",
    "UTF-8, EUC-KR, JOHAB 텍스트 파일을 지원합니다.",
    "Esc를 누르면 도움말 화면을 닫습니다.",
];

pub struct App {
    docs: [Document; 3],
    current: usize,
    mode: Mode,
    prefix: Option<Prefix>,
    clipboard: String,
    search: String,
    replace_search: String,
    status: String,
    forced_encoding: Option<Encoding>,
    quit: bool,
    started: Instant,
    mouse_was_down: bool,
    han_mode: bool,
    composer: Composer,
    macros: Recorder,
    held_macro_chord: Option<Key>,
}

impl App {
    fn new(forced_encoding: Option<Encoding>) -> Self {
        Self {
            docs: std::array::from_fn(|_| Document::empty()),
            current: 0,
            mode: Mode::Normal,
            prefix: None,
            clipboard: String::new(),
            search: String::new(),
            replace_search: String::new(),
            status: "F10 차림  F1 도움말  F4 그림  Alt-D 선".into(),
            forced_encoding,
            quit: false,
            started: Instant::now(),
            mouse_was_down: false,
            han_mode: false,
            composer: Composer::new(),
            macros: Recorder::new(),
            held_macro_chord: None,
        }
    }
    fn doc(&self) -> &Document {
        &self.docs[self.current]
    }
    fn doc_mut(&mut self) -> &mut Document {
        &mut self.docs[self.current]
    }

    fn apply_hangul_edit(&mut self, edit: HangulEdit) {
        match edit {
            HangulEdit::Insert(ch) => self.doc_mut().insert_char(ch),
            HangulEdit::Replace(ch) => self.doc_mut().replace_previous(ch),
            HangulEdit::Split(previous, next) => {
                self.doc_mut().replace_previous(previous);
                self.doc_mut().insert_char(next);
            }
            HangulEdit::Delete => self.doc_mut().backspace(),
        }
    }

    fn toggle_han(&mut self) {
        self.composer.reset();
        self.han_mode = !self.han_mode;
        self.status = if self.han_mode {
            "한글 두벌식 입력"
        } else {
            "영문 입력"
        }
        .into();
    }

    fn open(&mut self, path: PathBuf) {
        if self.doc().dirty {
            self.status = "현재 문서를 먼저 저장해 주세요.".into();
            self.mode = Mode::Normal;
            return;
        }
        match Document::open(&path, self.forced_encoding) {
            Ok(doc) => {
                self.docs[self.current] = doc;
                self.status = format!("읽었습니다: {}", path.display());
                self.mode = Mode::Normal;
            }
            Err(_) if !path.exists() => {
                let mut doc = Document::empty();
                doc.path = Some(path.clone());
                doc.encoding = self.forced_encoding.unwrap_or(Encoding::Utf8);
                self.docs[self.current] = doc;
                self.status = format!("새 파일: {}", path.display());
                self.mode = Mode::Normal;
            }
            Err(error) => {
                self.status = format!("읽기 오류: {error}");
                self.mode = Mode::Normal;
            }
        }
    }
    fn save(&mut self, path: Option<PathBuf>) {
        if path.is_none() && self.doc().path.is_none() {
            self.mode = Mode::Prompt(Prompt::SaveAs, String::new());
            return;
        }
        let existed = path
            .as_ref()
            .or(self.doc().path.as_ref())
            .is_some_and(|p| p.exists());
        match self.doc_mut().save(path.as_deref()) {
            Ok(()) => {
                self.status = if existed {
                    "문서를 저장했습니다. 이전 내용은 .BAK에 보관했습니다."
                } else {
                    "문서를 저장했습니다."
                }
                .into()
            }
            Err(error) => self.status = format!("저장 오류: {error}"),
        }
        self.mode = Mode::Normal;
    }
    fn request_quit(&mut self) {
        if self.docs.iter().any(|d| d.dirty) {
            self.mode = Mode::ConfirmQuit;
        } else {
            self.quit = true;
        }
    }
    fn switch_task(&mut self, task: usize) {
        self.current = task;
        self.status = format!("작업 마당 {}", task + 1);
        self.mode = Mode::Normal;
    }
    fn find_next(&mut self) {
        let search = self.search.clone();
        if search.is_empty() {
            self.mode = Mode::Prompt(Prompt::Find, String::new());
            return;
        }
        if self.doc_mut().find_next(&search) {
            self.status = format!("찾음: {search}");
        } else {
            self.status = format!("찾을 수 없음: {search}");
        }
    }

    fn select_menu(&mut self, kind: MenuKind, index: usize) {
        if let MenuKind::Main = kind {
            match index {
                0 => self.mode = Mode::Menu(MenuKind::File, 0),
                3 => self.mode = Mode::Menu(MenuKind::Task, 0),
                4 => self.mode = Mode::Menu(MenuKind::Block, 0),
                5 | 6 => self.mode = Mode::Menu(MenuKind::Edit, 0),
                7 => self.mode = Mode::Menu(MenuKind::Find, 0),
                1 | 2 | 8 => {
                    self.mode = Mode::Normal;
                    self.status = "이 DOS 기능은 아직 포트되지 않았습니다.".into();
                }
                _ => self.request_quit(),
            }
            return;
        }
        self.mode = Mode::Normal;
        match (kind, index) {
            (MenuKind::File, 0) => {
                if self.doc().dirty {
                    self.status = "현재 문서를 먼저 저장해 주세요.".into();
                } else {
                    self.docs[self.current] = Document::empty();
                    self.status = "새 문서".into();
                }
            }
            (MenuKind::File, 1) => self.mode = Mode::Prompt(Prompt::Open, String::new()),
            (MenuKind::File, 2) => self.save(None),
            (MenuKind::File, 3) => {
                self.mode = Mode::Prompt(
                    Prompt::SaveAs,
                    self.doc()
                        .path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
            }
            (MenuKind::File, 4) => self.request_quit(),
            (MenuKind::Edit, 0) => self.doc_mut().undo(),
            (MenuKind::Edit, 1) => self.doc_mut().redo(),
            (MenuKind::Edit, 2) => self.doc_mut().insert = !self.doc().insert,
            (MenuKind::Edit, 3) => self.doc_mut().indent = !self.doc().indent,
            (MenuKind::Edit, 4) => self.doc_mut().delete_line(),
            (MenuKind::Edit, 5) => self.doc_mut().restore_line(),
            (MenuKind::Block, 0) => {
                let pos = self.doc().cursor;
                self.doc_mut().mark = Some(pos);
                self.status = "블록 시작을 표시했습니다.".into();
            }
            (MenuKind::Block, 1) => {
                self.doc_mut().finish_block();
                self.status = "블록 끝을 표시했습니다.".into();
            }
            (MenuKind::Block, 2) => self.copy_selection(false),
            (MenuKind::Block, 3) => self.paste(),
            (MenuKind::Block, 4) => self.copy_selection(true),
            (MenuKind::Find, 0) => self.mode = Mode::Prompt(Prompt::Find, self.search.clone()),
            (MenuKind::Find, 1) => self.find_next(),
            (MenuKind::Find, 2) => {
                self.mode = Mode::Prompt(Prompt::ReplaceFind, self.search.clone())
            }
            (MenuKind::Task, n @ 0..=2) => self.switch_task(n),
            _ => {}
        }
    }

    fn copy_selection(&mut self, cut: bool) {
        if let Some(text) = self.doc().selected_text() {
            self.clipboard = text;
            if cut {
                self.doc_mut().replace_selection("");
            }
            self.status = if cut {
                "블록을 잘랐습니다."
            } else {
                "블록을 복사했습니다."
            }
            .into();
        } else {
            self.status = "블록 처음과 끝을 표시하세요.".into();
        }
    }
    fn paste(&mut self) {
        if !self.clipboard.is_empty() {
            let text = self.clipboard.clone();
            self.doc_mut().insert_text(&text);
        }
    }

    fn receive_key(&mut self, key: Key, control: bool, alt: bool, shift: bool) {
        if let Some(index) = macros::slot_for_key(key).filter(|_| shift && (control || alt)) {
            if self.held_macro_chord == Some(key) {
                return;
            }
            self.held_macro_chord = Some(key);
            if control {
                let recording = self.macros.toggle_recording(index, self.han_mode);
                self.status = format!(
                    "매크로 {} {}",
                    (b'A' + index as u8) as char,
                    if recording {
                        "기록 중"
                    } else {
                        "기록 완료"
                    }
                );
            } else {
                self.play_macro(index);
            }
            return;
        }
        if shift && key == Key::Escape && self.macros.cancel_empty() {
            self.status = "빈 매크로 기록을 취소했습니다.".into();
            return;
        }
        if key == Key::Space && shift && !control && !alt {
            self.receive_han_toggle();
            return;
        }
        if control || alt || !is_printable_key(key) {
            self.macros.record(Event::Key(key, control, alt));
        }
        self.handle_key(key, control, alt);
    }

    fn receive_char(&mut self, ch: char) {
        self.macros.record(Event::Char(ch));
        self.handle_char(ch);
    }

    fn receive_han_toggle(&mut self) {
        self.macros.record(Event::ToggleHan);
        self.toggle_han();
    }

    fn play_macro(&mut self, index: usize) {
        let Some(recording) = self.macros.playback(index) else {
            self.status = format!(
                "매크로 {}에 기록된 글쇠가 없습니다.",
                (b'A' + index as u8) as char
            );
            return;
        };
        let previous_han = self.han_mode;
        self.composer.reset();
        self.han_mode = recording.han_mode;
        for event in recording.events {
            self.macros.record(event.clone());
            match event {
                Event::Key(key, control, alt) => self.handle_key(key, control, alt),
                Event::Char(ch) => self.handle_char(ch),
                Event::ToggleHan => self.toggle_han(),
            }
            if self.quit {
                break;
            }
        }
        self.composer.reset();
        self.han_mode = previous_han;
        self.macros.finish_playback();
        self.status = format!("매크로 {} 재생 완료", (b'A' + index as u8) as char);
    }

    fn commit_graphics(&mut self, picker: GraphicPicker) {
        let selected = picker.selected();
        if let Some(mut draw) = picker.drawing {
            if let Some(ch) = selected {
                draw.graphic = ch;
                draw.set_action(DrawAction::Graphic);
                draw.stamp(self.doc_mut());
            }
            self.mode = Mode::Drawing(draw);
        } else {
            let chars = if picker.sequence.is_empty() {
                selected.into_iter().collect::<Vec<_>>()
            } else {
                picker.sequence
            };
            for ch in chars {
                self.doc_mut().insert_char(ch);
            }
            self.mode = Mode::Normal;
        }
    }

    fn handle_picker_key(&mut self, key: Key, control: bool) {
        let key = if control {
            match key {
                Key::S => Key::Left,
                Key::D => Key::Right,
                Key::E => Key::Up,
                Key::X => Key::Down,
                Key::R => Key::PageUp,
                Key::C => Key::PageDown,
                _ => key,
            }
        } else {
            key
        };
        let Mode::GraphicPicker(mut picker) = std::mem::replace(&mut self.mode, Mode::Normal)
        else {
            unreachable!()
        };
        match key {
            Key::Escape => {
                self.mode = picker.drawing.map_or(Mode::Normal, Mode::Drawing);
                return;
            }
            Key::Enter => {
                self.commit_graphics(picker);
                return;
            }
            Key::Backspace => {
                picker.sequence.pop();
            }
            Key::PageUp => {
                picker.page = (picker.page + 6) % 7;
                picker.row = picker.row.min(picker.rows() - 1);
            }
            Key::PageDown => {
                picker.page = (picker.page + 1) % 7;
                picker.row = picker.row.min(picker.rows() - 1);
            }
            Key::Left => {
                if picker.col == 0 {
                    picker.col = 15;
                    picker.row = picker.row.saturating_sub(1);
                } else {
                    picker.col -= 1;
                }
            }
            Key::Right => picker.advance(),
            Key::Up => picker.row = picker.row.saturating_sub(1),
            Key::Down => picker.row = (picker.row + 1).min(picker.rows() - 1),
            Key::Home => {
                picker.row = 0;
                picker.col = 0;
            }
            Key::End => {
                picker.row = picker.rows() - 1;
                picker.col = 15;
            }
            _ => {}
        }
        self.mode = Mode::GraphicPicker(picker);
    }

    fn handle_picker_char(&mut self, ch: char) {
        let Mode::GraphicPicker(mut picker) = std::mem::replace(&mut self.mode, Mode::Normal)
        else {
            unreachable!()
        };
        if matches!(ch, '+' | ' ') {
            if picker.sequence.len() < 126 {
                if let Some(selected) = picker.selected() {
                    picker.sequence.push(selected);
                }
            }
            if ch == ' ' {
                picker.advance();
            }
        }
        self.mode = Mode::GraphicPicker(picker);
    }

    fn handle_style_key(&mut self, key: Key, control: bool) {
        let key = if control {
            match key {
                Key::S | Key::E => Key::Left,
                Key::D | Key::X => Key::Right,
                Key::R => Key::Home,
                Key::C => Key::End,
                _ => key,
            }
        } else {
            key
        };
        let Mode::StylePicker(mut draw, mut selection) =
            std::mem::replace(&mut self.mode, Mode::Normal)
        else {
            unreachable!()
        };
        match key {
            Key::Escape => {
                self.mode = Mode::Drawing(draw);
                return;
            }
            Key::Enter => {
                draw.style = selection;
                draw.set_action(DrawAction::Draw);
                self.status = format!("선 그리기: 선  모양 {}", selection + 1);
                self.mode = Mode::Drawing(draw);
                return;
            }
            Key::Left | Key::Up => selection = (selection + 7) % 8,
            Key::Right | Key::Down => selection = (selection + 1) % 8,
            Key::Home | Key::PageUp => selection = 0,
            Key::End | Key::PageDown => selection = 7,
            _ => {}
        }
        self.mode = Mode::StylePicker(draw, selection);
    }

    fn handle_style_char(&mut self, ch: char) {
        if let '1'..='8' = ch {
            let Mode::StylePicker(mut draw, _) = std::mem::replace(&mut self.mode, Mode::Normal)
            else {
                unreachable!()
            };
            draw.style = ch.to_digit(10).unwrap() as usize - 1;
            draw.set_action(DrawAction::Draw);
            self.status = format!("선 그리기: 선  모양 {}", draw.style + 1);
            self.mode = Mode::Drawing(draw);
        }
    }

    fn handle_drawing_key(&mut self, key: Key, control: bool, alt: bool) {
        let key = if control {
            match key {
                Key::S => Key::Left,
                Key::D => Key::Right,
                Key::E => Key::Up,
                Key::X => Key::Down,
                _ => key,
            }
        } else {
            key
        };
        let Mode::Drawing(mut draw) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            unreachable!()
        };
        if alt && key == Key::G {
            self.mode = Mode::GraphicPicker(GraphicPicker::new(Some(draw)));
            return;
        }
        match key {
            Key::Escape => {
                self.status = "선 그리기를 마쳤습니다.".into();
                return;
            }
            Key::F4 => {
                self.mode = Mode::GraphicPicker(GraphicPicker::new(Some(draw)));
                return;
            }
            Key::Left => draw.step(self.doc_mut(), Direction::Left),
            Key::Right => draw.step(self.doc_mut(), Direction::Right),
            Key::Up => draw.step(self.doc_mut(), Direction::Up),
            Key::Down => draw.step(self.doc_mut(), Direction::Down),
            _ => {}
        }
        self.mode = Mode::Drawing(draw);
    }

    fn handle_drawing_char(&mut self, ch: char) {
        let Mode::Drawing(mut draw) = std::mem::replace(&mut self.mode, Mode::Normal) else {
            unreachable!()
        };
        match ch.to_ascii_uppercase() {
            'D' => draw.set_action(DrawAction::Draw),
            'E' => {
                draw.set_action(DrawAction::Erase);
                draw.erase_current(self.doc_mut());
            }
            'M' | 'F' => draw.set_action(DrawAction::Move),
            'G' | 'A' => {
                self.mode = Mode::GraphicPicker(GraphicPicker::new(Some(draw)));
                return;
            }
            'L' | 'S' => {
                self.mode = Mode::StylePicker(draw.clone(), draw.style);
                return;
            }
            '1'..='8' => {
                draw.style = ch.to_digit(10).unwrap() as usize - 1;
                draw.set_action(DrawAction::Draw);
            }
            ' ' => draw.toggle_action(),
            _ => {}
        }
        self.status = format!(
            "선 그리기: {}  모양 {}",
            draw.action.label(),
            draw.style + 1
        );
        self.mode = Mode::Drawing(draw);
    }

    fn handle_prefix(&mut self, ch: char) {
        let prefix = self.prefix.take().unwrap();
        let ch = ch.to_ascii_uppercase();
        match (prefix, ch) {
            (Prefix::K, 'S') => self.save(None),
            (Prefix::K, 'Q') => self.mode = Mode::Menu(MenuKind::Main, 0),
            (Prefix::K, 'B') => {
                let pos = self.doc().cursor;
                self.doc_mut().mark = Some(pos);
                self.status = "블록 시작".into();
            }
            (Prefix::K, 'K') => {
                self.doc_mut().finish_block();
                self.status = "블록 끝".into();
            }
            (Prefix::K, 'C') => self.copy_selection(false),
            (Prefix::K, 'V') => self.paste(),
            (Prefix::K, 'Y') => self.copy_selection(true),
            (Prefix::Q, 'F') => self.mode = Mode::Prompt(Prompt::Find, self.search.clone()),
            (Prefix::Q, 'A') => self.mode = Mode::Prompt(Prompt::ReplaceFind, self.search.clone()),
            (Prefix::Q, 'S') => self.doc_mut().cursor.col = 0,
            (Prefix::Q, 'D') => {
                let d = self.doc_mut();
                d.cursor.col = d.lines[d.cursor.line].len();
            }
            (Prefix::Q, 'R') => {
                self.doc_mut().cursor = Pos::default();
            }
            (Prefix::Q, 'C') => {
                let d = self.doc_mut();
                d.cursor.line = d.lines.len() - 1;
                d.cursor.col = d.lines[d.cursor.line].len();
            }
            (Prefix::Q, 'Y') => self.doc_mut().delete_to_end(),
            (Prefix::Q, 'L') => self.doc_mut().undo(),
            (Prefix::O, 'I') => self.doc_mut().indent = !self.doc().indent,
            _ => {
                self.status = format!(
                    "알 수 없는 명령: ^{}{}",
                    match prefix {
                        Prefix::K => 'K',
                        Prefix::Q => 'Q',
                        Prefix::O => 'O',
                    },
                    ch
                )
            }
        }
    }

    fn handle_char(&mut self, ch: char) {
        if matches!(self.mode, Mode::StylePicker(_, _)) {
            self.handle_style_char(ch);
            return;
        }
        if matches!(self.mode, Mode::GraphicPicker(_)) {
            self.handle_picker_char(ch);
            return;
        }
        if matches!(self.mode, Mode::Drawing(_)) {
            self.handle_drawing_char(ch);
            return;
        }
        match &mut self.mode {
            Mode::Splash => {
                self.mode = Mode::Normal;
                return;
            }
            Mode::Prompt(_, input) => {
                if ch.is_control() {
                    return;
                }
                if self.han_mode {
                    if let Some(edit) = self.composer.feed(ch) {
                        apply_to_string(input, edit);
                        return;
                    }
                }
                self.composer.reset();
                input.push(ch);
                return;
            }
            Mode::ConfirmQuit => {
                match ch.to_ascii_uppercase() {
                    'Y' => self.quit = true,
                    'N' => self.mode = Mode::Normal,
                    _ => {}
                }
                return;
            }
            Mode::Menu(kind, _) => {
                let kind = *kind;
                let items = menu_items(kind);
                if let Some(index) = items
                    .iter()
                    .position(|(key, _)| key.eq_ignore_ascii_case(&ch.to_string()))
                {
                    self.select_menu(kind, index);
                }
                return;
            }
            Mode::Help(_) => return,
            Mode::Normal => {}
            Mode::GraphicPicker(_) | Mode::Drawing(_) | Mode::StylePicker(_, _) => unreachable!(),
        }
        if self.prefix.is_some() {
            self.handle_prefix(ch);
        } else if !ch.is_control() {
            if self.han_mode {
                if let Some(edit) = self.composer.feed(ch) {
                    self.apply_hangul_edit(edit);
                    return;
                }
            }
            self.composer.reset();
            self.doc_mut().insert_char(ch);
        }
    }

    fn handle_key(&mut self, key: Key, control: bool, alt: bool) {
        if matches!(self.mode, Mode::Splash) {
            self.mode = Mode::Normal;
            return;
        }
        if matches!(self.mode, Mode::GraphicPicker(_)) {
            self.handle_picker_key(key, control);
            return;
        }
        if matches!(self.mode, Mode::StylePicker(_, _)) {
            self.handle_style_key(key, control);
            return;
        }
        if matches!(self.mode, Mode::Drawing(_)) {
            self.handle_drawing_key(key, control, alt);
            return;
        }
        if control
            || alt
            || matches!(
                key,
                Key::Left
                    | Key::Right
                    | Key::Up
                    | Key::Down
                    | Key::Home
                    | Key::End
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Space
                    | Key::Delete
                    | Key::Enter
                    | Key::Tab
                    | Key::F1
                    | Key::F2
                    | Key::F3
                    | Key::F4
                    | Key::F10
                    | Key::Escape
            )
        {
            self.composer.reset();
        }
        if matches!(self.mode, Mode::ConfirmQuit) {
            match key {
                Key::Escape | Key::N => self.mode = Mode::Normal,
                Key::Y => self.quit = true,
                _ => {}
            }
            return;
        }
        if let Mode::Prompt(kind, input) = &mut self.mode {
            match key {
                Key::Escape => self.mode = Mode::Normal,
                Key::Backspace => {
                    if let Some(edit) = self.composer.backspace() {
                        apply_to_string(input, edit);
                    } else {
                        input.pop();
                    }
                }
                Key::Enter => {
                    let kind = *kind;
                    let value = input.to_string();
                    if value.is_empty() && !matches!(kind, Prompt::ReplaceWith) {
                        self.mode = Mode::Normal;
                        return;
                    }
                    match kind {
                        Prompt::Open => self.open(PathBuf::from(value)),
                        Prompt::SaveAs => self.save(Some(PathBuf::from(value))),
                        Prompt::Find => {
                            self.search = value;
                            self.mode = Mode::Normal;
                            self.find_next();
                        }
                        Prompt::ReplaceFind => {
                            self.replace_search = value.clone();
                            self.search = value;
                            self.mode = Mode::Prompt(Prompt::ReplaceWith, String::new());
                        }
                        Prompt::ReplaceWith => {
                            let needle = self.replace_search.clone();
                            self.mode = Mode::Normal;
                            if self.doc_mut().replace_next(&needle, &value) {
                                self.status = format!("치환: {needle} → {value}");
                            } else {
                                self.status = format!("찾을 수 없음: {needle}");
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        if let Mode::Help(top) = &mut self.mode {
            match key {
                Key::Escape | Key::F1 => self.mode = Mode::Normal,
                Key::Down => *top = (*top + 1).min(HELP.len().saturating_sub(1)),
                Key::Up => *top = top.saturating_sub(1),
                Key::PageDown => *top = (*top + 20).min(HELP.len().saturating_sub(1)),
                Key::PageUp => *top = top.saturating_sub(20),
                _ => {}
            }
            return;
        }
        if let Mode::Menu(kind, index) = &mut self.mode {
            let count = menu_items(*kind).len();
            match key {
                Key::Escape => self.mode = Mode::Normal,
                Key::Down => *index = (*index + 1) % count,
                Key::Up => *index = (*index + count - 1) % count,
                Key::Enter => {
                    let kind = *kind;
                    let index = *index;
                    self.select_menu(kind, index);
                }
                Key::Left if !matches!(*kind, MenuKind::Main) => {
                    self.mode = Mode::Menu(MenuKind::Main, 0)
                }
                _ => {}
            }
            return;
        }
        if alt {
            match key {
                Key::W => {
                    self.mode = Mode::Prompt(
                        Prompt::SaveAs,
                        self.doc()
                            .path
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )
                }
                Key::Key1 => self.switch_task(0),
                Key::Key2 => self.switch_task(1),
                Key::Key3 => self.switch_task(2),
                Key::Z => self.doc_mut().undo(),
                Key::R => self.doc_mut().redo(),
                Key::F => self.mode = Mode::Menu(MenuKind::File, 0),
                Key::Y => self.mode = Mode::Menu(MenuKind::Task, 0),
                Key::B => self.mode = Mode::Menu(MenuKind::Block, 0),
                Key::I | Key::E => self.mode = Mode::Menu(MenuKind::Edit, 0),
                Key::H => self.mode = Mode::Menu(MenuKind::Find, 0),
                Key::D => {
                    self.mode = Mode::Drawing(DrawState::new(self.doc()));
                    self.status = "선 그리기: 이동  모양 1".into();
                }
                Key::G => self.mode = Mode::GraphicPicker(GraphicPicker::new(None)),
                Key::C | Key::O | Key::P => {
                    self.status = "이 DOS 기능은 아직 포트되지 않았습니다.".into()
                }
                _ => {}
            }
            return;
        }
        if control {
            if self.prefix.is_some() {
                if let Some(ch) = key_to_letter(key) {
                    self.handle_prefix(ch);
                }
                return;
            }
            match key {
                Key::K => self.prefix = Some(Prefix::K),
                Key::Q => self.prefix = Some(Prefix::Q),
                Key::O => self.prefix = Some(Prefix::O),
                Key::S => self.doc_mut().move_horizontal(-1),
                Key::D => self.doc_mut().move_horizontal(1),
                Key::E => self.doc_mut().move_vertical(-1),
                Key::X => self.doc_mut().move_vertical(1),
                Key::R => self.doc_mut().move_vertical(-25),
                Key::C => self.doc_mut().move_vertical(25),
                Key::N => self.doc_mut().newline(),
                Key::V => self.doc_mut().insert = !self.doc().insert,
                Key::Y => self.doc_mut().delete_line(),
                Key::U => self.doc_mut().restore_line(),
                Key::L => self.find_next(),
                Key::F => self.word_right(),
                Key::A => self.word_left(),
                Key::Home => self.doc_mut().cursor = Pos::default(),
                Key::End => {
                    let d = self.doc_mut();
                    d.cursor.line = d.lines.len() - 1;
                    d.cursor.col = d.lines[d.cursor.line].len();
                }
                _ => {}
            }
            return;
        }
        match key {
            Key::F1 => self.mode = Mode::Help(0),
            Key::F2 => self.save(None),
            Key::F3 => self.mode = Mode::Prompt(Prompt::Open, String::new()),
            Key::F4 => self.mode = Mode::GraphicPicker(GraphicPicker::new(None)),
            Key::F10 => self.mode = Mode::Menu(MenuKind::Main, 0),
            Key::Escape => {
                self.prefix = None;
                self.doc_mut().mark = None;
                self.doc_mut().block = None;
            }
            Key::Left => self.doc_mut().move_horizontal(-1),
            Key::Right => self.doc_mut().move_horizontal(1),
            Key::Up => self.doc_mut().move_vertical(-1),
            Key::Down => self.doc_mut().move_vertical(1),
            Key::PageUp => self.doc_mut().move_vertical(-25),
            Key::PageDown => self.doc_mut().move_vertical(25),
            Key::Home => self.doc_mut().cursor.col = 0,
            Key::End => {
                let d = self.doc_mut();
                d.cursor.col = d.lines[d.cursor.line].len();
            }
            Key::Insert => self.doc_mut().insert = !self.doc().insert,
            Key::Delete => {
                if self.doc().selection().is_some() {
                    self.doc_mut().replace_selection("");
                } else {
                    self.doc_mut().delete();
                }
            }
            Key::Backspace => {
                if let Some(edit) = self.composer.backspace() {
                    self.apply_hangul_edit(edit);
                } else if self.doc().selection().is_some() {
                    self.doc_mut().replace_selection("");
                } else {
                    self.doc_mut().backspace();
                }
            }
            Key::Enter => self.doc_mut().newline(),
            Key::Tab => {
                for _ in 0..4 {
                    self.doc_mut().insert_char(' ');
                }
            }
            _ => {}
        }
    }

    fn word_right(&mut self) {
        let d = self.doc_mut();
        let line = &d.lines[d.cursor.line];
        while d.cursor.col < line.len() && !line[d.cursor.col].is_whitespace() {
            d.cursor.col += 1;
        }
        while d.cursor.col < line.len() && line[d.cursor.col].is_whitespace() {
            d.cursor.col += 1;
        }
    }
    fn word_left(&mut self) {
        let d = self.doc_mut();
        let line = &d.lines[d.cursor.line];
        while d.cursor.col > 0 && line[d.cursor.col - 1].is_whitespace() {
            d.cursor.col -= 1;
        }
        while d.cursor.col > 0 && !line[d.cursor.col - 1].is_whitespace() {
            d.cursor.col -= 1;
        }
    }

    fn update_mouse(&mut self, window: &Window) {
        let down = window.get_mouse_down(MouseButton::Left);
        if down && !self.mouse_was_down && matches!(self.mode, Mode::Normal) {
            if let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard) {
                let col = (x as usize / 8).min(79);
                let row = y as usize / 16;
                if (3..=27).contains(&row) {
                    let d = self.doc_mut();
                    d.cursor.line = (d.top + row - 3).min(d.lines.len() - 1);
                    d.set_visual_col(col + d.left);
                }
            }
        }
        self.mouse_was_down = down;
    }

    fn draw(&mut self, canvas: &mut Canvas) {
        if matches!(self.mode, Mode::Splash) {
            self.draw_splash(canvas);
            return;
        }
        if let Mode::Drawing(draw) = &self.mode {
            let (line, col) = (draw.line, draw.col);
            self.doc_mut().ensure_visible_at(line, col);
        } else {
            self.doc_mut().ensure_visible();
        }
        canvas.clear();
        canvas.text(
            0,
            0,
            "  F)문서  C)실행  O)선택  Y)마당  B)블럭  I)삽입  E)입력  H)찾기  P)특수  산..",
            FG,
        );
        canvas.separator(1, 137);
        let d = self.doc();
        let name = d
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "(이름 없음)".into());
        canvas.text(
            1,
            2,
            &format!(" SAN 1.2   {name}   [{}]", d.encoding.label()),
            HI,
        );
        let selection = d.selection();
        for screen_row in 3..=27 {
            let line_index = d.top + screen_row - 3;
            if line_index >= d.lines.len() {
                continue;
            }
            let mut visual_col = 0;
            for (col, &ch) in d.lines[line_index].iter().enumerate() {
                let width = font::char_width(ch);
                if visual_col + width > d.left && visual_col >= d.left {
                    let x = visual_col - d.left;
                    if x + width <= 80 {
                        let selected = selection.is_some_and(|(start, end)| {
                            (Pos {
                                line: line_index,
                                col,
                            }) >= start
                                && (Pos {
                                    line: line_index,
                                    col,
                                }) < end
                        });
                        if selected {
                            canvas.cell_bg(x, screen_row, width, FG);
                        }
                        canvas.glyph(x, screen_row, ch, if selected { BG } else { FG });
                    }
                }
                visual_col += width;
                if visual_col >= d.left + 80 {
                    break;
                }
            }
        }
        canvas.separator(28, 138);
        let (line, col) = if let Mode::Drawing(draw) = &self.mode {
            (draw.line + 1, draw.col + 1)
        } else {
            (d.cursor.line + 1, d.column() + 1)
        };
        let indicator = if d.insert { "삽입" } else { "수정" };
        let dirty = if d.dirty { "*" } else { " " };
        let status = if let Some((slot, count)) = self.macros.active() {
            format!("● 매크로 {slot} 기록 {count}/127")
        } else if let Some(prefix) = self.prefix {
            format!(
                "^{}",
                match prefix {
                    Prefix::K => 'K',
                    Prefix::Q => 'Q',
                    Prefix::O => 'O',
                }
            )
        } else {
            self.status.clone()
        };
        canvas.text(
            0,
            29,
            &format!(
                " {:>4}/{:<4} {:>3}  {} {} {} 마당{} {}  {}",
                line,
                d.lines.len(),
                col,
                indicator,
                dirty,
                if self.han_mode { "한글" } else { "영문" },
                self.current + 1,
                d.encoding.label(),
                status
            ),
            FG,
        );
        if matches!(self.mode, Mode::Normal | Mode::Drawing(_))
            && self.started.elapsed().as_millis() % 1000 < 550
        {
            let (cursor_line, cursor_col) = if let Mode::Drawing(draw) = &self.mode {
                (draw.line, draw.col)
            } else {
                (d.cursor.line, d.column())
            };
            let x = cursor_col.saturating_sub(d.left);
            let y = cursor_line.saturating_sub(d.top) + 3;
            if x < 80 && y <= 27 {
                canvas.rect(
                    x * 8,
                    y * 16 + 14,
                    if matches!(self.mode, Mode::Drawing(_)) {
                        16
                    } else {
                        8
                    },
                    2,
                    HI,
                );
            }
        }
        match &self.mode {
            Mode::Splash => {}
            Mode::Menu(kind, index) => self.draw_menu(canvas, *kind, *index),
            Mode::Prompt(kind, input) => self.draw_prompt(canvas, *kind, input),
            Mode::Help(top) => self.draw_help(canvas, *top),
            Mode::GraphicPicker(picker) => self.draw_picker(canvas, picker),
            Mode::StylePicker(_, selection) => self.draw_style_picker(canvas, *selection),
            Mode::Drawing(_) => {}
            Mode::ConfirmQuit => {
                canvas.box_outline(13, 11, 54, 7);
                canvas.text(16, 13, "저장하지 않은 문서가 있습니다.", HI);
                canvas.text(16, 15, "정말 종료합니까?  Y) 예   N) 아니오", FG);
            }
            Mode::Normal => {}
        }
    }

    fn draw_splash(&self, canvas: &mut Canvas) {
        canvas.clear();
        canvas.box_outline(21, 7, 37, 14);
        canvas.title_bitmap(22, 8);
        canvas.text(28, 16, "한글 에디터 산 1.2", HI);
        canvas.text(24, 18, "(C) 1991-93 박규현", FG);
        canvas.text(26, 19, "Rust 포트", DIM);
        canvas.text(26, 23, "아무 키나 누르세요", FG);
    }

    fn draw_menu(&self, canvas: &mut Canvas, kind: MenuKind, index: usize) {
        let items = menu_items(kind);
        let title = match kind {
            MenuKind::Main => "차림",
            MenuKind::File => "문서",
            MenuKind::Edit => "편집",
            MenuKind::Block => "블록",
            MenuKind::Find => "검색",
            MenuKind::Task => "작업",
        };
        let width = 31;
        let x = match kind {
            MenuKind::Main => 2,
            MenuKind::File => 2,
            MenuKind::Edit => 12,
            MenuKind::Block => 22,
            MenuKind::Find => 32,
            MenuKind::Task => 42,
        };
        let x = x.min(80 - width);
        let h = items.len() + 4;
        canvas.box_outline(x, 2, width, h);
        canvas.text(x + 2, 3, title, HI);
        for (n, (hotkey, label)) in items.iter().enumerate() {
            let row = n + 5;
            let text = format!("{hotkey}) {label}");
            if n == index {
                canvas.text_inverse(x + 1, row, width - 2, &text);
            } else {
                canvas.text(x + 2, row, &text, FG);
            }
        }
    }
    fn draw_prompt(&self, canvas: &mut Canvas, kind: Prompt, input: &str) {
        let title = match kind {
            Prompt::Open => "문서 읽기",
            Prompt::SaveAs => "새 이름으로 저장",
            Prompt::Find => "단어 검색",
            Prompt::ReplaceFind => "치환할 단어",
            Prompt::ReplaceWith => "바꿀 단어 (빈칸이면 삭제)",
        };
        canvas.box_outline(7, 10, 66, 8);
        canvas.text(10, 12, title, HI);
        let mut visible = Vec::new();
        let mut width = 0;
        for ch in input.chars().rev() {
            let glyph_width = font::char_width(ch);
            if width + glyph_width > 59 {
                break;
            }
            visible.push(ch);
            width += glyph_width;
        }
        visible.reverse();
        canvas.text(10, 14, &visible.into_iter().collect::<String>(), FG);
        canvas.rect((10 + width) * 8, 14 * 16 + 14, 8, 2, HI);
        canvas.rect(10 * 8, 15 * 16, 60 * 8, 1, FG);
        canvas.text(10, 16, "Enter 확인     Esc 취소", DIM);
    }
    fn draw_help(&self, canvas: &mut Canvas, top: usize) {
        canvas.box_outline(4, 2, 72, 26);
        canvas.text(7, 3, "도움말", HI);
        for (row, line) in HELP.iter().skip(top).take(21).enumerate() {
            canvas.text(7, row + 5, line, FG);
        }
    }

    fn draw_picker(&self, canvas: &mut Canvas, picker: &GraphicPicker) {
        const PAGES: [&str; 7] = [
            "그림/괘선",
            "도형/마크",
            "영문/한글",
            "로마/괘선",
            "기호/라틴",
            "라틴/일본",
            "일/러시아",
        ];
        canvas.box_outline(32, 3, 46, 16);
        for (page, label) in PAGES.iter().enumerate() {
            if page == picker.page {
                canvas.text_inverse(34, 4 + page, 9, label);
            } else {
                canvas.text(34, 4 + page, label, FG);
            }
        }
        for row in 0..picker.rows() {
            for col in 0..16 {
                let x = 44 + col * 2;
                let y = 4 + row;
                let selected = picker.row == row && picker.col == col;
                if selected {
                    canvas.cell_bg(x, y, 2, FG);
                }
                if let Some(ch) = graphics::page_cell(picker.page, row, col) {
                    canvas.glyph(x, y, ch, if selected { BG } else { FG });
                }
            }
        }
        canvas.text(
            44,
            17,
            &picker
                .sequence
                .iter()
                .rev()
                .take(16)
                .copied()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>(),
            FG,
        );
        canvas.cell_bg(0, 29, 80, BG);
        canvas.text(
            2,
            29,
            "화살표 선택  PgUp/Dn 묶음  +/Space 추가  Backspace 삭제  Enter 입력  Esc 닫기",
            FG,
        );
    }

    fn draw_style_picker(&self, canvas: &mut Canvas, selection: usize) {
        canvas.box_outline(27, 2, 51, 8);
        canvas.text(29, 3, "선 종류 선택  1~8", HI);
        for style in 0..8 {
            let x = 28 + style * 6;
            let selected = style == selection;
            for row in 0..3 {
                if selected {
                    canvas.cell_bg(x, 4 + row, 6, FG);
                }
                for col in 0..3 {
                    let ch = graphics::line_char(style, row * 3 + col);
                    canvas.glyph(x + col * 2, 4 + row, ch, if selected { BG } else { FG });
                }
            }
        }
        canvas.text(29, 8, "←→ 선택    Enter 확인    Esc 취소", DIM);
    }
}

fn menu_items(kind: MenuKind) -> &'static [(&'static str, &'static str)] {
    match kind {
        MenuKind::Main => MAIN,
        MenuKind::File => FILE,
        MenuKind::Edit => EDIT,
        MenuKind::Block => BLOCK,
        MenuKind::Find => FIND,
        MenuKind::Task => TASK,
    }
}
fn apply_to_string(input: &mut String, edit: HangulEdit) {
    match edit {
        HangulEdit::Insert(ch) => input.push(ch),
        HangulEdit::Replace(ch) => {
            input.pop();
            input.push(ch);
        }
        HangulEdit::Split(previous, next) => {
            input.pop();
            input.push(previous);
            input.push(next);
        }
        HangulEdit::Delete => {
            input.pop();
        }
    }
}
fn key_to_letter(key: Key) -> Option<char> {
    let code = key as u32;
    let a = Key::A as u32;
    let z = Key::Z as u32;
    if (a..=z).contains(&code) {
        char::from_u32('A' as u32 + code - a)
    } else {
        None
    }
}

fn is_printable_key(key: Key) -> bool {
    macros::slot_for_key(key).is_some()
        || ((Key::Key0 as u32)..=(Key::Key9 as u32)).contains(&(key as u32))
        || matches!(
            key,
            Key::Space
                | Key::Minus
                | Key::Equal
                | Key::LeftBracket
                | Key::RightBracket
                | Key::Backslash
                | Key::Semicolon
                | Key::Apostrophe
                | Key::Comma
                | Key::Period
                | Key::Slash
                | Key::Backquote
        )
}

pub fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let mut forced = None;
    let mut path = None;
    while let Some(arg) = args.next() {
        if arg == "--encoding" {
            let value = args
                .next()
                .ok_or("--encoding에는 utf8, euc-kr, johab 중 하나가 필요합니다.")?;
            forced = Some(
                Encoding::parse(&value.to_string_lossy())
                    .ok_or("인코딩은 utf8, euc-kr, johab 중 하나여야 합니다.")?,
            );
        } else if arg == "--help" || arg == "-h" {
            println!("사용법: san [--encoding utf8|euc-kr|johab] [파일]");
            return Ok(());
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err("파일은 하나만 지정할 수 있습니다.".into());
        }
    }
    let mut app = App::new(forced);
    if let Some(encoding) = forced {
        for doc in &mut app.docs {
            doc.encoding = encoding;
        }
    }
    if let Some(path) = path {
        app.open(path);
    }
    app.mode = Mode::Splash;
    let mut window = Window::new(
        "한글 에디터 산 1.2",
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale: Scale::FitScreen,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
    window.set_target_fps(30);
    let input = InputQueue::new(RefCell::new(Vec::new()));
    window.set_input_callback(Box::new(TextInput(input.clone())));
    let mut canvas = Canvas::new();
    while window.is_open() && !app.quit {
        app.held_macro_chord = app.held_macro_chord.filter(|key| window.is_key_down(*key));
        let was_splash = matches!(app.mode, Mode::Splash);
        let control = window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
        let alt = window.is_key_down(Key::LeftAlt)
            || window.is_key_down(Key::RightAlt)
            || window.is_key_down(Key::LeftSuper)
            || window.is_key_down(Key::RightSuper);
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        let mut suppress_space = false;
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            if key == Key::Space && shift && !control && !alt {
                suppress_space = true;
            }
            app.receive_key(key, control, alt, shift);
        }
        let chars: Vec<char> = input.borrow_mut().drain(..).collect();
        if !control && !alt && !was_splash {
            for ch in chars {
                if !(suppress_space && ch == ' ') {
                    app.receive_char(ch);
                }
            }
        }
        app.update_mouse(&window);
        app.draw(&mut canvas);
        window
            .update_with_buffer(&canvas.pixels, WIDTH, HEIGHT)
            .map_err(|e| e.to_string())?;
    }
    if !app.quit {
        for (task, doc) in app.docs.iter().enumerate() {
            if doc.dirty {
                match doc.write_recovery(task + 1) {
                    Ok(path) => eprintln!("SAN 복구 파일: {}", path.display()),
                    Err(error) => eprintln!("SAN 복구 파일을 저장하지 못했습니다: {error}"),
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_space_mode_composes_a_korean_document() {
        let mut app = App::new(None);
        app.toggle_han();
        for ch in "gksrmf".chars() {
            app.handle_char(ch);
        }
        assert_eq!(app.doc().lines[0].iter().collect::<String>(), "한글");
        app.handle_key(Key::Backspace, false, false);
        assert_eq!(app.doc().lines[0].iter().collect::<String>(), "한그");
        app.handle_char('f');
        assert_eq!(app.doc().lines[0].iter().collect::<String>(), "한글");
    }

    #[test]
    fn original_block_keys_copy_to_new_position() {
        let mut app = App::new(None);
        for ch in "산 editor".chars() {
            app.handle_char(ch);
        }
        app.doc_mut().cursor = Pos::default();
        app.prefix = Some(Prefix::K);
        app.handle_prefix('B');
        app.doc_mut().move_horizontal(1);
        app.prefix = Some(Prefix::K);
        app.handle_prefix('K');
        app.doc_mut().cursor.col = app.doc().lines[0].len();
        app.prefix = Some(Prefix::K);
        app.handle_prefix('C');
        app.prefix = Some(Prefix::K);
        app.handle_prefix('V');
        assert_eq!(app.doc().lines[0].iter().collect::<String>(), "산 editor산");
    }

    #[test]
    fn macro_replays_korean_and_restores_input_mode() {
        let mut app = App::new(None);
        app.receive_han_toggle();
        app.receive_key(Key::A, true, false, true);
        for ch in "gksrmf".chars() {
            app.receive_char(ch);
        }
        app.held_macro_chord = None;
        app.receive_key(Key::A, true, false, true);
        app.docs[0] = Document::empty();
        app.han_mode = false;
        app.held_macro_chord = None;
        app.receive_key(Key::A, false, true, true);
        assert_eq!(app.doc().lines[0].iter().collect::<String>(), "한글");
        assert!(!app.han_mode);
        assert_eq!(app.macros.active(), None);
    }

    #[test]
    fn picker_adds_original_graphics_and_drawing_connects_lines() {
        let mut app = App::new(None);
        app.handle_key(Key::F4, false, false);
        app.handle_char('+');
        app.handle_key(Key::Right, false, false);
        app.handle_char('+');
        app.handle_key(Key::Enter, false, false);
        assert!(matches!(app.mode, Mode::Normal));
        let text = app.doc().lines[0].iter().collect::<String>();
        assert_eq!(text.chars().count(), 2);
        assert_eq!(encode_johab_for_test(&text), vec![0xd4, 0x81, 0xd4, 0x82]);

        app.docs[0] = Document::empty();
        app.handle_key(Key::D, false, true);
        app.handle_char('D');
        app.handle_key(Key::Right, false, false);
        app.handle_key(Key::Down, false, false);
        assert_eq!(app.doc().char_at_visual(0, 0), Some('─'));
        assert_eq!(app.doc().char_at_visual(0, 2), Some('┐'));

        app.handle_char('L');
        assert!(matches!(app.mode, Mode::StylePicker(_, _)));
        app.handle_char('8');
        assert!(matches!(app.mode, Mode::Drawing(_)));
        if let Mode::Drawing(draw) = &app.mode {
            assert_eq!(draw.style, 7);
            assert_eq!(draw.action, DrawAction::Draw);
        }
        app.handle_key(Key::F4, false, false);
        app.handle_key(Key::Enter, false, false);
        if let Mode::Drawing(draw) = &app.mode {
            assert_eq!(draw.action, DrawAction::Graphic);
        }
    }

    fn encode_johab_for_test(text: &str) -> Vec<u8> {
        text.chars()
            .flat_map(|ch| graphics::encode_pair(ch).unwrap())
            .collect()
    }
}
