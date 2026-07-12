use super::{GameEvent, WorldSnapshot};
use std::collections::{BTreeMap, VecDeque};
use wmvm::Value;

pub trait InputPort {
    fn poll(&mut self, tick: u64) -> Vec<(String, Value)>;
}
pub trait RenderPort {
    fn render(&mut self, frame: &RenderFrame);
}
pub trait AudioPort {
    fn play(&mut self, asset: &str, volume: f32);
}
pub trait StoragePort {
    fn store(&mut self, slot: u32, snapshot: WorldSnapshot) -> Result<(), String>;
    fn load(&mut self, slot: u32) -> Result<Option<WorldSnapshot>, String>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderFrame {
    pub tick: u64,
    pub events: Vec<GameEvent>,
}

#[derive(Default)]
pub struct HeadlessInput {
    queued: VecDeque<(u64, String, Value)>,
}
impl HeadlessInput {
    pub fn push(&mut self, tick: u64, name: impl Into<String>, payload: Value) {
        self.queued.push_back((tick, name.into(), payload));
    }
}
impl InputPort for HeadlessInput {
    fn poll(&mut self, tick: u64) -> Vec<(String, Value)> {
        let mut result = Vec::new();
        while self.queued.front().is_some_and(|event| event.0 <= tick) {
            let (_, name, value) = self.queued.pop_front().unwrap();
            result.push((name, value));
        }
        result
    }
}

#[derive(Default)]
pub struct NullRender;
impl RenderPort for NullRender {
    fn render(&mut self, _frame: &RenderFrame) {}
}
#[derive(Default)]
pub struct NullAudio;
impl AudioPort for NullAudio {
    fn play(&mut self, _asset: &str, _volume: f32) {}
}

#[derive(Default)]
pub struct MemoryStorage {
    slots: BTreeMap<u32, WorldSnapshot>,
}
impl StoragePort for MemoryStorage {
    fn store(&mut self, slot: u32, snapshot: WorldSnapshot) -> Result<(), String> {
        self.slots.insert(slot, snapshot);
        Ok(())
    }
    fn load(&mut self, slot: u32) -> Result<Option<WorldSnapshot>, String> {
        Ok(self.slots.get(&slot).cloned())
    }
}
