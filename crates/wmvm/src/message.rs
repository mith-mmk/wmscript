use super::{Value, WorkerId};

/// Message payload exchanged between workers.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub from: WorkerId,
    pub to: WorkerId,
    pub msg_type: u32,
    pub payload: Value,
}

impl Message {
    pub fn new(from: WorkerId, to: WorkerId, msg_type: u32, payload: Value) -> Self {
        Self {
            from,
            to,
            msg_type,
            payload,
        }
    }
}
