use std::collections::{BTreeMap, BTreeSet};

use wmvm::Value;

pub type EntityId = u64;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldSnapshot {
    pub next_entity: EntityId,
    pub entities: BTreeSet<EntityId>,
    pub components: BTreeMap<String, BTreeMap<EntityId, Value>>,
    pub resources: BTreeMap<String, Value>,
}

/// Deterministic entity/component/resource storage.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    snapshot: WorldSnapshot,
    persistent_components: BTreeSet<String>,
    persistent_resources: BTreeSet<String>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            snapshot: WorldSnapshot {
                next_entity: 1,
                ..WorldSnapshot::default()
            },
            persistent_components: BTreeSet::new(),
            persistent_resources: BTreeSet::new(),
        }
    }

    pub fn register_component(&mut self, name: impl Into<String>, persistent: bool) {
        let name = name.into();
        self.snapshot.components.entry(name.clone()).or_default();
        if persistent {
            self.persistent_components.insert(name);
        }
    }

    pub fn register_resource(&mut self, name: impl Into<String>, value: Value, persistent: bool) {
        let name = name.into();
        self.snapshot.resources.insert(name.clone(), value);
        if persistent {
            self.persistent_resources.insert(name);
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        let entity = self.snapshot.next_entity;
        self.snapshot.next_entity = self.snapshot.next_entity.saturating_add(1);
        self.snapshot.entities.insert(entity);
        entity
    }

    pub fn despawn(&mut self, entity: EntityId) -> bool {
        if !self.snapshot.entities.remove(&entity) {
            return false;
        }
        for store in self.snapshot.components.values_mut() {
            store.remove(&entity);
        }
        true
    }

    pub fn set_component(
        &mut self,
        entity: EntityId,
        component: &str,
        value: Value,
    ) -> Result<(), WorldError> {
        if !self.snapshot.entities.contains(&entity) {
            return Err(WorldError::UnknownEntity(entity));
        }
        let store = self
            .snapshot
            .components
            .get_mut(component)
            .ok_or_else(|| WorldError::UnknownSchema(component.to_owned()))?;
        store.insert(entity, value);
        Ok(())
    }

    pub fn component(&self, entity: EntityId, component: &str) -> Option<&Value> {
        self.snapshot.components.get(component)?.get(&entity)
    }
    pub fn remove_component(&mut self, entity: EntityId, component: &str) -> Option<Value> {
        self.snapshot.components.get_mut(component)?.remove(&entity)
    }
    pub fn resource(&self, name: &str) -> Option<&Value> {
        self.snapshot.resources.get(name)
    }
    pub fn set_resource(&mut self, name: &str, value: Value) -> Result<(), WorldError> {
        let resource = self
            .snapshot
            .resources
            .get_mut(name)
            .ok_or_else(|| WorldError::UnknownSchema(name.to_owned()))?;
        *resource = value;
        Ok(())
    }

    /// Returns matching entities in ascending EntityId order.
    pub fn query<'a>(&self, components: impl IntoIterator<Item = &'a str>) -> Vec<EntityId> {
        let names = components.into_iter().collect::<Vec<_>>();
        self.snapshot
            .entities
            .iter()
            .copied()
            .filter(|entity| {
                names.iter().all(|name| {
                    self.snapshot
                        .components
                        .get(*name)
                        .is_some_and(|store| store.contains_key(entity))
                })
            })
            .collect()
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        self.snapshot.clone()
    }

    pub fn persistent_snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            next_entity: self.snapshot.next_entity,
            entities: self.snapshot.entities.clone(),
            components: self
                .snapshot
                .components
                .iter()
                .filter(|(name, _)| self.persistent_components.contains(*name))
                .map(|(name, values)| (name.clone(), values.clone()))
                .collect(),
            resources: self
                .snapshot
                .resources
                .iter()
                .filter(|(name, _)| self.persistent_resources.contains(*name))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        }
    }

    pub fn restore_persistent(&mut self, saved: WorldSnapshot) -> Result<(), WorldError> {
        if saved
            .components
            .keys()
            .any(|name| !self.persistent_components.contains(name))
            || saved
                .resources
                .keys()
                .any(|name| !self.persistent_resources.contains(name))
        {
            return Err(WorldError::IncompatibleSnapshot);
        }
        self.snapshot.next_entity = saved.next_entity;
        self.snapshot.entities = saved.entities;
        for name in &self.persistent_components {
            self.snapshot.components.insert(
                name.clone(),
                saved.components.get(name).cloned().unwrap_or_default(),
            );
        }
        for name in &self.persistent_resources {
            if let Some(value) = saved.resources.get(name) {
                self.snapshot.resources.insert(name.clone(), value.clone());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldError {
    UnknownEntity(EntityId),
    UnknownSchema(String),
    IncompatibleSnapshot,
}

impl core::fmt::Display for WorldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownEntity(id) => write!(f, "unknown entity: {id}"),
            Self::UnknownSchema(name) => write!(f, "unknown world schema: {name}"),
            Self::IncompatibleSnapshot => {
                f.write_str("snapshot does not match the persistent schema")
            }
        }
    }
}
impl std::error::Error for WorldError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_entity_id_ordered() {
        let mut world = World::new();
        world.register_component("Position", true);
        world.register_component("Health", true);
        let first = world.spawn();
        let second = world.spawn();
        world
            .set_component(second, "Position", Value::Integer(2))
            .unwrap();
        world
            .set_component(second, "Health", Value::Integer(2))
            .unwrap();
        world
            .set_component(first, "Position", Value::Integer(1))
            .unwrap();
        world
            .set_component(first, "Health", Value::Integer(1))
            .unwrap();
        assert_eq!(world.query(["Position", "Health"]), vec![first, second]);
    }

    #[test]
    fn persistent_snapshot_excludes_transient_state() {
        let mut world = World::new();
        world.register_resource("Score", Value::Integer(1), true);
        world.register_resource("Cursor", Value::Integer(2), false);
        let saved = world.persistent_snapshot();
        assert!(saved.resources.contains_key("Score"));
        assert!(!saved.resources.contains_key("Cursor"));
    }
}
