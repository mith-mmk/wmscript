use std::env;
use std::fs;
use std::path::PathBuf;

use wmlfrontend::{FrontendConfig, run_frontend};
use wmlplatform::PlatformProfile;
use wmltoolchain::GameProject;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let script_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("samples/easynovel/main.wml"));
    let source = fs::read_to_string(&script_path)?;
    let package_name = script_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("wml-game");
    let project = GameProject::new(
        package_name.to_owned(),
        script_path.to_string_lossy().to_string(),
        source,
    );
    let config = FrontendConfig::new(PlatformProfile::native(), project);
    let report = run_frontend(config)?;

    println!("=== frontend summary ===");
    println!("package: {}", report.build.manifest.package_name);
    println!("archive bytes: {}", report.build.archive.len());
    println!("worker id: {}", report.execution.worker_id);
    if let Some((_, outcome)) = report.execution.outcomes.last() {
        println!("final outcome: {outcome:?}");
    }
    Ok(())
}
