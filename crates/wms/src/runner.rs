use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use wmbytecode::{Op, encode_op};
use wmcompiler::v2::standard::{host_id, resolve_host};
use wmcompiler::v2::{CompileOutput, RecordKind, SchemaType, SystemEntry};
use wmhost::{HostFunction, HostRegistry};
use wmplatform::PlatformProfile;
use wmruntime::game::{World, WorldSnapshot};
use wmvm::{Function, HostApi, HostError, Message, Program, RunOutcome, Value, Vm, VmConfig};

use crate::Target;

#[derive(Clone, Debug, PartialEq)]
pub struct RunReport {
    pub rounds: usize,
    pub value: Option<Value>,
    pub host_calls: Vec<u16>,
    pub messages: Vec<String>,
    pub world: WorldSnapshot,
    pub events_processed: usize,
}

pub fn run(program: Program, target: Target, inputs: Vec<String>) -> Result<RunReport, String> {
    run_program(program, target, inputs, &[], 1)
}

pub fn run_compiled(
    output: &CompileOutput,
    target: Target,
    inputs: Vec<String>,
    seed: u64,
) -> Result<RunReport, String> {
    run_program_with_systems(
        output.program.clone(),
        target,
        inputs,
        &output.schema,
        seed,
        &output.systems,
    )
}

pub fn run_program_with_systems(
    program: Program,
    target: Target,
    inputs: Vec<String>,
    schema: &[SchemaType],
    seed: u64,
    systems: &[SystemEntry],
) -> Result<RunReport, String> {
    let shared = SharedHostState::new(schema, seed);
    let profile = target_profile(target);
    let first = execute(program.clone(), profile, shared.clone(), inputs)?;
    let mut rounds = first.rounds;
    let mut events_processed = 0usize;
    while let Some((event_type, payload)) = shared.events.borrow_mut().pop_front() {
        events_processed += 1;
        if events_processed > 4096 {
            return Err("script event limit exceeded".to_owned());
        }
        for system in systems
            .iter()
            .filter(|system| system.event_type.as_deref() == Some(event_type.as_str()))
        {
            let call = call_entry(program.clone(), system.function_id, vec![payload.clone()])?;
            rounds += execute(call, profile, shared.clone(), Vec::new())?.rounds;
        }
    }
    finish_report(target, shared, rounds, first.value, events_processed)
}

pub fn run_program(
    program: Program,
    target: Target,
    inputs: Vec<String>,
    schema: &[SchemaType],
    seed: u64,
) -> Result<RunReport, String> {
    let profile = target_profile(target);
    let shared = SharedHostState::new(schema, seed);
    let execution = execute(program, profile, shared.clone(), inputs)?;
    finish_report(target, shared, execution.rounds, execution.value, 0)
}

fn execute(
    program: Program,
    profile: PlatformProfile,
    shared: SharedHostState,
    inputs: Vec<String>,
) -> Result<Execution, String> {
    let registry = standard_registry(profile);
    let host = StandardHost { shared };
    let mut vm =
        Vm::with_program_and_host_api(VmConfig::new(profile, registry, 10_000), program, host);
    let mut inputs = inputs
        .into_iter()
        .map(Value::String)
        .collect::<VecDeque<_>>();
    for round in 1..=10_000 {
        match vm.run_frame(10_000) {
            RunOutcome::Halted { value, .. } => {
                return Ok(Execution {
                    rounds: round,
                    value,
                });
            }
            RunOutcome::WaitingMessage { .. } => {
                let value = inputs
                    .pop_front()
                    .unwrap_or_else(|| Value::String("default".to_owned()));
                vm.push_message(Message::new(0, vm.worker_id(), 0, value));
                vm.wake();
            }
            RunOutcome::Sleeping { .. } => vm.wake(),
            RunOutcome::Yielded { .. } | RunOutcome::StepLimitReached { .. } => {}
            RunOutcome::Error { error, .. } => return Err(error.to_string()),
        }
    }
    Err("runtime round limit exceeded".to_owned())
}

fn finish_report(
    target: Target,
    shared: SharedHostState,
    rounds: usize,
    value: Option<Value>,
    events_processed: usize,
) -> Result<RunReport, String> {
    let report = RunReport {
        rounds,
        value,
        host_calls: shared.calls.borrow().clone(),
        messages: shared.messages.borrow().clone(),
        world: shared.world.borrow().snapshot(),
        events_processed,
    };
    if target == Target::Egui {
        wmfrontend::v2_adapter::show_report("WMScript v2", report.messages.clone())?;
    }
    Ok(report)
}

fn target_profile(target: Target) -> PlatformProfile {
    match target {
        Target::Headless => PlatformProfile::native(),
        Target::Egui => PlatformProfile::egui(),
    }
}

fn call_entry(mut program: Program, function_id: u16, args: Vec<Value>) -> Result<Program, String> {
    let wrapper = program
        .function_ids()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "function id overflow".to_owned())?;
    let argc = u8::try_from(args.len()).map_err(|_| "argument count overflow".to_owned())?;
    let mut code = Vec::new();
    for value in args {
        let constant = program.push_constant(value);
        encode_op(&Op::PushConst(constant), &mut code);
    }
    encode_op(&Op::Call(function_id, argc), &mut code);
    encode_op(&Op::Return, &mut code);
    program.insert_function(Function::new(wrapper, code, 0, 0));
    program.set_entry(wrapper);
    Ok(program)
}

struct Execution {
    rounds: usize,
    value: Option<Value>,
}

#[derive(Clone)]
struct SharedHostState {
    calls: Rc<RefCell<Vec<u16>>>,
    messages: Rc<RefCell<Vec<String>>>,
    world: Rc<RefCell<World>>,
    saves: Rc<RefCell<BTreeMap<u32, WorldSnapshot>>>,
    events: Rc<RefCell<VecDeque<(String, Value)>>>,
    random_state: Rc<Cell<u64>>,
}

impl SharedHostState {
    fn new(schema: &[SchemaType], seed: u64) -> Self {
        let mut world = World::new();
        for item in schema {
            match item.kind {
                RecordKind::Component => world.register_component(&item.name, item.persistent),
                RecordKind::Resource => {
                    world.register_resource(&item.name, Value::Nil, item.persistent)
                }
                RecordKind::Struct | RecordKind::Event => {}
            }
        }
        Self {
            calls: Rc::new(RefCell::new(Vec::new())),
            messages: Rc::new(RefCell::new(Vec::new())),
            world: Rc::new(RefCell::new(world)),
            saves: Rc::new(RefCell::new(BTreeMap::new())),
            events: Rc::new(RefCell::new(VecDeque::new())),
            random_state: Rc::new(Cell::new(if seed == 0 { 1 } else { seed })),
        }
    }
}

fn standard_registry(profile: PlatformProfile) -> HostRegistry {
    let mut registry = HostRegistry::new(profile);
    for (path, min, max) in [
        ("core.len", 1, 1),
        ("core.assert", 1, 2),
        ("ui.say", 2, 2),
        ("ui.choice", 1, 1),
        ("input.choice", 1, 1),
        ("input.text", 1, 1),
        ("time.sleep", 1, 1),
        ("time.tick", 0, 0),
        ("random.int", 2, 2),
        ("world.spawn", 0, 0),
        ("world.get", 2, 2),
        ("world.set", 3, 3),
        ("world.emit", 1, 1),
        ("save.store", 1, 1),
        ("save.load", 1, 1),
        ("asset.load", 1, 1),
        ("audio.play", 1, 2),
        ("scene.set", 1, 1),
    ] {
        registry.register(HostFunction::new(resolve_host(path).unwrap(), min, max, 0));
    }
    registry.register(HostFunction::new(host_id::CORE_SET_FIELD, 3, 3, 0));
    registry.register(HostFunction::new(host_id::CORE_SET_INDEX, 3, 3, 0));
    registry
}

struct StandardHost {
    shared: SharedHostState,
}
impl HostApi for StandardHost {
    fn call_host(&mut self, host: u16, args: &[Value]) -> Result<Value, HostError> {
        self.shared.calls.borrow_mut().push(host);
        Ok(match host {
            host_id::CORE_LEN => match args.first() {
                Some(Value::Array(values)) => Value::Integer(values.len() as i64),
                Some(Value::Table(values)) => Value::Integer(values.len() as i64),
                Some(Value::String(value)) => Value::Integer(value.chars().count() as i64),
                _ => {
                    return Err(HostError::InvalidArguments(
                        "core.len expects Array, Table, or string".to_owned(),
                    ));
                }
            },
            host_id::CORE_SET_FIELD => {
                let Some(Value::Table(table)) = args.first() else {
                    return Err(HostError::InvalidArguments(
                        "core.set_field expects Table".to_owned(),
                    ));
                };
                let field = u16::try_from(integer(args.get(1))?).map_err(|_| {
                    HostError::InvalidArguments("field id is out of range".to_owned())
                })?;
                let mut table = table.as_ref().clone();
                table.insert(field, args.get(2).cloned().unwrap_or(Value::Nil));
                Value::Table(std::rc::Rc::new(table))
            }
            host_id::CORE_SET_INDEX => {
                let index = usize::try_from(integer(args.get(1))?)
                    .map_err(|_| HostError::InvalidArguments("index is out of range".to_owned()))?;
                let Some(Value::Array(values)) = args.first() else {
                    return Err(HostError::InvalidArguments(
                        "core.set_index expects Array".to_owned(),
                    ));
                };
                let mut values = values.as_ref().clone();
                if index >= values.len() {
                    return Err(HostError::InvalidArguments(
                        "index is out of bounds".to_owned(),
                    ));
                }
                values[index] = args.get(2).cloned().unwrap_or(Value::Nil);
                Value::Array(std::rc::Rc::new(values))
            }
            host_id::CORE_ASSERT => {
                let passed = matches!(args.first(), Some(Value::Bool(true)));
                if !passed {
                    let message = match args.get(1) {
                        Some(Value::String(message)) => message.clone(),
                        _ => "WMScript assertion failed".to_owned(),
                    };
                    return Err(HostError::Failed(message));
                }
                Value::Nil
            }
            host_id::UI_SAY => {
                let speaker = match args.first() {
                    Some(Value::String(value)) => value.as_str(),
                    _ => "",
                };
                let text = match args.get(1) {
                    Some(Value::String(value)) => value.as_str(),
                    _ => "",
                };
                self.shared
                    .messages
                    .borrow_mut()
                    .push(format!("{speaker}: {text}"));
                Value::Nil
            }
            host_id::RANDOM_INT => {
                let min = integer(args.first())?;
                let max = integer(args.get(1))?;
                let mut state = self.shared.random_state.get();
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                self.shared.random_state.set(state);
                if max <= min {
                    Value::Integer(min)
                } else {
                    Value::Integer(min + (state % (max - min) as u64) as i64)
                }
            }
            host_id::TIME_TICK => Value::Integer(0),
            host_id::WORLD_SPAWN => Value::Integer(self.shared.world.borrow_mut().spawn() as i64),
            host_id::WORLD_GET => {
                let entity = u64::try_from(integer(args.first())?).map_err(|_| {
                    HostError::InvalidArguments("entity id is out of range".to_owned())
                })?;
                let name = string(args.get(1))?;
                self.shared
                    .world
                    .borrow()
                    .component(entity, name)
                    .cloned()
                    .unwrap_or(Value::Nil)
            }
            host_id::WORLD_SET => {
                let entity = u64::try_from(integer(args.first())?).map_err(|_| {
                    HostError::InvalidArguments("entity id is out of range".to_owned())
                })?;
                let name = string(args.get(1))?;
                let value = args.get(2).cloned().unwrap_or(Value::Nil);
                let first =
                    self.shared
                        .world
                        .borrow_mut()
                        .set_component(entity, name, value.clone());
                match first {
                    Ok(()) => {}
                    Err(wmruntime::game::WorldError::UnknownSchema(_)) => {
                        self.shared
                            .world
                            .borrow_mut()
                            .register_component(name, false);
                        self.shared
                            .world
                            .borrow_mut()
                            .set_component(entity, name, value)
                            .map_err(|error| HostError::Failed(error.to_string()))?;
                    }
                    Err(error) => return Err(HostError::Failed(error.to_string())),
                }
                Value::Nil
            }
            host_id::WORLD_EMIT => {
                let payload = args.first().cloned().unwrap_or(Value::Nil);
                let event_type = match &payload {
                    Value::Table(table) => match table.get(&0) {
                        Some(Value::String(name)) => name.clone(),
                        _ => "event".to_owned(),
                    },
                    _ => "event".to_owned(),
                };
                self.shared
                    .events
                    .borrow_mut()
                    .push_back((event_type, payload));
                Value::Nil
            }
            host_id::SAVE_STORE => {
                let slot = u32::try_from(integer(args.first())?).map_err(|_| {
                    HostError::InvalidArguments("save slot is out of range".to_owned())
                })?;
                self.shared
                    .saves
                    .borrow_mut()
                    .insert(slot, self.shared.world.borrow().persistent_snapshot());
                Value::Bool(true)
            }
            host_id::SAVE_LOAD => {
                let slot = u32::try_from(integer(args.first())?).map_err(|_| {
                    HostError::InvalidArguments("save slot is out of range".to_owned())
                })?;
                let Some(snapshot) = self.shared.saves.borrow().get(&slot).cloned() else {
                    return Ok(Value::Bool(false));
                };
                self.shared
                    .world
                    .borrow_mut()
                    .restore_persistent(snapshot)
                    .map_err(|error| HostError::Failed(error.to_string()))?;
                Value::Bool(true)
            }
            _ => Value::Nil,
        })
    }
}
fn integer(value: Option<&Value>) -> Result<i64, HostError> {
    match value {
        Some(Value::Integer(value)) => Ok(*value),
        _ => Err(HostError::InvalidArguments("expected integer".to_owned())),
    }
}
fn string(value: Option<&Value>) -> Result<&str, HostError> {
    match value {
        Some(Value::String(value)) => Ok(value),
        _ => Err(HostError::InvalidArguments("expected string".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmcompiler::v2::compile_module;
    #[test]
    fn headless_runs_v2_start_handler() {
        let output = compile_module(
            "main.wms",
            "on start { ui.say(\"Guide\", \"Hello\"); return; }",
        )
        .unwrap();
        let report = run(output.program, Target::Headless, Vec::new()).unwrap();
        assert!(report.host_calls.contains(&host_id::UI_SAY));
    }

    #[test]
    fn headless_runs_collection_record_for_and_match_lowering() {
        let source = r#"
            struct Point { x: int }
            on start {
                let point = Point { x: 1 };
                point.x = 2;
                let values = [1, 2];
                values[0] = 3;
                let sum: int = 0;
                for value in values { sum = sum + value; }
                match sum {
                    5 => { return; },
                    _ => { return; }
                }
            }
        "#;
        let output = compile_module("main.wms", source).unwrap();
        let report = run(output.program, Target::Headless, Vec::new()).unwrap();
        assert!(report.host_calls.contains(&host_id::CORE_SET_FIELD));
        assert!(report.host_calls.contains(&host_id::CORE_SET_INDEX));
        assert!(report.host_calls.contains(&host_id::CORE_LEN));
    }

    #[test]
    fn compiled_schema_drives_real_world_storage() {
        let source = r#"
            component Position persistent { x: int, y: int }
            on start {
                let entity: int = world.spawn();
                world.set(entity, "Position", Position { x: 3, y: 4 });
                save.store(1);
                return;
            }
        "#;
        let output = compile_module("main.wms", source).unwrap();
        let report = run_compiled(&output, Target::Headless, Vec::new(), 9).unwrap();
        let entities = report.world.components["Position"]
            .keys()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(entities, vec![1]);
        assert!(report.world.components["Position"].contains_key(&1));
    }

    #[test]
    fn emitted_events_dispatch_matching_systems_in_name_order() {
        let source = r#"
            event Ping { value: int }
            system ping_system(event: Ping) { ui.say("system", "ping"); return; }
            on start { emit Ping { value: 1 }; return; }
        "#;
        let output = compile_module("main.wms", source).unwrap();
        let report = run_compiled(&output, Target::Headless, Vec::new(), 1).unwrap();
        assert_eq!(report.events_processed, 1);
        assert_eq!(report.messages, vec!["system: ping"]);
    }

    #[test]
    fn script_assertion_fails_test_execution() {
        let output = compile_module(
            "main.wms",
            "test func failure() { core.assert(false, \"expected failure\"); return; }",
        )
        .unwrap();
        let mut program = output.program;
        program.set_entry(output.test_functions["failure"]);
        assert!(
            run(program, Target::Headless, Vec::new())
                .unwrap_err()
                .ends_with("expected failure")
        );
    }
}
