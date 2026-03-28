#![forbid(unsafe_code)]

//! Resource pipeline support for WML assets.

mod catalog;
mod error;
mod manager;
mod types;

pub use catalog::*;
pub use error::*;
pub use manager::*;
pub use types::*;

use wmplatform::PlatformProfile;

/// Resource loading plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourcePlan {
    pub platform: PlatformProfile,
    pub resource_id: ResourceId,
}

impl ResourcePlan {
    pub const fn new(platform: PlatformProfile, resource_id: ResourceId) -> Self {
        Self {
            platform,
            resource_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_plan_keeps_id() {
        let plan = ResourcePlan::new(PlatformProfile::native(), 42);
        assert_eq!(plan.resource_id, 42);
    }
}
