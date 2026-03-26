#![forbid(unsafe_code)]

//! Compiler crate for WML scripts.
//!
//! The later compiler pipeline will expand from this module. For now it exposes
//! a small planning surface so workspace consumers can depend on a stable crate.

use wmlbytecode::Opcode;
use wmlplatform::PlatformProfile;

/// Compiler configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerConfig {
    /// Target platform profile.
    pub platform: PlatformProfile,
}

impl CompilerConfig {
    /// Creates a new compiler configuration.
    pub const fn new(platform: PlatformProfile) -> Self {
        Self { platform }
    }
}

/// Placeholder compiler front end.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compiler {
    config: CompilerConfig,
}

impl Compiler {
    /// Creates a compiler for the given platform.
    pub const fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    /// Returns the compiler configuration.
    pub const fn config(&self) -> CompilerConfig {
        self.config
    }

    /// Reports whether a bytecode opcode is in the current bootstrap set.
    pub const fn supports_opcode(opcode: Opcode) -> bool {
        matches!(opcode, Opcode::Nop | Opcode::Halt | Opcode::PushConst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_keeps_config() {
        let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
        assert!(compiler.config().platform.capabilities.file_system);
    }
}
