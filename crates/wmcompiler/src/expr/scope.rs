use std::collections::BTreeMap;

use super::{Result, unsupported_expression};

#[derive(Clone, Debug, Default)]
pub(super) struct LocalScope {
    slots: BTreeMap<String, u8>,
    next_slot: u8,
    max_slot: u8,
}

impl LocalScope {
    pub(super) fn new(initial_locals: &[String]) -> Result<Self> {
        let mut scope = Self::default();
        for name in initial_locals {
            scope.declare(name.clone())?;
        }
        Ok(scope)
    }

    pub(super) fn lookup(&self, name: &str) -> Result<u8> {
        self.slots
            .get(name)
            .copied()
            .ok_or_else(|| unsupported_expression(format!("unknown local `{name}`")))
    }

    pub(super) fn declare(&mut self, name: String) -> Result<u8> {
        if self.slots.contains_key(&name) {
            return Err(unsupported_expression(format!("duplicate local `{name}`")));
        }
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or_else(|| unsupported_expression("too many local variables"))?;
        self.max_slot = self.max_slot.max(self.next_slot);
        self.slots.insert(name, slot);
        Ok(slot)
    }

    pub(super) fn local_count(&self) -> usize {
        self.max_slot as usize
    }

    pub(super) fn merge_max(&mut self, other_local_count: usize) {
        self.max_slot = self
            .max_slot
            .max(other_local_count.min(u8::MAX as usize) as u8);
    }
}
