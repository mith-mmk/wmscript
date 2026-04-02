#![forbid(unsafe_code)]

//! Extension registry and namespace management for WML scripts.
//!
//! The compiler uses this crate to resolve `ext.*` calls into stable compile-time
//! IDs. The VM then dispatches those IDs through the host bridge.

use std::collections::BTreeMap;

use wmhost::{CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_GUI, CAP_NETWORK, CapabilityMask, HostId};

/// Stable identifier assigned to an extension function.
pub type ExtId = u32;

/// Stable identifier assigned to a namespace.
pub type NamespaceId = u32;

/// Best-effort static return type metadata for extension functions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtValueType {
    Unknown,
    Nil,
    Bool,
    Integer,
    Float,
    String,
}

/// Result type used by the extension registry.
pub type Result<T> = core::result::Result<T, ExtError>;

/// Errors raised while managing extensions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtError {
    InvalidNamespace(String),
    InvalidFunctionName(String),
    DuplicateNamespace(String),
    DuplicateFunction { full_name: String },
    UnknownNamespace(String),
    UnknownExtId(ExtId),
    InvalidPath(String),
}

impl core::fmt::Display for ExtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidNamespace(name) => write!(f, "invalid namespace: {name}"),
            Self::InvalidFunctionName(name) => write!(f, "invalid function name: {name}"),
            Self::DuplicateNamespace(name) => write!(f, "duplicate namespace: {name}"),
            Self::DuplicateFunction { full_name } => {
                write!(f, "duplicate extension function: {full_name}")
            }
            Self::UnknownNamespace(name) => write!(f, "unknown namespace: {name}"),
            Self::UnknownExtId(id) => write!(f, "unknown ext id: {id}"),
            Self::InvalidPath(path) => write!(f, "invalid extension path: {path}"),
        }
    }
}

impl std::error::Error for ExtError {}

/// Policy used to validate namespaces and extension names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespacePolicy {
    require_ext_root: bool,
}

impl NamespacePolicy {
    /// Creates the default policy.
    pub const fn new() -> Self {
        Self {
            require_ext_root: true,
        }
    }

    /// Returns a policy that accepts any valid namespace root.
    pub const fn permissive() -> Self {
        Self {
            require_ext_root: false,
        }
    }

    fn validate_namespace(&self, namespace: &str) -> Result<()> {
        if namespace.is_empty() {
            return Err(ExtError::InvalidNamespace(namespace.to_owned()));
        }
        if self.require_ext_root && !namespace.starts_with("ext") {
            return Err(ExtError::InvalidNamespace(namespace.to_owned()));
        }
        if namespace.starts_with('.') || namespace.ends_with('.') || namespace.contains("..") {
            return Err(ExtError::InvalidNamespace(namespace.to_owned()));
        }
        for segment in namespace.split('.') {
            if segment.is_empty()
                || !segment
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
            {
                return Err(ExtError::InvalidNamespace(namespace.to_owned()));
            }
        }
        Ok(())
    }

    fn validate_function_name(&self, name: &str) -> Result<()> {
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            return Err(ExtError::InvalidFunctionName(name.to_owned()));
        }
        Ok(())
    }
}

impl Default for NamespacePolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata for a single extension function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtFunction {
    pub ext_id: ExtId,
    pub namespace_id: NamespaceId,
    pub namespace: String,
    pub name: String,
    pub host_id: HostId,
    pub min_args: u8,
    pub max_args: u8,
    pub required_capabilities: CapabilityMask,
    pub return_type: Option<ExtValueType>,
}

impl ExtFunction {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NamespaceEntry {
    namespace_id: NamespaceId,
    path: String,
    functions: BTreeMap<String, ExtId>,
}

/// Registry that assigns `ext_id` values and resolves `ext.*` paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRegistry {
    policy: NamespacePolicy,
    namespaces: BTreeMap<String, NamespaceEntry>,
    functions: BTreeMap<ExtId, ExtFunction>,
    next_namespace_id: NamespaceId,
    next_ext_id: ExtId,
}

impl ExtensionRegistry {
    /// Creates an empty registry with the default policy.
    pub fn new() -> Self {
        Self::with_policy(NamespacePolicy::default())
    }

    /// Creates an empty registry with a custom policy.
    pub fn with_policy(policy: NamespacePolicy) -> Self {
        Self {
            policy,
            namespaces: BTreeMap::new(),
            functions: BTreeMap::new(),
            next_namespace_id: 1,
            next_ext_id: 1,
        }
    }

    /// Returns the active namespace policy.
    pub const fn policy(&self) -> &NamespacePolicy {
        &self.policy
    }

    /// Registers a namespace and returns its stable namespace id.
    pub fn register_namespace(&mut self, namespace: &str) -> Result<NamespaceId> {
        self.policy.validate_namespace(namespace)?;
        if let Some(entry) = self.namespaces.get(namespace) {
            return Ok(entry.namespace_id);
        }
        let namespace_id = self.next_namespace_id;
        self.next_namespace_id = self
            .next_namespace_id
            .checked_add(1)
            .unwrap_or(self.next_namespace_id);
        self.namespaces.insert(
            namespace.to_owned(),
            NamespaceEntry {
                namespace_id,
                path: namespace.to_owned(),
                functions: BTreeMap::new(),
            },
        );
        Ok(namespace_id)
    }

    /// Registers a single extension function under an existing namespace.
    pub fn register_function(
        &mut self,
        namespace: &str,
        name: &str,
        host_id: HostId,
        min_args: u8,
        max_args: u8,
        required_capabilities: CapabilityMask,
    ) -> Result<ExtId> {
        self.policy.validate_namespace(namespace)?;
        self.policy.validate_function_name(name)?;
        let namespace_id = self.register_namespace(namespace)?;
        let entry = self
            .namespaces
            .get_mut(namespace)
            .ok_or_else(|| ExtError::UnknownNamespace(namespace.to_owned()))?;
        if entry.functions.contains_key(name) {
            return Err(ExtError::DuplicateFunction {
                full_name: format!("{namespace}.{name}"),
            });
        }
        let ext_id = self.next_ext_id;
        self.next_ext_id = self.next_ext_id.saturating_add(1);
        entry.functions.insert(name.to_owned(), ext_id);
        self.functions.insert(
            ext_id,
            ExtFunction {
                ext_id,
                namespace_id,
                namespace: namespace.to_owned(),
                name: name.to_owned(),
                host_id,
                min_args,
                max_args,
                required_capabilities,
                return_type: None,
            },
        );
        Ok(ext_id)
    }

    /// Registers multiple extension functions in one namespace.
    pub fn register_extension(
        &mut self,
        namespace: &str,
        functions: &[ExtensionFunctionSpec<'_>],
    ) -> Result<Vec<ExtId>> {
        let mut ids = Vec::with_capacity(functions.len());
        for spec in functions {
            let ext_id = self.register_function(
                namespace,
                spec.name,
                spec.host_id,
                spec.min_args,
                spec.max_args,
                spec.required_capabilities,
            )?;
            ids.push(ext_id);
            if let Some(return_type) = spec.return_type {
                if let Some(function) = self.functions.get_mut(&ext_id) {
                    function.return_type = Some(return_type);
                }
            }
        }
        Ok(ids)
    }

    /// Resolves a full extension path like `ext.physics.raycast`.
    pub fn resolve(&self, full_name: &str) -> Result<&ExtFunction> {
        let ext_id = self.resolve_id(full_name)?;
        self.function(ext_id).ok_or(ExtError::UnknownExtId(ext_id))
    }

    /// Resolves a full extension path to an extension id.
    pub fn resolve_id(&self, full_name: &str) -> Result<ExtId> {
        let (namespace, name) = split_full_name(full_name)?;
        let entry = self
            .namespaces
            .get(namespace)
            .ok_or_else(|| ExtError::UnknownNamespace(namespace.to_owned()))?;
        entry
            .functions
            .get(name)
            .copied()
            .ok_or_else(|| ExtError::InvalidPath(full_name.to_owned()))
    }

    /// Returns a function metadata record by ext id.
    pub fn function(&self, ext_id: ExtId) -> Option<&ExtFunction> {
        self.functions.get(&ext_id)
    }

    /// Returns a namespace record.
    pub fn namespace(&self, namespace: &str) -> Option<NamespaceView<'_>> {
        self.namespaces.get(namespace).map(|entry| NamespaceView {
            namespace_id: entry.namespace_id,
            path: &entry.path,
            functions: &entry.functions,
        })
    }

    /// Returns all registered namespace names.
    pub fn namespace_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.namespaces.keys().map(String::as_str)
    }

    /// Returns all registered extension ids.
    pub fn function_ids(&self) -> impl Iterator<Item = ExtId> + '_ {
        self.functions.keys().copied()
    }
}

/// Registers the built-in runtime extension set in a stable order.
pub fn standard_extension_registry() -> Result<ExtensionRegistry> {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry.register_extension(
        "ext.fs",
        &[
            ExtensionFunctionSpec::new("read", 100, 1, 1, CAP_FILE_SYSTEM)
                .with_return_type(ExtValueType::String),
            ExtensionFunctionSpec::new("write", 101, 2, 2, CAP_FILE_SYSTEM)
                .with_return_type(ExtValueType::Nil),
            ExtensionFunctionSpec::new("exists", 102, 1, 1, CAP_FILE_SYSTEM)
                .with_return_type(ExtValueType::Bool),
        ],
    )?;
    registry.register_extension(
        "ext.debug",
        &[
            ExtensionFunctionSpec::new("log", 110, 1, 1, 0).with_return_type(ExtValueType::Nil),
            ExtensionFunctionSpec::new("inspect", 111, 1, 1, 0)
                .with_return_type(ExtValueType::String),
        ],
    )?;
    registry.register_extension(
        "ext.net",
        &[
            ExtensionFunctionSpec::new("get", 120, 1, 1, CAP_NETWORK)
                .with_return_type(ExtValueType::String),
            ExtensionFunctionSpec::new("post", 121, 2, 2, CAP_NETWORK)
                .with_return_type(ExtValueType::String),
        ],
    )?;
    registry.register_extension(
        "ext.llm",
        &[
            ExtensionFunctionSpec::new("generate", 130, 1, 1, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::String),
        ],
    )?;
    registry.register_extension(
        "ext.scene",
        &[
            ExtensionFunctionSpec::new("layout", 180, 8, 8, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("reset", 181, 0, 0, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
        ],
    )?;
    registry.register_extension(
        "ext.message",
        &[
            ExtensionFunctionSpec::new("show", 135, 1, 2, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("append", 136, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("choices", 137, 0, 16, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("choices_named", 134, 0, 16, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("prompt", 138, 0, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("hide", 139, 0, 0, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("speed", 131, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("auto", 132, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("skip", 133, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("log_clear", 159, 0, 0, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("clear", 149, 0, 0, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("box_style", 162, 8, 8, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("text_color", 163, 4, 4, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("speaker_color", 164, 4, 4, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("accent_color", 165, 4, 4, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("font_size", 166, 2, 2, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("reset_style", 167, 0, 0, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
        ],
    )?;
    registry.register_extension(
        "ext.image",
        &[
            ExtensionFunctionSpec::new("load", 140, 1, 1, CAP_GUI),
            ExtensionFunctionSpec::new("info", 141, 1, 1, CAP_GUI),
            ExtensionFunctionSpec::new("status", 142, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Integer),
            ExtensionFunctionSpec::new("release", 143, 1, 1, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("draw", 144, 3, 3, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("draw_part", 145, 7, 7, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("draw_ext", 146, 11, 11, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("set_icon_sheet", 147, 3, 3, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("draw_icon", 148, 4, 4, CAP_GUI)
                .with_return_type(ExtValueType::Bool),
        ],
    )?;
    registry.register_extension(
        "ext.audio",
        &[
            ExtensionFunctionSpec::new("load", 150, 1, 1, CAP_ASYNC_IO),
            ExtensionFunctionSpec::new("play", 151, 1, 2, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("playback", 158, 1, 2, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("pause", 152, 1, 1, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("stop", 153, 1, 1, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("seek", 154, 2, 2, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("volume", 155, 2, 2, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("release", 156, 1, 1, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("status", 157, 1, 1, CAP_ASYNC_IO)
                .with_return_type(ExtValueType::Integer),
        ],
    )?;
    registry.register_extension(
        "ext.vm",
        &[
            ExtensionFunctionSpec::new("save", 160, 1, 1, 0).with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("load", 161, 1, 1, 0).with_return_type(ExtValueType::Bool),
        ],
    )?;
    registry.register_extension(
        "state",
        &[
            ExtensionFunctionSpec::new("save", 170, 1, 1, 0).with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("load", 171, 1, 1, 0).with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("has", 172, 1, 1, 0).with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("get", 173, 1, 1, 0),
            ExtensionFunctionSpec::new("set", 174, 2, 2, 0).with_return_type(ExtValueType::Bool),
            ExtensionFunctionSpec::new("erase", 175, 1, 1, 0).with_return_type(ExtValueType::Bool),
        ],
    )?;
    Ok(registry)
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A lightweight view over a registered namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceView<'a> {
    pub namespace_id: NamespaceId,
    pub path: &'a str,
    pub functions: &'a BTreeMap<String, ExtId>,
}

/// Specification used to register a batch of extension functions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtensionFunctionSpec<'a> {
    pub name: &'a str,
    pub host_id: HostId,
    pub min_args: u8,
    pub max_args: u8,
    pub required_capabilities: CapabilityMask,
    pub return_type: Option<ExtValueType>,
}

impl<'a> ExtensionFunctionSpec<'a> {
    pub const fn new(
        name: &'a str,
        host_id: HostId,
        min_args: u8,
        max_args: u8,
        required_capabilities: CapabilityMask,
    ) -> Self {
        Self {
            name,
            host_id,
            min_args,
            max_args,
            required_capabilities,
            return_type: None,
        }
    }

    pub const fn with_return_type(mut self, return_type: ExtValueType) -> Self {
        self.return_type = Some(return_type);
        self
    }
}

impl ExtValueType {
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }
}

impl Default for ExtValueType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl<'a> ExtensionFunctionSpec<'a> {
    pub const fn with_optional_return_type(mut self, return_type: Option<ExtValueType>) -> Self {
        self.return_type = return_type;
        self
    }
}

impl ExtFunction {
    pub fn return_type(&self) -> Option<ExtValueType> {
        self.return_type
    }
}

fn split_full_name(full_name: &str) -> Result<(&str, &str)> {
    let (namespace, name) = full_name
        .rsplit_once('.')
        .ok_or_else(|| ExtError::InvalidPath(full_name.to_owned()))?;
    if namespace.is_empty() || name.is_empty() {
        return Err(ExtError::InvalidPath(full_name.to_owned()));
    }
    Ok((namespace, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_assigns_ext_ids_and_resolves_paths() {
        let mut registry = ExtensionRegistry::new();
        let raycast = registry
            .register_function("ext.physics", "raycast", 10, 4, 4, 0b001)
            .expect("register raycast");
        let overlap = registry
            .register_function("ext.physics", "overlap", 11, 2, 2, 0b001)
            .expect("register overlap");

        assert_eq!(raycast, 1);
        assert_eq!(overlap, 2);
        assert_eq!(registry.resolve_id("ext.physics.raycast"), Ok(raycast));
        assert_eq!(registry.resolve_id("ext.physics.overlap"), Ok(overlap));
        assert_eq!(registry.resolve("ext.physics.raycast").unwrap().host_id, 10);
    }

    #[test]
    fn registry_rejects_duplicate_functions() {
        let mut registry = ExtensionRegistry::new();
        let _ = registry
            .register_function("ext.net", "send", 12, 2, 2, 0)
            .expect("register send");
        assert!(matches!(
            registry.register_function("ext.net", "send", 13, 1, 1, 0),
            Err(ExtError::DuplicateFunction { .. })
        ));
    }

    #[test]
    fn registry_keeps_namespace_views() {
        let mut registry = ExtensionRegistry::new();
        registry
            .register_extension(
                "ext.fs",
                &[
                    ExtensionFunctionSpec::new("read", 20, 1, 1, 0b010),
                    ExtensionFunctionSpec::new("write", 21, 2, 2, 0b010),
                ],
            )
            .expect("register fs extension");

        let namespace = registry.namespace("ext.fs").expect("namespace view");
        assert_eq!(namespace.namespace_id, 1);
        assert_eq!(namespace.functions.len(), 2);
        assert!(namespace.functions.contains_key("read"));
        assert!(namespace.functions.contains_key("write"));
    }

    #[test]
    fn policy_rejects_invalid_names() {
        let mut registry = ExtensionRegistry::new();
        assert!(matches!(
            registry.register_namespace("net"),
            Err(ExtError::InvalidNamespace(_))
        ));
        assert!(matches!(
            registry.register_function("ext.net", "bad-name", 1, 0, 0, 0),
            Err(ExtError::InvalidFunctionName(_))
        ));
    }
}
