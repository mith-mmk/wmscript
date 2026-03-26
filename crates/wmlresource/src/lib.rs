#![forbid(unsafe_code)]

//! Resource pipeline support for WML assets.
//!
//! This crate will later cover asset decoding, caching, and resource identity
//! resolution. The scaffold exposes a small set of types that other crates can
//! already depend on.

use wmlplatform::PlatformProfile;

/// Identifier for a logical resource.
pub type ResourceId = u32;

/// Resource loading plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourcePlan {
    /// Target platform profile.
    pub platform: PlatformProfile,
    /// Logical resource identifier.
    pub resource_id: ResourceId,
}

impl ResourcePlan {
    /// Creates a new resource plan.
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
