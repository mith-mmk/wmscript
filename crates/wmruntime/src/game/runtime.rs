use super::*;
use wmvm::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub tick_hz: u32,
    pub seed: u64,
    pub max_events_per_tick: usize,
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            tick_hz: 60,
            seed: 1,
            max_events_per_tick: 4096,
        }
    }
}

pub trait GameSystem {
    fn name(&self) -> &str;
    fn handle(
        &mut self,
        world: &mut World,
        event: &GameEvent,
        commands: &mut Commands<'_>,
    ) -> Result<(), String>;
}

pub struct Commands<'a> {
    tick: u64,
    queue: &'a mut EventQueue,
}
impl Commands<'_> {
    pub fn emit(&mut self, name: impl Into<String>, payload: Value) {
        self.queue.emit(self.tick, name, payload);
    }
}

pub struct GameRuntime {
    config: RuntimeConfig,
    world: World,
    events: EventQueue,
    tick: u64,
    rng: DeterministicRng,
    systems: Vec<Box<dyn GameSystem>>,
    input: Box<dyn InputPort>,
    render: Box<dyn RenderPort>,
    audio: Box<dyn AudioPort>,
    storage: Box<dyn StoragePort>,
    last_events: Vec<GameEvent>,
}

impl GameRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self::with_ports(
            config,
            Box::<HeadlessInput>::default(),
            Box::<NullRender>::default(),
            Box::<NullAudio>::default(),
            Box::<MemoryStorage>::default(),
        )
    }
    pub fn with_ports(
        config: RuntimeConfig,
        input: Box<dyn InputPort>,
        render: Box<dyn RenderPort>,
        audio: Box<dyn AudioPort>,
        storage: Box<dyn StoragePort>,
    ) -> Self {
        Self {
            config,
            world: World::new(),
            events: EventQueue::default(),
            tick: 0,
            rng: DeterministicRng::new(config.seed),
            systems: Vec::new(),
            input,
            render,
            audio,
            storage,
            last_events: Vec::new(),
        }
    }
    pub const fn config(&self) -> RuntimeConfig {
        self.config
    }
    pub const fn tick_index(&self) -> u64 {
        self.tick
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
    pub fn register_system(&mut self, system: Box<dyn GameSystem>) -> Result<(), RuntimeError> {
        if self
            .systems
            .iter()
            .any(|existing| existing.name() == system.name())
        {
            return Err(RuntimeError::DuplicateSystem(system.name().to_owned()));
        }
        self.systems.push(system);
        self.systems
            .sort_by(|left, right| left.name().cmp(right.name()));
        Ok(())
    }
    pub fn emit(&mut self, name: impl Into<String>, payload: Value) {
        self.events.emit(self.tick, name, payload);
    }
    pub fn random_int(&mut self, min: i64, max: i64) -> i64 {
        self.rng.range(min, max)
    }
    pub fn audio(&mut self) -> &mut dyn AudioPort {
        &mut *self.audio
    }

    pub fn tick(&mut self) -> Result<RenderFrame, RuntimeError> {
        for (name, payload) in self.input.poll(self.tick) {
            self.events.emit(self.tick, name, payload);
        }
        self.events
            .emit(self.tick, "game.tick", Value::Integer(self.tick as i64));
        self.last_events.clear();
        let mut processed = 0usize;
        while let Some(event) = self.events.pop() {
            processed += 1;
            if processed > self.config.max_events_per_tick {
                return Err(RuntimeError::EventLimit(self.config.max_events_per_tick));
            }
            for system in &mut self.systems {
                let mut commands = Commands {
                    tick: self.tick,
                    queue: &mut self.events,
                };
                system
                    .handle(&mut self.world, &event, &mut commands)
                    .map_err(|message| RuntimeError::System {
                        system: system.name().to_owned(),
                        message,
                    })?;
            }
            self.last_events.push(event);
        }
        let frame = RenderFrame {
            tick: self.tick,
            events: self.last_events.clone(),
        };
        self.render.render(&frame);
        self.tick = self.tick.saturating_add(1);
        Ok(frame)
    }

    pub fn save(&mut self, slot: u32) -> Result<(), RuntimeError> {
        self.storage
            .store(slot, self.world.persistent_snapshot())
            .map_err(RuntimeError::Storage)
    }
    pub fn load(&mut self, slot: u32) -> Result<bool, RuntimeError> {
        let Some(snapshot) = self.storage.load(slot).map_err(RuntimeError::Storage)? else {
            return Ok(false);
        };
        self.world.restore_persistent(snapshot)?;
        Ok(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    DuplicateSystem(String),
    EventLimit(usize),
    System { system: String, message: String },
    Storage(String),
    World(WorldError),
}
impl From<WorldError> for RuntimeError {
    fn from(value: WorldError) -> Self {
        Self::World(value)
    }
}
impl core::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateSystem(name) => write!(f, "duplicate system: {name}"),
            Self::EventLimit(limit) => write!(f, "event limit exceeded: {limit}"),
            Self::System { system, message } => write!(f, "system `{system}` failed: {message}"),
            Self::Storage(message) => write!(f, "storage failed: {message}"),
            Self::World(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RuntimeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeterministicRng {
    state: u64,
}
impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
        }
    }
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn range(&mut self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        min + (self.next() % (max - min) as u64) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Counter;
    impl GameSystem for Counter {
        fn name(&self) -> &str {
            "counter"
        }
        fn handle(
            &mut self,
            world: &mut World,
            event: &GameEvent,
            _commands: &mut Commands<'_>,
        ) -> Result<(), String> {
            if event.name == "game.tick" {
                let value = match world.resource("Ticks") {
                    Some(Value::Integer(v)) => *v,
                    _ => 0,
                };
                world
                    .set_resource("Ticks", Value::Integer(value + 1))
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    }

    #[test]
    fn fixed_tick_and_save_are_deterministic() {
        let mut runtime = GameRuntime::new(RuntimeConfig {
            seed: 7,
            ..RuntimeConfig::default()
        });
        runtime
            .world_mut()
            .register_resource("Ticks", Value::Integer(0), true);
        runtime.register_system(Box::new(Counter)).unwrap();
        runtime.tick().unwrap();
        runtime.save(1).unwrap();
        runtime.tick().unwrap();
        assert_eq!(runtime.world().resource("Ticks"), Some(&Value::Integer(2)));
        runtime.load(1).unwrap();
        assert_eq!(runtime.world().resource("Ticks"), Some(&Value::Integer(1)));
    }

    #[test]
    fn seeded_rng_replays() {
        let mut a = GameRuntime::new(RuntimeConfig {
            seed: 42,
            ..RuntimeConfig::default()
        });
        let mut b = GameRuntime::new(RuntimeConfig {
            seed: 42,
            ..RuntimeConfig::default()
        });
        assert_eq!(
            (0..8).map(|_| a.random_int(0, 100)).collect::<Vec<_>>(),
            (0..8).map(|_| b.random_int(0, 100)).collect::<Vec<_>>()
        );
    }
}
