use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Seek};

use wmarchive::{
    Archive, ArchiveBuilder, ArchiveError, ArchiveSection, ArchiveStreamReader, Manifest,
    ManifestBuilder, ManifestResourceEntry, ManifestSectionLocation, ManifestWorkerEntry,
    SectionDigest, SectionKind, Version, digest_section,
};
use wmcompiler::{CompileError, Compiler, CompilerConfig, ModuleCatalog, ModuleItem};
use wmext::standard_extension_registry;
use wmruntime::{Runtime, RuntimeError, StandardExtensions};

use crate::archive_io::{
    decode_program_from_manifest_module, decode_program_from_stream_manifest_module,
    decode_worker_programs_from_archive, decode_worker_programs_from_stream, spawn_worker_programs,
};
use crate::util::{
    encode_resource_payload, platform_mask, resolve_import_path, stable_hash64, stable_hash128,
};
use crate::{
    BuildArtifact, ExecutionReport, GameProject, GameWorkerRole, Result, ToolchainConfig,
    ToolchainError, WorkerProgram,
};

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
        let worker_programs = self.compile_worker_programs(project)?;
        let program = worker_programs
            .iter()
            .find(|worker| worker.role == GameWorkerRole::Engine)
            .or_else(|| worker_programs.first())
            .expect("project must have at least one worker")
            .program
            .clone();
        let manifest = self.build_manifest(project, &worker_programs)?;
        let archive = self.build_archive(project, &worker_programs, &manifest)?;
        let archive_size = archive.len();
        Ok(BuildArtifact {
            program,
            worker_programs,
            manifest,
            archive,
            archive_size,
        })
    }

    fn compile_worker_programs(&self, project: &GameProject) -> Result<Vec<WorkerProgram>> {
        let mut scripts = project.scripts();
        scripts.sort_by_key(|script| script.role.module_section_id());
        let mut programs = Vec::new();
        for script in scripts {
            let mut catalog = ModuleCatalog::new();
            self.seed_catalog_with_imports(&mut catalog, &script.script_path, &script.source)?;
            let program = self.compiler.compile_program(
                script.script_path.clone(),
                script.source.clone(),
                &mut catalog,
            )?;
            programs.push(WorkerProgram {
                role: script.role,
                section_id: script.role.module_section_id(),
                program,
            });
        }
        Ok(programs)
    }

    fn seed_catalog_with_imports(
        &self,
        catalog: &mut ModuleCatalog,
        root_path: &str,
        root_source: &str,
    ) -> Result<()> {
        let mut pending = vec![(root_path.to_owned(), root_source.to_owned())];
        let mut visited = BTreeSet::new();

        while let Some((module_path, source)) = pending.pop() {
            if !visited.insert(module_path.clone()) {
                continue;
            }

            catalog.register(&module_path);
            let ast = self
                .compiler
                .parse_module(module_path.clone(), source)
                .map_err(ToolchainError::from)?;

            for item in ast.items {
                if let ModuleItem::Import(import_decl) = item {
                    catalog.register(&import_decl.path);
                    let import_path = resolve_import_path(&module_path, &import_decl.path);
                    catalog.register(&import_path);
                    if visited.contains(&import_path) {
                        continue;
                    }
                    let import_source = fs::read_to_string(&import_path).map_err(|_| {
                        ToolchainError::Compile(CompileError::UnknownModule {
                            path: import_path.clone(),
                        })
                    })?;
                    pending.push((import_path, import_source));
                }
            }
        }

        Ok(())
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
            program: program.clone(),
            worker_programs: decode_worker_programs_from_archive(&archive, &manifest, program)?,
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
            program: program.clone(),
            worker_programs: decode_worker_programs_from_stream(&mut archive, &manifest, program)?,
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
        let worker_id = spawn_worker_programs(runtime, &build.worker_programs)?;
        runtime.save_checkpoint(0);
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
        let worker_id = spawn_worker_programs(runtime, &build.worker_programs)?;
        runtime.save_checkpoint(0);
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
        let worker_programs =
            decode_worker_programs_from_stream(&mut archive, &manifest, program.clone())?;
        let worker_id = spawn_worker_programs(runtime, &worker_programs)?;
        runtime.save_checkpoint(0);
        let outcomes = runtime.run_until_idle(self.config.step_limit);
        Ok(ExecutionReport {
            build: BuildArtifact {
                program,
                worker_programs,
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

    fn build_manifest(
        &self,
        project: &GameProject,
        worker_programs: &[WorkerProgram],
    ) -> Result<Manifest> {
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
            .entry(
                worker_programs
                    .iter()
                    .find(|worker| worker.role == GameWorkerRole::Engine)
                    .or_else(|| worker_programs.first())
                    .map(|worker| worker.section_id)
                    .unwrap_or(2),
                worker_programs
                    .iter()
                    .find(|worker| worker.role == GameWorkerRole::Engine)
                    .or_else(|| worker_programs.first())
                    .and_then(|worker| worker.program.entry())
                    .unwrap_or(1) as u32,
            );

        for worker in worker_programs {
            let module_bytes = worker.program.encode_binary();
            builder = builder
                .push_section_digest(SectionDigest {
                    section_id: worker.section_id,
                    section_kind: SectionKind::Module,
                    flags_canonical: 0,
                    unpacked_size: module_bytes.len() as u64,
                    digest: digest_section(
                        worker.section_id,
                        SectionKind::Module,
                        0,
                        module_bytes.len() as u64,
                        &module_bytes,
                    ),
                })
                .push_worker_entry(ManifestWorkerEntry::new(
                    worker.role.as_str(),
                    worker.section_id,
                    worker.program.entry().unwrap_or(1) as u32,
                    u64::MAX,
                ));
        }

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
            if let Some(location) = &asset.external_location {
                builder = builder.push_external_section_location(ManifestSectionLocation::new(
                    asset.section_id,
                    location.url.clone(),
                    location.cache_key.clone(),
                    location.flags,
                ));
            }
        }

        Ok(builder.build())
    }

    fn build_archive(
        &self,
        project: &GameProject,
        worker_programs: &[WorkerProgram],
        manifest: &Manifest,
    ) -> Result<Vec<u8>> {
        let mut builder = ArchiveBuilder::new().push_manifest(1, manifest);
        for worker in worker_programs {
            let mut module = ArchiveSection::new(
                worker.section_id,
                SectionKind::Module,
                worker.program.encode_binary(),
            );
            module.align = 16;
            let script_path = project
                .scripts()
                .into_iter()
                .find(|script| script.role == worker.role)
                .map(|script| script.script_path)
                .unwrap_or_else(|| project.script_path.clone());
            module.name_hash = stable_hash64(script_path.as_bytes());
            builder = builder.push_section(module);
        }

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
