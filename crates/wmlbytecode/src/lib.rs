#![forbid(unsafe_code)]

//! Bytecode types shared by the compiler and VM crates.
//!
//! The crate starts with the instruction model and low-level decode helpers so
//! the runtime implementation can grow without duplicating opcode definitions.

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
    /// Push a constant.
    PushConst = 0x10,
    /// Push nil.
    PushNil = 0x11,
    /// Push true.
    PushTrue = 0x12,
    /// Push false.
    PushFalse = 0x13,
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
            other => Err(BytecodeError::InvalidOpcode(other)),
        }
    }
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

    /// Reads the next opcode byte.
    pub fn read_opcode(&mut self) -> Result<Opcode> {
        let byte = self.read_u8()?;
        Opcode::from_byte(byte)
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
        let lo = self.read_u8()? as u16;
        let hi = self.read_u8()? as u16;
        Ok(lo | (hi << 8))
    }

    /// Reads a little-endian `u32`.
    pub fn read_u32(&mut self) -> Result<u32> {
        let b0 = self.read_u8()? as u32;
        let b1 = self.read_u8()? as u32;
        let b2 = self.read_u8()? as u32;
        let b3 = self.read_u8()? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }
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
}
