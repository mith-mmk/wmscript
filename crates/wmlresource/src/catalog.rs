#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use wmlarchive::{Archive, Manifest, SectionKind};

use crate::{ResourceError, ResourceHeader, ResourceId, ResourceType, Result as ResourceResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLocation {
    pub section_id: u32,
    pub offset: u64,
    pub size: u64,
    pub resource_type: ResourceType,
    pub flags: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceCatalog {
    by_id: BTreeMap<ResourceId, ResourceLocation>,
    by_name_hash: BTreeMap<u64, ResourceId>,
}

impl ResourceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        resource_id: ResourceId,
        location: ResourceLocation,
    ) -> ResourceResult<()> {
        if self.by_id.insert(resource_id, location).is_some() {
            return Err(ResourceError::DuplicateResourceId(resource_id));
        }
        Ok(())
    }

    pub fn insert_name_hash(&mut self, name_hash: u64, resource_id: ResourceId) {
        self.by_name_hash.insert(name_hash, resource_id);
    }

    pub fn location(&self, resource_id: ResourceId) -> Option<&ResourceLocation> {
        self.by_id.get(&resource_id)
    }

    pub fn resource_id_by_name_hash(&self, name_hash: u64) -> Option<ResourceId> {
        self.by_name_hash.get(&name_hash).copied()
    }

    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut catalog = Self::new();
        for entry in &manifest.resource_map {
            catalog.insert_name_hash(entry.name_hash, entry.resource_id);
        }
        catalog
    }

    pub fn from_archive(archive: &Archive<'_>) -> ResourceResult<Self> {
        let mut catalog = archive
            .manifest()?
            .map(|manifest| Self::from_manifest(&manifest))
            .unwrap_or_default();

        for section in archive.sections() {
            if !matches!(section.kind, SectionKind::Asset) {
                continue;
            }
            let bytes = archive.section_bytes(section.id).ok_or_else(|| {
                ResourceError::InvalidArchiveSection(format!(
                    "asset section {} is out of range",
                    section.id
                ))
            })?;
            let header = decode_resource_header(bytes)?;
            catalog.insert(
                header.resource_id,
                ResourceLocation {
                    section_id: section.id,
                    offset: header.data_offset as u64,
                    size: header.unpacked_size as u64,
                    resource_type: header.resource_type,
                    flags: header.flags,
                },
            )?;
        }

        Ok(catalog)
    }
}

pub fn decode_resource_header(bytes: &[u8]) -> ResourceResult<ResourceHeader> {
    if bytes.len() < ResourceHeader::BYTE_SIZE {
        return Err(ResourceError::InvalidResourceHeader(
            "asset section is too small for a resource header".to_owned(),
        ));
    }
    let mut offset = 0usize;
    let read_u16 = |bytes: &[u8], offset: &mut usize| -> ResourceResult<u16> {
        let end = offset.saturating_add(2);
        let slice = bytes.get(*offset..end).ok_or_else(|| {
            ResourceError::InvalidResourceHeader("unexpected end of resource header".to_owned())
        })?;
        *offset = end;
        Ok(u16::from_le_bytes([slice[0], slice[1]]))
    };
    let read_u32 = |bytes: &[u8], offset: &mut usize| -> ResourceResult<u32> {
        let end = offset.saturating_add(4);
        let slice = bytes.get(*offset..end).ok_or_else(|| {
            ResourceError::InvalidResourceHeader("unexpected end of resource header".to_owned())
        })?;
        *offset = end;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    };
    let resource_id = read_u32(bytes, &mut offset)?;
    let resource_type = ResourceType::from_u16(read_u16(bytes, &mut offset)?);
    let flags = read_u16(bytes, &mut offset)?;
    let compression = read_u16(bytes, &mut offset)?;
    let encoding = read_u16(bytes, &mut offset)?;
    let unpacked_size = read_u32(bytes, &mut offset)?;
    let packed_size = read_u32(bytes, &mut offset)?;
    let data_offset = read_u32(bytes, &mut offset)?;

    Ok(ResourceHeader {
        resource_id,
        resource_type,
        flags,
        compression,
        encoding,
        unpacked_size,
        packed_size,
        data_offset,
    })
}
