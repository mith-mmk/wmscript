#![forbid(unsafe_code)]

pub type ArchiveId = u128;
pub type SectionId = u32;
pub type Digest = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind {
    Manifest,
    Module,
    StringTable,
    ConstPool,
    ExtTable,
    Asset,
    Debug,
    DependencyTable,
    SymbolTable,
    RelocationStub,
    Metadata,
    Unknown(u16),
}

impl SectionKind {
    pub const fn from_u16(value: u16) -> Self {
        match value {
            0x0001 => Self::Manifest,
            0x0002 => Self::Module,
            0x0003 => Self::StringTable,
            0x0004 => Self::ConstPool,
            0x0005 => Self::ExtTable,
            0x0006 => Self::Asset,
            0x0007 => Self::Debug,
            0x0008 => Self::DependencyTable,
            0x0009 => Self::SymbolTable,
            0x000A => Self::RelocationStub,
            0x000B => Self::Metadata,
            other => Self::Unknown(other),
        }
    }

    pub const fn as_u16(self) -> u16 {
        match self {
            Self::Manifest => 0x0001,
            Self::Module => 0x0002,
            Self::StringTable => 0x0003,
            Self::ConstPool => 0x0004,
            Self::ExtTable => 0x0005,
            Self::Asset => 0x0006,
            Self::Debug => 0x0007,
            Self::DependencyTable => 0x0008,
            Self::SymbolTable => 0x0009,
            Self::RelocationStub => 0x000A,
            Self::Metadata => 0x000B,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedHeader {
    pub magic: [u8; 4],
    pub archive_version: u16,
    pub header_size: u16,
    pub flags: u32,
    pub section_count: u32,
    pub section_table_offset: u64,
    pub section_table_size: u64,
    pub security_offset: u64,
    pub security_size: u64,
    pub signature_offset: u64,
    pub signature_size: u64,
}

impl FixedHeader {
    pub const MAGIC: [u8; 4] = *b"WARC";
    pub const VERSION: u16 = 1;
    pub const BYTE_SIZE: usize = 64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityHeader {
    pub security_version: u16,
    pub sig_alg: u16,
    pub hash_alg: u16,
    pub enc_alg: u16,
    pub key_id: [u8; 16],
    pub nonce_len: u16,
    pub reserved: u16,
    pub manifest_digest_offset: u64,
    pub manifest_digest_size: u64,
}

impl SecurityHeader {
    pub const BYTE_SIZE: usize = 44;
}

impl Default for SecurityHeader {
    fn default() -> Self {
        Self {
            security_version: 1,
            sig_alg: 0,
            hash_alg: 0,
            enc_alg: 0,
            key_id: [0; 16],
            nonce_len: 0,
            reserved: 0,
            manifest_digest_offset: 0,
            manifest_digest_size: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionEntry {
    pub id: SectionId,
    pub kind: SectionKind,
    pub flags: u16,
    pub offset: u64,
    pub packed_size: u64,
    pub unpacked_size: u64,
    pub align: u32,
    pub reserved: u32,
    pub name_hash: u64,
}

impl SectionEntry {
    pub const BYTE_SIZE: usize = 48;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveSection {
    pub id: SectionId,
    pub kind: SectionKind,
    pub flags: u16,
    pub align: u32,
    pub name_hash: u64,
    pub payload: Vec<u8>,
}

impl ArchiveSection {
    pub fn new(id: SectionId, kind: SectionKind, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            kind,
            flags: 0,
            align: 16,
            name_hash: 0,
            payload: payload.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub extra: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16, extra: u16) -> Self {
        Self {
            major,
            minor,
            patch,
            extra,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionDigest {
    pub section_id: SectionId,
    pub section_kind: SectionKind,
    pub flags_canonical: u16,
    pub unpacked_size: u64,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestExtEntry {
    pub name: String,
    pub min_version: u16,
    pub max_version: u16,
    pub required_flags: u32,
}

impl ManifestExtEntry {
    pub fn new(
        name: impl Into<String>,
        min_version: u16,
        max_version: u16,
        required_flags: u32,
    ) -> Self {
        Self {
            name: name.into(),
            min_version,
            max_version,
            required_flags,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestArchiveDepEntry {
    pub archive_id: ArchiveId,
    pub package_name: String,
    pub min_version: Version,
    pub flags: u16,
}

impl ManifestArchiveDepEntry {
    pub fn new(
        archive_id: ArchiveId,
        package_name: impl Into<String>,
        min_version: Version,
        flags: u16,
    ) -> Self {
        Self {
            archive_id,
            package_name: package_name.into(),
            min_version,
            flags,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestResourceEntry {
    pub name_hash: u64,
    pub resource_id: u32,
}

impl ManifestResourceEntry {
    pub const fn new(name_hash: u64, resource_id: u32) -> Self {
        Self {
            name_hash,
            resource_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSectionLocation {
    pub section_id: SectionId,
    pub url: String,
    pub cache_key: String,
    pub flags: u32,
}

impl ManifestSectionLocation {
    pub fn new(
        section_id: SectionId,
        url: impl Into<String>,
        cache_key: impl Into<String>,
        flags: u32,
    ) -> Self {
        Self {
            section_id,
            url: url.into(),
            cache_key: cache_key.into(),
            flags,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestPolicyBlock {
    pub save_compat_version: u16,
    pub hot_reload_policy: u16,
    pub mod_policy: u16,
    pub encryption_policy: u16,
    pub max_worker_count: u16,
    pub max_message_size_kib: u16,
    pub max_heap_size_mib: u16,
    pub max_stack_depth: u16,
    pub reserved: [u8; 16],
}

impl ManifestPolicyBlock {
    pub const fn new() -> Self {
        Self {
            save_compat_version: 0,
            hot_reload_policy: 0,
            mod_policy: 0,
            encryption_policy: 0,
            max_worker_count: 0,
            max_message_size_kib: 0,
            max_heap_size_mib: 0,
            max_stack_depth: 0,
            reserved: [0; 16],
        }
    }
}

impl Default for ManifestPolicyBlock {
    fn default() -> Self {
        Self::new()
    }
}
