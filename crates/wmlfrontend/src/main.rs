use std::env;
use std::fs;
use std::path::PathBuf;

use wmlfrontend::{FrontendConfig, GuiFontPreset, launch_frontend_gui, run_frontend};
use wmlplatform::{PlatformKind, PlatformProfile};
use wmlresource::ResourceType;
use wmltoolchain::{GameAsset, GameProject};

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
    let mut project = GameProject::new(
        package_name,
        script_path.to_string_lossy().to_string(),
        source,
    );
    for asset in &args.assets {
        let payload = fs::read(&asset.path)?;
        let section_id = 10 + project.assets.len() as u32;
        let resource_id = 100 + project.assets.len() as u32;
        project = project.push_asset(match asset.resource_type {
            ResourceType::Image => {
                GameAsset::image(asset.name.clone(), section_id, resource_id, payload)
            }
            _ => GameAsset::script_data(asset.name.clone(), section_id, resource_id, payload),
        });
    }
    let mut config = FrontendConfig::new(args.platform, project);
    config.step_limit = args.step_limit.unwrap_or(config.step_limit);
    config.auto_run = true;
    let egui_mode = matches!(args.platform.kind, PlatformKind::Egui);
    let report = run_frontend(config)?;
    if egui_mode {
        launch_frontend_gui(report.clone(), args.font)?;
    }

    println!("=== frontend summary ===");
    println!("package: {}", report.build.manifest.package_name);
    println!("archive bytes: {}", report.build.archive.len());
    println!("worker id: {}", report.execution.worker_id);
    println!(
        "message window: visible={} speaker={:?}",
        report.ui_state.scene.message_window.visible, report.ui_state.scene.message_window.speaker
    );
    println!("images: {}", report.ui_state.scene.images.len());
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
    assets: Vec<CliAsset>,
    font: GuiFontPreset,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut script_path = None;
        let mut package_name = None;
        let mut step_limit = None;
        let mut platform = PlatformProfile::native();
        let mut assets = Vec::new();
        let mut font = GuiFontPreset::default_preset();

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
                "--asset" => {
                    let value = args.next().ok_or("--asset requires a value")?;
                    assets.push(parse_asset_spec(&value, ResourceType::ScriptData)?);
                }
                "--image" => {
                    let value = args.next().ok_or("--image requires a value")?;
                    assets.push(parse_asset_spec(&value, ResourceType::Image)?);
                }
                "--font" => {
                    let value = args.next().ok_or("--font requires a value")?;
                    font = parse_font(&value)?;
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
            assets,
            font,
        })
    }
}

#[derive(Debug)]
struct CliAsset {
    name: String,
    path: PathBuf,
    resource_type: ResourceType,
}

fn parse_asset_spec(
    spec: &str,
    resource_type: ResourceType,
) -> Result<CliAsset, Box<dyn std::error::Error>> {
    let (name, path) = spec
        .split_once('=')
        .ok_or("asset must be specified as NAME=PATH")?;
    Ok(CliAsset {
        name: name.to_owned(),
        path: PathBuf::from(path),
        resource_type,
    })
}

fn parse_platform(value: &str) -> Result<PlatformProfile, Box<dyn std::error::Error>> {
    match value {
        "native" => Ok(PlatformProfile::native()),
        "wasm" => Ok(PlatformProfile::wasm()),
        "egui" => Ok(PlatformProfile::egui()),
        other => Err(format!("unknown platform: {other}").into()),
    }
}

fn parse_font(value: &str) -> Result<GuiFontPreset, Box<dyn std::error::Error>> {
    match value {
        "noto" | "noto-sans" | "noto-sans-jp" => Ok(GuiFontPreset::NotoSans),
        "default" | "egui" => Ok(GuiFontPreset::EguiDefault),
        "mono" | "monospace" => Ok(GuiFontPreset::Monospace),
        other => Err(format!("unknown font preset: {other}").into()),
    }
}

fn print_usage() {
    eprintln!(
        "usage: wmlfrontend <script.wml> [--package NAME] [--step-limit N] [--platform native|wasm|egui] [--font noto|default|mono] [--asset NAME=PATH] [--image NAME=PATH]"
    );
}
