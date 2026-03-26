#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use wmlarchive::{Archive, SectionKind};

use crate::{
    Handle, HandleEntry, LoadResult, RequestId, RequestState, RequestStatus, ResourceCatalog,
    ResourceData, ResourceEntry, ResourceError, ResourceId, ResourceLocation, ResourceRequest,
    ResourceState, ResourceType, Result as ResourceResult, decode_resource_header,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HandleTable {
    entries: Vec<HandleEntry>,
}

impl HandleTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self, resource_id: ResourceId) -> Handle {
        let index = self.entries.len() as u32;
        self.entries.push(HandleEntry::new(resource_id));
        Handle::new(index, 1)
    }

    pub fn resolve(&self, handle: Handle) -> ResourceResult<ResourceId> {
        let index = handle.index() as usize;
        let entry = self
            .entries
            .get(index)
            .ok_or(ResourceError::UnknownHandle(handle))?;
        if !entry.alive || entry.generation != handle.generation() {
            return Err(ResourceError::HandleExpired(handle));
        }
        Ok(entry.resource_id)
    }

    pub fn retain(&mut self, handle: Handle) -> ResourceResult<()> {
        let index = handle.index() as usize;
        let entry = self
            .entries
            .get_mut(index)
            .ok_or(ResourceError::UnknownHandle(handle))?;
        if !entry.alive || entry.generation != handle.generation() {
            return Err(ResourceError::HandleExpired(handle));
        }
        entry.ref_count = entry.ref_count.saturating_add(1);
        Ok(())
    }

    pub fn release(&mut self, handle: Handle) -> ResourceResult<()> {
        let index = handle.index() as usize;
        let entry = self
            .entries
            .get_mut(index)
            .ok_or(ResourceError::UnknownHandle(handle))?;
        if !entry.alive || entry.generation != handle.generation() {
            return Err(ResourceError::HandleExpired(handle));
        }
        if entry.ref_count > 0 {
            entry.ref_count -= 1;
        }
        if entry.ref_count == 0 {
            entry.alive = false;
            entry.generation = entry.generation.wrapping_add(1);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceManager {
    catalog: ResourceCatalog,
    entries: BTreeMap<ResourceId, ResourceEntry>,
    requests: BTreeMap<RequestId, ResourceRequest>,
    handle_table: HandleTable,
    memory_used: usize,
    memory_limit: usize,
    next_request_id: RequestId,
}

impl ResourceManager {
    pub fn new(memory_limit: usize) -> Self {
        Self {
            memory_limit,
            ..Self::default()
        }
    }

    pub fn with_catalog(catalog: ResourceCatalog, memory_limit: usize) -> Self {
        Self {
            catalog,
            memory_limit,
            ..Self::default()
        }
    }

    pub fn catalog(&self) -> &ResourceCatalog {
        &self.catalog
    }

    pub fn catalog_mut(&mut self) -> &mut ResourceCatalog {
        &mut self.catalog
    }

    pub fn entries(&self) -> impl Iterator<Item = &ResourceEntry> {
        self.entries.values()
    }

    pub fn declare_resource(&mut self, resource_id: ResourceId, flags: u32) -> ResourceResult<()> {
        let entry = self
            .entries
            .entry(resource_id)
            .or_insert_with(|| ResourceEntry::new(resource_id));
        entry.flags = flags;
        Ok(())
    }

    pub fn register_ready(
        &mut self,
        resource_id: ResourceId,
        data: ResourceData,
        flags: u32,
    ) -> ResourceResult<Handle> {
        let size = data.bytes().len();
        let entry = self
            .entries
            .entry(resource_id)
            .or_insert_with(|| ResourceEntry::new(resource_id));
        entry.state = ResourceState::Ready;
        entry.ref_count = 1;
        entry.data = Some(data);
        entry.flags = flags;
        self.memory_used = self.memory_used.saturating_add(size);
        Ok(self.handle_table.allocate(resource_id))
    }

    pub fn ingest_archive(&mut self, archive: &Archive<'_>) -> ResourceResult<usize> {
        let mut loaded = 0usize;
        if let Some(manifest) = archive.manifest()? {
            for entry in manifest.resource_map {
                self.catalog
                    .insert_name_hash(entry.name_hash, entry.resource_id);
            }
        }

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
            let start = header.data_offset as usize;
            let end = start
                .checked_add(header.unpacked_size as usize)
                .ok_or_else(|| {
                    ResourceError::InvalidArchiveSection(format!(
                        "asset section {} payload overflows",
                        section.id
                    ))
                })?;
            let payload = bytes.get(start..end).ok_or_else(|| {
                ResourceError::InvalidArchiveSection(format!(
                    "asset section {} payload missing",
                    section.id
                ))
            })?;
            let data = match header.resource_type {
                ResourceType::Image => ResourceData::Image(payload.to_vec()),
                ResourceType::Audio => ResourceData::Audio(payload.to_vec()),
                ResourceType::Binary => ResourceData::Binary(payload.to_vec()),
                ResourceType::Font => ResourceData::Font(payload.to_vec()),
                ResourceType::Video => ResourceData::Video(payload.to_vec()),
                ResourceType::ScriptData | ResourceType::Unknown(_) => {
                    ResourceData::ScriptData(payload.to_vec())
                }
            };
            self.register_ready(header.resource_id, data, header.flags as u32)?;
            self.catalog.insert(
                header.resource_id,
                ResourceLocation {
                    section_id: section.id,
                    offset: header.data_offset as u64,
                    size: header.unpacked_size as u64,
                    resource_type: header.resource_type,
                    flags: header.flags,
                },
            )?;
            loaded += 1;
        }
        Ok(loaded)
    }

    pub fn load_resource(&mut self, resource_id: ResourceId) -> ResourceResult<LoadResult> {
        let entry = self
            .entries
            .entry(resource_id)
            .or_insert_with(|| ResourceEntry::new(resource_id));
        match (&entry.state, entry.data.as_ref()) {
            (ResourceState::Ready, Some(_)) => {
                let handle = self.handle_table.allocate(resource_id);
                entry.ref_count = entry.ref_count.saturating_add(1);
                Ok(LoadResult::Ready(handle))
            }
            _ => {
                if self.catalog.location(resource_id).is_none() {
                    return Err(ResourceError::UnknownResourceId(resource_id));
                }
                let request_id = self.next_request_id;
                self.next_request_id = self.next_request_id.saturating_add(1);
                let mut request = ResourceRequest::new(request_id, resource_id, 0, 0);
                request.state = RequestState::Pending;
                entry.state = ResourceState::Loading;
                self.requests.insert(request_id, request);
                Ok(LoadResult::Pending(request_id))
            }
        }
    }

    pub fn complete_request(
        &mut self,
        request_id: RequestId,
        data: ResourceData,
    ) -> ResourceResult<Handle> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(ResourceError::RequestNotFound(request_id))?;
        let resource_id = request.resource_id;
        let handle = self.handle_table.allocate(resource_id);
        let entry = self
            .entries
            .entry(resource_id)
            .or_insert_with(|| ResourceEntry::new(resource_id));
        entry.state = ResourceState::Ready;
        entry.ref_count = entry.ref_count.saturating_add(1);
        self.memory_used = self.memory_used.saturating_add(data.bytes().len());
        entry.data = Some(data);
        request.state = RequestState::Completed;
        request.result_handle = Some(handle);
        Ok(handle)
    }

    pub fn poll_request(&self, request_id: RequestId) -> ResourceResult<RequestStatus> {
        let request = self
            .requests
            .get(&request_id)
            .ok_or(ResourceError::RequestNotFound(request_id))?;
        Ok(RequestStatus {
            done: matches!(
                request.state,
                RequestState::Completed | RequestState::Failed | RequestState::Cancelled
            ),
            ok: matches!(request.state, RequestState::Completed),
            handle: request.result_handle,
            error: request.error.clone(),
        })
    }

    pub fn await_request(&self, request_id: RequestId) -> ResourceResult<Option<Handle>> {
        let status = self.poll_request(request_id)?;
        if status.done {
            Ok(status.handle)
        } else {
            Ok(None)
        }
    }

    pub fn retain(&mut self, handle: Handle) -> ResourceResult<()> {
        self.handle_table.retain(handle)
    }

    pub fn release(&mut self, handle: Handle) -> ResourceResult<()> {
        self.handle_table.release(handle)
    }

    pub fn status(&self, handle: Handle) -> ResourceResult<ResourceState> {
        let resource_id = self.handle_table.resolve(handle)?;
        Ok(self
            .entries
            .get(&resource_id)
            .ok_or(ResourceError::UnknownResourceId(resource_id))?
            .state)
    }

    pub fn entry(&self, resource_id: ResourceId) -> Option<&ResourceEntry> {
        self.entries.get(&resource_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceType;
    use wmlarchive::{ArchiveBuilder, ArchiveSection, Manifest};

    #[test]
    fn handle_generation_protects_against_reuse() {
        let mut table = HandleTable::new();
        let handle = table.allocate(7);
        assert_eq!(table.resolve(handle), Ok(7));
        table.release(handle).expect("release");
        assert!(matches!(
            table.resolve(handle),
            Err(ResourceError::HandleExpired(_))
        ));
    }

    #[test]
    fn manager_registers_ready_resources() {
        let mut manager = ResourceManager::new(1024);
        let handle = manager
            .register_ready(42, ResourceData::Binary(vec![1, 2, 3]), 0)
            .expect("register");
        assert_eq!(
            manager.status(handle).expect("status"),
            ResourceState::Ready
        );
    }

    #[test]
    fn manager_loads_resources_from_archive() {
        let manifest = Manifest::builder("demo", 1, 2).build();
        let mut resource_bytes = vec![0u8; 24];
        resource_bytes[0..4].copy_from_slice(&42u32.to_le_bytes());
        resource_bytes[4..6].copy_from_slice(&(ResourceType::Binary.as_u16()).to_le_bytes());
        resource_bytes[6..8].copy_from_slice(&0u16.to_le_bytes());
        resource_bytes[8..10].copy_from_slice(&0u16.to_le_bytes());
        resource_bytes[10..12].copy_from_slice(&0u16.to_le_bytes());
        resource_bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
        resource_bytes[16..20].copy_from_slice(&3u32.to_le_bytes());
        resource_bytes[20..24].copy_from_slice(&24u32.to_le_bytes());
        resource_bytes.extend_from_slice(&[9, 8, 7]);

        let archive_bytes = ArchiveBuilder::new()
            .push_manifest(1, &manifest)
            .push_section(ArchiveSection::new(
                2,
                wmlarchive::SectionKind::Asset,
                resource_bytes,
            ))
            .build()
            .expect("build archive");
        let archive = Archive::decode(&archive_bytes).expect("decode archive");
        let mut manager = ResourceManager::new(1024);
        let loaded = manager.ingest_archive(&archive).expect("ingest archive");
        assert_eq!(loaded, 1);
        let location = manager.catalog().location(42).expect("location");
        assert_eq!(location.section_id, 2);
    }
}
