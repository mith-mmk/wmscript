use wmarchive::{
    ArchiveBuilder, ArchiveSection, ManifestBuilder, ManifestResourceEntry, SectionDigest,
    SectionKind, digest_section,
};
use wmplatform::PlatformProfile;
use wmresource::{LoadResult, ResourceType};
use wmruntime::{Runtime, RuntimeConfig};

fn main() {
    let mut runtime =
        Runtime::new(RuntimeConfig::new(PlatformProfile::native()).with_step_limit(8));

    let asset_payload = build_asset_payload(100, ResourceType::ScriptData, b"asset:hero");
    let manifest = ManifestBuilder::new("asset-sample", 42, 1)
        .push_resource_mapping(ManifestResourceEntry::new(0xDEAD_BEEF, 100))
        .push_section_digest(SectionDigest {
            section_id: 2,
            section_kind: SectionKind::Asset,
            flags_canonical: 0,
            unpacked_size: asset_payload.len() as u64,
            digest: digest_section(
                2,
                SectionKind::Asset,
                0,
                asset_payload.len() as u64,
                &asset_payload,
            ),
        })
        .build();

    let archive_bytes = ArchiveBuilder::new()
        .push_manifest(1, &manifest)
        .push_section(ArchiveSection::new(2, SectionKind::Asset, asset_payload))
        .build()
        .expect("build archive");

    let loaded = runtime.load_archive(&archive_bytes).expect("load archive");
    match runtime
        .resource_manager_mut()
        .load_resource(100)
        .expect("load resource")
    {
        LoadResult::Ready(handle) => {
            let resources = runtime.resource_manager();
            let entry = resources.entry(100).expect("resource entry");
            let bytes = entry.data.as_ref().map(|data| data.bytes()).unwrap_or(&[]);
            println!(
                "asset load => handle {}, bytes {:?}, archive resources {}",
                handle.raw(),
                bytes,
                loaded.resources_loaded
            );
        }
        LoadResult::Pending(request_id) => {
            println!("asset load pending: {request_id}");
        }
    }
}

fn build_asset_payload(resource_id: u32, resource_type: ResourceType, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + data.len());
    bytes.extend_from_slice(&resource_id.to_le_bytes());
    bytes.extend_from_slice(&resource_type.as_u16().to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(24u32).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes
}
