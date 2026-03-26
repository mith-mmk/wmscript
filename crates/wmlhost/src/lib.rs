#![forbid(unsafe_code)]

//! Host integration layer for WML runtimes.
//!
//! The crate defines the small stable surface used by the VM to call into
//! platform services without exposing native details.

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

/// Minimal host registry used by the VM bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostRegistry {
    profile: PlatformProfile,
}

impl HostRegistry {
    /// Creates a registry for the given platform profile.
    pub const fn new(profile: PlatformProfile) -> Self {
        Self { profile }
    }

    /// Returns the underlying platform profile.
    pub const fn profile(&self) -> PlatformProfile {
        self.profile
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
}
