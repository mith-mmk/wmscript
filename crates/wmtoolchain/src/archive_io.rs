use std::io::{Read, Seek};

use wmarchive::{Archive, ArchiveError, ArchiveStreamReader, Manifest};
use wmruntime::Runtime;
use wmvm::{Program as VmProgram, WorkerId};

use crate::util::module_section_id;
use crate::{GameWorkerRole, Result, WorkerProgram};

pub(crate) fn decode_program_from_manifest_module(
    archive: &Archive<'_>,
    manifest: &Manifest,
) -> Result<VmProgram> {
    let section_id = module_section_id(archive.sections(), manifest);
    let bytes = archive.section_bytes(section_id).ok_or_else(|| {
        ArchiveError::InvalidManifest(format!("missing module section {section_id}"))
    })?;
    Ok(VmProgram::decode_binary(bytes)?)
}

pub(crate) fn decode_program_from_stream_manifest_module<R: Read + Seek>(
    archive: &mut ArchiveStreamReader<R>,
    manifest: &Manifest,
) -> Result<VmProgram> {
    let section_id = module_section_id(archive.sections(), manifest);
    let bytes = archive.read_section(section_id)?;
    Ok(VmProgram::decode_binary(&bytes)?)
}

pub(crate) fn decode_worker_programs_from_archive(
    archive: &Archive<'_>,
    manifest: &Manifest,
    fallback: VmProgram,
) -> Result<Vec<WorkerProgram>> {
    if manifest.worker_entries.is_empty() {
        return Ok(vec![WorkerProgram {
            role: GameWorkerRole::Engine,
            section_id: module_section_id(archive.sections(), manifest),
            program: fallback,
        }]);
    }
    let mut programs = Vec::new();
    for entry in &manifest.worker_entries {
        let role = GameWorkerRole::parse(&entry.role).unwrap_or(GameWorkerRole::Engine);
        let bytes = archive
            .section_bytes(entry.module_section_id)
            .ok_or_else(|| {
                ArchiveError::InvalidManifest(format!(
                    "missing worker module section {}",
                    entry.module_section_id
                ))
            })?;
        programs.push(WorkerProgram {
            role,
            section_id: entry.module_section_id,
            program: VmProgram::decode_binary(bytes)?,
        });
    }
    Ok(programs)
}

pub(crate) fn decode_worker_programs_from_stream<R: Read + Seek>(
    archive: &mut ArchiveStreamReader<R>,
    manifest: &Manifest,
    fallback: VmProgram,
) -> Result<Vec<WorkerProgram>> {
    if manifest.worker_entries.is_empty() {
        return Ok(vec![WorkerProgram {
            role: GameWorkerRole::Engine,
            section_id: module_section_id(archive.sections(), manifest),
            program: fallback,
        }]);
    }
    let mut programs = Vec::new();
    for entry in &manifest.worker_entries {
        let role = GameWorkerRole::parse(&entry.role).unwrap_or(GameWorkerRole::Engine);
        let bytes = archive.read_section(entry.module_section_id)?;
        programs.push(WorkerProgram {
            role,
            section_id: entry.module_section_id,
            program: VmProgram::decode_binary(&bytes)?,
        });
    }
    Ok(programs)
}

pub fn spawn_worker_programs(
    runtime: &mut Runtime,
    programs: &[WorkerProgram],
) -> Result<WorkerId> {
    let mut ordered = programs.to_vec();
    ordered.sort_by_key(|worker| worker.role.spawn_rank());
    let mut engine_worker_id = None;
    let mut first_worker_id = None;
    for worker in ordered {
        let worker_id = runtime.spawn_program(worker.program)?;
        if first_worker_id.is_none() {
            first_worker_id = Some(worker_id);
        }
        if worker.role == GameWorkerRole::Engine {
            engine_worker_id = Some(worker_id);
        }
    }
    engine_worker_id.or(first_worker_id).ok_or_else(|| {
        ArchiveError::InvalidManifest("project has no worker programs".to_owned()).into()
    })
}
