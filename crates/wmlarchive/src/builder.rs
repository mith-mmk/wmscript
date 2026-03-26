#![forbid(unsafe_code)]

use crate::{
    ArchiveError, ArchiveSection, ArchiveSigner, FixedHeader, Manifest, Result, SectionEntry,
    SectionId, SectionKind, SecurityHeader, digest_section,
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

fn read_bytes<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = offset.checked_add(len).ok_or(ArchiveError::UnexpectedEof)?;
    let slice = bytes.get(*offset..end).ok_or(ArchiveError::UnexpectedEof)?;
    *offset = end;
    Ok(slice)
}

fn align_up(value: usize, align: usize) -> usize {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn patch_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let end = offset.checked_add(8).ok_or(ArchiveError::BrokenLayout)?;
    let slice = bytes
        .get_mut(offset..end)
        .ok_or(ArchiveError::BrokenLayout)?;
    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// Builder used to emit archive bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveBuilder {
    security: SecurityHeader,
    sections: Vec<ArchiveSection>,
}

impl ArchiveBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_security(mut self, security: SecurityHeader) -> Self {
        self.security = security;
        self
    }

    pub fn push_section(mut self, section: ArchiveSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn push_manifest(mut self, section_id: SectionId, manifest: &Manifest) -> Self {
        self.sections.push(ArchiveSection {
            id: section_id,
            kind: SectionKind::Manifest,
            flags: 0,
            align: 16,
            name_hash: 0,
            payload: manifest.encode(),
        });
        self
    }

    pub fn build_signed(self, signer: &ArchiveSigner) -> Result<Vec<u8>> {
        let mut builder = self.clone();
        let signer_security = signer.security_header();
        builder.security.security_version = signer_security.security_version;
        builder.security.sig_alg = signer_security.sig_alg;
        builder.security.hash_alg = signer_security.hash_alg;
        builder.security.enc_alg = signer_security.enc_alg;
        builder.security.key_id = signer_security.key_id;
        let mut bytes = builder.build()?;
        let signature = signer.sign(&bytes);
        let signature_offset = bytes.len() as u64;
        let signature_size = signature.len() as u64;
        patch_u64(&mut bytes, 48, signature_offset)?;
        patch_u64(&mut bytes, 56, signature_size)?;
        bytes.extend_from_slice(&signature);
        Ok(bytes)
    }

    pub fn build(self) -> Result<Vec<u8>> {
        let mut sections = self.sections;
        sections.sort_by_key(|section| section.id);
        for pair in sections.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(ArchiveError::DuplicateSectionId(pair[0].id));
            }
        }

        let section_table_offset = FixedHeader::BYTE_SIZE as u64 + SecurityHeader::BYTE_SIZE as u64;
        let section_table_size = sections.len() as u64 * SectionEntry::BYTE_SIZE as u64;
        let header = FixedHeader {
            magic: FixedHeader::MAGIC,
            archive_version: FixedHeader::VERSION,
            header_size: FixedHeader::BYTE_SIZE as u16,
            flags: 0,
            section_count: sections.len() as u32,
            section_table_offset,
            section_table_size,
            security_offset: FixedHeader::BYTE_SIZE as u64,
            security_size: SecurityHeader::BYTE_SIZE as u64,
            signature_offset: 0,
            signature_size: 0,
        };

        let mut entries = Vec::with_capacity(sections.len());
        let mut payloads = Vec::with_capacity(sections.len());
        let mut cursor = align_up((section_table_offset + section_table_size) as usize, 16);
        for section in sections {
            cursor = align_up(cursor, section.align.max(1) as usize);
            let offset = cursor as u64;
            let size = section.payload.len() as u64;
            entries.push(SectionEntry {
                id: section.id,
                kind: section.kind,
                flags: section.flags,
                offset,
                packed_size: size,
                unpacked_size: size,
                align: section.align,
                reserved: 0,
                name_hash: section.name_hash,
            });
            payloads.push((cursor, section.payload));
            cursor = cursor.saturating_add(size as usize);
        }

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&header.magic);
        write_u16(&mut bytes, header.archive_version);
        write_u16(&mut bytes, header.header_size);
        write_u32(&mut bytes, header.flags);
        write_u32(&mut bytes, header.section_count);
        write_u64(&mut bytes, header.section_table_offset);
        write_u64(&mut bytes, header.section_table_size);
        write_u64(&mut bytes, header.security_offset);
        write_u64(&mut bytes, header.security_size);
        write_u64(&mut bytes, header.signature_offset);
        write_u64(&mut bytes, header.signature_size);

        write_u16(&mut bytes, self.security.security_version);
        write_u16(&mut bytes, self.security.sig_alg);
        write_u16(&mut bytes, self.security.hash_alg);
        write_u16(&mut bytes, self.security.enc_alg);
        bytes.extend_from_slice(&self.security.key_id);
        write_u16(&mut bytes, self.security.nonce_len);
        write_u16(&mut bytes, self.security.reserved);
        write_u64(&mut bytes, self.security.manifest_digest_offset);
        write_u64(&mut bytes, self.security.manifest_digest_size);

        for entry in &entries {
            write_u32(&mut bytes, entry.id);
            write_u16(&mut bytes, entry.kind.as_u16());
            write_u16(&mut bytes, entry.flags);
            write_u64(&mut bytes, entry.offset);
            write_u64(&mut bytes, entry.packed_size);
            write_u64(&mut bytes, entry.unpacked_size);
            write_u32(&mut bytes, entry.align);
            write_u32(&mut bytes, entry.reserved);
            write_u64(&mut bytes, entry.name_hash);
        }

        if bytes.len() < section_table_offset as usize + section_table_size as usize {
            bytes.resize(
                section_table_offset as usize + section_table_size as usize,
                0,
            );
        }

        for (offset, payload) in payloads {
            if bytes.len() < offset {
                bytes.resize(offset, 0);
            }
            bytes.extend_from_slice(&payload);
        }

        Ok(bytes)
    }
}

/// Parsed archive view.
#[derive(Clone, Debug, PartialEq)]
pub struct Archive<'a> {
    data: &'a [u8],
    header: FixedHeader,
    security: SecurityHeader,
    sections: Vec<SectionEntry>,
}

impl<'a> Archive<'a> {
    pub fn decode(data: &'a [u8]) -> Result<Self> {
        let mut offset = 0usize;
        let magic = read_bytes(data, &mut offset, 4)?;
        if magic != b"WARC" {
            return Err(ArchiveError::InvalidMagic);
        }
        let archive_version = read_u16(data, &mut offset)?;
        if archive_version != FixedHeader::VERSION {
            return Err(ArchiveError::UnsupportedVersion(archive_version));
        }
        let header_size = read_u16(data, &mut offset)?;
        let flags = read_u32(data, &mut offset)?;
        let section_count = read_u32(data, &mut offset)?;
        let section_table_offset = read_u64(data, &mut offset)?;
        let section_table_size = read_u64(data, &mut offset)?;
        let security_offset = read_u64(data, &mut offset)?;
        let security_size = read_u64(data, &mut offset)?;
        let signature_offset = read_u64(data, &mut offset)?;
        let signature_size = read_u64(data, &mut offset)?;
        let header = FixedHeader {
            magic: FixedHeader::MAGIC,
            archive_version,
            header_size,
            flags,
            section_count,
            section_table_offset,
            section_table_size,
            security_offset,
            security_size,
            signature_offset,
            signature_size,
        };
        let mut security_offset_cursor = FixedHeader::BYTE_SIZE;
        let security = SecurityHeader {
            security_version: read_u16(data, &mut security_offset_cursor)?,
            sig_alg: read_u16(data, &mut security_offset_cursor)?,
            hash_alg: read_u16(data, &mut security_offset_cursor)?,
            enc_alg: read_u16(data, &mut security_offset_cursor)?,
            key_id: {
                let slice = read_bytes(data, &mut security_offset_cursor, 16)?;
                let mut key_id = [0u8; 16];
                key_id.copy_from_slice(slice);
                key_id
            },
            nonce_len: read_u16(data, &mut security_offset_cursor)?,
            reserved: read_u16(data, &mut security_offset_cursor)?,
            manifest_digest_offset: read_u64(data, &mut security_offset_cursor)?,
            manifest_digest_size: read_u64(data, &mut security_offset_cursor)?,
        };

        let table_start = FixedHeader::BYTE_SIZE + SecurityHeader::BYTE_SIZE;
        let table_end = table_start
            .checked_add(section_count as usize * SectionEntry::BYTE_SIZE)
            .ok_or(ArchiveError::BrokenLayout)?;
        let table = data
            .get(table_start..table_end)
            .ok_or(ArchiveError::UnexpectedEof)?;
        let mut sections = Vec::with_capacity(section_count as usize);
        let mut table_offset = 0usize;
        for _ in 0..section_count {
            let id = read_u32(table, &mut table_offset)?;
            let kind = SectionKind::from_u16(read_u16(table, &mut table_offset)?);
            let flags = read_u16(table, &mut table_offset)?;
            let offset = read_u64(table, &mut table_offset)?;
            let packed_size = read_u64(table, &mut table_offset)?;
            let unpacked_size = read_u64(table, &mut table_offset)?;
            let align = read_u32(table, &mut table_offset)?;
            let reserved = read_u32(table, &mut table_offset)?;
            let name_hash = read_u64(table, &mut table_offset)?;
            sections.push(SectionEntry {
                id,
                kind,
                flags,
                offset,
                packed_size,
                unpacked_size,
                align,
                reserved,
                name_hash,
            });
        }

        let archive = Self {
            data,
            header,
            security,
            sections,
        };
        archive.verify_layout()?;
        Ok(archive)
    }

    pub const fn header(&self) -> FixedHeader {
        self.header
    }

    pub const fn security(&self) -> SecurityHeader {
        self.security
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.data
    }

    pub fn sections(&self) -> &[SectionEntry] {
        &self.sections
    }

    pub fn section(&self, section_id: SectionId) -> Option<&SectionEntry> {
        self.sections
            .iter()
            .find(|section| section.id == section_id)
    }

    pub fn section_bytes(&self, section_id: SectionId) -> Option<&'a [u8]> {
        let section = self.section(section_id)?;
        let start = section.offset as usize;
        let end = start.checked_add(section.packed_size as usize)?;
        self.data.get(start..end)
    }

    pub fn manifest_bytes(&self) -> Option<&'a [u8]> {
        self.sections
            .iter()
            .find(|section| matches!(section.kind, SectionKind::Manifest))
            .and_then(|section| self.section_bytes(section.id))
    }

    pub fn manifest(&self) -> Result<Option<Manifest>> {
        match self.manifest_bytes() {
            Some(bytes) => Ok(Some(Manifest::decode(bytes)?)),
            None => Ok(None),
        }
    }

    pub fn signature_bytes(&self) -> Option<&'a [u8]> {
        let size = self.header.signature_size as usize;
        if size == 0 {
            return None;
        }
        let start = self.header.signature_offset as usize;
        let end = start.checked_add(size)?;
        self.data.get(start..end)
    }

    pub fn verify_layout(&self) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for section in &self.sections {
            if !seen.insert(section.id) {
                return Err(ArchiveError::DuplicateSectionId(section.id));
            }
            let start = section.offset as usize;
            let end = start
                .checked_add(section.packed_size as usize)
                .ok_or(ArchiveError::BrokenLayout)?;
            if end > self.data.len() {
                return Err(ArchiveError::SectionOutOfRange {
                    section_id: section.id,
                    offset: section.offset,
                    size: section.packed_size,
                    data_len: self.data.len(),
                });
            }
        }
        Ok(())
    }

    pub fn verify_manifest_digests(&self) -> Result<()> {
        let Some(manifest) = self.manifest()? else {
            return Ok(());
        };
        for digest in &manifest.section_digests {
            let section = self.section(digest.section_id).ok_or_else(|| {
                ArchiveError::InvalidManifest(format!(
                    "missing section for digest {}",
                    digest.section_id
                ))
            })?;
            let payload = self
                .section_bytes(section.id)
                .ok_or(ArchiveError::BrokenLayout)?;
            let actual = digest_section(
                section.id,
                section.kind,
                digest.flags_canonical,
                section.unpacked_size,
                payload,
            );
            if actual != digest.digest {
                return Err(ArchiveError::InvalidManifest(format!(
                    "digest mismatch for section {}",
                    section.id
                )));
            }
        }
        Ok(())
    }

    pub fn verify_signature(&self, keyring: &crate::KeyRing) -> Result<()> {
        crate::ArchiveVerifier::new(keyring).verify(self)
    }

    pub fn unpack(&self) -> Result<crate::ArchiveBundle> {
        crate::ArchiveBundle::from_archive(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ManifestArchiveDepEntry, ManifestBuilder, ManifestExtEntry, ManifestResourceEntry, Version,
    };

    #[test]
    fn archive_builder_roundtrip_sections() {
        let manifest = ManifestBuilder::new("demo", 77, 88)
            .package_version(Version::new(1, 2, 3, 4))
            .push_extension(ManifestExtEntry::new("ext.fs.read", 1, 2, 3))
            .push_archive_dependency(ManifestArchiveDepEntry::new(
                99,
                "dep",
                Version::new(1, 0, 0, 0),
                4,
            ))
            .push_resource_mapping(ManifestResourceEntry::new(123, 456))
            .build();
        let bytes = ArchiveBuilder::new()
            .push_section(ArchiveSection::new(2, SectionKind::Module, vec![1, 2, 3]))
            .push_manifest(1, &manifest)
            .build()
            .expect("build archive");
        let archive = Archive::decode(&bytes).expect("decode archive");
        assert_eq!(archive.sections().len(), 2);
        assert_eq!(
            archive.section(1).expect("manifest section").kind,
            SectionKind::Manifest
        );
        assert!(archive.manifest().expect("manifest").is_some());
    }

    #[test]
    fn signed_archive_verifies_with_keyring() {
        let manifest = ManifestBuilder::new("signed", 7, 9).build();
        let signer = ArchiveSigner::new([1; 16], b"secret");
        let bytes = ArchiveBuilder::new()
            .push_manifest(1, &manifest)
            .build_signed(&signer)
            .expect("build signed archive");
        let archive = Archive::decode(&bytes).expect("decode archive");
        let mut keyring = crate::KeyRing::new();
        keyring.register(crate::SigningKey::new([1; 16], b"secret"));
        assert!(archive.verify_signature(&keyring).is_ok());
    }

    #[test]
    fn archive_unpacks_sections() {
        let manifest = ManifestBuilder::new("unpack", 11, 22).build();
        let bytes = ArchiveBuilder::new()
            .push_section(ArchiveSection::new(2, SectionKind::Module, vec![1, 2, 3]))
            .push_manifest(1, &manifest)
            .build()
            .expect("build archive");
        let archive = Archive::decode(&bytes).expect("decode archive");
        let bundle = archive.unpack().expect("unpack archive");
        assert_eq!(bundle.sections().len(), 2);
        assert!(bundle.manifest().is_some());
        assert_eq!(bundle.asset_sections().count(), 0);
    }
}
