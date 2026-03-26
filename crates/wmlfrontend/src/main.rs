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
    let args = CliArgs::parse(env::args().skip(1))?;
    let script_path = args.script_path;
    let source = fs::read_to_string(&script_path)?;
    let package_name = args.package_name.unwrap_or_else(|| {
        script_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("wml-game")
            .to_owned()
    });
    let project = GameProject::new(
        package_name,
        script_path.to_string_lossy().to_string(),
        source,
    );
    let mut config = FrontendConfig::new(args.platform, project);
    config.step_limit = args.step_limit.unwrap_or(config.step_limit);
    config.auto_run = true;
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

#[derive(Debug)]
struct CliArgs {
    script_path: PathBuf,
    package_name: Option<String>,
    step_limit: Option<usize>,
    platform: PlatformProfile,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut script_path = None;
        let mut package_name = None;
        let mut step_limit = None;
        let mut platform = PlatformProfile::native();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--package" => {
                    let value = args.next().ok_or("--package requires a value")?;
                    package_name = Some(value);
                }
                "--step-limit" => {
                    let value = args.next().ok_or("--step-limit requires a value")?;
                    step_limit = Some(value.parse()?);
                }
                "--platform" => {
                    let value = args.next().ok_or("--platform requires a value")?;
                    platform = parse_platform(&value)?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}").into());
                }
                value => {
                    if script_path.is_some() {
                        return Err(format!("unexpected extra positional argument: {value}").into());
                    }
                    script_path = Some(PathBuf::from(value));
                }
            }
        }

        let script_path = script_path.ok_or("missing script path")?;
        Ok(Self {
            script_path,
            package_name,
            step_limit,
            platform,
        })
    }
}

fn parse_platform(value: &str) -> Result<PlatformProfile, Box<dyn std::error::Error>> {
    match value {
        "native" => Ok(PlatformProfile::native()),
        "wasm" => Ok(PlatformProfile::wasm()),
        "egui" => Ok(PlatformProfile::egui()),
        other => Err(format!("unknown platform: {other}").into()),
    }
}

fn print_usage() {
    eprintln!(
        "usage: wmlfrontend <script.wml> [--package NAME] [--step-limit N] [--platform native|wasm|egui]"
    );
}
