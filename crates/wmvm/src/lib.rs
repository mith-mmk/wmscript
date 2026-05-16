#![forbid(unsafe_code)]

//! Virtual machine crate for WML scripts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use wmbytecode::{BytecodeError, Op, Opcode, decode_at};
use wmhost::{CapabilityMask, HostId, HostRegistry};
use wmplatform::PlatformProfile;

mod message;
mod program;
mod scheduler;
mod value;

pub use message::Message;
pub use program::{Function, Program};
pub use scheduler::{Scheduler, SchedulerSnapshot, WorkerState};
pub use value::Value;

/// Worker identifier.
pub type WorkerId = u32;

/// Binary codec error for compiled programs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramCodecError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnexpectedEof,
    InvalidUtf8,
    InvalidValueTag(u8),
}

impl core::fmt::Display for ProgramCodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidMagic => f.write_str("invalid program magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported program version: {version}")
            }
            Self::UnexpectedEof => f.write_str("unexpected end of program data"),
            Self::InvalidUtf8 => f.write_str("invalid utf-8 string in program data"),
            Self::InvalidValueTag(tag) => write!(f, "invalid program value tag: {tag}"),
        }
    }
}

impl std::error::Error for ProgramCodecError {}

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

    /// Wakes a sleeping or waiting worker VM so it can be scheduled again.
    pub fn wake(&mut self) {
        if matches!(self.state, VmState::Sleeping | VmState::WaitingMessage) {
            self.state = VmState::Idle;
        }
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
        let next_pc = pc.checked_add(len).ok_or(VmError::InvalidJumpTarget {
            target: u32::MAX,
            code_len: code.len(),
        })?;
        let frame = self.current_frame.as_mut().ok_or(VmError::NoActiveFrame)?;
        frame.pc = next_pc;
        let effect = self.execute_op(op, code.len())?;
        if matches!(effect, StepEffect::WaitingMessage)
            && let Some(frame) = self.current_frame.as_mut()
        {
            frame.pc = pc;
        }
        Ok(effect)
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

#[cfg(test)]
#[path = "../tests/support/lib_tests.rs"]
mod tests;
