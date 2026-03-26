use wmlbytecode::{Op, encode_ops};
use wmlplatform::PlatformProfile;
use wmlruntime::{Runtime, RuntimeConfig};
use wmlvm::{Function, Program, Value};

fn main() {
    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(8));

    let mut engine = Program::new();
    let cmd_show = engine.push_constant(Value::String("show".to_owned()));
    let speaker = engine.push_constant(Value::String("Narrator".to_owned()));
    let line = engine.push_constant(Value::String(
        "The engine worker streams dialogue without touching window state.".to_owned(),
    ));
    let cmd_choices = engine.push_constant(Value::String("choices".to_owned()));
    let choice_a = engine.push_constant(Value::String("prologue".to_owned()));
    let choice_b = engine.push_constant(Value::String("chapter_1".to_owned()));
    let choice_c = engine.push_constant(Value::String("chapter_2".to_owned()));
    engine.insert_function(Function::new(
        1,
        encode_ops([
            Op::PushConst(cmd_show),
            Op::PushConst(speaker),
            Op::PushConst(line),
            Op::PushConst(cmd_choices),
            Op::PushConst(choice_a),
            Op::PushConst(choice_b),
            Op::PushConst(choice_c),
            Op::Send(2, 7),
            Op::Return,
        ]),
        0,
        0,
    ));
    engine.set_entry(1);

    let mut ui = Program::new();
    let idx_command = ui.push_constant(Value::Integer(0));
    let idx_speaker = ui.push_constant(Value::Integer(1));
    let idx_text = ui.push_constant(Value::Integer(2));
    ui.insert_function(Function::new(
        1,
        encode_ops([
            Op::Recv,
            Op::Dup,
            Op::PushConst(idx_command),
            Op::LoadIndex,
            Op::Pop,
            Op::Dup,
            Op::PushConst(idx_speaker),
            Op::LoadIndex,
            Op::Pop,
            Op::Dup,
            Op::PushConst(idx_text),
            Op::LoadIndex,
            Op::Return,
        ]),
        0,
        0,
    ));
    ui.set_entry(1);

    let engine_id = runtime.spawn_program(engine).expect("spawn engine");
    let ui_id = runtime.spawn_program(ui).expect("spawn ui");
    let outcomes = runtime.run_until_idle(8);

    println!("=== engine/ui worker split ===");
    println!("engine worker: {engine_id}");
    println!("ui worker: {ui_id}");
    println!("outcomes:");
    for (worker_id, outcome) in outcomes {
        println!("  worker {worker_id} => {outcome:?}");
    }
}
