use core::fmt;

use wmarchive::ArchiveError;
use wmcompiler::CompileError;
use wmplatform::PlatformProfile;
use wmruntime::RuntimeError;
use wmvm::ProgramCodecError;

/// Toolchain configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolchainConfig {
    pub platform: PlatformProfile,
    pub step_limit: usize,
    pub release: bool,
}

impl ToolchainConfig {
    pub const fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            step_limit: 128,
            release: false,
        }
    }

    pub const fn with_step_limit(mut self, step_limit: usize) -> Self {
        self.step_limit = step_limit;
        self
    }

    pub const fn with_release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }
}

/// Result type used by the toolchain.
pub type Result<T> = core::result::Result<T, ToolchainError>;

/// Toolchain error.
#[derive(Debug)]
pub enum ToolchainError {
    Compile(CompileError),
    Archive(ArchiveError),
    ProgramCodec(ProgramCodecError),
    Runtime(RuntimeError),
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
            Self::ProgramCodec(error) => write!(f, "{error}"),
            Self::Runtime(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ToolchainError {}

impl From<CompileError> for ToolchainError {
    fn from(value: CompileError) -> Self {
        Self::Compile(value)
    }
}

impl From<ArchiveError> for ToolchainError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl From<ProgramCodecError> for ToolchainError {
    fn from(value: ProgramCodecError) -> Self {
        Self::ProgramCodec(value)
    }
}

impl From<RuntimeError> for ToolchainError {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}
