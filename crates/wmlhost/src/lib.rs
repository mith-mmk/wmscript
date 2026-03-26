#![forbid(unsafe_code)]

//! Host integration layer for WML runtimes.
//!
//! The crate defines the small stable surface used by the VM to call into
//! platform services without exposing native details.

use std::collections::BTreeMap;

use wmlplatform::PlatformProfile;

/// Identifier of a host function.
pub type HostId = u16;

/// Capability bitmask for host functions.
pub type CapabilityMask = u32;

/// Metadata attached to a host function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostFunction {
    /// Stable host function identifier.
    pub id: HostId,
    /// Minimum number of accepted arguments.
    pub min_args: u8,
    /// Maximum number of accepted arguments.
    pub max_args: u8,
    /// Capability mask required by the host call.
    pub required_capabilities: CapabilityMask,
}

impl HostFunction {
    /// Creates a host function description.
    pub const fn new(
        id: HostId,
        min_args: u8,
        max_args: u8,
        required_capabilities: CapabilityMask,
    ) -> Self {
        Self {
            id,
            min_args,
            max_args,
            required_capabilities,
        }
    }
}

/// Host registry used by the VM bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostRegistry {
    profile: PlatformProfile,
    functions: BTreeMap<HostId, HostFunction>,
}

impl HostRegistry {
    /// Creates a registry for the given platform profile.
    pub fn new(profile: PlatformProfile) -> Self {
        Self {
            profile,
            functions: BTreeMap::new(),
        }
    }

    /// Returns the underlying platform profile.
    pub const fn profile(&self) -> PlatformProfile {
        self.profile
    }

    /// Registers a host function description.
    pub fn register(&mut self, function: HostFunction) -> Option<HostFunction> {
        self.functions.insert(function.id, function)
    }

    /// Returns a host function description by id.
    pub fn function(&self, id: HostId) -> Option<&HostFunction> {
        self.functions.get(&id)
    }

    /// Returns `true` when a host function is registered.
    pub fn contains(&self, id: HostId) -> bool {
        self.functions.contains_key(&id)
    }

    /// Returns an iterator over registered host function ids.
    pub fn function_ids(&self) -> impl Iterator<Item = HostId> + '_ {
        self.functions.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmlplatform::PlatformKind;

    #[test]
    fn registry_stores_platform_profile() {
        let registry = HostRegistry::new(PlatformProfile::native());
        assert_eq!(registry.profile().kind, PlatformKind::Native);
    }

    #[test]
    fn registry_registers_functions() {
        let mut registry = HostRegistry::new(PlatformProfile::native());
        let function = HostFunction::new(7, 1, 2, 0b101);

        assert!(registry.register(function).is_none());
        assert!(registry.contains(7));
        assert_eq!(registry.function(7), Some(&function));
    }
}
