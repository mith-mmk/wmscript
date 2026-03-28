#![forbid(unsafe_code)]

//! Bytecode types shared by the compiler and VM crates.
//!
//! The crate defines the opcode universe, operand-bearing instruction model,
//! and little-endian decode helpers used by the runtime.

/// Result type used by bytecode helpers.
pub type Result<T> = core::result::Result<T, BytecodeError>;

/// Errors raised while handling bytecode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BytecodeError {
    /// An opcode number was not recognized.
    InvalidOpcode(u8),
    /// A read would go past the end of the input buffer.
    UnexpectedEof,
}

impl core::fmt::Display for BytecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidOpcode(opcode) => write!(f, "invalid opcode: 0x{opcode:02X}"),
            Self::UnexpectedEof => f.write_str("unexpected end of bytecode"),
        }
    }
}

impl std::error::Error for BytecodeError {}

/// Raw opcode identifiers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opcode {
    /// No operation.
    Nop = 0x00,
    /// Halt execution.
    Halt = 0x01,
    /// Push a constant from the constant pool.
    PushConst = 0x10,
    /// Push nil.
    PushNil = 0x11,
    /// Push true.
    PushTrue = 0x12,
    /// Push false.
    PushFalse = 0x13,
    /// Load a local value.
    LoadLocal = 0x20,
    /// Store a local value.
    StoreLocal = 0x21,
    /// Load a global value.
    LoadGlobal = 0x22,
    /// Store a global value.
    StoreGlobal = 0x23,
    /// Load a field from a table.
    LoadField = 0x24,
    /// Store a field into a table.
    StoreField = 0x25,
    /// Load an index from an array or table.
    LoadIndex = 0x26,
    /// Store an index into an array or table.
    StoreIndex = 0x27,
    /// Pop the top of the stack.
    Pop = 0x30,
    /// Duplicate the top of the stack.
    Dup = 0x31,
    /// Add two operands.
    Add = 0x40,
    /// Subtract two operands.
    Sub = 0x41,
    /// Multiply two operands.
    Mul = 0x42,
    /// Divide two operands.
    Div = 0x43,
    /// Modulo of two operands.
    Mod = 0x44,
    /// Negate the top operand.
    Neg = 0x45,
    /// Equality comparison.
    Eq = 0x50,
    /// Inequality comparison.
    Ne = 0x51,
    /// Less-than comparison.
    Lt = 0x52,
    /// Less-than-or-equal comparison.
    Le = 0x53,
    /// Greater-than comparison.
    Gt = 0x54,
    /// Greater-than-or-equal comparison.
    Ge = 0x55,
    /// Logical not.
    Not = 0x56,
    /// Jump to an absolute address.
    Jump = 0x60,
    /// Jump to an absolute address when the condition is false.
    JumpIfFalse = 0x61,
    /// Jump to an absolute address when the condition is true.
    JumpIfTrue = 0x62,
    /// Call a script function.
    Call = 0x70,
    /// Call a host function.
    CallHost = 0x71,
    /// Return from the current function.
    Return = 0x72,
    /// Allocate an array.
    NewArray = 0x80,
    /// Allocate a table.
    NewTable = 0x81,
    /// Send a message to another worker.
    Send = 0x90,
    /// Receive a message, blocking if necessary.
    Recv = 0x91,
    /// Try to receive a message without blocking.
    TryRecv = 0x92,
    /// Yield control to the scheduler.
    Yield = 0xA0,
    /// Sleep until the scheduler wakes the worker.
    Sleep = 0xA1,
}

impl Opcode {
    /// Converts a raw opcode byte into a typed opcode.
    pub fn from_byte(byte: u8) -> Result<Self> {
        match byte {
            0x00 => Ok(Self::Nop),
            0x01 => Ok(Self::Halt),
            0x10 => Ok(Self::PushConst),
            0x11 => Ok(Self::PushNil),
            0x12 => Ok(Self::PushTrue),
            0x13 => Ok(Self::PushFalse),
            0x20 => Ok(Self::LoadLocal),
            0x21 => Ok(Self::StoreLocal),
            0x22 => Ok(Self::LoadGlobal),
            0x23 => Ok(Self::StoreGlobal),
            0x24 => Ok(Self::LoadField),
            0x25 => Ok(Self::StoreField),
            0x26 => Ok(Self::LoadIndex),
            0x27 => Ok(Self::StoreIndex),
            0x30 => Ok(Self::Pop),
            0x31 => Ok(Self::Dup),
            0x40 => Ok(Self::Add),
            0x41 => Ok(Self::Sub),
            0x42 => Ok(Self::Mul),
            0x43 => Ok(Self::Div),
            0x44 => Ok(Self::Mod),
            0x45 => Ok(Self::Neg),
            0x50 => Ok(Self::Eq),
            0x51 => Ok(Self::Ne),
            0x52 => Ok(Self::Lt),
            0x53 => Ok(Self::Le),
            0x54 => Ok(Self::Gt),
            0x55 => Ok(Self::Ge),
            0x56 => Ok(Self::Not),
            0x60 => Ok(Self::Jump),
            0x61 => Ok(Self::JumpIfFalse),
            0x62 => Ok(Self::JumpIfTrue),
            0x70 => Ok(Self::Call),
            0x71 => Ok(Self::CallHost),
            0x72 => Ok(Self::Return),
            0x80 => Ok(Self::NewArray),
            0x81 => Ok(Self::NewTable),
            0x90 => Ok(Self::Send),
            0x91 => Ok(Self::Recv),
            0x92 => Ok(Self::TryRecv),
            0xA0 => Ok(Self::Yield),
            0xA1 => Ok(Self::Sleep),
            other => Err(BytecodeError::InvalidOpcode(other)),
        }
    }
}

/// Decoded instruction set used by the VM.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// No operation.
    Nop,
    /// Halt execution.
    Halt,
    /// Push a constant from the constant pool.
    PushConst(u16),
    /// Push nil.
    PushNil,
    /// Push true.
    PushTrue,
    /// Push false.
    PushFalse,
    /// Load a local value.
    LoadLocal(u8),
    /// Store a local value.
    StoreLocal(u8),
    /// Load a global value.
    LoadGlobal(u16),
    /// Store a global value.
    StoreGlobal(u16),
    /// Load a field from a table.
    LoadField(u16),
    /// Store a field into a table.
    StoreField(u16),
    /// Load an index from an array or table.
    LoadIndex,
    /// Store an index into an array or table.
    StoreIndex,
    /// Pop the top of the stack.
    Pop,
    /// Duplicate the top of the stack.
    Dup,
    /// Add two operands.
    Add,
    /// Subtract two operands.
    Sub,
    /// Multiply two operands.
    Mul,
    /// Divide two operands.
    Div,
    /// Modulo of two operands.
    Mod,
    /// Negate the top operand.
    Neg,
    /// Equality comparison.
    Eq,
    /// Inequality comparison.
    Ne,
    /// Less-than comparison.
    Lt,
    /// Less-than-or-equal comparison.
    Le,
    /// Greater-than comparison.
    Gt,
    /// Greater-than-or-equal comparison.
    Ge,
    /// Logical not.
    Not,
    /// Jump to an absolute address.
    Jump(u32),
    /// Jump to an absolute address when the condition is false.
    JumpIfFalse(u32),
    /// Jump to an absolute address when the condition is true.
    JumpIfTrue(u32),
    /// Call a script function.
    Call(u16, u8),
    /// Call a host function.
    CallHost(u16, u8),
    /// Return from the current function.
    Return,
    /// Allocate an array.
    NewArray(u16),
    /// Allocate a table.
    NewTable(u16),
    /// Send a message to another worker.
    Send(u16, u8),
    /// Receive a message, blocking if necessary.
    Recv,
    /// Try to receive a message without blocking.
    TryRecv,
    /// Yield control to the scheduler.
    Yield,
    /// Sleep until the scheduler wakes the worker.
    Sleep,
}

/// Simple cursor over a bytecode buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BytecodeCursor<'a> {
    code: &'a [u8],
    pc: usize,
}

impl<'a> BytecodeCursor<'a> {
    /// Creates a new cursor.
    pub const fn new(code: &'a [u8]) -> Self {
        Self { code, pc: 0 }
    }

    /// Returns the current program counter.
    pub const fn pc(&self) -> usize {
        self.pc
    }

    /// Returns `true` when the cursor has reached the end.
    pub fn is_end(&self) -> bool {
        self.pc >= self.code.len()
    }

    /// Reads the next raw opcode byte.
    pub fn read_opcode(&mut self) -> Result<Opcode> {
        let byte = self.read_u8()?;
        Opcode::from_byte(byte)
    }

    /// Reads the next decoded instruction.
    pub fn read_op(&mut self) -> Result<Op> {
        let (op, len) = decode_at(self.code, self.pc)?;
        self.pc += len;
        Ok(op)
    }

    /// Reads a little-endian `u8`.
    pub fn read_u8(&mut self) -> Result<u8> {
        let byte = self
            .code
            .get(self.pc)
            .copied()
            .ok_or(BytecodeError::UnexpectedEof)?;
        self.pc += 1;
        Ok(byte)
    }

    /// Reads a little-endian `u16`.
    pub fn read_u16(&mut self) -> Result<u16> {
        read_u16_at(self.code, self.pc).inspect(|_| {
            self.pc += 2;
        })
    }

    /// Reads a little-endian `u32`.
    pub fn read_u32(&mut self) -> Result<u32> {
        read_u32_at(self.code, self.pc).inspect(|_| {
            self.pc += 4;
        })
    }
}

/// Decodes one instruction at `pc`, returning the instruction and its length.
pub fn decode_at(code: &[u8], pc: usize) -> Result<(Op, usize)> {
    let opcode = *code.get(pc).ok_or(BytecodeError::UnexpectedEof)?;
    let op = match Opcode::from_byte(opcode)? {
        Opcode::Nop => Op::Nop,
        Opcode::Halt => Op::Halt,
        Opcode::PushConst => Op::PushConst(read_u16_at(code, pc + 1)?),
        Opcode::PushNil => Op::PushNil,
        Opcode::PushTrue => Op::PushTrue,
        Opcode::PushFalse => Op::PushFalse,
        Opcode::LoadLocal => Op::LoadLocal(read_u8_at(code, pc + 1)?),
        Opcode::StoreLocal => Op::StoreLocal(read_u8_at(code, pc + 1)?),
        Opcode::LoadGlobal => Op::LoadGlobal(read_u16_at(code, pc + 1)?),
        Opcode::StoreGlobal => Op::StoreGlobal(read_u16_at(code, pc + 1)?),
        Opcode::LoadField => Op::LoadField(read_u16_at(code, pc + 1)?),
        Opcode::StoreField => Op::StoreField(read_u16_at(code, pc + 1)?),
        Opcode::LoadIndex => Op::LoadIndex,
        Opcode::StoreIndex => Op::StoreIndex,
        Opcode::Pop => Op::Pop,
        Opcode::Dup => Op::Dup,
        Opcode::Add => Op::Add,
        Opcode::Sub => Op::Sub,
        Opcode::Mul => Op::Mul,
        Opcode::Div => Op::Div,
        Opcode::Mod => Op::Mod,
        Opcode::Neg => Op::Neg,
        Opcode::Eq => Op::Eq,
        Opcode::Ne => Op::Ne,
        Opcode::Lt => Op::Lt,
        Opcode::Le => Op::Le,
        Opcode::Gt => Op::Gt,
        Opcode::Ge => Op::Ge,
        Opcode::Not => Op::Not,
        Opcode::Jump => Op::Jump(read_u32_at(code, pc + 1)?),
        Opcode::JumpIfFalse => Op::JumpIfFalse(read_u32_at(code, pc + 1)?),
        Opcode::JumpIfTrue => Op::JumpIfTrue(read_u32_at(code, pc + 1)?),
        Opcode::Call => Op::Call(read_u16_at(code, pc + 1)?, read_u8_at(code, pc + 3)?),
        Opcode::CallHost => Op::CallHost(read_u16_at(code, pc + 1)?, read_u8_at(code, pc + 3)?),
        Opcode::Return => Op::Return,
        Opcode::NewArray => Op::NewArray(read_u16_at(code, pc + 1)?),
        Opcode::NewTable => Op::NewTable(read_u16_at(code, pc + 1)?),
        Opcode::Send => Op::Send(read_u16_at(code, pc + 1)?, read_u8_at(code, pc + 3)?),
        Opcode::Recv => Op::Recv,
        Opcode::TryRecv => Op::TryRecv,
        Opcode::Yield => Op::Yield,
        Opcode::Sleep => Op::Sleep,
    };

    Ok((op, instruction_len(opcode)))
}

/// Returns the byte length of a raw opcode instruction.
pub fn instruction_len(opcode: u8) -> usize {
    match opcode {
        0x00 | 0x01 | 0x11 | 0x12 | 0x13 | 0x26 | 0x27 | 0x30 | 0x31 | 0x40 | 0x41 | 0x42
        | 0x43 | 0x44 | 0x45 | 0x50 | 0x51 | 0x52 | 0x53 | 0x54 | 0x55 | 0x56 | 0x72 | 0x91
        | 0x92 | 0xA0 | 0xA1 => 1,
        0x10 | 0x22 | 0x23 | 0x24 | 0x25 | 0x80 | 0x81 => 3,
        0x20 | 0x21 => 2,
        0x60 | 0x61 | 0x62 => 5,
        0x70 | 0x71 | 0x90 => 4,
        _ => 1,
    }
}

/// Encodes one decoded instruction into a byte buffer.
pub fn encode_op(op: &Op, out: &mut Vec<u8>) {
    match op {
        Op::Nop => out.push(Opcode::Nop as u8),
        Op::Halt => out.push(Opcode::Halt as u8),
        Op::PushConst(index) => {
            out.push(Opcode::PushConst as u8);
            append_u16(out, *index);
        }
        Op::PushNil => out.push(Opcode::PushNil as u8),
        Op::PushTrue => out.push(Opcode::PushTrue as u8),
        Op::PushFalse => out.push(Opcode::PushFalse as u8),
        Op::LoadLocal(index) => {
            out.push(Opcode::LoadLocal as u8);
            out.push(*index);
        }
        Op::StoreLocal(index) => {
            out.push(Opcode::StoreLocal as u8);
            out.push(*index);
        }
        Op::LoadGlobal(index) => {
            out.push(Opcode::LoadGlobal as u8);
            append_u16(out, *index);
        }
        Op::StoreGlobal(index) => {
            out.push(Opcode::StoreGlobal as u8);
            append_u16(out, *index);
        }
        Op::LoadField(index) => {
            out.push(Opcode::LoadField as u8);
            append_u16(out, *index);
        }
        Op::StoreField(index) => {
            out.push(Opcode::StoreField as u8);
            append_u16(out, *index);
        }
        Op::LoadIndex => out.push(Opcode::LoadIndex as u8),
        Op::StoreIndex => out.push(Opcode::StoreIndex as u8),
        Op::Pop => out.push(Opcode::Pop as u8),
        Op::Dup => out.push(Opcode::Dup as u8),
        Op::Add => out.push(Opcode::Add as u8),
        Op::Sub => out.push(Opcode::Sub as u8),
        Op::Mul => out.push(Opcode::Mul as u8),
        Op::Div => out.push(Opcode::Div as u8),
        Op::Mod => out.push(Opcode::Mod as u8),
        Op::Neg => out.push(Opcode::Neg as u8),
        Op::Eq => out.push(Opcode::Eq as u8),
        Op::Ne => out.push(Opcode::Ne as u8),
        Op::Lt => out.push(Opcode::Lt as u8),
        Op::Le => out.push(Opcode::Le as u8),
        Op::Gt => out.push(Opcode::Gt as u8),
        Op::Ge => out.push(Opcode::Ge as u8),
        Op::Not => out.push(Opcode::Not as u8),
        Op::Jump(target) => {
            out.push(Opcode::Jump as u8);
            append_u32(out, *target);
        }
        Op::JumpIfFalse(target) => {
            out.push(Opcode::JumpIfFalse as u8);
            append_u32(out, *target);
        }
        Op::JumpIfTrue(target) => {
            out.push(Opcode::JumpIfTrue as u8);
            append_u32(out, *target);
        }
        Op::Call(func_id, argc) => {
            out.push(Opcode::Call as u8);
            append_u16(out, *func_id);
            out.push(*argc);
        }
        Op::CallHost(host_id, argc) => {
            out.push(Opcode::CallHost as u8);
            append_u16(out, *host_id);
            out.push(*argc);
        }
        Op::Return => out.push(Opcode::Return as u8),
        Op::NewArray(size_hint) => {
            out.push(Opcode::NewArray as u8);
            append_u16(out, *size_hint);
        }
        Op::NewTable(size_hint) => {
            out.push(Opcode::NewTable as u8);
            append_u16(out, *size_hint);
        }
        Op::Send(worker_id, argc) => {
            out.push(Opcode::Send as u8);
            append_u16(out, *worker_id);
            out.push(*argc);
        }
        Op::Recv => out.push(Opcode::Recv as u8),
        Op::TryRecv => out.push(Opcode::TryRecv as u8),
        Op::Yield => out.push(Opcode::Yield as u8),
        Op::Sleep => out.push(Opcode::Sleep as u8),
    }
}

/// Encodes a sequence of instructions into a byte buffer.
pub fn encode_ops<I>(ops: I) -> Vec<u8>
where
    I: IntoIterator<Item = Op>,
{
    let mut out = Vec::new();
    for op in ops {
        encode_op(&op, &mut out);
    }
    out
}

fn read_u8_at(code: &[u8], pc: usize) -> Result<u8> {
    code.get(pc).copied().ok_or(BytecodeError::UnexpectedEof)
}

fn read_u16_at(code: &[u8], pc: usize) -> Result<u16> {
    let lo = read_u8_at(code, pc)? as u16;
    let hi = read_u8_at(code, pc + 1)? as u16;
    Ok(lo | (hi << 8))
}

fn read_u32_at(code: &[u8], pc: usize) -> Result<u32> {
    let b0 = read_u8_at(code, pc)? as u32;
    let b1 = read_u8_at(code, pc + 1)? as u32;
    let b2 = read_u8_at(code, pc + 2)? as u32;
    let b3 = read_u8_at(code, pc + 3)? as u32;
    Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
}

fn append_u16(out: &mut Vec<u8>, value: u16) {
    out.push((value & 0xFF) as u8);
    out.push((value >> 8) as u8);
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.push((value & 0xFF) as u8);
    out.push(((value >> 8) & 0xFF) as u8);
    out.push(((value >> 16) & 0xFF) as u8);
    out.push(((value >> 24) & 0xFF) as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_lookup_accepts_known_bytes() {
        assert_eq!(Opcode::from_byte(0x10), Ok(Opcode::PushConst));
        assert_eq!(
            Opcode::from_byte(0x99),
            Err(BytecodeError::InvalidOpcode(0x99))
        );
    }

    #[test]
    fn cursor_reads_little_endian_values() {
        let mut cursor = BytecodeCursor::new(&[0x10, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12]);

        assert_eq!(cursor.read_opcode(), Ok(Opcode::PushConst));
        assert_eq!(cursor.read_u16(), Ok(0x1234));
        assert_eq!(cursor.read_u32(), Ok(0x12345678));
    }

    #[test]
    fn decode_instruction_with_operands() {
        let (op, len) = decode_at(&[0x70, 0x34, 0x12, 0x05], 0).expect("decode");
        assert_eq!(op, Op::Call(0x1234, 5));
        assert_eq!(len, 4);
    }

    #[test]
    fn encode_roundtrip_matches_decode() {
        let ops = vec![Op::PushConst(0x1234), Op::Call(7, 2), Op::Return];
        let encoded = encode_ops(ops.clone());
        let mut cursor = BytecodeCursor::new(&encoded);

        for expected in ops {
            assert_eq!(cursor.read_op().expect("read op"), expected);
        }
        assert!(cursor.is_end());
    }
}
