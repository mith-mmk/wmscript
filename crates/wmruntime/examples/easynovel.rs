use wmcompiler::{Compiler, CompilerConfig, ModuleCatalog};
use wmext::standard_extension_registry;
use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmvm::{Message, RunOutcome, Value};

fn main() {
    let source = include_str!("../../../samples/easynovel/main.wms");
    let compiler = Compiler::new(
        CompilerConfig::new(PlatformProfile::native())
            .with_extension_registry(standard_extension_registry().expect("standard extensions")),
    );
    let mut catalog = ModuleCatalog::new();
    let program = compiler
        .compile_program("samples/easynovel/main.wms", source, &mut catalog)
        .expect("compile easynovel sample");

    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(16));
    runtime
        .install_standard_extensions()
        .expect("install standard extensions");
    let worker_id = runtime.spawn_program(program).expect("spawn program");

    let mut outcomes = runtime.run_until_idle(16);
    println!("=== easynovel ===");
    println!("worker: {worker_id}");
    print_message_window(&runtime);
    print_last_outcome(&outcomes);

    runtime.set_state_value("ui.last_choice", Value::String("prologue".to_owned()));
    runtime.send_message(Message::new(
        0,
        worker_id,
        0,
        Value::String("prologue".to_owned()),
    ));
    outcomes = runtime.run_until_idle(16);
    println!("--- after selecting prologue ---");
    print_message_window(&runtime);
    print_last_outcome(&outcomes);

    while !runtime.waiting_workers().is_empty() {
        runtime.send_message(Message::new(0, worker_id, 0, Value::Nil));
        outcomes = runtime.run_until_idle(16);
        println!("--- next page ---");
        print_message_window(&runtime);
        print_last_outcome(&outcomes);
        if matches!(outcomes.last(), Some((_, RunOutcome::Halted { .. }))) {
            break;
        }
    }
}

fn print_message_window(runtime: &Runtime) {
    let message = runtime.message_window_state();
    println!(
        "message: visible={} speaker={:?}",
        message.visible, message.speaker
    );
    if !message.text.is_empty() {
        println!("{}", message.text);
    }
    if !message.choices.is_empty() {
        println!(
            "choices: {}",
            message
                .choices
                .iter()
                .map(|choice| format!("{}={}", choice.id, choice.label))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if let Some(prompt) = message.input_prompt {
        println!("prompt: {prompt}");
    }
    println!(
        "speed={} auto={} skip={}",
        message.text_speed, message.auto_mode, message.skip_mode
    );
}

fn print_last_outcome(outcomes: &[(u32, RunOutcome)]) {
    if let Some((_, outcome)) = outcomes.last() {
        println!("outcome: {outcome:?}");
    }
}
