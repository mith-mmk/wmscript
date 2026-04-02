#![forbid(unsafe_code)]

use std::io::{Read, Seek, SeekFrom};

use crate::{
    ArchiveError, Digest, FixedHeader, Manifest, Result, SectionEntry, SectionId, SectionKind,
    SecurityHeader, digest_section,
};

fn read_exact<const N: usize, R: Read>(reader: &mut R) -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16> {
    Ok(u16::from_le_bytes(read_exact::<2, _>(reader)?))
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    Ok(u32::from_le_bytes(read_exact::<4, _>(reader)?))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    Ok(u64::from_le_bytes(read_exact::<8, _>(reader)?))
}

/// Streaming archive reader backed by any `Read + Seek` source.
#[derive(Debug)]
pub struct ArchiveStreamReader<R> {
    reader: R,
    header: FixedHeader,
    security: SecurityHeader,
    sections: Vec<SectionEntry>,
    data_len: u64,
}

impl<R: Read + Seek> ArchiveStreamReader<R> {
    pub fn open(mut reader: R) -> Result<Self> {
        let data_len = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(0))?;

        let magic = read_exact::<4, _>(&mut reader)?;
        if magic != FixedHeader::MAGIC {
            return Err(ArchiveError::InvalidMagic);
        }
        let archive_version = read_u16(&mut reader)?;
        if archive_version != FixedHeader::VERSION {
            return Err(ArchiveError::UnsupportedVersion(archive_version));
        }
        let header = FixedHeader {
            magic,
            archive_version,
            header_size: read_u16(&mut reader)?,
            flags: read_u32(&mut reader)?,
            section_count: read_u32(&mut reader)?,
            section_table_offset: read_u64(&mut reader)?,
            section_table_size: read_u64(&mut reader)?,
            security_offset: read_u64(&mut reader)?,
            security_size: read_u64(&mut reader)?,
            signature_offset: read_u64(&mut reader)?,
            signature_size: read_u64(&mut reader)?,
        };

        reader.seek(SeekFrom::Start(header.security_offset))?;
        let security = SecurityHeader {
            security_version: read_u16(&mut reader)?,
            sig_alg: read_u16(&mut reader)?,
            hash_alg: read_u16(&mut reader)?,
            enc_alg: read_u16(&mut reader)?,
            key_id: read_exact::<16, _>(&mut reader)?,
            nonce_len: read_u16(&mut reader)?,
            reserved: read_u16(&mut reader)?,
            manifest_digest_offset: read_u64(&mut reader)?,
            manifest_digest_size: read_u64(&mut reader)?,
        };

        reader.seek(SeekFrom::Start(header.section_table_offset))?;
        let mut sections = Vec::with_capacity(header.section_count as usize);
        for _ in 0..header.section_count {
            sections.push(SectionEntry {
                id: read_u32(&mut reader)?,
                kind: SectionKind::from_u16(read_u16(&mut reader)?),
                flags: read_u16(&mut reader)?,
                offset: read_u64(&mut reader)?,
                packed_size: read_u64(&mut reader)?,
                unpacked_size: read_u64(&mut reader)?,
                align: read_u32(&mut reader)?,
                reserved: read_u32(&mut reader)?,
                name_hash: read_u64(&mut reader)?,
            });
        }

        let archive = Self {
            reader,
            header,
            security,
            sections,
            data_len,
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

    pub fn data_len(&self) -> u64 {
        self.data_len
    }

    pub fn sections(&self) -> &[SectionEntry] {
        &self.sections
    }

    pub fn section(&self, section_id: SectionId) -> Option<&SectionEntry> {
        self.sections
            .iter()
            .find(|section| section.id == section_id)
    }

    pub fn read_section(&mut self, section_id: SectionId) -> Result<Vec<u8>> {
        let section = *self.section(section_id).ok_or_else(|| {
            ArchiveError::InvalidManifest(format!("missing section {section_id}"))
        })?;
        self.read_section_entry(&section)
    }

    pub fn read_section_entry(&mut self, section: &SectionEntry) -> Result<Vec<u8>> {
        self.reader.seek(SeekFrom::Start(section.offset))?;
        let mut bytes = vec![0u8; section.packed_size as usize];
        self.reader.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    pub fn manifest(&mut self) -> Result<Option<Manifest>> {
        let Some(section) = self
            .sections
            .iter()
            .find(|section| matches!(section.kind, SectionKind::Manifest))
            .copied()
        else {
            return Ok(None);
        };
        Ok(Some(Manifest::decode(&self.read_section_entry(&section)?)?))
    }

    pub fn signature_bytes(&mut self) -> Result<Option<Vec<u8>>> {
        if self.header.signature_size == 0 {
            return Ok(None);
        }
        self.reader
            .seek(SeekFrom::Start(self.header.signature_offset))?;
        let mut bytes = vec![0u8; self.header.signature_size as usize];
        self.reader.read_exact(&mut bytes)?;
        Ok(Some(bytes))
    }

    pub fn verify_layout(&self) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for section in &self.sections {
            if !seen.insert(section.id) {
                return Err(ArchiveError::DuplicateSectionId(section.id));
            }
            let end = section
                .offset
                .checked_add(section.packed_size)
                .ok_or(ArchiveError::BrokenLayout)?;
            if end > self.data_len {
                return Err(ArchiveError::SectionOutOfRange {
                    section_id: section.id,
                    offset: section.offset,
                    size: section.packed_size,
                    data_len: usize::try_from(self.data_len).unwrap_or(usize::MAX),
                });
            }
        }
        Ok(())
    }

    pub fn verify_manifest_digests(&mut self) -> Result<()> {
        let Some(manifest) = self.manifest()? else {
            return Ok(());
        };
        for digest in &manifest.section_digests {
            let section = *self.section(digest.section_id).ok_or_else(|| {
                ArchiveError::InvalidManifest(format!(
                    "missing section for digest {}",
                    digest.section_id
                ))
            })?;
            let payload = self.read_section_entry(&section)?;
            let actual = digest_section(
                section.id,
                section.kind,
                digest.flags_canonical,
                section.unpacked_size,
                &payload,
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

    pub fn into_inner(self) -> R {
        self.reader
    }
}

pub fn normalize_stream_signature_bytes<R: Read + Seek>(
    archive: &mut ArchiveStreamReader<R>,
) -> Result<(Vec<u8>, Option<Digest>)> {
    let manifest = archive.manifest()?;
    archive.verify_layout()?;
    let normalized = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&archive.header.magic);
        bytes.extend_from_slice(&archive.header.archive_version.to_le_bytes());
        bytes.extend_from_slice(&archive.header.header_size.to_le_bytes());
        bytes.extend_from_slice(&archive.header.flags.to_le_bytes());
        bytes.extend_from_slice(&archive.header.section_count.to_le_bytes());
        bytes.extend_from_slice(&archive.header.section_table_offset.to_le_bytes());
        bytes.extend_from_slice(&archive.header.section_table_size.to_le_bytes());
        bytes.extend_from_slice(&archive.header.security_offset.to_le_bytes());
        bytes.extend_from_slice(&archive.header.security_size.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&archive.security.security_version.to_le_bytes());
        bytes.extend_from_slice(&archive.security.sig_alg.to_le_bytes());
        bytes.extend_from_slice(&archive.security.hash_alg.to_le_bytes());
        bytes.extend_from_slice(&archive.security.enc_alg.to_le_bytes());
        bytes.extend_from_slice(&archive.security.key_id);
        bytes.extend_from_slice(&archive.security.nonce_len.to_le_bytes());
        bytes.extend_from_slice(&archive.security.reserved.to_le_bytes());
        bytes.extend_from_slice(&archive.security.manifest_digest_offset.to_le_bytes());
        bytes.extend_from_slice(&archive.security.manifest_digest_size.to_le_bytes());
        for section in &archive.sections {
            bytes.extend_from_slice(&section.id.to_le_bytes());
            bytes.extend_from_slice(&section.kind.as_u16().to_le_bytes());
            bytes.extend_from_slice(&section.flags.to_le_bytes());
            bytes.extend_from_slice(&section.offset.to_le_bytes());
            bytes.extend_from_slice(&section.packed_size.to_le_bytes());
            bytes.extend_from_slice(&section.unpacked_size.to_le_bytes());
            bytes.extend_from_slice(&section.align.to_le_bytes());
            bytes.extend_from_slice(&section.reserved.to_le_bytes());
            bytes.extend_from_slice(&section.name_hash.to_le_bytes());
        }
        for section in archive.sections.clone() {
            bytes.extend_from_slice(&archive.read_section_entry(&section)?);
        }
        bytes
    };
    Ok((
        normalized,
        manifest.map(|manifest| manifest_digest(&manifest)),
    ))
}

fn manifest_digest(manifest: &Manifest) -> Digest {
    digest_section(
        1,
        SectionKind::Manifest,
        0,
        manifest.encode().len() as u64,
        &manifest.encode(),
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{ArchiveBuilder, ArchiveSection, ManifestBuilder};

    use super::*;

    #[test]
    fn stream_reader_reads_manifest_and_sections() {
        let manifest = ManifestBuilder::new("stream", 1, 2).build();
        let bytes = ArchiveBuilder::new()
            .push_manifest(1, &manifest)
            .push_section(ArchiveSection::new(
                2,
                SectionKind::Module,
                vec![1, 2, 3, 4],
            ))
            .build()
            .expect("build archive");

        let mut reader = ArchiveStreamReader::open(Cursor::new(bytes)).expect("open stream");
        assert_eq!(reader.sections().len(), 2);
        assert_eq!(
            reader.manifest().expect("manifest").expect("some manifest"),
            manifest
        );
        assert_eq!(reader.read_section(2).expect("section"), vec![1, 2, 3, 4]);
    }
}
