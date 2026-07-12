use std::collections::BTreeSet;

use crate::{ArchiveError, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveFormat {
    V1,
    V2,
}

pub fn detect_format(bytes: &[u8]) -> Result<ArchiveFormat> {
    if bytes.get(..4) != Some(b"WARC") {
        return Err(ArchiveError::InvalidMagic);
    }
    let version = u16::from_le_bytes(
        bytes
            .get(4..6)
            .ok_or(ArchiveError::UnexpectedEof)?
            .try_into()
            .unwrap(),
    );
    match version {
        1 => Ok(ArchiveFormat::V1),
        2 => Ok(ArchiveFormat::V2),
        other => Err(ArchiveError::UnsupportedVersion(other)),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestV2 {
    pub package: String,
    pub package_version: String,
    pub entry_system: String,
    pub tick_hz: u32,
    pub seed: u64,
    pub save_compat_version: u32,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetKindV2 {
    Data,
    Image,
    Audio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetV2 {
    pub id: u32,
    pub name: String,
    pub kind: AssetKindV2,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveV2 {
    pub manifest: ManifestV2,
    pub program: Vec<u8>,
    pub schema: Vec<u8>,
    pub assets: Vec<AssetV2>,
}

impl ArchiveV2 {
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_assets(&self.assets)?;
        let mut out = Vec::new();
        out.extend_from_slice(b"WARC");
        write_u16(&mut out, 2);
        out.extend_from_slice(b"MNF2");
        write_string(&mut out, &self.manifest.package)?;
        write_string(&mut out, &self.manifest.package_version)?;
        write_string(&mut out, &self.manifest.entry_system)?;
        write_u32(&mut out, self.manifest.tick_hz);
        write_u64(&mut out, self.manifest.seed);
        write_u32(&mut out, self.manifest.save_compat_version);
        write_u16(&mut out, checked_u16(self.manifest.capabilities.len())?);
        for capability in &self.manifest.capabilities {
            write_string(&mut out, capability)?;
        }
        write_blob(&mut out, &self.program)?;
        write_blob(&mut out, &self.schema)?;
        write_u32(&mut out, checked_u32(self.assets.len())?);
        for asset in &self.assets {
            write_u32(&mut out, asset.id);
            out.push(match asset.kind {
                AssetKindV2::Data => 0,
                AssetKindV2::Image => 1,
                AssetKindV2::Audio => 2,
            });
            write_string(&mut out, &asset.name)?;
            write_blob(&mut out, &asset.bytes)?;
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if detect_format(bytes)? != ArchiveFormat::V2 {
            return Err(ArchiveError::UnsupportedVersion(1));
        }
        let mut cursor = Cursor { bytes, offset: 6 };
        if cursor.read(4)? != b"MNF2" {
            return Err(ArchiveError::InvalidManifest(
                "invalid v2 manifest magic".to_owned(),
            ));
        }
        let package = cursor.string()?;
        let package_version = cursor.string()?;
        let entry_system = cursor.string()?;
        let tick_hz = cursor.u32()?;
        let seed = cursor.u64()?;
        let save_compat_version = cursor.u32()?;
        let capability_count = cursor.u16()? as usize;
        let mut capabilities = Vec::with_capacity(capability_count);
        for _ in 0..capability_count {
            capabilities.push(cursor.string()?);
        }
        let program = cursor.blob()?;
        let schema = cursor.blob()?;
        let asset_count = cursor.u32()? as usize;
        let mut assets = Vec::with_capacity(asset_count);
        for _ in 0..asset_count {
            let id = cursor.u32()?;
            let kind = match cursor.byte()? {
                0 => AssetKindV2::Data,
                1 => AssetKindV2::Image,
                2 => AssetKindV2::Audio,
                other => {
                    return Err(ArchiveError::InvalidManifest(format!(
                        "unknown v2 asset kind: {other}"
                    )));
                }
            };
            assets.push(AssetV2 {
                id,
                kind,
                name: cursor.string()?,
                bytes: cursor.blob()?,
            });
        }
        if cursor.offset != bytes.len() {
            return Err(ArchiveError::BrokenLayout);
        }
        validate_assets(&assets)?;
        Ok(Self {
            manifest: ManifestV2 {
                package,
                package_version,
                entry_system,
                tick_hz,
                seed,
                save_compat_version,
                capabilities,
            },
            program,
            schema,
            assets,
        })
    }
}

fn validate_assets(assets: &[AssetV2]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for asset in assets {
        if !ids.insert(asset.id) {
            return Err(ArchiveError::InvalidManifest(format!(
                "duplicate v2 asset id: {}",
                asset.id
            )));
        }
        if !names.insert(&asset.name) {
            return Err(ArchiveError::InvalidManifest(format!(
                "duplicate v2 asset name: {}",
                asset.name
            )));
        }
    }
    Ok(())
}

fn checked_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| ArchiveError::BrokenLayout)
}
fn checked_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| ArchiveError::BrokenLayout)
}
fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn write_string(out: &mut Vec<u8>, value: &str) -> Result<()> {
    write_u32(out, checked_u32(value.len())?);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn write_blob(out: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    write_u32(out, checked_u32(value.len())?);
    out.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Cursor<'_> {
    fn read(&mut self, len: usize) -> Result<&[u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ArchiveError::UnexpectedEof)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ArchiveError::UnexpectedEof)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.read(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.read(len)?.to_vec()).map_err(|_| {
            ArchiveError::InvalidManifest("v2 manifest contains invalid UTF-8".to_owned())
        })
    }
    fn blob(&mut self) -> Result<Vec<u8>> {
        let len = self.u32()? as usize;
        Ok(self.read(len)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> ArchiveV2 {
        ArchiveV2 {
            manifest: ManifestV2 {
                package: "game".to_owned(),
                package_version: "1.0.0".to_owned(),
                entry_system: "start".to_owned(),
                tick_hz: 60,
                seed: 7,
                save_compat_version: 1,
                capabilities: vec!["gui".to_owned()],
            },
            program: b"program".to_vec(),
            schema: b"schema".to_vec(),
            assets: vec![AssetV2 {
                id: 1,
                name: "image/title".to_owned(),
                kind: AssetKindV2::Image,
                bytes: b"png".to_vec(),
            }],
        }
    }

    #[test]
    fn v2_round_trips() {
        let expected = archive();
        let bytes = expected.encode().unwrap();
        assert_eq!(detect_format(&bytes).unwrap(), ArchiveFormat::V2);
        assert_eq!(ArchiveV2::decode(&bytes).unwrap(), expected);
    }
    #[test]
    fn v2_rejects_duplicate_asset_ids() {
        let mut value = archive();
        value.assets.push(AssetV2 {
            id: 1,
            name: "other".to_owned(),
            kind: AssetKindV2::Data,
            bytes: vec![],
        });
        assert!(value.encode().is_err());
    }
}
