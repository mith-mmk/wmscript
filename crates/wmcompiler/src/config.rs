use wmext::ExtensionRegistry;
use wmplatform::PlatformProfile;

/// Compiler configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerConfig {
    /// Target platform profile.
    pub platform: PlatformProfile,
    /// Maximum accepted source size in bytes.
    pub max_source_bytes: usize,
    /// Extension registry used to resolve `ext.*` calls.
    pub extension_registry: Option<ExtensionRegistry>,
}

impl CompilerConfig {
    /// Creates a new compiler configuration.
    pub const fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            max_source_bytes: 1 << 20,
            extension_registry: None,
        }
    }

    /// Sets the maximum accepted source size.
    pub const fn with_max_source_bytes(mut self, max_source_bytes: usize) -> Self {
        self.max_source_bytes = max_source_bytes;
        self
    }

    pub fn with_extension_registry(mut self, extension_registry: ExtensionRegistry) -> Self {
        self.extension_registry = Some(extension_registry);
        self
    }

    pub fn extension_registry(&self) -> Option<&ExtensionRegistry> {
        self.extension_registry.as_ref()
    }
}

/// Lightweight module identifier used during import resolution.
pub type ModuleId = u32;

/// Identifier assigned to a function within a module.
pub type FunctionId = u32;

/// Identifier assigned to a symbol.
pub type SymbolId = u32;
