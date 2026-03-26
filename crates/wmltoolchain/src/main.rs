use std::env;
use std::fs;
use std::path::PathBuf;

use wmlplatform::PlatformProfile;
use wmlresource::ResourceType;
use wmltoolchain::{GameAsset, GameProject, Toolchain, ToolchainConfig};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse(env::args().skip(1))?;
    let source = fs::read_to_string(&args.script_path)?;
    let package_name = args.package_name.unwrap_or_else(|| {
        args.script_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("wml-game")
            .to_owned()
    });

    let mut project = GameProject::new(
        package_name,
        args.script_path.to_string_lossy().to_string(),
        source,
    );

    for asset in &args.assets {
        let payload = fs::read(&asset.path)?;
        let section_id = 10 + project.assets.len() as u32;
        let resource_id = 100 + project.assets.len() as u32;
        project = project.push_asset(GameAsset::new(
            asset.name.clone(),
            section_id,
            resource_id,
            asset.resource_type,
            payload,
        ));
    }

    let toolchain = Toolchain::new(
        ToolchainConfig::new(args.platform)
            .with_step_limit(args.step_limit.unwrap_or(128))
            .with_release(args.release),
    );
    let build = toolchain.build_project(&project)?;
    let out_path = args.output.unwrap_or_else(|| {
        let mut path = args.script_path.clone();
        path.set_extension("warc");
        path
    });
    fs::write(&out_path, &build.archive)?;

    println!("=== toolchain summary ===");
    println!("package: {}", build.manifest.package_name);
    println!("archive bytes: {}", build.archive.len());
    println!("entry func: {:?}", build.program.entry());
    println!("output: {}", out_path.display());
    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    script_path: PathBuf,
    package_name: Option<String>,
    output: Option<PathBuf>,
    step_limit: Option<usize>,
    platform: PlatformProfile,
    release: bool,
    assets: Vec<CliAsset>,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut script_path = None;
        let mut package_name = None;
        let mut output = None;
        let mut step_limit = None;
        let mut platform = PlatformProfile::native();
        let mut release = false;
        let mut assets = Vec::new();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--package" => package_name = Some(next_value(&mut args, "--package")?),
                "--out" => output = Some(PathBuf::from(next_value(&mut args, "--out")?)),
                "--step-limit" => {
                    step_limit = Some(next_value(&mut args, "--step-limit")?.parse()?)
                }
                "--platform" => platform = parse_platform(&next_value(&mut args, "--platform")?)?,
                "--release" => release = true,
                "--asset" => assets.push(parse_asset_spec(&next_value(&mut args, "--asset")?)?),
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
            output,
            step_limit,
            platform,
            release,
            assets,
        })
    }
}

#[derive(Debug)]
struct CliAsset {
    name: String,
    path: PathBuf,
    resource_type: ResourceType,
}

fn parse_asset_spec(spec: &str) -> Result<CliAsset, Box<dyn std::error::Error>> {
    let (name, path) = spec
        .split_once('=')
        .ok_or("asset must be specified as NAME=PATH")?;
    Ok(CliAsset {
        name: name.to_owned(),
        path: PathBuf::from(path),
        resource_type: ResourceType::ScriptData,
    })
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
    eprintln!(
        "usage: wmltoolchain <script.wml> [--package NAME] [--out FILE] [--step-limit N] [--platform native|wasm|egui] [--release] [--asset NAME=PATH]"
    );
}
