//! Original 26-slot, 127-keystroke macro recorder.

use minifb::Key;

#[derive(Clone, Debug)]
pub enum Event {
    Key(Key, bool, bool),
    Char(char),
    ToggleHan,
}

#[derive(Clone, Default)]
pub struct Macro {
    pub events: Vec<Event>,
    pub han_mode: bool,
}

pub struct Recorder {
    slots: [Macro; 26],
    active: Option<usize>,
    backup: Option<Macro>,
    playing: bool,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| Macro::default()),
            active: None,
            backup: None,
            playing: false,
        }
    }

    pub fn active(&self) -> Option<(char, usize)> {
        self.active
            .map(|index| ((b'A' + index as u8) as char, self.slots[index].events.len()))
    }

    pub fn toggle_recording(&mut self, index: usize, han_mode: bool) -> bool {
        if self.active == Some(index) {
            self.active = None;
            self.backup = None;
            return false;
        }
        self.active = Some(index);
        self.backup = Some(self.slots[index].clone());
        self.slots[index] = Macro {
            events: Vec::new(),
            han_mode,
        };
        true
    }

    pub fn cancel_empty(&mut self) -> bool {
        let Some(index) = self.active else {
            return false;
        };
        if !self.slots[index].events.is_empty() {
            return false;
        }
        if let Some(previous) = self.backup.take() {
            self.slots[index] = previous;
        }
        self.active = None;
        true
    }

    pub fn record(&mut self, event: Event) {
        if let Some(index) = self.active {
            let events = &mut self.slots[index].events;
            if events.len() < 127 {
                events.push(event);
            }
            if events.len() == 127 {
                self.active = None;
                self.backup = None;
            }
        }
    }

    pub fn playback(&mut self, index: usize) -> Option<Macro> {
        if self.playing || self.slots[index].events.is_empty() {
            return None;
        }
        self.playing = true;
        Some(self.slots[index].clone())
    }

    pub fn finish_playback(&mut self) {
        self.playing = false;
    }
}

pub fn slot_for_key(key: Key) -> Option<usize> {
    let code = key as u32;
    let a = Key::A as u32;
    let z = Key::Z as u32;
    (a..=z).contains(&code).then_some((code - a) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_up_to_127_events_and_restores_cancelled_slot() {
        let mut recorder = Recorder::new();
        assert!(recorder.toggle_recording(0, true));
        recorder.record(Event::Char('x'));
        assert!(!recorder.toggle_recording(0, true));
        assert!(recorder.toggle_recording(0, false));
        assert!(recorder.cancel_empty());
        let old = recorder.playback(0).unwrap();
        assert_eq!(old.events.len(), 1);
        assert!(old.han_mode);
        recorder.finish_playback();

        assert!(recorder.toggle_recording(1, false));
        for _ in 0..128 {
            recorder.record(Event::Char('z'));
        }
        assert_eq!(recorder.active(), None);
        assert_eq!(recorder.playback(1).unwrap().events.len(), 127);
    }
}
