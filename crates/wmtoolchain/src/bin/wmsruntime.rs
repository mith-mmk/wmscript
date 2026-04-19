use std::env;
use std::fs;
use std::path::PathBuf;

use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig, StandardExtensions};
use wmtoolchain::{Toolchain, ToolchainConfig};
use wmvm::RunOutcome;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse(env::args().skip(1))?;
    let bytes = fs::read(&args.packed_path)?;

    let toolchain = Toolchain::new(
        ToolchainConfig::new(args.platform)
            .with_step_limit(args.step_limit)
            .with_release(false),
    );
    let mut runtime = Runtime::new(RuntimeConfig::new(args.platform));
    let _extensions: StandardExtensions = runtime.install_standard_extensions()?;
    let report = toolchain.run_archive(&mut runtime, &bytes)?;

    println!("=== wmsruntime summary ===");
    println!("archive: {}", args.packed_path.display());
    println!("package: {}", report.build.manifest.package_name);
    println!("archive bytes: {}", report.build.archive_size);
    println!("worker: {}", report.worker_id);
    println!("outcomes: {}", report.outcomes.len());

    if let Some((_, outcome)) = report.outcomes.last() {
        match outcome {
            RunOutcome::Halted { steps, value } => {
                println!("last: halted (steps={steps}) value={value:?}");
            }
            RunOutcome::Yielded { steps } => {
                println!("last: yielded (steps={steps})");
            }
            RunOutcome::StepLimitReached { steps } => {
                println!("last: step-limit-reached (steps={steps})");
            }
            RunOutcome::Sleeping { steps } => {
                println!("last: sleeping (steps={steps})");
            }
            RunOutcome::WaitingMessage { steps } => {
                println!("last: waiting-message (steps={steps})");
            }
            RunOutcome::Error { steps, error } => {
                println!("last: error (steps={steps}) {error}");
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    packed_path: PathBuf,
    platform: PlatformProfile,
    step_limit: usize,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut packed_path = None;
        let mut platform = PlatformProfile::native();
        let mut step_limit = 128usize;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--platform" => platform = parse_platform(&next_value(&mut args, "--platform")?)?,
                "--step-limit" => {
                    step_limit = next_value(&mut args, "--step-limit")?.parse()?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}").into());
                }
                value => {
                    if packed_path.is_some() {
                        return Err(format!("unexpected extra positional argument: {value}").into());
                    }
                    packed_path = Some(PathBuf::from(value));
                }
            }
        }

        let packed_path = packed_path.ok_or("missing packed archive path")?;
        Ok(Self {
            packed_path,
            platform,
            step_limit,
        })
    }
}

fn next_value(
    args: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
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
    eprintln!("usage: wmsruntime <packed.warc> [--platform native|wasm|egui] [--step-limit N]");
}
