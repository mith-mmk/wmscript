#![forbid(unsafe_code)]

use crate::{
    ArchiveError, ArchiveId, Digest, ManifestArchiveDepEntry, ManifestExtEntry,
    ManifestPolicyBlock, ManifestResourceEntry, ManifestSectionLocation, ManifestWorkerEntry,
    Result, SectionDigest, SectionKind, Version,
};

fn write_u16(dst: &mut Vec<u8>, value: u16) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(dst: &mut Vec<u8>, value: u32) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(dst: &mut Vec<u8>, value: u64) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn write_u128(dst: &mut Vec<u8>, value: u128) {
    dst.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> Result<u16> {
    let end = offset.saturating_add(2);
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32> {
    let end = offset.saturating_add(4);
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64> {
    let end = offset.saturating_add(8);
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_u128(bytes: &[u8], offset: &mut usize) -> Result<u128> {
    let end = offset.saturating_add(16);
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(u128::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7], slice[8],
        slice[9], slice[10], slice[11], slice[12], slice[13], slice[14], slice[15],
    ]))
}

fn read_bytes<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = offset.checked_add(len).ok_or(ArchiveError::UnexpectedEof)?;
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(slice)
}

fn hash_bytes(parts: &[&[u8]]) -> Digest {
    let mut state = [
        0x0123_4567_89AB_CDEF_u64,
        0x0F1E_2D3C_4B5A_6978_u64,
        0x1020_3040_5060_7080_u64,
        0x9988_7766_5544_3322_u64,
    ];
    for part in parts {
        for &byte in *part {
            for lane in 0..4 {
                state[lane] ^= byte as u64;
                state[lane] = state[lane].wrapping_mul(0x1000_0000_01B3);
                state[lane] = state[lane].rotate_left(5 + lane as u32);
                state[lane] ^= state[(lane + 1) % 4].rotate_right(7);
            }
        }
    }
    let mut digest = [0u8; 32];
    digest[0..8].copy_from_slice(&state[0].to_le_bytes());
    digest[8..16].copy_from_slice(&state[1].to_le_bytes());
    digest[16..24].copy_from_slice(&state[2].to_le_bytes());
    digest[24..32].copy_from_slice(&state[3].to_le_bytes());
    digest
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub package_name: String,
    pub package_version: Version,
    pub bytecode_version: u16,
    pub runtime_min_version: u16,
    pub archive_id: ArchiveId,
    pub build_id: u128,
    pub target_platform_mask: u64,
    pub capability_mask: u64,
    pub policy_flags: u64,
    pub entry_module_id: u32,
    pub entry_func_id: u32,
    pub signer_policy: u16,
    pub trust_policy: u16,
    pub extensions: Vec<ManifestExtEntry>,
    pub archives: Vec<ManifestArchiveDepEntry>,
    pub section_digests: Vec<SectionDigest>,
    pub resource_map: Vec<ManifestResourceEntry>,
    pub external_section_locations: Vec<ManifestSectionLocation>,
    pub worker_entries: Vec<ManifestWorkerEntry>,
    pub policy: ManifestPolicyBlock,
}

impl Manifest {
    pub fn new(package_name: impl Into<String>, archive_id: ArchiveId, build_id: u128) -> Self {
        Self {
            package_name: package_name.into(),
            package_version: Version::new(0, 1, 0, 0),
            bytecode_version: 1,
            runtime_min_version: 1,
            archive_id,
            build_id,
            target_platform_mask: u64::MAX,
            capability_mask: u64::MAX,
            policy_flags: 0,
            entry_module_id: 0,
            entry_func_id: 0,
            signer_policy: 0,
            trust_policy: 0,
            extensions: Vec::new(),
            archives: Vec::new(),
            section_digests: Vec::new(),
            resource_map: Vec::new(),
            external_section_locations: Vec::new(),
            worker_entries: Vec::new(),
            policy: ManifestPolicyBlock::default(),
        }
    }

    pub fn builder(
        package_name: impl Into<String>,
        archive_id: ArchiveId,
        build_id: u128,
    ) -> ManifestBuilder {
        ManifestBuilder::new(package_name, archive_id, build_id)
    }

    pub fn resource_id_by_name_hash(&self, name_hash: u64) -> Option<u32> {
        self.resource_map
            .iter()
            .find(|entry| entry.name_hash == name_hash)
            .map(|entry| entry.resource_id)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MNF1");
        write_u16(&mut bytes, 1);
        write_u16(&mut bytes, 0);
        write_u32(&mut bytes, self.package_name.len() as u32);
        bytes.extend_from_slice(self.package_name.as_bytes());
        write_u128(&mut bytes, self.archive_id);
        write_u128(&mut bytes, self.build_id);
        write_u16(&mut bytes, self.package_version.major);
        write_u16(&mut bytes, self.package_version.minor);
        write_u16(&mut bytes, self.package_version.patch);
        write_u16(&mut bytes, self.package_version.extra);
        write_u16(&mut bytes, self.bytecode_version);
        write_u16(&mut bytes, self.runtime_min_version);
        write_u64(&mut bytes, self.target_platform_mask);
        write_u64(&mut bytes, self.capability_mask);
        write_u64(&mut bytes, self.policy_flags);
        write_u32(&mut bytes, self.entry_module_id);
        write_u32(&mut bytes, self.entry_func_id);
        write_u16(&mut bytes, self.signer_policy);
        write_u16(&mut bytes, self.trust_policy);
        write_u32(&mut bytes, self.extensions.len() as u32);
        write_u32(&mut bytes, self.archives.len() as u32);
        write_u32(&mut bytes, self.section_digests.len() as u32);
        write_u32(&mut bytes, self.resource_map.len() as u32);
        write_u16(&mut bytes, self.policy.save_compat_version);
        write_u16(&mut bytes, self.policy.hot_reload_policy);
        write_u16(&mut bytes, self.policy.mod_policy);
        write_u16(&mut bytes, self.policy.encryption_policy);
        write_u16(&mut bytes, self.policy.max_worker_count);
        write_u16(&mut bytes, self.policy.max_message_size_kib);
        write_u16(&mut bytes, self.policy.max_heap_size_mib);
        write_u16(&mut bytes, self.policy.max_stack_depth);
        bytes.extend_from_slice(&self.policy.reserved);

        for ext in &self.extensions {
            write_u32(&mut bytes, ext.name.len() as u32);
            bytes.extend_from_slice(ext.name.as_bytes());
            write_u16(&mut bytes, ext.min_version);
            write_u16(&mut bytes, ext.max_version);
            write_u32(&mut bytes, ext.required_flags);
        }

        for dep in &self.archives {
            write_u128(&mut bytes, dep.archive_id);
            write_u32(&mut bytes, dep.package_name.len() as u32);
            bytes.extend_from_slice(dep.package_name.as_bytes());
            write_u16(&mut bytes, dep.min_version.major);
            write_u16(&mut bytes, dep.min_version.minor);
            write_u16(&mut bytes, dep.min_version.patch);
            write_u16(&mut bytes, dep.min_version.extra);
            write_u16(&mut bytes, dep.flags);
        }

        for digest in &self.section_digests {
            write_u32(&mut bytes, digest.section_id);
            write_u16(&mut bytes, digest.section_kind.as_u16());
            write_u16(&mut bytes, digest.flags_canonical);
            write_u64(&mut bytes, digest.unpacked_size);
            write_u32(&mut bytes, digest.digest.len() as u32);
            bytes.extend_from_slice(&digest.digest);
        }

        for entry in &self.resource_map {
            write_u64(&mut bytes, entry.name_hash);
            write_u32(&mut bytes, entry.resource_id);
        }

        write_u32(&mut bytes, self.external_section_locations.len() as u32);
        for location in &self.external_section_locations {
            write_u32(&mut bytes, location.section_id);
            write_u32(&mut bytes, location.url.len() as u32);
            bytes.extend_from_slice(location.url.as_bytes());
            write_u32(&mut bytes, location.cache_key.len() as u32);
            bytes.extend_from_slice(location.cache_key.as_bytes());
            write_u32(&mut bytes, location.flags);
        }

        write_u32(&mut bytes, self.worker_entries.len() as u32);
        for worker in &self.worker_entries {
            write_u32(&mut bytes, worker.role.len() as u32);
            bytes.extend_from_slice(worker.role.as_bytes());
            write_u32(&mut bytes, worker.module_section_id);
            write_u32(&mut bytes, worker.entry_func_id);
            write_u64(&mut bytes, worker.capability_mask);
        }

        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut offset = 0usize;
        let magic = read_bytes(bytes, &mut offset, 4)?;
        if magic != b"MNF1" {
            return Err(ArchiveError::InvalidManifest(
                "invalid manifest magic".to_owned(),
            ));
        }
        let version = read_u16(bytes, &mut offset)?;
        if version != 1 {
            return Err(ArchiveError::InvalidManifest(format!(
                "unsupported manifest version: {version}"
            )));
        }
        let _reserved = read_u16(bytes, &mut offset)?;
        let package_name_len = read_u32(bytes, &mut offset)? as usize;
        let package_name = String::from_utf8(
            read_bytes(bytes, &mut offset, package_name_len)?.to_vec(),
        )
        .map_err(|_| ArchiveError::InvalidManifest("package name is not valid UTF-8".to_owned()))?;
        let archive_id = read_u128(bytes, &mut offset)?;
        let build_id = read_u128(bytes, &mut offset)?;
        let package_version = Version::new(
            read_u16(bytes, &mut offset)?,
            read_u16(bytes, &mut offset)?,
            read_u16(bytes, &mut offset)?,
            read_u16(bytes, &mut offset)?,
        );
        let bytecode_version = read_u16(bytes, &mut offset)?;
        let runtime_min_version = read_u16(bytes, &mut offset)?;
        let target_platform_mask = read_u64(bytes, &mut offset)?;
        let capability_mask = read_u64(bytes, &mut offset)?;
        let policy_flags = read_u64(bytes, &mut offset)?;
        let entry_module_id = read_u32(bytes, &mut offset)?;
        let entry_func_id = read_u32(bytes, &mut offset)?;
        let signer_policy = read_u16(bytes, &mut offset)?;
        let trust_policy = read_u16(bytes, &mut offset)?;
        let ext_count = read_u32(bytes, &mut offset)? as usize;
        let archive_count = read_u32(bytes, &mut offset)? as usize;
        let digest_count = read_u32(bytes, &mut offset)? as usize;
        let resource_count = read_u32(bytes, &mut offset)? as usize;
        let policy = ManifestPolicyBlock {
            save_compat_version: read_u16(bytes, &mut offset)?,
            hot_reload_policy: read_u16(bytes, &mut offset)?,
            mod_policy: read_u16(bytes, &mut offset)?,
            encryption_policy: read_u16(bytes, &mut offset)?,
            max_worker_count: read_u16(bytes, &mut offset)?,
            max_message_size_kib: read_u16(bytes, &mut offset)?,
            max_heap_size_mib: read_u16(bytes, &mut offset)?,
            max_stack_depth: read_u16(bytes, &mut offset)?,
            reserved: {
                let slice = read_bytes(bytes, &mut offset, 16)?;
                let mut reserved = [0u8; 16];
                reserved.copy_from_slice(slice);
                reserved
            },
        };

        let mut extensions = Vec::with_capacity(ext_count);
        for _ in 0..ext_count {
            let name_len = read_u32(bytes, &mut offset)? as usize;
            let name = String::from_utf8(read_bytes(bytes, &mut offset, name_len)?.to_vec())
                .map_err(|_| ArchiveError::InvalidManifest("invalid ext name".to_owned()))?;
            let min_version = read_u16(bytes, &mut offset)?;
            let max_version = read_u16(bytes, &mut offset)?;
            let required_flags = read_u32(bytes, &mut offset)?;
            extensions.push(ManifestExtEntry {
                name,
                min_version,
                max_version,
                required_flags,
            });
        }

        let mut archives = Vec::with_capacity(archive_count);
        for _ in 0..archive_count {
            let archive_id_entry = read_u128(bytes, &mut offset)?;
            let package_name_len = read_u32(bytes, &mut offset)? as usize;
            let package_name_dep =
                String::from_utf8(read_bytes(bytes, &mut offset, package_name_len)?.to_vec())
                    .map_err(|_| {
                        ArchiveError::InvalidManifest("invalid archive dep name".to_owned())
                    })?;
            let min_version = Version::new(
                read_u16(bytes, &mut offset)?,
                read_u16(bytes, &mut offset)?,
                read_u16(bytes, &mut offset)?,
                read_u16(bytes, &mut offset)?,
            );
            let flags = read_u16(bytes, &mut offset)?;
            archives.push(ManifestArchiveDepEntry {
                archive_id: archive_id_entry,
                package_name: package_name_dep,
                min_version,
                flags,
            });
        }

        let mut section_digests = Vec::with_capacity(digest_count);
        for _ in 0..digest_count {
            let section_id = read_u32(bytes, &mut offset)?;
            let section_kind = SectionKind::from_u16(read_u16(bytes, &mut offset)?);
            let flags_canonical = read_u16(bytes, &mut offset)?;
            let unpacked_size = read_u64(bytes, &mut offset)?;
            let digest_len = read_u32(bytes, &mut offset)? as usize;
            if digest_len != 32 {
                return Err(ArchiveError::InvalidManifest(
                    "section digest length must be 32".to_owned(),
                ));
            }
            let digest_slice = read_bytes(bytes, &mut offset, digest_len)?;
            let mut digest = [0u8; 32];
            digest.copy_from_slice(digest_slice);
            section_digests.push(SectionDigest {
                section_id,
                section_kind,
                flags_canonical,
                unpacked_size,
                digest,
            });
        }

        let mut resource_map = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            let name_hash = read_u64(bytes, &mut offset)?;
            let resource_id = read_u32(bytes, &mut offset)?;
            resource_map.push(ManifestResourceEntry {
                name_hash,
                resource_id,
            });
        }

        let mut external_section_locations = Vec::new();
        if offset < bytes.len() {
            let location_count = read_u32(bytes, &mut offset)? as usize;
            external_section_locations = Vec::with_capacity(location_count);
            for _ in 0..location_count {
                let section_id = read_u32(bytes, &mut offset)?;
                let url_len = read_u32(bytes, &mut offset)? as usize;
                let url = String::from_utf8(read_bytes(bytes, &mut offset, url_len)?.to_vec())
                    .map_err(|_| {
                        ArchiveError::InvalidManifest("invalid section location url".to_owned())
                    })?;
                let cache_key_len = read_u32(bytes, &mut offset)? as usize;
                let cache_key =
                    String::from_utf8(read_bytes(bytes, &mut offset, cache_key_len)?.to_vec())
                        .map_err(|_| {
                            ArchiveError::InvalidManifest(
                                "invalid section location cache key".to_owned(),
                            )
                        })?;
                let flags = read_u32(bytes, &mut offset)?;
                external_section_locations.push(ManifestSectionLocation {
                    section_id,
                    url,
                    cache_key,
                    flags,
                });
            }
        }

        let mut worker_entries = Vec::new();
        if offset < bytes.len() {
            let worker_count = read_u32(bytes, &mut offset)? as usize;
            worker_entries = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                let role_len = read_u32(bytes, &mut offset)? as usize;
                let role = String::from_utf8(read_bytes(bytes, &mut offset, role_len)?.to_vec())
                    .map_err(|_| ArchiveError::InvalidManifest("invalid worker role".to_owned()))?;
                let module_section_id = read_u32(bytes, &mut offset)?;
                let entry_func_id = read_u32(bytes, &mut offset)?;
                let capability_mask = read_u64(bytes, &mut offset)?;
                worker_entries.push(ManifestWorkerEntry {
                    role,
                    module_section_id,
                    entry_func_id,
                    capability_mask,
                });
            }
        }

        Ok(Self {
            package_name,
            package_version,
            bytecode_version,
            runtime_min_version,
            archive_id,
            build_id,
            target_platform_mask,
            capability_mask,
            policy_flags,
            entry_module_id,
            entry_func_id,
            signer_policy,
            trust_policy,
            extensions,
            archives,
            section_digests,
            resource_map,
            external_section_locations,
            worker_entries,
            policy,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestBuilder {
    manifest: Manifest,
}

impl ManifestBuilder {
    pub fn new(package_name: impl Into<String>, archive_id: ArchiveId, build_id: u128) -> Self {
        Self {
            manifest: Manifest::new(package_name, archive_id, build_id),
        }
    }

    pub fn package_version(mut self, version: Version) -> Self {
        self.manifest.package_version = version;
        self
    }

    pub fn bytecode_version(mut self, version: u16) -> Self {
        self.manifest.bytecode_version = version;
        self
    }

    pub fn runtime_min_version(mut self, version: u16) -> Self {
        self.manifest.runtime_min_version = version;
        self
    }

    pub fn target_platform_mask(mut self, mask: u64) -> Self {
        self.manifest.target_platform_mask = mask;
        self
    }

    pub fn capability_mask(mut self, mask: u64) -> Self {
        self.manifest.capability_mask = mask;
        self
    }

    pub fn policy_flags(mut self, flags: u64) -> Self {
        self.manifest.policy_flags = flags;
        self
    }

    pub fn entry(mut self, module_id: u32, func_id: u32) -> Self {
        self.manifest.entry_module_id = module_id;
        self.manifest.entry_func_id = func_id;
        self
    }

    pub fn signer_policy(mut self, policy: u16) -> Self {
        self.manifest.signer_policy = policy;
        self
    }

    pub fn trust_policy(mut self, policy: u16) -> Self {
        self.manifest.trust_policy = policy;
        self
    }

    pub fn policy(mut self, policy: ManifestPolicyBlock) -> Self {
        self.manifest.policy = policy;
        self
    }

    pub fn push_extension(mut self, entry: ManifestExtEntry) -> Self {
        self.manifest.extensions.push(entry);
        self
    }

    pub fn push_archive_dependency(mut self, entry: ManifestArchiveDepEntry) -> Self {
        self.manifest.archives.push(entry);
        self
    }

    pub fn push_section_digest(mut self, entry: SectionDigest) -> Self {
        self.manifest.section_digests.push(entry);
        self
    }

    pub fn push_resource_mapping(mut self, entry: ManifestResourceEntry) -> Self {
        self.manifest.resource_map.push(entry);
        self
    }

    pub fn push_external_section_location(mut self, entry: ManifestSectionLocation) -> Self {
        self.manifest.external_section_locations.push(entry);
        self
    }

    pub fn push_worker_entry(mut self, entry: ManifestWorkerEntry) -> Self {
        self.manifest.worker_entries.push(entry);
        self
    }

    pub fn build(self) -> Manifest {
        self.manifest
    }
}

pub fn digest_section(
    section_id: u32,
    section_kind: SectionKind,
    flags_canonical: u16,
    unpacked_size: u64,
    payload: &[u8],
) -> Digest {
    hash_bytes(&[
        &section_id.to_le_bytes(),
        &section_kind.as_u16().to_le_bytes(),
        &flags_canonical.to_le_bytes(),
        &unpacked_size.to_le_bytes(),
        payload,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrips_external_section_locations() {
        let manifest = ManifestBuilder::new("web-dist", 10, 20)
            .push_resource_mapping(ManifestResourceEntry::new(0x1234, 100))
            .push_external_section_location(ManifestSectionLocation::new(
                10,
                "assets/section-10.bin",
                "sha256:section-10",
                1,
            ))
            .push_worker_entry(ManifestWorkerEntry::new("engine", 4, 1, u64::MAX))
            .build();

        let decoded = Manifest::decode(&manifest.encode()).expect("decode manifest");

        assert_eq!(decoded.resource_map, manifest.resource_map);
        assert_eq!(
            decoded.external_section_locations,
            vec![ManifestSectionLocation::new(
                10,
                "assets/section-10.bin",
                "sha256:section-10",
                1
            )]
        );
        assert_eq!(
            decoded.worker_entries,
            vec![ManifestWorkerEntry::new("engine", 4, 1, u64::MAX)]
        );
    }

    #[test]
    fn manifest_decodes_without_external_section_locations() {
        let manifest = ManifestBuilder::new("legacy", 10, 20)
            .push_resource_mapping(ManifestResourceEntry::new(0x1234, 100))
            .build();
        let mut bytes = manifest.encode();
        bytes.truncate(bytes.len() - 4);

        let decoded = Manifest::decode(&bytes).expect("decode legacy manifest");

        assert!(decoded.external_section_locations.is_empty());
        assert!(decoded.worker_entries.is_empty());
        assert_eq!(decoded.resource_id_by_name_hash(0x1234), Some(100));
    }
}
