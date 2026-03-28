use std::env;

use wmlcompiler::{Compiler, CompilerConfig, ModuleCatalog};
use wmlplatform::PlatformProfile;
use wmlruntime::{Runtime, RuntimeConfig};
use wmlvm::{RunOutcome, Value};

fn main() {
    let source = include_str!("../../../samples/easynovel/main.wms");
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let mut catalog = ModuleCatalog::new();
    let program = compiler
        .compile_program("samples/easynovel/main.wms", source, &mut catalog)
        .expect("compile easynovel sample");

    let selected = env::args().nth(1).unwrap_or_else(|| "main".to_owned());
    let entry = match selected.as_str() {
        "prologue" => 1,
        "chapter_1" => 2,
        "chapter_2" => 3,
        "main" => 4,
        other => {
            eprintln!("unknown chapter `{other}`, defaulting to `main`");
            4
        }
    };

    let mut program = program;
    program.set_entry(entry);

    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(16));
    let worker_id = runtime.spawn_program(program).expect("spawn program");
    let outcomes = runtime.run_until_idle(4);

    println!("=== easynovel ===");
    println!("worker: {worker_id}");
    println!("chapter: {selected}");
    if let Some((_, outcome)) = outcomes.last() {
        match outcome {
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            } => {
                println!("--- message window ---");
                println!("{text}");
            }
            other => println!("outcome: {other:?}"),
        }
    }
}
