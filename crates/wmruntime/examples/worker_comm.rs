use wmbytecode::{Op, encode_ops};
use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmvm::{Function, Program, Value};

fn main() {
    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(8));

    let mut sender = Program::new();
    let msg = sender.push_constant(Value::String("hello worker".to_owned()));
    sender.insert_function(Function::new(
        1,
        encode_ops([Op::PushConst(msg), Op::Send(2, 1), Op::Return]),
        0,
        0,
    ));
    sender.set_entry(1);

    let mut receiver = Program::new();
    receiver.insert_function(Function::new(1, encode_ops([Op::Recv, Op::Return]), 0, 0));
    receiver.set_entry(1);

    let sender_id = runtime.spawn_program(sender).expect("spawn sender");
    let receiver_id = runtime.spawn_program(receiver).expect("spawn receiver");
    let outcomes = runtime.run_until_idle(8);
    println!(
        "sender {sender_id}, receiver {receiver_id} => {:?}",
        outcomes.last()
    );
}
