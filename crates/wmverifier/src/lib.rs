#![forbid(unsafe_code)]

//! Bytecode verifier for WML programs.
//!
//! The verifier checks structural validity before execution:
//! function references, host references, jump targets, and constant lookups.

use std::fmt;

use wmbytecode::{BytecodeError, Op, decode_at};
use wmhost::HostRegistry;
use wmvm::Program;

/// Result type used by the verifier.
pub type Result<T> = core::result::Result<T, VerificationError>;

/// Verification error classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationError {
    /// The program refers to a missing entry function.
    MissingEntry(u16),
    /// Bytecode could not be decoded at the given location.
    InvalidOpcode {
        func_id: u16,
        pc: usize,
        error: BytecodeError,
    },
    /// A jump target escaped the current function body.
    InvalidJumpTarget {
        func_id: u16,
        pc: usize,
        target: u32,
        code_len: usize,
    },
    /// A constant reference is outside the constant pool.
    InvalidConstantIndex { func_id: u16, pc: usize, index: u16 },
    /// A script call points at a missing function.
    InvalidFunctionTarget {
        func_id: u16,
        pc: usize,
        target: u16,
    },
    /// A host call points at a missing host function.
    InvalidHostTarget {
        func_id: u16,
        pc: usize,
        host_id: u16,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEntry(func_id) => write!(f, "missing entry function: {func_id}"),
            Self::InvalidOpcode { func_id, pc, error } => {
                write!(f, "invalid opcode in function {func_id} at {pc}: {error}")
            }
            Self::InvalidJumpTarget {
                func_id,
                pc,
                target,
                code_len,
            } => {
                write!(
                    f,
                    "invalid jump target in function {func_id} at {pc}: {target} (code length {code_len})"
                )
            }
            Self::InvalidConstantIndex { func_id, pc, index } => {
                write!(
                    f,
                    "invalid constant index in function {func_id} at {pc}: {index}"
                )
            }
            Self::InvalidFunctionTarget {
                func_id,
                pc,
                target,
            } => {
                write!(
                    f,
                    "invalid function target in function {func_id} at {pc}: {target}"
                )
            }
            Self::InvalidHostTarget {
                func_id,
                pc,
                host_id,
            } => {
                write!(
                    f,
                    "invalid host target in function {func_id} at {pc}: {host_id}"
                )
            }
        }
    }
}

impl std::error::Error for VerificationError {}

/// Verifier over a program and host registry.
pub struct Verifier<'a> {
    program: &'a Program,
    host_registry: &'a HostRegistry,
}

impl<'a> Verifier<'a> {
    pub fn new(program: &'a Program, host_registry: &'a HostRegistry) -> Self {
        Self {
            program,
            host_registry,
        }
    }

    pub fn verify(&self) -> Result<()> {
        if let Some(entry) = self.program.entry() {
            if self.program.function(entry).is_none() {
                return Err(VerificationError::MissingEntry(entry));
            }
        }

        for func_id in self.program.function_ids() {
            let function = self
                .program
                .function(func_id)
                .expect("function id obtained from function_ids");
            self.verify_function(function)?;
        }
        Ok(())
    }

    fn verify_function(&self, function: &wmvm::Function) -> Result<()> {
        let code = function.code.as_slice();
        let mut pc = 0usize;

        while pc < code.len() {
            let (op, len) =
                decode_at(code, pc).map_err(|error| VerificationError::InvalidOpcode {
                    func_id: function.id,
                    pc,
                    error,
                })?;

            self.verify_op(function.id, pc, code.len(), &op)?;
            pc = pc.saturating_add(len);
        }

        Ok(())
    }

    fn verify_op(&self, func_id: u16, pc: usize, code_len: usize, op: &Op) -> Result<()> {
        match *op {
            Op::PushConst(index) => {
                if self.program.constant(index).is_none() {
                    return Err(VerificationError::InvalidConstantIndex { func_id, pc, index });
                }
            }
            Op::Jump(target) | Op::JumpIfFalse(target) | Op::JumpIfTrue(target) => {
                let target = target as usize;
                if target > code_len {
                    return Err(VerificationError::InvalidJumpTarget {
                        func_id,
                        pc,
                        target: target as u32,
                        code_len,
                    });
                }
            }
            Op::Call(target, _) => {
                if self.program.function(target).is_none() {
                    return Err(VerificationError::InvalidFunctionTarget {
                        func_id,
                        pc,
                        target,
                    });
                }
            }
            Op::CallHost(host_id, _) => {
                if !self.host_registry.contains(host_id) {
                    return Err(VerificationError::InvalidHostTarget {
                        func_id,
                        pc,
                        host_id,
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }
}

/// Verifies a program against a host registry.
pub fn verify_program(program: &Program, host_registry: &HostRegistry) -> Result<()> {
    Verifier::new(program, host_registry).verify()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmhost::HostRegistry;
    use wmplatform::PlatformProfile;
    use wmvm::{Function, Value};

    fn registry() -> HostRegistry {
        HostRegistry::new(PlatformProfile::native())
    }

    #[test]
    fn verifier_accepts_valid_program() {
        let mut program = Program::new();
        let index = program.push_constant(Value::Integer(42));
        program.insert_function(Function::new(1, vec![0x10, index as u8, 0x00, 0x72], 0, 0));
        program.set_entry(1);

        assert_eq!(verify_program(&program, &registry()), Ok(()));
    }

    #[test]
    fn verifier_rejects_invalid_opcode() {
        let mut program = Program::new();
        program.insert_function(Function::new(1, vec![0xFF], 0, 0));
        program.set_entry(1);

        assert!(matches!(
            verify_program(&program, &registry()),
            Err(VerificationError::InvalidOpcode {
                func_id: 1,
                pc: 0,
                ..
            })
        ));
    }

    #[test]
    fn verifier_rejects_invalid_jump_target() {
        let mut program = Program::new();
        program.insert_function(Function::new(1, vec![0x60, 0x08, 0x00, 0x00, 0x00], 0, 0));
        program.set_entry(1);

        assert!(matches!(
            verify_program(&program, &registry()),
            Err(VerificationError::InvalidJumpTarget {
                func_id: 1,
                pc: 0,
                target: 8,
                code_len: 5
            })
        ));
    }

    #[test]
    fn verifier_rejects_invalid_host_target() {
        let mut program = Program::new();
        program.insert_function(Function::new(1, vec![0x71, 0x07, 0x00, 0x00], 0, 0));
        program.set_entry(1);

        assert!(matches!(
            verify_program(&program, &registry()),
            Err(VerificationError::InvalidHostTarget {
                func_id: 1,
                pc: 0,
                host_id: 7
            })
        ));
    }

    #[test]
    fn verifier_rejects_invalid_function_target() {
        let mut program = Program::new();
        program.insert_function(Function::new(1, vec![0x70, 0x02, 0x00, 0x00], 0, 0));
        program.set_entry(1);

        assert!(matches!(
            verify_program(&program, &registry()),
            Err(VerificationError::InvalidFunctionTarget {
                func_id: 1,
                pc: 0,
                target: 2
            })
        ));
    }
}
