use super::*;

impl Runtime {
    pub fn load_archive(&mut self, bytes: &[u8]) -> Result<LoadedArchive, RuntimeError> {
        let archive = Archive::decode(bytes)?;
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?;
        let resources_loaded = self.resources.borrow_mut().ingest_archive(&archive)?;
        if let Some(manifest) = &manifest {
            self.loaded_archives.borrow_mut().push(manifest.clone());
        }
        Ok(LoadedArchive {
            manifest,
            resources_loaded,
        })
    }

    pub fn load_archive_reader<R: std::io::Read + std::io::Seek>(
        &mut self,
        archive: &mut ArchiveStreamReader<R>,
    ) -> Result<LoadedArchive, RuntimeError> {
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?;
        if let Some(manifest) = &manifest {
            for entry in &manifest.resource_map {
                self.resources
                    .borrow_mut()
                    .catalog_mut()
                    .insert_name_hash(entry.name_hash, entry.resource_id);
            }
            self.loaded_archives.borrow_mut().push(manifest.clone());
        }

        let mut resources_loaded = 0usize;
        let sections = archive.sections().to_vec();
        for section in sections {
            if !matches!(section.kind, wmarchive::SectionKind::Asset) {
                continue;
            }
            let bytes = archive.read_section_entry(&section)?;
            let header = decode_resource_header(&bytes)?;
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
            self.resources.borrow_mut().register_ready(
                header.resource_id,
                data,
                header.flags as u32,
            )?;
            self.resources.borrow_mut().catalog_mut().insert(
                header.resource_id,
                wmresource::ResourceLocation {
                    section_id: section.id,
                    offset: header.data_offset as u64,
                    size: header.unpacked_size as u64,
                    resource_type: header.resource_type,
                    flags: header.flags,
                },
            )?;
            resources_loaded += 1;
        }

        Ok(LoadedArchive {
            manifest,
            resources_loaded,
        })
    }
}
