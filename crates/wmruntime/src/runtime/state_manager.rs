use std::collections::BTreeMap;

use wmvm::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct StateManager {
    current: BTreeMap<String, Value>,
    slots: BTreeMap<u32, BTreeMap<String, Value>>,
}

impl StateManager {
    pub(super) fn save(&mut self, slot: u32) {
        self.slots.insert(slot, self.current.clone());
    }

    pub(super) fn load(&mut self, slot: u32) -> bool {
        if let Some(saved) = self.slots.get(&slot).cloned() {
            self.current = saved;
            true
        } else {
            false
        }
    }

    pub(super) fn has(&self, key: &str) -> bool {
        self.current.contains_key(key)
    }

    pub(super) fn get(&self, key: &str) -> Option<Value> {
        self.current.get(key).cloned()
    }

    pub(super) fn set(&mut self, key: String, value: Value) {
        self.current.insert(key, value);
    }

    pub(super) fn erase(&mut self, key: &str) -> bool {
        self.current.remove(key).is_some()
    }
}
