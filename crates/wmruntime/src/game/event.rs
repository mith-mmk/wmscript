use std::collections::VecDeque;
use wmvm::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct GameEvent {
    pub sequence: u64,
    pub tick: u64,
    pub name: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventQueue {
    next_sequence: u64,
    events: VecDeque<GameEvent>,
}

impl EventQueue {
    pub fn emit(&mut self, tick: u64, name: impl Into<String>, payload: Value) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.events.push_back(GameEvent {
            sequence,
            tick,
            name: name.into(),
            payload,
        });
        sequence
    }
    pub fn pop(&mut self) -> Option<GameEvent> {
        self.events.pop_front()
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
