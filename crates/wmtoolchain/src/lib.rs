#![forbid(unsafe_code)]

//! Toolchain support for compiling, packaging, and running WML game scripts.

use core::fmt;
use std::io::{Read, Seek};

use wmarchive::{
    Archive, ArchiveBuilder, ArchiveError, ArchiveSection, ArchiveStreamReader, Manifest,
    ManifestBuilder, ManifestResourceEntry, SectionDigest, SectionKind, Version, digest_section,
};
use wmcompiler::{CompileError, Compiler, CompilerConfig, ModuleCatalog};
use wmext::standard_extension_registry;
use wmplatform::PlatformProfile;
use wmresource::ResourceType;
use wmruntime::{LoadedArchive, Runtime, RuntimeError, StandardExtensions};
use wmvm::{Program as VmProgram, ProgramCodecError, RunOutcome, WorkerId};

/// Toolchain configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolchainConfig {
    pub platform: PlatformProfile,
    pub step_limit: usize,
    pub release: bool,
}

impl ToolchainConfig {
    pub const fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            step_limit: 128,
            release: false,
        }
    }

    pub const fn with_step_limit(mut self, step_limit: usize) -> Self {
        self.step_limit = step_limit;
        self
    }

    pub const fn with_release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }
}

/// Result type used by the toolchain.
pub type Result<T> = core::result::Result<T, ToolchainError>;

/// Toolchain error.
#[derive(Debug)]
pub enum ToolchainError {
    Compile(CompileError),
    Archive(ArchiveError),
    ProgramCodec(ProgramCodecError),
    Runtime(RuntimeError),
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(f, "{error}"),
            Self::Archive(error) => write!(f, "{error}"),
            Self::ProgramCodec(error) => write!(f, "{error}"),
            Self::Runtime(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ToolchainError {}

impl From<CompileError> for ToolchainError {
    fn from(value: CompileError) -> Self {
        Self::Compile(value)
    }
}

impl From<ArchiveError> for ToolchainError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl From<ProgramCodecError> for ToolchainError {
    fn from(value: ProgramCodecError) -> Self {
        Self::ProgramCodec(value)
    }
}

impl From<RuntimeError> for ToolchainError {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

/// Asset bundled into a game project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameAsset {
    pub name: String,
    pub section_id: u32,
    pub resource_id: u32,
    pub resource_type: ResourceType,
    pub payload: Vec<u8>,
    pub flags: u16,
    pub align: u32,
}

impl GameAsset {
    pub fn new(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        resource_type: ResourceType,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            name: name.into(),
            section_id,
            resource_id,
            resource_type,
            payload: payload.into(),
            flags: 0,
            align: 16,
        }
    }

    pub fn script_data(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(
            name,
            section_id,
            resource_id,
            ResourceType::ScriptData,
            payload,
        )
    }

    pub fn image(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(name, section_id, resource_id, ResourceType::Image, payload)
    }

    pub fn audio(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(name, section_id, resource_id, ResourceType::Audio, payload)
    }

    pub fn name_hash(&self) -> u64 {
        stable_hash64(self.name.as_bytes())
    }
}

/// Source-driven game project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameProject {
    pub package_name: String,
    pub script_path: String,
    pub source: String,
    pub assets: Vec<GameAsset>,
}

impl GameProject {
    pub fn new(
        package_name: impl Into<String>,
        script_path: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            script_path: script_path.into(),
            source: source.into(),
            assets: Vec::new(),
        }
    }

    pub fn push_asset(mut self, asset: GameAsset) -> Self {
        self.assets.push(asset);
        self
    }

    pub fn with_asset(mut self, asset: GameAsset) -> Self {
        self.assets.push(asset);
        self
    }
}

/// Artifact produced by the build step.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildArtifact {
    pub program: VmProgram,
    pub manifest: Manifest,
    pub archive: Vec<u8>,
    pub archive_size: usize,
}

/// Result produced by a full build and run cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionReport {
    pub build: BuildArtifact,
    pub loaded_archive: LoadedArchive,
    pub worker_id: WorkerId,
    pub outcomes: Vec<(WorkerId, RunOutcome)>,
}

/// Toolchain that compiles and packages a single game project.
#[derive(Clone, Debug)]
pub struct Toolchain {
    config: ToolchainConfig,
    compiler: Compiler,
}

impl Toolchain {
    pub fn new(config: ToolchainConfig) -> Self {
        let extension_registry = standard_extension_registry().expect("standard extensions");
        Self {
            compiler: Compiler::new(
                CompilerConfig::new(config.platform).with_extension_registry(extension_registry),
            ),
            config,
        }
    }

    pub const fn config(&self) -> ToolchainConfig {
        self.config
    }

    pub fn build_project(&self, project: &GameProject) -> Result<BuildArtifact> {
        let mut catalog = ModuleCatalog::new();
        let program = self.compiler.compile_program(
            project.script_path.clone(),
            project.source.clone(),
            &mut catalog,
        )?;
        let manifest = self.build_manifest(project, &program)?;
        let archive = self.build_archive(project, &program, &manifest)?;
        let archive_size = archive.len();
        Ok(BuildArtifact {
            program,
            manifest,
            archive,
            archive_size,
        })
    }

    pub fn load_archive(&self, bytes: &[u8]) -> Result<BuildArtifact> {
        let archive = Archive::decode(bytes)?;
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?.ok_or_else(|| {
            ArchiveError::InvalidManifest("archive is missing manifest".to_owned())
        })?;
        let program = decode_program_from_manifest_module(&archive, &manifest)?;
        Ok(BuildArtifact {
            program,
            manifest,
            archive: bytes.to_vec(),
            archive_size: bytes.len(),
        })
    }

    pub fn load_archive_reader<R: Read + Seek>(&self, reader: R) -> Result<BuildArtifact> {
        let mut archive = ArchiveStreamReader::open(reader)?;
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?.ok_or_else(|| {
            ArchiveError::InvalidManifest("archive is missing manifest".to_owned())
        })?;
        let program = decode_program_from_stream_manifest_module(&mut archive, &manifest)?;
        Ok(BuildArtifact {
            program,
            manifest,
            archive: Vec::new(),
            archive_size: archive.data_len() as usize,
        })
    }

    pub fn run_project(
        &self,
        runtime: &mut Runtime,
        project: &GameProject,
    ) -> Result<ExecutionReport> {
        let build = self.build_project(project)?;
        let loaded_archive = runtime.load_archive(&build.archive)?;
        let worker_id = runtime.spawn_program(build.program.clone())?;
        let outcomes = runtime.run_until_idle(self.config.step_limit);
        Ok(ExecutionReport {
            build,
            loaded_archive,
            worker_id,
            outcomes,
        })
    }

    pub fn run_archive(&self, runtime: &mut Runtime, bytes: &[u8]) -> Result<ExecutionReport> {
        let build = self.load_archive(bytes)?;
        let loaded_archive = runtime.load_archive(bytes)?;
        let worker_id = runtime.spawn_program(build.program.clone())?;
        let outcomes = runtime.run_until_idle(self.config.step_limit);
        Ok(ExecutionReport {
            build,
            loaded_archive,
            worker_id,
            outcomes,
        })
    }

    pub fn run_archive_reader<R: Read + Seek>(
        &self,
        runtime: &mut Runtime,
        reader: R,
    ) -> Result<ExecutionReport> {
        let mut archive = ArchiveStreamReader::open(reader)?;
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?.ok_or_else(|| {
            ArchiveError::InvalidManifest("archive is missing manifest".to_owned())
        })?;
        let program = decode_program_from_stream_manifest_module(&mut archive, &manifest)?;
        let archive_size = archive.data_len() as usize;
        let loaded_archive = runtime.load_archive_reader(&mut archive)?;
        let worker_id = runtime.spawn_program(program.clone())?;
        let outcomes = runtime.run_until_idle(self.config.step_limit);
        Ok(ExecutionReport {
            build: BuildArtifact {
                program,
                manifest,
                archive: Vec::new(),
                archive_size,
            },
            loaded_archive,
            worker_id,
            outcomes,
        })
    }

    pub fn bootstrap_runtime(
        &self,
        runtime: &mut Runtime,
    ) -> core::result::Result<StandardExtensions, RuntimeError> {
        Ok(runtime.install_standard_extensions()?)
    }

    fn build_manifest(&self, project: &GameProject, program: &VmProgram) -> Result<Manifest> {
        let archive_id = stable_hash128(&[
            project.package_name.as_bytes(),
            project.script_path.as_bytes(),
            project.source.as_bytes(),
        ]);
        let build_id = stable_hash128(&[
            project.package_name.as_bytes(),
            project.script_path.as_bytes(),
            project.source.as_bytes(),
            &project.assets.len().to_le_bytes(),
        ]);

        let mut builder = ManifestBuilder::new(project.package_name.clone(), archive_id, build_id)
            .package_version(Version::new(0, 1, 0, 0))
            .bytecode_version(1)
            .runtime_min_version(1)
            .target_platform_mask(platform_mask(self.config.platform))
            .capability_mask(u64::MAX)
            .policy_flags(if self.config.release { 1 } else { 0 })
            .entry(2, program.entry().unwrap_or(1) as u32);

        let module_bytes = program.encode_binary();
        let module_digest = digest_section(
            2,
            SectionKind::Module,
            0,
            module_bytes.len() as u64,
            &module_bytes,
        );
        builder = builder.push_section_digest(SectionDigest {
            section_id: 2,
            section_kind: SectionKind::Module,
            flags_canonical: 0,
            unpacked_size: module_bytes.len() as u64,
            digest: module_digest,
        });

        for asset in &project.assets {
            let payload = encode_resource_payload(asset);
            builder = builder.push_resource_mapping(ManifestResourceEntry::new(
                asset.name_hash(),
                asset.resource_id,
            ));
            builder = builder.push_section_digest(SectionDigest {
                section_id: asset.section_id,
                section_kind: SectionKind::Asset,
                flags_canonical: asset.flags,
                unpacked_size: payload.len() as u64,
                digest: digest_section(
                    asset.section_id,
                    SectionKind::Asset,
                    asset.flags,
                    payload.len() as u64,
                    &payload,
                ),
            });
        }

        Ok(builder.build())
    }

    fn build_archive(
        &self,
        project: &GameProject,
        program: &VmProgram,
        manifest: &Manifest,
    ) -> Result<Vec<u8>> {
        let mut builder = ArchiveBuilder::new().push_manifest(1, manifest);
        let module_bytes = program.encode_binary();

        let mut module = ArchiveSection::new(2, SectionKind::Module, module_bytes);
        module.align = 16;
        module.name_hash = stable_hash64(project.script_path.as_bytes());
        builder = builder.push_section(module);

        for asset in &project.assets {
            let mut section = ArchiveSection::new(
                asset.section_id,
                SectionKind::Asset,
                encode_resource_payload(asset),
            );
            section.flags = asset.flags;
            section.align = asset.align;
            section.name_hash = asset.name_hash();
            builder = builder.push_section(section);
        }

        Ok(builder.build()?)
    }
}

fn decode_program_from_manifest_module(
    archive: &Archive<'_>,
    manifest: &Manifest,
) -> Result<VmProgram> {
    let section_id = module_section_id(archive.sections(), manifest);
    let bytes = archive.section_bytes(section_id).ok_or_else(|| {
        ArchiveError::InvalidManifest(format!("missing module section {section_id}"))
    })?;
    Ok(VmProgram::decode_binary(bytes)?)
}

fn decode_program_from_stream_manifest_module<R: Read + Seek>(
    archive: &mut ArchiveStreamReader<R>,
    manifest: &Manifest,
) -> Result<VmProgram> {
    let section_id = module_section_id(archive.sections(), manifest);
    let bytes = archive.read_section(section_id)?;
    Ok(VmProgram::decode_binary(&bytes)?)
}

fn module_section_id(sections: &[wmarchive::SectionEntry], manifest: &Manifest) -> u32 {
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

fn platform_mask(platform: PlatformProfile) -> u64 {
    match platform.kind {
        wmplatform::PlatformKind::Native => 1 << 0,
        wmplatform::PlatformKind::Wasm => 1 << 1,
        wmplatform::PlatformKind::Egui => 1 << 2,
    }
}

fn stable_hash64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }
    hash
}

fn stable_hash128(parts: &[&[u8]]) -> u128 {
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

fn encode_resource_payload(asset: &GameAsset) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use wmplatform::PlatformProfile;
    use wmruntime::{Runtime, RuntimeConfig};
    use wmvm::Value;

    #[test]
    fn build_project_creates_archive_and_program() {
        let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
        let project = GameProject::new("sample", "main.wms", r#"export func main() { return 7; }"#)
            .push_asset(GameAsset::new(
                "ui/title",
                10,
                42,
                ResourceType::ScriptData,
                b"title".to_vec(),
            ));

        let build = toolchain.build_project(&project).expect("build");
        assert!(build.archive_size > 64);
        assert_eq!(build.program.entry(), Some(1));
        assert_eq!(build.manifest.resource_map.len(), 1);
    }

    #[test]
    fn run_project_executes_program_and_loads_assets() {
        let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
        let project = GameProject::new(
            "sample",
            "main.wms",
            r#"export func main() { return "ok"; }"#,
        )
        .push_asset(GameAsset::new(
            "script/data",
            11,
            77,
            ResourceType::ScriptData,
            b"payload".to_vec(),
        ));
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));

        let report = toolchain.run_project(&mut runtime, &project).expect("run");
        assert_eq!(report.worker_id, 1);
        assert!(matches!(
            report.outcomes.last(),
            Some((_, RunOutcome::Halted { value: Some(_), .. }))
        ));
        assert!(report.loaded_archive.resources_loaded >= 1);
    }

    #[test]
    fn run_archive_reader_executes_compiled_module_from_archive() {
        let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
        let project = GameProject::new(
            "sample",
            "main.wms",
            r#"export func main() { return "archive-ok"; }"#,
        );
        let build = toolchain.build_project(&project).expect("build");
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));

        let report = toolchain
            .run_archive_reader(&mut runtime, std::io::Cursor::new(build.archive.clone()))
            .expect("run archive reader");

        assert_eq!(report.worker_id, 1);
        assert_eq!(report.build.manifest.entry_module_id, 2);
        assert_eq!(report.build.archive_size, build.archive_size);
        assert!(report.build.archive.is_empty());
        assert!(matches!(
            report.outcomes.last(),
            Some((
                _,
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                }
            )) if text == "archive-ok"
        ));
    }
}
