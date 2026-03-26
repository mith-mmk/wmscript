use wmlhost::HostFunction;
use wmlplatform::PlatformProfile;
use wmlruntime::{Runtime, RuntimeConfig};
use wmlvm::{Function, Program, Value};

fn main() {
    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(16));
    runtime.register_host_function(HostFunction::new(1, 1, 1, 0), |args| {
        let message = match args.first() {
            Some(Value::String(text)) => text.clone(),
            Some(other) => format!("{other:?}"),
            None => String::new(),
        };
        println!("{message}");
        Ok(args.first().cloned().unwrap_or(Value::Nil))
    });

    let mut program = Program::new();
    let message = program.push_constant(Value::String("hello from WML runtime".to_owned()));
    program.insert_function(Function::new(
        1,
        vec![
            0x10,
            message as u8,
            (message >> 8) as u8,
            0x71,
            0x01,
            0x00,
            0x01,
            0x01,
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let worker_id = runtime
        .spawn_program(program)
        .expect("spawn runtime program");
    let outcomes = runtime.run_until_idle(8);
    println!("worker {worker_id} => {:?}", outcomes.last());
}
