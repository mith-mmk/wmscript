use wmbytecode::{Op, encode_ops};
use wmhost::HostFunction;
use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmvm::{Function, Program, Value};

fn main() {
    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(8));
    runtime.register_host_function(HostFunction::new(2, 0, 0, 0), |_args| {
        Ok(Value::String("start".to_owned()))
    });

    let mut program = Program::new();
    program.insert_function(Function::new(
        1,
        encode_ops([Op::CallHost(2, 0), Op::Return]),
        0,
        0,
    ));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn input sample");
    let outcomes = runtime.run_until_idle(4);
    println!("input worker {worker_id} => {:?}", outcomes.last());
}
