#![forbid(unsafe_code)]

//! Shared platform abstraction for the WML workspace.
//!
//! This crate keeps the target-specific differences between native, wasm, and
//! egui-driven builds in one place so the runtime crates can stay focused on
//! VM, bytecode, and host concerns.

use core::fmt;

/// Runtime target family used by the workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformKind {
    /// Native desktop or server execution.
    Native,
    /// WebAssembly execution in a browser or embedded runtime.
    Wasm,
    /// Native execution with an egui front end.
    Egui,
}

/// Capability flags exposed to higher layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCapabilities {
    /// Whether blocking file system access is expected to work.
    pub file_system: bool,
    /// Whether async host integration is expected to be available.
    pub async_io: bool,
    /// Whether a GUI event loop is available.
    pub gui: bool,
    /// Whether network access is expected to be available.
    pub network: bool,
    /// Whether the build can rely on wasm target assumptions.
    pub web_compat: bool,
}

impl PlatformCapabilities {
    /// Creates a new capability set.
    pub const fn new(
        file_system: bool,
        async_io: bool,
        gui: bool,
        network: bool,
        web_compat: bool,
    ) -> Self {
        Self {
            file_system,
            async_io,
            gui,
            network,
            web_compat,
        }
    }
}

/// Platform profile composed of kind and capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformProfile {
    /// Family of the current runtime target.
    pub kind: PlatformKind,
    /// Capability flags for the target.
    pub capabilities: PlatformCapabilities,
}

impl PlatformProfile {
    /// Builds a native profile.
    pub const fn native() -> Self {
        Self {
            kind: PlatformKind::Native,
            capabilities: PlatformCapabilities::new(true, true, true, true, false),
        }
    }

    /// Builds a wasm profile.
    pub const fn wasm() -> Self {
        Self {
            kind: PlatformKind::Wasm,
            capabilities: PlatformCapabilities::new(false, true, false, false, true),
        }
    }

    /// Builds an egui profile.
    pub const fn egui() -> Self {
        Self {
            kind: PlatformKind::Egui,
            capabilities: PlatformCapabilities::new(true, true, true, true, false),
        }
    }
}

/// Returns the profile for the active compilation target.
pub const fn current_profile() -> PlatformProfile {
    if cfg!(target_arch = "wasm32") {
        PlatformProfile::wasm()
    } else {
        PlatformProfile::native()
    }
}

/// Returns the active platform kind.
pub const fn current_kind() -> PlatformKind {
    current_profile().kind
}

/// Error returned by platform-specific adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    /// The current platform cannot support a requested feature.
    UnsupportedFeature(&'static str),
    /// The provided configuration is invalid.
    InvalidConfiguration(&'static str),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFeature(feature) => {
                write!(f, "unsupported feature: {feature}")
            }
            Self::InvalidConfiguration(reason) => {
                write!(f, "invalid configuration: {reason}")
            }
        }
    }
}

impl std::error::Error for PlatformError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_profile_reports_expected_capabilities() {
        let profile = PlatformProfile::native();
        assert_eq!(profile.kind, PlatformKind::Native);
        assert!(profile.capabilities.file_system);
        assert!(profile.capabilities.async_io);
        assert!(profile.capabilities.gui);
        assert!(profile.capabilities.network);
        assert!(!profile.capabilities.web_compat);
    }

    #[test]
    fn wasm_profile_reports_expected_capabilities() {
        let profile = PlatformProfile::wasm();
        assert_eq!(profile.kind, PlatformKind::Wasm);
        assert!(!profile.capabilities.file_system);
        assert!(profile.capabilities.async_io);
        assert!(!profile.capabilities.gui);
        assert!(!profile.capabilities.network);
        assert!(profile.capabilities.web_compat);
    }

    #[test]
    fn error_formats_with_context() {
        let error = PlatformError::UnsupportedFeature("audio");
        assert_eq!(error.to_string(), "unsupported feature: audio");
    }
}
