#![forbid(unsafe_code)]

//! Archive and bundle support for WML distributions.

mod builder;
mod error;
mod manifest;
mod types;

pub use builder::Archive;
pub use builder::*;
pub use error::*;
pub use manifest::*;
pub use types::*;

use wmlplatform::PlatformProfile;

/// Archive metadata shared with packaging tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivePlan {
    pub platform: PlatformProfile,
    pub release: bool,
}

impl ArchivePlan {
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
