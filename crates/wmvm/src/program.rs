use std::collections::BTreeMap;
use std::rc::Rc;

use super::{ProgramCodecError, Value};

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
    const BINARY_MAGIC: [u8; 4] = *b"WMP1";
    const BINARY_VERSION: u16 = 1;

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

    pub fn encode_binary(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&Self::BINARY_MAGIC);
        out.extend_from_slice(&Self::BINARY_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.entry.unwrap_or(u16::MAX).to_le_bytes());
        out.extend_from_slice(&(self.constants.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.functions.len() as u32).to_le_bytes());
        for value in &self.constants {
            encode_program_value(value, &mut out);
        }
        for function in self.functions.values() {
            out.extend_from_slice(&function.id.to_le_bytes());
            out.push(function.arg_count);
            out.push(function.local_count);
            out.extend_from_slice(&(function.stack_max as u32).to_le_bytes());
            out.extend_from_slice(&(function.code.len() as u32).to_le_bytes());
            out.extend_from_slice(&function.code);
        }
        out
    }

    pub fn decode_binary(bytes: &[u8]) -> Result<Self, ProgramCodecError> {
        let mut cursor = ProgramDecodeCursor::new(bytes);
        let magic = cursor.read_bytes(4)?;
        if magic != Self::BINARY_MAGIC {
            return Err(ProgramCodecError::InvalidMagic);
        }
        let version = cursor.read_u16()?;
        if version != Self::BINARY_VERSION {
            return Err(ProgramCodecError::UnsupportedVersion(version));
        }
        let _reserved = cursor.read_u16()?;
        let entry = cursor.read_u16()?;
        let constant_count = cursor.read_u32()? as usize;
        let function_count = cursor.read_u32()? as usize;

        let mut program = Self::new();
        for _ in 0..constant_count {
            program.constants.push(decode_program_value(&mut cursor)?);
        }
        for _ in 0..function_count {
            let id = cursor.read_u16()?;
            let arg_count = cursor.read_u8()?;
            let local_count = cursor.read_u8()?;
            let stack_max = cursor.read_u32()? as usize;
            let code_len = cursor.read_u32()? as usize;
            let code = cursor.read_bytes(code_len)?.to_vec();
            let mut function = Function::new(id, code, arg_count, local_count);
            function.stack_max = stack_max;
            program.insert_function(function);
        }
        if entry != u16::MAX {
            program.set_entry(entry);
        }
        Ok(program)
    }
}

#[derive(Clone, Copy)]
struct ProgramDecodeCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ProgramDecodeCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, ProgramCodecError> {
        let byte = self
            .bytes
            .get(self.offset)
            .copied()
            .ok_or(ProgramCodecError::UnexpectedEof)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_u16(&mut self) -> Result<u16, ProgramCodecError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, ProgramCodecError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, ProgramCodecError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_i64(&mut self) -> Result<i64, ProgramCodecError> {
        Ok(self.read_u64()? as i64)
    }

    fn read_f64(&mut self) -> Result<f64, ProgramCodecError> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ProgramCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ProgramCodecError::UnexpectedEof)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProgramCodecError::UnexpectedEof)?;
        self.offset = end;
        Ok(slice)
    }
}

fn encode_program_value(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Nil => out.push(0),
        Value::Bool(false) => out.push(1),
        Value::Bool(true) => out.push(2),
        Value::Integer(value) => {
            out.push(3);
            out.extend_from_slice(&(*value as u64).to_le_bytes());
        }
        Value::Float(value) => {
            out.push(4);
            out.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        Value::String(value) => {
            out.push(5);
            out.extend_from_slice(&(value.len() as u32).to_le_bytes());
            out.extend_from_slice(value.as_bytes());
        }
        Value::Array(values) => {
            out.push(6);
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in values.iter() {
                encode_program_value(value, out);
            }
        }
        Value::Table(table) => {
            out.push(7);
            out.extend_from_slice(&(table.len() as u32).to_le_bytes());
            for (key, value) in table.iter() {
                out.extend_from_slice(&key.to_le_bytes());
                encode_program_value(value, out);
            }
        }
        Value::Handle(value) => {
            out.push(8);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn decode_program_value(cursor: &mut ProgramDecodeCursor<'_>) -> Result<Value, ProgramCodecError> {
    let tag = cursor.read_u8()?;
    match tag {
        0 => Ok(Value::Nil),
        1 => Ok(Value::Bool(false)),
        2 => Ok(Value::Bool(true)),
        3 => Ok(Value::Integer(cursor.read_i64()?)),
        4 => Ok(Value::Float(cursor.read_f64()?)),
        5 => {
            let len = cursor.read_u32()? as usize;
            let value = String::from_utf8(cursor.read_bytes(len)?.to_vec())
                .map_err(|_| ProgramCodecError::InvalidUtf8)?;
            Ok(Value::String(value))
        }
        6 => {
            let len = cursor.read_u32()? as usize;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(decode_program_value(cursor)?);
            }
            Ok(Value::Array(Rc::new(values)))
        }
        7 => {
            let len = cursor.read_u32()? as usize;
            let mut table = BTreeMap::new();
            for _ in 0..len {
                let key = cursor.read_u16()?;
                let value = decode_program_value(cursor)?;
                table.insert(key, value);
            }
            Ok(Value::Table(Rc::new(table)))
        }
        8 => Ok(Value::Handle(cursor.read_u64()?)),
        other => Err(ProgramCodecError::InvalidValueTag(other)),
    }
}
