#![forbid(unsafe_code)]

//! Toolchain support for compiling, packaging, and running WML game scripts.

mod archive_io;
mod config;
mod model;
mod toolchain;
mod util;

pub use archive_io::spawn_worker_programs;
pub use config::{Result, ToolchainConfig, ToolchainError};
pub use model::{
    BuildArtifact, ExecutionReport, GameAsset, GameAssetExternalLocation, GameProject, GameScript,
    GameWorkerRole, WorkerProgram,
};
pub use toolchain::Toolchain;

#[cfg(test)]
use util::stable_hash64;
#[cfg(test)]
use wmarchive::{Archive, ManifestSectionLocation, SectionKind, digest_section};
#[cfg(test)]
use wmresource::ResourceType;
#[cfg(test)]
use wmvm::RunOutcome;

#[cfg(test)]
#[path = "../tests/support/lib_tests.rs"]
mod tests;
