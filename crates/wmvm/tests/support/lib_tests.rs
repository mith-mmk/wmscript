use super::*;

fn vm_with_program(functions: Vec<Function>, constants: Vec<Value>, entry: u16) -> Vm {
    let mut program = Program::new();
    for function in functions {
        program.insert_function(function);
    }
    for constant in constants {
        program.push_constant(constant);
    }
    program.set_entry(entry);
    Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        program,
    )
}

#[test]
fn vm_stores_configuration() {
    let platform = PlatformProfile::native();
    let registry = HostRegistry::new(platform);
    let vm = Vm::new(VmConfig::new(platform, registry, 128));

    assert_eq!(vm.config().platform.kind, wmplatform::PlatformKind::Native);
    assert_eq!(vm.config().step_limit, 128);
}

#[test]
fn vm_executes_arithmetic_and_return() {
    let code = vec![0x10, 0x00, 0x00, 0x10, 0x01, 0x00, 0x40, 0x72];
    let mut vm = vm_with_program(
        vec![Function::new(1, code, 0, 0)],
        vec![Value::Integer(2), Value::Integer(3)],
        1,
    );

    let outcome = vm.run_frame(32);

    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.last_return(), Some(&Value::Integer(5)));
}

#[test]
fn vm_jumps_when_condition_is_false() {
    let code = vec![
        0x11, // push false
        0x61, 0x0a, 0x00, 0x00, 0x00, // jump_if_false -> 10
        0x10, 0x00, 0x00, // push const 0 (skipped)
        0x72, // return
        0x10, 0x01, 0x00, // push const 1
        0x72, // return
    ];
    let mut vm = vm_with_program(
        vec![Function::new(1, code, 0, 0)],
        vec![Value::Integer(7), Value::Integer(9)],
        1,
    );

    let outcome = vm.run_frame(32);

    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.last_return(), Some(&Value::Integer(9)));
}

#[test]
fn vm_calls_host_functions() {
    struct MockHost;

    impl HostApi for MockHost {
        fn call_host(&mut self, host_id: HostId, args: &[Value]) -> Result<Value, HostError> {
            assert_eq!(host_id, 7);
            assert_eq!(args, &[Value::Integer(4)]);
            Ok(Value::Integer(8))
        }
    }

    let mut registry = HostRegistry::new(PlatformProfile::native());
    registry.register(wmhost::HostFunction::new(7, 1, 1, 0));

    let mut program = Program::new();
    program.insert_function(Function::new(
        1,
        vec![0x10, 0x00, 0x00, 0x71, 0x07, 0x00, 0x01, 0x72],
        0,
        0,
    ));
    program.push_constant(Value::Integer(4));
    program.set_entry(1);

    let mut vm = Vm::with_host_api(
        VmConfig::new(PlatformProfile::native(), registry, 128),
        MockHost,
    );
    vm.install_program(program);

    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.last_return(), Some(&Value::Integer(8)));
}

#[test]
fn vm_sends_and_receives_messages() {
    let mut program = Program::new();
    program.push_constant(Value::Integer(1));
    program.insert_function(Function::new(
        1,
        vec![
            0x10, 0x00, 0x00, // push const 0
            0x90, 0x02, 0x00, 0x01, // send to worker 2, argc 1
            0x91, // recv
            0x72, // return
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        program,
    );
    vm.push_message(Message::new(9, 0, 0, Value::Integer(42)));

    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.outbox().len(), 1);
    assert_eq!(vm.last_return(), Some(&Value::Integer(42)));
}

#[test]
fn vm_retries_recv_after_waiting_for_message() {
    let mut program = Program::new();
    program.insert_function(Function::new(1, vec![0x91, 0x72], 0, 0));
    program.set_entry(1);

    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        program,
    );

    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::WaitingMessage { .. }));

    vm.push_message(Message::new(9, 0, 0, Value::String("resume".to_owned())));
    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.last_return(), Some(&Value::String("resume".to_owned())));
}

#[test]
fn vm_sleep_resumes_after_wake() {
    let mut program = Program::new();
    program.push_constant(Value::Integer(9));
    program.insert_function(Function::new(
        1,
        vec![
            0xA1, // sleep
            0x10, 0x00, 0x00, // push const 0
            0x72, // return
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        program,
    );

    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Sleeping { .. }));

    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Sleeping { steps: 0 }));

    vm.wake();
    let outcome = vm.run_frame(32);
    assert!(matches!(outcome, RunOutcome::Halted { .. }));
    assert_eq!(vm.last_return(), Some(&Value::Integer(9)));
}

#[test]
fn scheduler_routes_messages_between_workers() {
    let mut sender_program = Program::new();
    sender_program.push_constant(Value::Integer(7));
    sender_program.insert_function(Function::new(
        1,
        vec![
            0x10, 0x00, 0x00, // push const 0
            0x90, 0x02, 0x00, 0x01, // send to worker 2
            0x72, // return
        ],
        0,
        0,
    ));
    sender_program.set_entry(1);

    let mut receiver_program = Program::new();
    receiver_program.insert_function(Function::new(1, vec![0x91, 0x72], 0, 0));
    receiver_program.set_entry(1);

    let sender = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        sender_program,
    );
    let receiver = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        receiver_program,
    );

    let mut scheduler = Scheduler::new();
    let sender_id = scheduler.spawn(sender);
    let receiver_id = scheduler.spawn(receiver);

    let outcomes = scheduler.run_round(32);

    assert_eq!(sender_id, 1);
    assert_eq!(receiver_id, 2);
    assert!(outcomes.len() >= 2);
    assert!(matches!(
        scheduler
            .worker(receiver_id)
            .and_then(|vm| vm.last_return()),
        Some(Value::Integer(7))
    ));
    assert!(matches!(
        scheduler.worker_state(receiver_id),
        Some(WorkerState::Halted)
    ));
}

#[test]
fn scheduler_wake_resumes_sleeping_worker() {
    let mut program = Program::new();
    program.push_constant(Value::Integer(5));
    program.insert_function(Function::new(
        1,
        vec![
            0xA1, // sleep
            0x10, 0x00, 0x00, // push const 0
            0x72, // return
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ),
        program,
    );

    let mut scheduler = Scheduler::new();
    let worker_id = scheduler.spawn(vm);
    let outcomes = scheduler.run_round(32);
    assert!(matches!(
        outcomes.as_slice(),
        [(_, RunOutcome::Sleeping { .. })]
    ));
    assert!(matches!(
        scheduler.worker_state(worker_id),
        Some(WorkerState::Sleeping)
    ));

    assert!(scheduler.wake(worker_id));
    let outcomes = scheduler.run_round(32);
    assert!(matches!(
        outcomes.as_slice(),
        [(_, RunOutcome::Halted { .. })]
    ));
    assert!(matches!(
        scheduler.worker_state(worker_id),
        Some(WorkerState::Halted)
    ));
    assert!(!scheduler.wake(9999));
}

#[test]
fn program_binary_roundtrip_preserves_functions_and_constants() {
    let mut program = Program::new();
    program.push_constant(Value::String("hello".to_owned()));
    program.push_constant(Value::Array(Rc::new(vec![
        Value::Integer(1),
        Value::Bool(true),
    ])));
    let mut table = BTreeMap::new();
    table.insert(7, Value::Handle(99));
    program.push_constant(Value::Table(Rc::new(table)));
    let mut function = Function::new(1, vec![0x10, 0x00, 0x00, 0x72], 0, 2);
    function.stack_max = 8;
    program.insert_function(function);
    program.set_entry(1);

    let encoded = program.encode_binary();
    let decoded = Program::decode_binary(&encoded).expect("decode program");

    assert_eq!(decoded, program);
}
