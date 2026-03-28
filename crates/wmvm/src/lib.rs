#![forbid(unsafe_code)]

//! Virtual machine crate for WML scripts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use wmbytecode::{BytecodeError, Op, Opcode, decode_at};
use wmhost::{CapabilityMask, HostId, HostRegistry};
use wmplatform::PlatformProfile;

/// Worker identifier.
pub type WorkerId = u32;

/// VM configuration shared by runtime builders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmConfig {
    pub platform: PlatformProfile,
    pub host_registry: HostRegistry,
    pub step_limit: usize,
    pub capability_mask: CapabilityMask,
    pub worker_id: WorkerId,
}

impl VmConfig {
    pub fn new(platform: PlatformProfile, host_registry: HostRegistry, step_limit: usize) -> Self {
        Self {
            platform,
            host_registry,
            step_limit,
            capability_mask: CapabilityMask::MAX,
            worker_id: 0,
        }
    }

    pub fn with_worker_id(mut self, worker_id: WorkerId) -> Self {
        self.worker_id = worker_id;
        self
    }

    pub fn with_capability_mask(mut self, capability_mask: CapabilityMask) -> Self {
        self.capability_mask = capability_mask;
        self
    }
}

/// Runtime value stored on the VM stack.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Rc<Vec<Value>>),
    Table(Rc<BTreeMap<u16, Value>>),
    Handle(u64),
}

impl Value {
    pub const fn nil() -> Self {
        Self::Nil
    }

    pub fn truthy(&self) -> bool {
        match self {
            Self::Nil => false,
            Self::Bool(v) => *v,
            Self::Integer(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::String(v) => !v.is_empty(),
            Self::Array(v) => !v.is_empty(),
            Self::Table(v) => !v.is_empty(),
            Self::Handle(_) => true,
        }
    }

    fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(v) => Some(*v),
            Self::Bool(true) => Some(1),
            Self::Bool(false) => Some(0),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    fn as_field_id(&self) -> Option<u16> {
        self.as_integer()
            .and_then(|value| u16::try_from(value).ok())
    }
}

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

/// Host-side error returned by the host bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostError {
    UnknownHostId(HostId),
    CapabilityDenied {
        host_id: HostId,
        required: CapabilityMask,
    },
    InvalidArguments(String),
    Failed(String),
}

impl core::fmt::Display for HostError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownHostId(id) => write!(f, "unknown host id: {id}"),
            Self::CapabilityDenied { host_id, required } => {
                write!(
                    f,
                    "capability denied for host {host_id}: required mask {required:#X}"
                )
            }
            Self::InvalidArguments(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HostError {}

/// VM error.
#[derive(Clone, Debug, PartialEq)]
pub enum VmError {
    StackUnderflow,
    TypeMismatch {
        expected: &'static str,
        found: Value,
    },
    InvalidConstant(u16),
    InvalidGlobal(u16),
    InvalidLocal(u8),
    InvalidFunctionId(u16),
    InvalidJumpTarget {
        target: u32,
        code_len: usize,
    },
    DivisionByZero,
    Host(HostError),
    Bytecode(BytecodeError),
    NoActiveFrame,
}

impl core::fmt::Display for VmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StackUnderflow => f.write_str("stack underflow"),
            Self::TypeMismatch { expected, found } => {
                write!(f, "type mismatch: expected {expected}, found {found:?}")
            }
            Self::InvalidConstant(index) => write!(f, "invalid constant index: {index}"),
            Self::InvalidGlobal(index) => write!(f, "invalid global index: {index}"),
            Self::InvalidLocal(index) => write!(f, "invalid local index: {index}"),
            Self::InvalidFunctionId(id) => write!(f, "invalid function id: {id}"),
            Self::InvalidJumpTarget { target, code_len } => {
                write!(f, "invalid jump target {target} for code length {code_len}")
            }
            Self::DivisionByZero => f.write_str("division by zero"),
            Self::Host(error) => write!(f, "host error: {error}"),
            Self::Bytecode(error) => write!(f, "bytecode error: {error}"),
            Self::NoActiveFrame => f.write_str("no active frame"),
        }
    }
}

impl std::error::Error for VmError {}

impl From<BytecodeError> for VmError {
    fn from(value: BytecodeError) -> Self {
        Self::Bytecode(value)
    }
}

impl From<HostError> for VmError {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

/// Decoded function metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub id: u16,
    pub code: Vec<u8>,
    pub arg_count: u8,
    pub local_count: u8,
    pub stack_max: usize,
}

impl Function {
    pub fn new(id: u16, code: impl Into<Vec<u8>>, arg_count: u8, local_count: u8) -> Self {
        Self {
            id,
            code: code.into(),
            arg_count,
            local_count,
            stack_max: 0,
        }
    }
}

/// Program container with function table and constant pool.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    entry: Option<u16>,
    functions: BTreeMap<u16, Function>,
    constants: Vec<Value>,
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry(&self) -> Option<u16> {
        self.entry
    }

    pub fn set_entry(&mut self, func_id: u16) {
        self.entry = Some(func_id);
    }

    pub fn insert_function(&mut self, function: Function) -> Option<Function> {
        self.functions.insert(function.id, function)
    }

    pub fn function(&self, func_id: u16) -> Option<&Function> {
        self.functions.get(&func_id)
    }

    pub fn function_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.functions.keys().copied()
    }

    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn push_constant(&mut self, value: Value) -> u16 {
        self.constants.push(value);
        (self.constants.len() - 1) as u16
    }

    pub fn constant(&self, index: u16) -> Option<&Value> {
        self.constants.get(index as usize)
    }

    pub fn constant_count(&self) -> usize {
        self.constants.len()
    }
}

/// Call frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub func_id: u16,
    pub pc: usize,
    pub return_pc: usize,
    pub base_sp: usize,
    pub locals: Vec<Value>,
    pub arg_count: u8,
}

impl Frame {
    fn new(
        func_id: u16,
        base_sp: usize,
        return_pc: usize,
        locals: Vec<Value>,
        arg_count: u8,
    ) -> Self {
        Self {
            func_id,
            pc: 0,
            return_pc,
            base_sp,
            locals,
            arg_count,
        }
    }
}

/// VM state.
#[derive(Clone, Debug, PartialEq)]
pub enum VmState {
    Idle,
    Running,
    WaitingMessage,
    Sleeping,
    Halted,
    Error(VmError),
}

/// Result of running a frame budget.
#[derive(Clone, Debug, PartialEq)]
pub enum RunOutcome {
    StepLimitReached { steps: usize },
    Yielded { steps: usize },
    Sleeping { steps: usize },
    WaitingMessage { steps: usize },
    Halted { steps: usize, value: Option<Value> },
    Error { steps: usize, error: VmError },
}

/// Host API trait used by CALL_HOST.
pub trait HostApi {
    fn call_host(&mut self, host_id: HostId, args: &[Value]) -> Result<Value, HostError>;
}

/// Host API implementation that rejects every call.
#[derive(Default)]
pub struct NullHostApi;

impl HostApi for NullHostApi {
    fn call_host(&mut self, host_id: HostId, _args: &[Value]) -> Result<Value, HostError> {
        Err(HostError::UnknownHostId(host_id))
    }
}

enum StepEffect {
    Continue,
    Yielded,
    Sleeping,
    WaitingMessage,
    Halted(Option<Value>),
}

/// Virtual machine.
pub struct Vm {
    config: VmConfig,
    program: Program,
    stack: Vec<Value>,
    globals: Vec<Value>,
    call_stack: Vec<Frame>,
    current_frame: Option<Frame>,
    inbox: VecDeque<Message>,
    outbox: VecDeque<Message>,
    state: VmState,
    last_return: Option<Value>,
    host_api: Box<dyn HostApi>,
}

/// Serializable snapshot of a VM worker.
#[derive(Clone, Debug, PartialEq)]
pub struct VmSnapshot {
    pub config: VmConfig,
    pub program: Program,
    pub stack: Vec<Value>,
    pub globals: Vec<Value>,
    pub call_stack: Vec<Frame>,
    pub current_frame: Option<Frame>,
    pub inbox: VecDeque<Message>,
    pub outbox: VecDeque<Message>,
    pub state: VmState,
    pub last_return: Option<Value>,
}

impl Vm {
    pub fn new(config: VmConfig) -> Self {
        Self::with_host_api(config, NullHostApi)
    }

    pub fn with_host_api(config: VmConfig, host_api: impl HostApi + 'static) -> Self {
        Self {
            config,
            program: Program::new(),
            stack: Vec::new(),
            globals: Vec::new(),
            call_stack: Vec::new(),
            current_frame: None,
            inbox: VecDeque::new(),
            outbox: VecDeque::new(),
            state: VmState::Idle,
            last_return: None,
            host_api: Box::new(host_api),
        }
    }

    pub fn with_program(config: VmConfig, program: Program) -> Self {
        let mut vm = Self::new(config);
        vm.install_program(program);
        vm
    }

    pub fn with_program_and_host_api(
        config: VmConfig,
        program: Program,
        host_api: impl HostApi + 'static,
    ) -> Self {
        let mut vm = Self::with_host_api(config, host_api);
        vm.install_program(program);
        vm
    }

    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    pub fn state(&self) -> &VmState {
        &self.state
    }

    pub fn last_return(&self) -> Option<&Value> {
        self.last_return.as_ref()
    }

    pub fn stack(&self) -> &[Value] {
        &self.stack
    }

    pub fn outbox(&self) -> &VecDeque<Message> {
        &self.outbox
    }

    pub fn worker_id(&self) -> WorkerId {
        self.config.worker_id
    }

    pub fn set_worker_id(&mut self, worker_id: WorkerId) {
        self.config.worker_id = worker_id;
    }

    pub fn install_program(&mut self, program: Program) {
        self.program = program;
        self.stack.clear();
        self.globals.clear();
        self.call_stack.clear();
        self.current_frame = None;
        self.inbox.clear();
        self.outbox.clear();
        self.last_return = None;
        self.state = VmState::Idle;
    }

    pub fn set_host_api(&mut self, host_api: impl HostApi + 'static) {
        self.host_api = Box::new(host_api);
    }

    pub fn snapshot(&self) -> VmSnapshot {
        VmSnapshot {
            config: self.config.clone(),
            program: self.program.clone(),
            stack: self.stack.clone(),
            globals: self.globals.clone(),
            call_stack: self.call_stack.clone(),
            current_frame: self.current_frame.clone(),
            inbox: self.inbox.clone(),
            outbox: self.outbox.clone(),
            state: self.state.clone(),
            last_return: self.last_return.clone(),
        }
    }

    pub fn from_snapshot(snapshot: VmSnapshot, host_api: Box<dyn HostApi>) -> Self {
        Self {
            config: snapshot.config,
            program: snapshot.program,
            stack: snapshot.stack,
            globals: snapshot.globals,
            call_stack: snapshot.call_stack,
            current_frame: snapshot.current_frame,
            inbox: snapshot.inbox,
            outbox: snapshot.outbox,
            state: snapshot.state,
            last_return: snapshot.last_return,
            host_api,
        }
    }

    pub fn push_message(&mut self, message: Message) {
        self.inbox.push_back(message);
    }

    pub fn drain_outbox(&mut self) -> Vec<Message> {
        self.outbox.drain(..).collect()
    }

    pub const fn knows_opcode(opcode: Opcode) -> bool {
        matches!(
            opcode,
            Opcode::Nop
                | Opcode::Halt
                | Opcode::PushConst
                | Opcode::PushNil
                | Opcode::PushTrue
                | Opcode::PushFalse
                | Opcode::LoadLocal
                | Opcode::StoreLocal
                | Opcode::LoadGlobal
                | Opcode::StoreGlobal
                | Opcode::LoadField
                | Opcode::StoreField
                | Opcode::LoadIndex
                | Opcode::StoreIndex
                | Opcode::Pop
                | Opcode::Dup
                | Opcode::Add
                | Opcode::Sub
                | Opcode::Mul
                | Opcode::Div
                | Opcode::Mod
                | Opcode::Neg
                | Opcode::Eq
                | Opcode::Ne
                | Opcode::Lt
                | Opcode::Le
                | Opcode::Gt
                | Opcode::Ge
                | Opcode::Not
                | Opcode::Jump
                | Opcode::JumpIfFalse
                | Opcode::JumpIfTrue
                | Opcode::Call
                | Opcode::CallHost
                | Opcode::Return
                | Opcode::NewArray
                | Opcode::NewTable
                | Opcode::Send
                | Opcode::Recv
                | Opcode::TryRecv
                | Opcode::Yield
                | Opcode::Sleep
        )
    }

    pub fn run_frame(&mut self, step_limit: usize) -> RunOutcome {
        if matches!(self.state, VmState::Halted | VmState::Error(_)) {
            return self.outcome_from_state(0);
        }

        if self.current_frame.is_none() {
            if let Err(error) = self.start_entry_frame() {
                self.state = VmState::Error(error.clone());
                return RunOutcome::Error { steps: 0, error };
            }
        }

        if matches!(self.state, VmState::Sleeping) {
            return RunOutcome::Sleeping { steps: 0 };
        }

        if matches!(self.state, VmState::WaitingMessage) && self.inbox.is_empty() {
            return RunOutcome::WaitingMessage { steps: 0 };
        }

        self.state = VmState::Running;
        let budget = if step_limit == 0 {
            self.config.step_limit
        } else {
            step_limit
        };

        let mut steps = 0usize;
        while steps < budget {
            let effect = match self.step_once() {
                Ok(effect) => effect,
                Err(error) => {
                    self.state = VmState::Error(error.clone());
                    return RunOutcome::Error { steps, error };
                }
            };

            steps += 1;
            match effect {
                StepEffect::Continue => {}
                StepEffect::Yielded => {
                    self.state = VmState::Idle;
                    return RunOutcome::Yielded { steps };
                }
                StepEffect::Sleeping => {
                    self.state = VmState::Sleeping;
                    return RunOutcome::Sleeping { steps };
                }
                StepEffect::WaitingMessage => {
                    self.state = VmState::WaitingMessage;
                    return RunOutcome::WaitingMessage { steps };
                }
                StepEffect::Halted(value) => {
                    self.state = VmState::Halted;
                    self.last_return = value.clone();
                    return RunOutcome::Halted { steps, value };
                }
            }
        }

        self.state = VmState::Idle;
        RunOutcome::StepLimitReached { steps }
    }
}

impl Vm {
    fn outcome_from_state(&self, steps: usize) -> RunOutcome {
        match &self.state {
            VmState::Sleeping => RunOutcome::Sleeping { steps },
            VmState::WaitingMessage => RunOutcome::WaitingMessage { steps },
            VmState::Halted => RunOutcome::Halted {
                steps,
                value: self.last_return.clone(),
            },
            VmState::Error(error) => RunOutcome::Error {
                steps,
                error: error.clone(),
            },
            VmState::Idle | VmState::Running => RunOutcome::StepLimitReached { steps },
        }
    }

    fn start_entry_frame(&mut self) -> Result<(), VmError> {
        let entry = self.program.entry().ok_or(VmError::NoActiveFrame)?;
        self.push_frame(entry, Vec::new(), 0, 0)
    }

    fn push_frame(
        &mut self,
        func_id: u16,
        args: Vec<Value>,
        base_sp: usize,
        return_pc: usize,
    ) -> Result<(), VmError> {
        let function = self
            .program
            .function(func_id)
            .ok_or(VmError::InvalidFunctionId(func_id))?;
        let local_count = usize::max(function.local_count as usize, args.len());
        let mut locals = vec![Value::Nil; local_count];
        for (index, value) in args.into_iter().enumerate() {
            locals[index] = value;
        }

        self.current_frame = Some(Frame::new(
            func_id,
            base_sp,
            return_pc,
            locals,
            function.arg_count,
        ));
        Ok(())
    }

    fn step_once(&mut self) -> Result<StepEffect, VmError> {
        let (func_id, pc) = {
            let frame = self.current_frame.as_ref().ok_or(VmError::NoActiveFrame)?;
            (frame.func_id, frame.pc)
        };

        let code = self
            .program
            .function(func_id)
            .ok_or(VmError::InvalidFunctionId(func_id))?
            .code
            .as_slice();

        if pc >= code.len() {
            return self.finish_return(None);
        }

        let (op, len) = decode_at(code, pc)?;
        let frame = self.current_frame.as_mut().ok_or(VmError::NoActiveFrame)?;
        frame.pc = frame
            .pc
            .checked_add(len)
            .ok_or(VmError::InvalidJumpTarget {
                target: u32::MAX,
                code_len: code.len(),
            })?;
        self.execute_op(op, code.len())
    }

    fn execute_op(&mut self, op: Op, code_len: usize) -> Result<StepEffect, VmError> {
        match op {
            Op::Nop => Ok(StepEffect::Continue),
            Op::Halt => {
                let value = self.stack.pop();
                self.finish_return(value)
            }
            Op::PushConst(index) => {
                let value = self
                    .program
                    .constant(index)
                    .cloned()
                    .ok_or(VmError::InvalidConstant(index))?;
                self.stack.push(value);
                Ok(StepEffect::Continue)
            }
            Op::PushNil => {
                self.stack.push(Value::Nil);
                Ok(StepEffect::Continue)
            }
            Op::PushTrue => {
                self.stack.push(Value::Bool(true));
                Ok(StepEffect::Continue)
            }
            Op::PushFalse => {
                self.stack.push(Value::Bool(false));
                Ok(StepEffect::Continue)
            }
            Op::LoadLocal(index) => {
                let frame = self.current_frame.as_ref().ok_or(VmError::NoActiveFrame)?;
                let value = frame
                    .locals
                    .get(index as usize)
                    .cloned()
                    .ok_or(VmError::InvalidLocal(index))?;
                self.stack.push(value);
                Ok(StepEffect::Continue)
            }
            Op::StoreLocal(index) => {
                let value = self.pop_stack()?;
                let frame = self.current_frame.as_mut().ok_or(VmError::NoActiveFrame)?;
                let slot = frame
                    .locals
                    .get_mut(index as usize)
                    .ok_or(VmError::InvalidLocal(index))?;
                *slot = value;
                Ok(StepEffect::Continue)
            }
            Op::LoadGlobal(index) => {
                let value = self
                    .globals
                    .get(index as usize)
                    .cloned()
                    .ok_or(VmError::InvalidGlobal(index))?;
                self.stack.push(value);
                Ok(StepEffect::Continue)
            }
            Op::StoreGlobal(index) => {
                let value = self.pop_stack()?;
                self.ensure_global(index as usize);
                self.globals[index as usize] = value;
                Ok(StepEffect::Continue)
            }
            Op::LoadField(field_id) => {
                let object = self.pop_stack()?;
                let value = match object {
                    Value::Table(table) => table.get(&field_id).cloned().unwrap_or(Value::Nil),
                    other => {
                        return Err(VmError::TypeMismatch {
                            expected: "table",
                            found: other,
                        });
                    }
                };
                self.stack.push(value);
                Ok(StepEffect::Continue)
            }
            Op::StoreField(field_id) => {
                let value = self.pop_stack()?;
                let object = self.pop_stack()?;
                match object {
                    Value::Table(mut table) => {
                        Rc::make_mut(&mut table).insert(field_id, value);
                        Ok(StepEffect::Continue)
                    }
                    other => Err(VmError::TypeMismatch {
                        expected: "table",
                        found: other,
                    }),
                }
            }
            Op::LoadIndex => {
                let index = self.pop_stack()?;
                let object = self.pop_stack()?;
                self.stack.push(self.load_index(object, index)?);
                Ok(StepEffect::Continue)
            }
            Op::StoreIndex => {
                let value = self.pop_stack()?;
                let index = self.pop_stack()?;
                let object = self.pop_stack()?;
                self.store_index(object, index, value)?;
                Ok(StepEffect::Continue)
            }
            Op::Pop => {
                self.pop_stack()?;
                Ok(StepEffect::Continue)
            }
            Op::Dup => {
                let value = self.peek_stack()?.clone();
                self.stack.push(value);
                Ok(StepEffect::Continue)
            }
            Op::Add => self.binary_numeric(|a, b| a + b, |a, b| a + b),
            Op::Sub => self.binary_numeric(|a, b| a - b, |a, b| a - b),
            Op::Mul => self.binary_numeric(|a, b| a * b, |a, b| a * b),
            Op::Div => self.binary_div(),
            Op::Mod => self.binary_mod(),
            Op::Neg => {
                let value = self.pop_stack()?;
                match value {
                    Value::Integer(v) => self.stack.push(Value::Integer(-v)),
                    Value::Float(v) => self.stack.push(Value::Float(-v)),
                    other => {
                        return Err(VmError::TypeMismatch {
                            expected: "number",
                            found: other,
                        });
                    }
                }
                Ok(StepEffect::Continue)
            }
            Op::Eq => self.compare(|a, b| a == b),
            Op::Ne => self.compare(|a, b| a != b),
            Op::Lt => self.compare_ordering(|a, b| a < b),
            Op::Le => self.compare_ordering(|a, b| a <= b),
            Op::Gt => self.compare_ordering(|a, b| a > b),
            Op::Ge => self.compare_ordering(|a, b| a >= b),
            Op::Not => {
                let value = self.pop_stack()?;
                self.stack.push(Value::Bool(!value.truthy()));
                Ok(StepEffect::Continue)
            }
            Op::Jump(target) => {
                self.jump_to(target, code_len)?;
                Ok(StepEffect::Continue)
            }
            Op::JumpIfFalse(target) => {
                let value = self.pop_stack()?;
                if !value.truthy() {
                    self.jump_to(target, code_len)?;
                }
                Ok(StepEffect::Continue)
            }
            Op::JumpIfTrue(target) => {
                let value = self.pop_stack()?;
                if value.truthy() {
                    self.jump_to(target, code_len)?;
                }
                Ok(StepEffect::Continue)
            }
            Op::Call(func_id, argc) => self.call_function(func_id, argc),
            Op::CallHost(host_id, argc) => self.call_host(host_id, argc),
            Op::Return => {
                let value = self.stack.pop();
                self.finish_return(value)
            }
            Op::NewArray(size_hint) => {
                self.stack.push(Value::Array(Rc::new(Vec::with_capacity(
                    size_hint as usize,
                ))));
                Ok(StepEffect::Continue)
            }
            Op::NewTable(size_hint) => {
                let _ = size_hint;
                self.stack.push(Value::Table(Rc::new(BTreeMap::new())));
                Ok(StepEffect::Continue)
            }
            Op::Send(worker_id, argc) => {
                let payload = self.collect_payload(argc)?;
                self.outbox.push_back(Message::new(
                    self.worker_id(),
                    worker_id as WorkerId,
                    0,
                    payload,
                ));
                Ok(StepEffect::Continue)
            }
            Op::Recv => {
                if let Some(message) = self.inbox.pop_front() {
                    self.stack.push(message.payload);
                    Ok(StepEffect::Continue)
                } else {
                    Ok(StepEffect::WaitingMessage)
                }
            }
            Op::TryRecv => {
                let payload = self
                    .inbox
                    .pop_front()
                    .map_or(Value::Nil, |message| message.payload);
                self.stack.push(payload);
                Ok(StepEffect::Continue)
            }
            Op::Yield => Ok(StepEffect::Yielded),
            Op::Sleep => Ok(StepEffect::Sleeping),
        }
    }

    fn call_function(&mut self, func_id: u16, argc: u8) -> Result<StepEffect, VmError> {
        let arg_count = self
            .program
            .function(func_id)
            .ok_or(VmError::InvalidFunctionId(func_id))?
            .arg_count;
        let argc = argc as usize;
        if argc > self.stack.len() {
            return Err(VmError::StackUnderflow);
        }

        let base_sp = self.stack.len() - argc;
        let args = self.stack.split_off(base_sp);
        let return_pc = self.current_frame.as_ref().map_or(0, |frame| frame.pc);
        let caller = self.current_frame.take().ok_or(VmError::NoActiveFrame)?;
        self.call_stack.push(caller);
        self.push_frame(func_id, args, base_sp, return_pc)?;
        let _ = arg_count;
        Ok(StepEffect::Continue)
    }

    fn call_host(&mut self, host_id: HostId, argc: u8) -> Result<StepEffect, VmError> {
        let function = self
            .config
            .host_registry
            .function(host_id)
            .ok_or(HostError::UnknownHostId(host_id))?;
        if self.config.capability_mask & function.required_capabilities
            != function.required_capabilities
        {
            return Err(HostError::CapabilityDenied {
                host_id,
                required: function.required_capabilities,
            }
            .into());
        }

        let args = self.collect_args(argc)?;
        let value = self.host_api.call_host(host_id, &args)?;
        self.stack.push(value);
        Ok(StepEffect::Continue)
    }

    fn collect_args(&mut self, argc: u8) -> Result<Vec<Value>, VmError> {
        let argc = argc as usize;
        if argc > self.stack.len() {
            return Err(VmError::StackUnderflow);
        }
        Ok(self.stack.split_off(self.stack.len() - argc))
    }

    fn collect_payload(&mut self, argc: u8) -> Result<Value, VmError> {
        let args = self.collect_args(argc)?;
        Ok(match args.as_slice() {
            [] => Value::Nil,
            [single] => single.clone(),
            _ => Value::Array(Rc::new(args)),
        })
    }

    fn load_index(&self, object: Value, index: Value) -> Result<Value, VmError> {
        match object {
            Value::Array(values) => {
                let index = index
                    .as_integer()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(VmError::TypeMismatch {
                        expected: "array index",
                        found: index,
                    })?;
                Ok(values.get(index).cloned().unwrap_or(Value::Nil))
            }
            Value::Table(table) => {
                let key = index.as_field_id().ok_or(VmError::TypeMismatch {
                    expected: "field id",
                    found: index,
                })?;
                Ok(table.get(&key).cloned().unwrap_or(Value::Nil))
            }
            other => Err(VmError::TypeMismatch {
                expected: "array or table",
                found: other,
            }),
        }
    }

    fn store_index(&mut self, object: Value, index: Value, value: Value) -> Result<(), VmError> {
        match object {
            Value::Array(mut values) => {
                let index = index
                    .as_integer()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(VmError::TypeMismatch {
                        expected: "array index",
                        found: index,
                    })?;
                let values = Rc::make_mut(&mut values);
                if index >= values.len() {
                    values.resize(index + 1, Value::Nil);
                }
                values[index] = value;
                Ok(())
            }
            Value::Table(mut table) => {
                let key = index.as_field_id().ok_or(VmError::TypeMismatch {
                    expected: "field id",
                    found: index,
                })?;
                Rc::make_mut(&mut table).insert(key, value);
                Ok(())
            }
            other => Err(VmError::TypeMismatch {
                expected: "array or table",
                found: other,
            }),
        }
    }

    fn binary_numeric<FInt, FFloat>(
        &mut self,
        int_op: FInt,
        float_op: FFloat,
    ) -> Result<StepEffect, VmError>
    where
        FInt: FnOnce(i64, i64) -> i64,
        FFloat: FnOnce(f64, f64) -> f64,
    {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        let value = match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(int_op(a, b)),
            (left, right) => {
                let left = left.as_float().ok_or(VmError::TypeMismatch {
                    expected: "number",
                    found: left,
                })?;
                let right = right.as_float().ok_or(VmError::TypeMismatch {
                    expected: "number",
                    found: right,
                })?;
                Value::Float(float_op(left, right))
            }
        };
        self.stack.push(value);
        Ok(StepEffect::Continue)
    }

    fn binary_div(&mut self) -> Result<StepEffect, VmError> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        let value = match (left, right) {
            (Value::Integer(_), Value::Integer(0)) => return Err(VmError::DivisionByZero),
            (Value::Integer(a), Value::Integer(b)) => Value::Integer(a / b),
            (left, right) => {
                let left = left.as_float().ok_or(VmError::TypeMismatch {
                    expected: "number",
                    found: left,
                })?;
                let right = right.as_float().ok_or(VmError::TypeMismatch {
                    expected: "number",
                    found: right,
                })?;
                if right == 0.0 {
                    return Err(VmError::DivisionByZero);
                }
                Value::Float(left / right)
            }
        };
        self.stack.push(value);
        Ok(StepEffect::Continue)
    }

    fn binary_mod(&mut self) -> Result<StepEffect, VmError> {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        match (left, right) {
            (Value::Integer(_), Value::Integer(0)) => Err(VmError::DivisionByZero),
            (Value::Integer(a), Value::Integer(b)) => {
                self.stack.push(Value::Integer(a % b));
                Ok(StepEffect::Continue)
            }
            (left, right) => Err(VmError::TypeMismatch {
                expected: "integer",
                found: Value::Array(Rc::new(vec![left, right])),
            }),
        }
    }

    fn compare<F>(&mut self, predicate: F) -> Result<StepEffect, VmError>
    where
        F: FnOnce(Value, Value) -> bool,
    {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        self.stack.push(Value::Bool(predicate(left, right)));
        Ok(StepEffect::Continue)
    }

    fn compare_ordering<F>(&mut self, predicate: F) -> Result<StepEffect, VmError>
    where
        F: FnOnce(f64, f64) -> bool,
    {
        let right = self.pop_stack()?;
        let left = self.pop_stack()?;
        let left = left.as_float().ok_or(VmError::TypeMismatch {
            expected: "number",
            found: left,
        })?;
        let right = right.as_float().ok_or(VmError::TypeMismatch {
            expected: "number",
            found: right,
        })?;
        self.stack.push(Value::Bool(predicate(left, right)));
        Ok(StepEffect::Continue)
    }

    fn jump_to(&mut self, target: u32, code_len: usize) -> Result<(), VmError> {
        let target =
            usize::try_from(target).map_err(|_| VmError::InvalidJumpTarget { target, code_len })?;
        if target > code_len {
            return Err(VmError::InvalidJumpTarget {
                target: target as u32,
                code_len,
            });
        }
        let frame = self.current_frame.as_mut().ok_or(VmError::NoActiveFrame)?;
        frame.pc = target;
        Ok(())
    }

    fn finish_return(&mut self, value: Option<Value>) -> Result<StepEffect, VmError> {
        let current = self.current_frame.take().ok_or(VmError::NoActiveFrame)?;
        self.stack.truncate(current.base_sp);
        if let Some(value) = value.clone() {
            self.stack.push(value);
        }

        if let Some(previous) = self.call_stack.pop() {
            self.current_frame = Some(previous);
            Ok(StepEffect::Continue)
        } else {
            Ok(StepEffect::Halted(value))
        }
    }

    fn pop_stack(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn peek_stack(&self) -> Result<&Value, VmError> {
        self.stack.last().ok_or(VmError::StackUnderflow)
    }

    fn ensure_global(&mut self, index: usize) {
        if self.globals.len() <= index {
            self.globals.resize(index + 1, Value::Nil);
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            128,
        ))
    }
}

/// Worker-facing scheduler state.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkerState {
    Runnable,
    WaitingMessage,
    Sleeping,
    Halted,
    Error(VmError),
}

/// Cooperative scheduler for VM workers.
pub struct Scheduler {
    workers: BTreeMap<WorkerId, Vm>,
    runnable: VecDeque<WorkerId>,
    waiting: BTreeSet<WorkerId>,
    sleeping: BTreeSet<WorkerId>,
    halted: BTreeSet<WorkerId>,
    errors: BTreeMap<WorkerId, VmError>,
    next_worker_id: WorkerId,
}

/// Serializable snapshot of the scheduler and all workers.
#[derive(Clone, Debug, PartialEq)]
pub struct SchedulerSnapshot {
    pub workers: BTreeMap<WorkerId, VmSnapshot>,
    pub runnable: VecDeque<WorkerId>,
    pub waiting: BTreeSet<WorkerId>,
    pub sleeping: BTreeSet<WorkerId>,
    pub halted: BTreeSet<WorkerId>,
    pub errors: BTreeMap<WorkerId, VmError>,
    pub next_worker_id: WorkerId,
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            workers: BTreeMap::new(),
            runnable: VecDeque::new(),
            waiting: BTreeSet::new(),
            sleeping: BTreeSet::new(),
            halted: BTreeSet::new(),
            errors: BTreeMap::new(),
            next_worker_id: 1,
        }
    }

    /// Spawns a worker VM and returns its id.
    pub fn spawn(&mut self, mut vm: Vm) -> WorkerId {
        let worker_id = self.next_worker_id;
        self.next_worker_id = self.next_worker_id.saturating_add(1);
        vm.set_worker_id(worker_id);
        self.workers.insert(worker_id, vm);
        self.runnable.push_back(worker_id);
        worker_id
    }

    /// Returns a worker VM by id.
    pub fn worker(&self, worker_id: WorkerId) -> Option<&Vm> {
        self.workers.get(&worker_id)
    }

    /// Returns a mutable worker VM by id.
    pub fn worker_mut(&mut self, worker_id: WorkerId) -> Option<&mut Vm> {
        self.workers.get_mut(&worker_id)
    }

    /// Returns all worker ids currently known to the scheduler.
    pub fn worker_ids(&self) -> impl Iterator<Item = WorkerId> + '_ {
        self.workers.keys().copied()
    }

    /// Returns a serializable snapshot of the scheduler and all workers.
    pub fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            workers: self
                .workers
                .iter()
                .map(|(worker_id, vm)| (*worker_id, vm.snapshot()))
                .collect(),
            runnable: self.runnable.clone(),
            waiting: self.waiting.clone(),
            sleeping: self.sleeping.clone(),
            halted: self.halted.clone(),
            errors: self.errors.clone(),
            next_worker_id: self.next_worker_id,
        }
    }

    /// Restores a scheduler from a snapshot.
    pub fn from_snapshot(
        snapshot: SchedulerSnapshot,
        mut host_api_factory: impl FnMut(&VmConfig) -> Box<dyn HostApi>,
    ) -> Self {
        let workers = snapshot
            .workers
            .into_iter()
            .map(|(worker_id, vm_snapshot)| {
                let host_api = host_api_factory(&vm_snapshot.config);
                (worker_id, Vm::from_snapshot(vm_snapshot, host_api))
            })
            .collect();
        Self {
            workers,
            runnable: snapshot.runnable,
            waiting: snapshot.waiting,
            sleeping: snapshot.sleeping,
            halted: snapshot.halted,
            errors: snapshot.errors,
            next_worker_id: snapshot.next_worker_id,
        }
    }

    /// Returns the current state of a worker.
    pub fn worker_state(&self, worker_id: WorkerId) -> Option<WorkerState> {
        self.workers.get(&worker_id).map(|vm| match vm.state() {
            VmState::Idle | VmState::Running => {
                if self.runnable.contains(&worker_id) {
                    WorkerState::Runnable
                } else {
                    WorkerState::Runnable
                }
            }
            VmState::WaitingMessage => WorkerState::WaitingMessage,
            VmState::Sleeping => WorkerState::Sleeping,
            VmState::Halted => WorkerState::Halted,
            VmState::Error(error) => WorkerState::Error(error.clone()),
        })
    }

    /// Wakes a worker that is waiting or sleeping.
    pub fn wake(&mut self, worker_id: WorkerId) {
        self.waiting.remove(&worker_id);
        self.sleeping.remove(&worker_id);
        self.halted.remove(&worker_id);
        self.errors.remove(&worker_id);
        if self.workers.contains_key(&worker_id) && !self.runnable.contains(&worker_id) {
            self.runnable.push_back(worker_id);
        }
    }

    /// Delivers a message to the target worker.
    pub fn deliver(&mut self, message: Message) {
        if let Some(worker) = self.workers.get_mut(&message.to) {
            worker.push_message(message);
            if self.waiting.remove(&worker.worker_id()) {
                self.runnable.push_back(worker.worker_id());
            }
        }
    }

    /// Runs one scheduling round over the currently runnable workers.
    pub fn run_round(&mut self, step_limit: usize) -> Vec<(WorkerId, RunOutcome)> {
        let mut outcomes = Vec::new();
        let runnable_count = self.runnable.len();
        for _ in 0..runnable_count {
            let Some(worker_id) = self.runnable.pop_front() else {
                break;
            };
            let Some(vm) = self.workers.get_mut(&worker_id) else {
                continue;
            };

            let outcome = vm.run_frame(step_limit);
            self.route_outbox(worker_id);
            self.reconcile(worker_id, &outcome);
            outcomes.push((worker_id, outcome));
        }
        outcomes
    }

    fn reconcile(&mut self, worker_id: WorkerId, outcome: &RunOutcome) {
        self.waiting.remove(&worker_id);
        self.sleeping.remove(&worker_id);
        self.halted.remove(&worker_id);
        self.errors.remove(&worker_id);

        match outcome {
            RunOutcome::StepLimitReached { .. } | RunOutcome::Yielded { .. } => {
                self.runnable.push_back(worker_id);
            }
            RunOutcome::WaitingMessage { .. } => {
                self.waiting.insert(worker_id);
            }
            RunOutcome::Sleeping { .. } => {
                self.sleeping.insert(worker_id);
            }
            RunOutcome::Halted { .. } => {
                self.halted.insert(worker_id);
            }
            RunOutcome::Error { error, .. } => {
                self.errors.insert(worker_id, error.clone());
            }
        }
    }

    fn route_outbox(&mut self, worker_id: WorkerId) {
        let messages = self
            .workers
            .get_mut(&worker_id)
            .map(Vm::drain_outbox)
            .unwrap_or_default();
        for message in messages {
            self.deliver(message);
        }
    }

    /// Returns `true` when no worker is runnable.
    pub fn is_idle(&self) -> bool {
        self.runnable.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
