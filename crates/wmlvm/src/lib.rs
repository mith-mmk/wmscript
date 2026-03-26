#![forbid(unsafe_code)]

//! Virtual machine crate for WML scripts.
//!
//! This crate will host the execution engine, scheduler integration, and the
//! worker model. The initial scaffold keeps the API surface small while the
//! implementation grows in later tasks.

use wmlbytecode::Opcode;
use wmlhost::HostRegistry;
use wmlplatform::PlatformProfile;

/// VM configuration shared by runtime builders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmConfig {
    /// Platform profile used to adapt native versus wasm behavior.
    pub platform: PlatformProfile,
    /// Host registry exposed to the VM.
    pub host_registry: HostRegistry,
    /// Maximum number of bytecode steps per frame.
    pub step_limit: usize,
}

impl VmConfig {
    /// Creates a new VM configuration.
    pub const fn new(
        platform: PlatformProfile,
        host_registry: HostRegistry,
        step_limit: usize,
    ) -> Self {
        Self {
            platform,
            host_registry,
            step_limit,
        }
    }
}

/// Lightweight VM placeholder that records its configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vm {
    config: VmConfig,
}

impl Vm {
    /// Creates a new VM instance.
    pub const fn new(config: VmConfig) -> Self {
        Self { config }
    }

    /// Returns the active configuration.
    pub const fn config(&self) -> VmConfig {
        self.config
    }

    /// Reports whether the VM knows a given opcode.
    pub const fn knows_opcode(opcode: Opcode) -> bool {
        matches!(
            opcode,
            Opcode::Nop
                | Opcode::Halt
                | Opcode::PushConst
                | Opcode::PushNil
                | Opcode::PushTrue
                | Opcode::PushFalse
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmlplatform::PlatformKind;

    #[test]
    fn vm_stores_configuration() {
        let platform = PlatformProfile::native();
        let registry = HostRegistry::new(platform);
        let vm = Vm::new(VmConfig::new(platform, registry, 128));

        assert_eq!(vm.config().platform.kind, PlatformKind::Native);
        assert_eq!(vm.config().step_limit, 128);
    }

    #[test]
    fn vm_recognizes_known_opcode_set() {
        assert!(Vm::knows_opcode(Opcode::Nop));
    }
}
