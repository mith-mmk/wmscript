#![forbid(unsafe_code)]

use crate::{Archive, ArchiveError, Manifest, Result, SectionEntry, SectionKind};

/// In-memory bundle extracted from an archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveBundle {
    manifest: Option<Manifest>,
    sections: Vec<BundleSection>,
}

impl ArchiveBundle {
    pub fn from_archive(archive: &Archive<'_>) -> Result<Self> {
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?;
        let mut sections = Vec::with_capacity(archive.sections().len());
        for section in archive.sections() {
            let bytes = archive
                .section_bytes(section.id)
                .ok_or(ArchiveError::BrokenLayout)?;
            sections.push(BundleSection::new(*section, bytes.to_vec()));
        }
        Ok(Self { manifest, sections })
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    pub fn sections(&self) -> &[BundleSection] {
        &self.sections
    }

    pub fn section(&self, section_id: u32) -> Option<&BundleSection> {
        self.sections
            .iter()
            .find(|section| section.entry.id == section_id)
    }

    pub fn asset_sections(&self) -> impl Iterator<Item = &BundleSection> {
        self.sections
            .iter()
            .filter(|section| matches!(section.entry.kind, SectionKind::Asset))
    }
}

/// One extracted section and its payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleSection {
    pub entry: SectionEntry,
    pub bytes: Vec<u8>,
}

impl BundleSection {
    pub fn new(entry: SectionEntry, bytes: Vec<u8>) -> Self {
        Self { entry, bytes }
    }
}
