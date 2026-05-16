use std::path::{Path, PathBuf};

use wmarchive::{Manifest, SectionKind};
use wmplatform::PlatformProfile;

use crate::GameAsset;

pub(crate) fn module_section_id(sections: &[wmarchive::SectionEntry], manifest: &Manifest) -> u32 {
    if manifest.entry_module_id != 0
        && sections
            .iter()
            .any(|section| section.id == manifest.entry_module_id)
    {
        return manifest.entry_module_id;
    }
    sections
        .iter()
        .find(|section| matches!(section.kind, SectionKind::Module))
        .map(|section| section.id)
        .unwrap_or(2)
}

pub(crate) fn platform_mask(platform: PlatformProfile) -> u64 {
    match platform.kind {
        wmplatform::PlatformKind::Native => 1 << 0,
        wmplatform::PlatformKind::Wasm => 1 << 1,
        wmplatform::PlatformKind::Egui => 1 << 2,
    }
}

pub(crate) fn stable_hash64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }
    hash
}

pub(crate) fn resolve_import_path(module_path: &str, import_path: &str) -> String {
    let import = Path::new(import_path);
    if import.is_absolute() {
        return import.to_string_lossy().to_string();
    }

    let base_dir = Path::new(module_path)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base_dir.join(import).to_string_lossy().to_string()
}

pub(crate) fn stable_hash128(parts: &[&[u8]]) -> u128 {
    let mut left = 0xA5A5_A5A5_A5A5_A5A5_u64;
    let mut right = 0x5A5A_5A5A_5A5A_5A5A_u64;
    for part in parts {
        for &byte in *part {
            left ^= byte as u64;
            left = left.rotate_left(7).wrapping_mul(0x9E37_79B1_85EB_CA87);
            right ^= (byte as u64).rotate_left(1);
            right = right.rotate_right(3).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        }
    }
    ((left as u128) << 64) | right as u128
}

pub(crate) fn encode_resource_payload(asset: &GameAsset) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + asset.payload.len());
    bytes.extend_from_slice(&asset.resource_id.to_le_bytes());
    bytes.extend_from_slice(&asset.resource_type.as_u16().to_le_bytes());
    bytes.extend_from_slice(&asset.flags.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(asset.payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(asset.payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(24u32).to_le_bytes());
    bytes.extend_from_slice(&asset.payload);
    bytes
}
