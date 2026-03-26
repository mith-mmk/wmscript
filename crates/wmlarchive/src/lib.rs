#![forbid(unsafe_code)]

//! Archive and bundle support for WML distributions.
//!
//! The archive crate will later manage packaging, verification, and optional
//! signing. The initial scaffold keeps the dependency surface available to the
//! rest of the workspace.

use wmlplatform::PlatformProfile;

/// Archive metadata shared with packaging tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivePlan {
    /// Target runtime profile.
    pub platform: PlatformProfile,
    /// Whether the output should be optimized for release builds.
    pub release: bool,
}

impl ArchivePlan {
    /// Creates a new archive plan.
    pub const fn new(platform: PlatformProfile, release: bool) -> Self {
        Self { platform, release }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_plan_keeps_release_flag() {
        let plan = ArchivePlan::new(PlatformProfile::native(), true);
        assert!(plan.release);
    }
}
