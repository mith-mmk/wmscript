#![forbid(unsafe_code)]

mod build;
mod project;
mod runner;

pub use build::{
    PackageMetadata, ProjectBuild, build_project, decode_metadata, decode_schema, package_project,
    write_file,
};
pub use project::{Asset, Project, ProjectError, Target};
pub use runner::{RunReport, run, run_compiled, run_program, run_program_with_systems};

use std::fs;
use std::path::Path;
use wmarchive::{ArchiveFormat, ArchiveV2, detect_format};
use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmtoolchain::{Toolchain, ToolchainConfig};
use wmvm::Program;

pub fn run_archive(path: &Path, target: Target, inputs: Vec<String>) -> Result<RunReport, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    match detect_format(&bytes).map_err(|error| error.to_string())? {
        ArchiveFormat::V2 => {
            let archive = ArchiveV2::decode(&bytes).map_err(|error| error.to_string())?;
            let program =
                Program::decode_binary(&archive.program).map_err(|error| error.to_string())?;
            let metadata = decode_metadata(&archive.schema)?;
            run_program_with_systems(
                program,
                target,
                inputs,
                &metadata.schema,
                archive.manifest.seed,
                &metadata.systems,
            )
        }
        ArchiveFormat::V1 => Err("WARC v1 requires `wms legacy run`".to_owned()),
    }
}

pub fn run_legacy_archive(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if detect_format(&bytes).map_err(|error| error.to_string())? != ArchiveFormat::V1 {
        return Err("legacy run accepts WARC v1 only".to_owned());
    }
    let profile = PlatformProfile::native();
    let toolchain = Toolchain::new(ToolchainConfig::new(profile));
    let mut runtime = Runtime::new(RuntimeConfig::new(profile));
    toolchain
        .bootstrap_runtime(&mut runtime)
        .map_err(|error| error.to_string())?;
    toolchain
        .run_archive(&mut runtime, &bytes)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmtoolchain::GameProject;

    #[test]
    fn generated_v1_archive_runs_only_through_legacy_adapter() {
        let root = std::path::PathBuf::from(format!(".test-wms-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let profile = PlatformProfile::native();
        let toolchain = Toolchain::new(ToolchainConfig::new(profile));
        let project = GameProject::new(
            "legacy",
            "legacy/main.wms",
            r#"export func main() { return "legacy-ok"; }"#,
        );
        let build = toolchain.build_project(&project).unwrap();
        let path = root.join("legacy-v1.warc");
        fs::write(&path, &build.archive).unwrap();
        assert_eq!(detect_format(&build.archive).unwrap(), ArchiveFormat::V1);
        assert!(run_archive(&path, Target::Headless, Vec::new()).is_err());
        run_legacy_archive(&path).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
