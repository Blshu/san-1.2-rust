//! Two-column form and line drawing, based on LINE.C's eight line styles.

use crate::{document::Document, graphics};
use std::collections::HashMap;

const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const UP: u8 = 4;
const DOWN: u8 = 8;
const SHAPE_PORTS: [u8; 11] = [
    DOWN | RIGHT,
    LEFT | RIGHT | DOWN,
    DOWN | LEFT,
    UP | DOWN | RIGHT,
    15,
    UP | DOWN | LEFT,
    UP | RIGHT,
    UP | LEFT | RIGHT,
    UP | LEFT,
    LEFT | RIGHT,
    UP | DOWN,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawAction {
    Move,
    Draw,
    Erase,
    Graphic,
}
impl DrawAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "이동",
            Self::Draw => "선",
            Self::Erase => "지움",
            Self::Graphic => "그림",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
impl Direction {
    fn port(self) -> u8 {
        match self {
            Self::Left => LEFT,
            Self::Right => RIGHT,
            Self::Up => UP,
            Self::Down => DOWN,
        }
    }
    fn opposite(self) -> u8 {
        match self {
            Self::Left => RIGHT,
            Self::Right => LEFT,
            Self::Up => DOWN,
            Self::Down => UP,
        }
    }
}

#[derive(Clone)]
pub struct DrawState {
    pub line: usize,
    pub col: usize,
    pub style: usize,
    pub action: DrawAction,
    pub last_action: DrawAction,
    pub graphic: char,
    connections: HashMap<(usize, usize), u8>,
}

impl DrawState {
    pub fn new(doc: &Document) -> Self {
        Self {
            line: doc.cursor.line,
            col: doc.column() / 2 * 2,
            style: 0,
            action: DrawAction::Move,
            last_action: DrawAction::Draw,
            graphic: graphics::page_cell(0, 0, 1).unwrap_or('┌'),
            connections: HashMap::new(),
        }
    }
    pub fn set_action(&mut self, action: DrawAction) {
        self.action = action;
        if action != DrawAction::Move {
            self.last_action = action;
        }
    }
    pub fn toggle_action(&mut self) {
        self.action = if self.action == DrawAction::Move {
            self.last_action
        } else {
            DrawAction::Move
        };
    }
    pub fn stamp(&self, doc: &mut Document) {
        if self.action == DrawAction::Graphic {
            doc.put_visual(self.line, self.col, Some(self.graphic));
        }
    }
    pub fn erase_current(&mut self, doc: &mut Document) {
        erase(doc, (self.line, self.col), &mut self.connections);
    }
    pub fn step(&mut self, doc: &mut Document, direction: Direction) {
        let old = (self.line, self.col);
        let next = match direction {
            Direction::Left => (self.line, self.col.saturating_sub(2)),
            Direction::Right => (self.line, (self.col + 2).min(252)),
            Direction::Up => (self.line.saturating_sub(1), self.col),
            Direction::Down => (self.line + 1, self.col),
        };
        if next == old {
            return;
        }
        if matches!(self.action, DrawAction::Move | DrawAction::Erase) && next.0 >= doc.lines.len()
        {
            return;
        }
        if self.action == DrawAction::Draw {
            self.connect(doc, old, direction.port());
            self.connect(doc, next, direction.opposite());
        } else if self.action == DrawAction::Erase {
            erase(doc, next, &mut self.connections);
        } else if self.action == DrawAction::Graphic {
            doc.put_visual(next.0, next.1, Some(self.graphic));
        }
        self.line = next.0;
        self.col = next.1;
        if self.line < doc.lines.len() {
            doc.cursor.line = self.line;
            doc.set_visual_col(self.col);
        }
    }
    fn connect(&mut self, doc: &mut Document, position: (usize, usize), port: u8) {
        let previous = self
            .connections
            .get(&position)
            .copied()
            .unwrap_or_else(|| existing_ports(doc, position.0, position.1));
        let ports = previous | port;
        self.connections.insert(position, ports);
        if let Some(shape) = shape_for_ports(ports) {
            doc.put_visual(
                position.0,
                position.1,
                Some(graphics::line_char(self.style, shape)),
            );
        }
    }
}

fn shape_for_ports(ports: u8) -> Option<usize> {
    Some(match ports {
        0 => return None,
        15 => 4,
        11 => 1,
        14 => 3,
        13 => 5,
        7 => 7,
        10 => 0,
        9 => 2,
        6 => 6,
        5 => 8,
        1..=3 => 9,
        4 | 8 | 12 => 10,
        _ => return None,
    })
}

fn existing_ports(doc: &Document, line: usize, col: usize) -> u8 {
    doc.char_at_visual(line, col)
        .and_then(graphics::line_shape)
        .map(|(_, shape)| SHAPE_PORTS[shape])
        .unwrap_or(0)
}

fn erase(
    doc: &mut Document,
    position: (usize, usize),
    connections: &mut HashMap<(usize, usize), u8>,
) {
    if !doc
        .char_at_visual(position.0, position.1)
        .is_some_and(graphics::is_graphic)
    {
        return;
    }
    doc.put_visual(position.0, position.1, None);
    connections.remove(&position);
    let neighbors = [
        (position.0, position.1.saturating_sub(2), RIGHT),
        (position.0, position.1 + 2, LEFT),
        (position.0.saturating_sub(1), position.1, DOWN),
        (position.0 + 1, position.1, UP),
    ];
    for (line, col, port) in neighbors {
        if let Some(ch) = doc.char_at_visual(line, col) {
            if let Some((style, shape)) = graphics::line_shape(ch) {
                let remaining = connections
                    .get(&(line, col))
                    .copied()
                    .unwrap_or(SHAPE_PORTS[shape])
                    & !port;
                connections.insert((line, col), remaining);
                doc.put_visual(
                    line,
                    col,
                    shape_for_ports(remaining).map(|shape| graphics::line_char(style, shape)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draws_corner_and_junction_with_original_glyphs() {
        let mut doc = Document::empty();
        let mut draw = DrawState::new(&doc);
        draw.set_action(DrawAction::Draw);
        draw.step(&mut doc, Direction::Right);
        draw.step(&mut doc, Direction::Down);
        assert_eq!(doc.char_at_visual(0, 0), Some('─'));
        assert_eq!(doc.char_at_visual(0, 2), Some('┐'));
        assert_eq!(doc.char_at_visual(1, 2), Some('│'));
    }
}
