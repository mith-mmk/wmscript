use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use wmfrontend::{
    FrontendConfig, GuiFontPreset, demo::build_engine_worker_demo_project,
    demo::build_image_audio_demo_project, demo::build_message_window_demo_project,
    demo::build_ui_image_demo_project, launch_frontend_gui, run_frontend,
    run_frontend_archive_path,
};
use wmplatform::{PlatformKind, PlatformProfile};
use wmresource::ResourceType;
use wmtoolchain::{GameAsset, GameProject, GameScript, GameWorkerRole};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse(env::args().skip(1))?;
    let launch = LaunchArgs::resolve(args)?;
    let egui_mode = matches!(launch.platform.kind, PlatformKind::Egui);
    if let Some(archive_path) = &launch.archive_path {
        let mut report = run_frontend_archive_path(
            launch.platform,
            archive_path,
            launch.step_limit.unwrap_or(128),
        )?;
        if egui_mode {
            report = launch_frontend_gui(report, launch.font)?;
        }
        println!("=== frontend summary ===");
        println!("package: {}", report.build.manifest.package_name);
        println!("archive bytes: {}", report.build.archive_size);
        println!("worker id: {}", report.execution.worker_id);
        println!(
            "message window: visible={} speaker={:?}",
            report.ui_state.scene.message_window.visible,
            report.ui_state.scene.message_window.speaker
        );
        println!("images: {}", report.ui_state.scene.images.len());
        if let Some((_, outcome)) = report.execution.outcomes.last() {
            println!("final outcome: {outcome:?}");
        }
        let _ = report.audio_backend.clear();
        return Ok(());
    }

    let mut project = if let Some(demo) = &launch.demo {
        match demo.as_str() {
            "uiimage" => build_ui_image_demo_project(),
            "image-audio" => build_image_audio_demo_project(),
            "engineworker" => build_engine_worker_demo_project(),
            "messagewindow" => build_message_window_demo_project(),
            other => return Err(format!("unknown demo: {other}").into()),
        }
    } else {
        let script_path = launch.script_path.clone().ok_or("missing script path")?;
        let source = fs::read_to_string(&script_path)?;
        let package_name = launch.package_name.clone().unwrap_or_else(|| {
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
        for script in &launch.extra_scripts {
            project = project.push_script(GameScript::new(
                script.role,
                script.path.to_string_lossy().to_string(),
                fs::read_to_string(&script.path)?,
            ));
        }
        for asset in &launch.assets {
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
        project
    };
    if let Some(package_name) = &launch.package_name {
        project.package_name = package_name.clone();
    }
    let mut config = FrontendConfig::new(launch.platform, project);
    config.step_limit = launch.step_limit.unwrap_or(config.step_limit);
    config.auto_run = true;
    let mut report = run_frontend(config)?;
    if egui_mode {
        report = launch_frontend_gui(report, launch.font)?;
    }

    println!("=== frontend summary ===");
    println!("package: {}", report.build.manifest.package_name);
    println!("archive bytes: {}", report.build.archive_size);
    println!("worker id: {}", report.execution.worker_id);
    println!(
        "message window: visible={} speaker={:?}",
        report.ui_state.scene.message_window.visible, report.ui_state.scene.message_window.speaker
    );
    println!("images: {}", report.ui_state.scene.images.len());
    if let Some((_, outcome)) = report.execution.outcomes.last() {
        println!("final outcome: {outcome:?}");
    }
    let _ = report.audio_backend.clear();
    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    demo: Option<String>,
    script_path: Option<PathBuf>,
    archive_path: Option<PathBuf>,
    package_name: Option<String>,
    step_limit: Option<usize>,
    platform: PlatformProfile,
    assets: Vec<CliAsset>,
    extra_scripts: Vec<CliScript>,
    font: GuiFontPreset,
    platform_from_cli: bool,
    font_from_cli: bool,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut demo = None;
        let mut script_path = None;
        let mut archive_path = None;
        let mut package_name = None;
        let mut step_limit = None;
        let mut platform = PlatformProfile::native();
        let mut assets = Vec::new();
        let mut extra_scripts = Vec::new();
        let mut font = GuiFontPreset::default_preset();
        let mut platform_from_cli = false;
        let mut font_from_cli = false;

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
                    platform_from_cli = true;
                }
                "--demo" => {
                    let value = args.next().ok_or("--demo requires a value")?;
                    demo = Some(value);
                }
                "--archive" => {
                    let value = args.next().ok_or("--archive requires a value")?;
                    archive_path = Some(PathBuf::from(value));
                }
                "--frontend" => {
                    let value = args.next().ok_or("--frontend requires a value")?;
                    script_path = Some(PathBuf::from(value));
                }
                "--engine" => {
                    let value = args.next().ok_or("--engine requires a value")?;
                    script_path = Some(PathBuf::from(value));
                }
                "--ui" => {
                    let value = args.next().ok_or("--ui requires a value")?;
                    extra_scripts.push(CliScript {
                        role: GameWorkerRole::Ui,
                        path: PathBuf::from(value),
                    });
                }
                "--loader" => {
                    let value = args.next().ok_or("--loader requires a value")?;
                    extra_scripts.push(CliScript {
                        role: GameWorkerRole::Loader,
                        path: PathBuf::from(value),
                    });
                }
                "--middleware" => {
                    let value = args.next().ok_or("--middleware requires a value")?;
                    extra_scripts.push(CliScript {
                        role: GameWorkerRole::Loader,
                        path: PathBuf::from(value),
                    });
                }
                "--background" => {
                    let value = args.next().ok_or("--background requires a value")?;
                    extra_scripts.push(CliScript {
                        role: GameWorkerRole::Ui,
                        path: PathBuf::from(value),
                    });
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
                    font_from_cli = true;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}").into());
                }
                value => {
                    if script_path.is_some() || archive_path.is_some() {
                        return Err(format!("unexpected extra positional argument: {value}").into());
                    }
                    let path = PathBuf::from(value);
                    if path.extension().and_then(|ext| ext.to_str()) == Some("warc") {
                        archive_path = Some(path);
                    } else {
                        script_path = Some(path);
                    }
                }
            }
        }

        if demo.is_none() && script_path.is_none() && archive_path.is_none() {
            return Err("missing script path, archive path, or demo".into());
        }
        if demo.is_some() && (script_path.is_some() || archive_path.is_some()) {
            return Err("demo mode does not accept a positional script or archive path".into());
        }
        if archive_path.is_some() && script_path.is_some() {
            return Err("archive mode does not accept a script path".into());
        }
        Ok(Self {
            demo,
            script_path,
            archive_path,
            package_name,
            step_limit,
            platform,
            assets,
            extra_scripts,
            font,
            platform_from_cli,
            font_from_cli,
        })
    }
}

#[derive(Debug)]
struct LaunchArgs {
    demo: Option<String>,
    script_path: Option<PathBuf>,
    archive_path: Option<PathBuf>,
    package_name: Option<String>,
    step_limit: Option<usize>,
    platform: PlatformProfile,
    assets: Vec<CliAsset>,
    extra_scripts: Vec<CliScript>,
    font: GuiFontPreset,
}

impl LaunchArgs {
    fn resolve(args: CliArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = None;
        let mut script_path = args.script_path.clone();
        let mut archive_path = args.archive_path.clone();

        if args.demo.is_none()
            && archive_path.is_none()
            && let Some(path) = script_path.as_deref()
            && should_resolve_project_config(path)
        {
            if let Some(config_path) = find_project_config(path) {
                let loaded = ProjectConfig::load(&config_path)?;
                let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
                script_path = loaded.script.as_ref().map(|path| base_dir.join(path));
                archive_path = loaded.archive.as_ref().map(|path| base_dir.join(path));
                config = Some((loaded, base_dir.to_path_buf()));
            } else if let Some(resolved_path) = resolve_extensionless_path(path) {
                if resolved_path.extension().and_then(|ext| ext.to_str()) == Some("warc") {
                    archive_path = Some(resolved_path);
                    script_path = None;
                } else {
                    script_path = Some(resolved_path);
                }
            }
        }

        let mut package_name = args.package_name.clone();
        let mut step_limit = args.step_limit;
        let mut platform = args.platform;
        let mut font = args.font;
        let mut assets = Vec::new();
        let mut extra_scripts = args.extra_scripts;

        if let Some((loaded, base_dir)) = config {
            if package_name.is_none() {
                package_name = loaded.package_name;
            }
            if step_limit.is_none() {
                step_limit = loaded.step_limit;
            }
            if !args.platform_from_cli
                && let Some(value) = loaded.platform
            {
                platform = parse_platform(&value)?;
            }
            if !args.font_from_cli
                && let Some(value) = loaded.font
            {
                font = parse_font(&value)?;
            }
            assets.extend(loaded.assets.into_iter().map(|asset| CliAsset {
                name: asset.name,
                path: base_dir.join(asset.path),
                resource_type: asset.resource_type,
            }));
            if let Some(path) = loaded.middleware {
                extra_scripts.push(CliScript {
                    role: GameWorkerRole::Loader,
                    path: base_dir.join(path),
                });
            }
            if let Some(path) = loaded.background {
                extra_scripts.push(CliScript {
                    role: GameWorkerRole::Ui,
                    path: base_dir.join(path),
                });
            }
            for package in loaded.packages {
                if package.path.as_os_str().is_empty() {
                    continue;
                }
                let path = base_dir.join(package.path);
                match package.role {
                    GameWorkerRole::Engine => script_path = Some(path),
                    GameWorkerRole::Ui | GameWorkerRole::Loader => {
                        extra_scripts.push(CliScript {
                            role: package.role,
                            path,
                        });
                    }
                }
            }
        }

        assets.extend(args.assets);

        if args.demo.is_none() && script_path.is_none() && archive_path.is_none() {
            return Err("missing script path, archive path, or demo".into());
        }
        if archive_path.is_some() && script_path.is_some() {
            return Err("archive mode does not accept a script path".into());
        }

        Ok(Self {
            demo: args.demo,
            script_path,
            archive_path,
            package_name,
            step_limit,
            platform,
            assets,
            extra_scripts,
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

#[derive(Debug)]
struct CliScript {
    role: GameWorkerRole,
    path: PathBuf,
}

#[derive(Debug, Default)]
struct ProjectConfig {
    script: Option<PathBuf>,
    middleware: Option<PathBuf>,
    background: Option<PathBuf>,
    packages: Vec<ProjectConfigPackage>,
    archive: Option<PathBuf>,
    package_name: Option<String>,
    platform: Option<String>,
    font: Option<String>,
    step_limit: Option<usize>,
    assets: Vec<ProjectConfigAsset>,
}

impl ProjectConfig {
    fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let source = fs::read_to_string(path)?;
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => parse_project_toml(&source),
            Some("yaml") | Some("yml") => parse_project_yaml(&source),
            _ => Err(format!("unsupported project config: {}", path.display()).into()),
        }
    }
}

#[derive(Debug)]
struct ProjectConfigAsset {
    name: String,
    path: PathBuf,
    resource_type: ResourceType,
}

#[derive(Debug)]
struct ProjectConfigPackage {
    role: GameWorkerRole,
    path: PathBuf,
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

fn should_resolve_project_config(path: &Path) -> bool {
    path.extension().is_none()
}

fn find_project_config(path: &Path) -> Option<PathBuf> {
    let candidates = project_config_candidates(path);
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn resolve_extensionless_path(path: &Path) -> Option<PathBuf> {
    for ext in ["warc", "wms"] {
        let candidate = path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn project_config_candidates(path: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if path.is_dir() {
        let dir_name = path.file_name().and_then(|name| name.to_str());
        push_named_config_candidates(&mut candidates, path, "wmfrontend");
        push_named_config_candidates(&mut candidates, path, "wmscript");
        push_named_config_candidates(&mut candidates, path, "game");
        push_named_config_candidates(&mut candidates, path, "project");
        push_named_config_candidates(&mut candidates, path, "main");
        if let Some(dir_name) = dir_name {
            push_named_config_candidates(&mut candidates, path, dir_name);
        }
    } else if let Some(stem) = path.file_name().and_then(|name| name.to_str()) {
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        push_named_config_candidates(&mut candidates, dir, stem);
    }
    candidates
}

fn push_named_config_candidates(candidates: &mut Vec<PathBuf>, dir: &Path, stem: &str) {
    for ext in ["toml", "yaml", "yml"] {
        candidates.push(dir.join(format!("{stem}.{ext}")));
    }
}

fn parse_project_toml(source: &str) -> Result<ProjectConfig, Box<dyn std::error::Error>> {
    let mut config = ProjectConfig::default();
    let mut section = ConfigSection::Root;
    let mut pending_asset: Option<ProjectConfigAssetBuilder> = None;

    for raw_line in source.lines() {
        let line = strip_comment(raw_line, '#').trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "[[package]]" | "[[packages]]" => {
                finish_config_asset(&mut config, pending_asset.take())?;
                section = ConfigSection::Package;
                continue;
            }
            "[[asset]]" | "[[assets]]" => {
                finish_config_asset(&mut config, pending_asset.take())?;
                pending_asset = Some(ProjectConfigAssetBuilder::new(ResourceType::ScriptData));
                section = ConfigSection::Asset;
                continue;
            }
            "[[image]]" | "[[images]]" => {
                finish_config_asset(&mut config, pending_asset.take())?;
                pending_asset = Some(ProjectConfigAssetBuilder::new(ResourceType::Image));
                section = ConfigSection::Image;
                continue;
            }
            "[[script_data]]" | "[[script_datas]]" => {
                finish_config_asset(&mut config, pending_asset.take())?;
                pending_asset = Some(ProjectConfigAssetBuilder::new(ResourceType::ScriptData));
                section = ConfigSection::Asset;
                continue;
            }
            _ if line.starts_with('[') => {
                return Err(format!("unsupported config section: {line}").into());
            }
            _ => {}
        }

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid toml config line: {line}"))?;
        let key = key.trim();
        let value = parse_config_string(value.trim())?;
        match section {
            ConfigSection::Root => apply_root_config_value(&mut config, key, value)?,
            ConfigSection::Package => apply_package_config_value(&mut config, key, value)?,
            ConfigSection::Asset | ConfigSection::Image => {
                let asset = pending_asset
                    .as_mut()
                    .ok_or("internal config parser error: missing asset section")?;
                apply_asset_config_value(asset, key, value)?;
            }
        }
    }
    finish_config_asset(&mut config, pending_asset)?;
    Ok(config)
}

fn parse_project_yaml(source: &str) -> Result<ProjectConfig, Box<dyn std::error::Error>> {
    let mut config = ProjectConfig::default();
    let mut section = ConfigSection::Root;
    let mut pending_asset: Option<ProjectConfigAssetBuilder> = None;

    for raw_line in source.lines() {
        let line_without_comment = strip_comment(raw_line, '#');
        let line = line_without_comment.trim();
        if line.is_empty() {
            continue;
        }
        if line == "assets:" || line == "asset:" {
            finish_config_asset(&mut config, pending_asset.take())?;
            section = ConfigSection::Asset;
            continue;
        }
        if line == "images:" || line == "image:" {
            finish_config_asset(&mut config, pending_asset.take())?;
            section = ConfigSection::Image;
            continue;
        }
        if line == "packages:" || line == "package:" {
            finish_config_asset(&mut config, pending_asset.take())?;
            section = ConfigSection::Package;
            continue;
        }

        let line = if let Some(rest) = line.strip_prefix("- ") {
            finish_config_asset(&mut config, pending_asset.take())?;
            if section != ConfigSection::Package {
                pending_asset = Some(ProjectConfigAssetBuilder::new(match section {
                    ConfigSection::Image => ResourceType::Image,
                    _ => ResourceType::ScriptData,
                }));
            }
            rest.trim()
        } else {
            line
        };

        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("invalid yaml config line: {line}"))?;
        let key = key.trim();
        let value = parse_config_string(value.trim())?;
        match section {
            ConfigSection::Root => apply_root_config_value(&mut config, key, value)?,
            ConfigSection::Package => apply_package_config_value(&mut config, key, value)?,
            ConfigSection::Asset | ConfigSection::Image => {
                let asset = pending_asset
                    .as_mut()
                    .ok_or("asset entry must start with '-' in yaml config")?;
                apply_asset_config_value(asset, key, value)?;
            }
        }
    }
    finish_config_asset(&mut config, pending_asset)?;
    Ok(config)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigSection {
    Root,
    Package,
    Asset,
    Image,
}

#[derive(Debug)]
struct ProjectConfigAssetBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    resource_type: ResourceType,
}

impl ProjectConfigAssetBuilder {
    const fn new(resource_type: ResourceType) -> Self {
        Self {
            name: None,
            path: None,
            resource_type,
        }
    }
}

fn apply_root_config_value(
    config: &mut ProjectConfig,
    key: &str,
    value: String,
) -> Result<(), Box<dyn std::error::Error>> {
    match key {
        "script" | "script_path" | "main" | "frontend" | "engine" => {
            config.script = Some(PathBuf::from(value))
        }
        "middleware" => config.middleware = Some(PathBuf::from(value)),
        "background" => config.background = Some(PathBuf::from(value)),
        "loader" => config.middleware = Some(PathBuf::from(value)),
        "ui" => config.background = Some(PathBuf::from(value)),
        "archive" | "archive_path" => config.archive = Some(PathBuf::from(value)),
        "package" | "package_name" | "name" => config.package_name = Some(value),
        "platform" => config.platform = Some(value),
        "font" => config.font = Some(value),
        "step_limit" | "step-limit" => config.step_limit = Some(value.parse()?),
        other => return Err(format!("unknown config key: {other}").into()),
    }
    Ok(())
}

fn apply_package_config_value(
    config: &mut ProjectConfig,
    key: &str,
    value: String,
) -> Result<(), Box<dyn std::error::Error>> {
    match key {
        "entry" | "path" | "script" => {
            let role = config
                .packages
                .last()
                .map(|package| package.role)
                .unwrap_or(GameWorkerRole::Engine);
            config.packages.push(ProjectConfigPackage {
                role,
                path: PathBuf::from(value),
            });
        }
        "name" => {
            let role = GameWorkerRole::parse(&value)
                .ok_or_else(|| format!("unknown package name: {value}"))?;
            config.packages.push(ProjectConfigPackage {
                role,
                path: PathBuf::new(),
            });
        }
        other => return Err(format!("unknown package config key: {other}").into()),
    }
    Ok(())
}

fn apply_asset_config_value(
    asset: &mut ProjectConfigAssetBuilder,
    key: &str,
    value: String,
) -> Result<(), Box<dyn std::error::Error>> {
    match key {
        "name" => asset.name = Some(value),
        "path" | "file" => asset.path = Some(PathBuf::from(value)),
        other => return Err(format!("unknown asset config key: {other}").into()),
    }
    Ok(())
}

fn finish_config_asset(
    config: &mut ProjectConfig,
    asset: Option<ProjectConfigAssetBuilder>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(asset) = asset else {
        return Ok(());
    };
    let name = asset.name.ok_or("asset entry is missing name")?;
    let path = asset.path.ok_or("asset entry is missing path")?;
    config.assets.push(ProjectConfigAsset {
        name,
        path,
        resource_type: asset.resource_type,
    });
    Ok(())
}

fn parse_config_string(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = value.trim();
    if value.is_empty() {
        return Err("config value is empty".into());
    }
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        if value.len() < 2 {
            return Err("invalid quoted config value".into());
        }
        return Ok(value[1..value.len() - 1].to_owned());
    }
    Ok(value.to_owned())
}

fn strip_comment(line: &str, marker: char) -> &str {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            _ if ch == marker && !in_single_quote && !in_double_quote => return &line[..index],
            _ => {}
        }
    }
    line
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
        "usage: wmfrontend [--demo uiimage|image-audio|engineworker|messagewindow | <script.wms> | <archive.warc> | <project-without-extension> | --archive FILE] [--engine FILE] [--ui FILE] [--loader FILE] [--frontend FILE] [--middleware FILE] [--background FILE] [--package NAME] [--step-limit N] [--platform native|wasm|egui] [--font noto|default|mono] [--asset NAME=PATH] [--image NAME=PATH]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_toml_project_config() {
        let config = parse_project_toml(
            r#"
package = "sample-game"
frontend = "main.wms"
middleware = "middleware.wms"
background = "background.wms"
platform = "egui"
font = "noto"
step_limit = 64

[[package]]
name = "ui"
entry = "ui/main.wms"

[[package]]
name = "loader"
entry = "loader/main.wms"

[[package]]
name = "engine"
entry = "engine/main.wms"

[[asset]]
name = "story/guide"
path = "guide.txt"

[[image]]
name = "ui/background"
path = "background.png"
"#,
        )
        .expect("parse toml config");

        assert_eq!(config.package_name.as_deref(), Some("sample-game"));
        assert_eq!(config.script.as_deref(), Some(Path::new("main.wms")));
        assert_eq!(
            config.middleware.as_deref(),
            Some(Path::new("middleware.wms"))
        );
        assert_eq!(
            config.background.as_deref(),
            Some(Path::new("background.wms"))
        );
        assert_eq!(config.platform.as_deref(), Some("egui"));
        assert_eq!(config.font.as_deref(), Some("noto"));
        assert_eq!(config.step_limit, Some(64));
        let packages = config
            .packages
            .iter()
            .filter(|package| !package.path.as_os_str().is_empty())
            .collect::<Vec<_>>();
        assert_eq!(packages.len(), 3);
        assert_eq!(packages[0].role, GameWorkerRole::Ui);
        assert_eq!(packages[0].path, Path::new("ui/main.wms"));
        assert_eq!(packages[1].role, GameWorkerRole::Loader);
        assert_eq!(packages[1].path, Path::new("loader/main.wms"));
        assert_eq!(packages[2].role, GameWorkerRole::Engine);
        assert_eq!(packages[2].path, Path::new("engine/main.wms"));
        assert_eq!(config.assets.len(), 2);
        assert_eq!(config.assets[0].resource_type, ResourceType::ScriptData);
        assert_eq!(config.assets[1].resource_type, ResourceType::Image);
    }

    #[test]
    fn parse_yaml_project_config() {
        let config = parse_project_yaml(
            r#"
package: sample-game
frontend: main.wms
middleware: middleware.wms
background: background.wms
platform: native
assets:
  - name: story/guide
    path: guide.txt
images:
  - name: ui/background
    path: background.png
"#,
        )
        .expect("parse yaml config");

        assert_eq!(config.package_name.as_deref(), Some("sample-game"));
        assert_eq!(config.script.as_deref(), Some(Path::new("main.wms")));
        assert_eq!(
            config.middleware.as_deref(),
            Some(Path::new("middleware.wms"))
        );
        assert_eq!(
            config.background.as_deref(),
            Some(Path::new("background.wms"))
        );
        assert_eq!(config.platform.as_deref(), Some("native"));
        assert_eq!(config.assets.len(), 2);
        assert_eq!(config.assets[0].resource_type, ResourceType::ScriptData);
        assert_eq!(config.assets[1].resource_type, ResourceType::Image);
    }

    #[test]
    fn resolve_extensionless_directory_config() {
        let root = PathBuf::from(format!(".test-wmfrontend-config-{}", std::process::id()));
        let project_dir = root.join("novel");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&project_dir).expect("create project dir");
        fs::write(
            project_dir.join("wmfrontend.toml"),
            r#"
package = "novel"
script = "main.wms"
middleware = "middleware.wms"
background = "background.wms"
platform = "egui"

[[image]]
name = "ui/background"
path = "background.png"
"#,
        )
        .expect("write config");

        let args = CliArgs::parse(
            [
                project_dir.to_string_lossy().to_string(),
                "--font".to_owned(),
                "mono".to_owned(),
            ]
            .into_iter(),
        )
        .expect("parse args");
        let launch = LaunchArgs::resolve(args).expect("resolve launch args");

        assert_eq!(launch.package_name.as_deref(), Some("novel"));
        assert_eq!(
            launch.script_path.as_deref(),
            Some(project_dir.join("main.wms").as_path())
        );
        assert!(matches!(launch.platform.kind, PlatformKind::Egui));
        assert_eq!(launch.font, GuiFontPreset::Monospace);
        assert_eq!(launch.assets.len(), 1);
        assert_eq!(launch.assets[0].path, project_dir.join("background.png"));
        assert_eq!(launch.extra_scripts.len(), 2);
        assert_eq!(
            launch.extra_scripts[0].path,
            project_dir.join("middleware.wms")
        );
        assert_eq!(
            launch.extra_scripts[1].path,
            project_dir.join("background.wms")
        );

        fs::remove_dir_all(&root).expect("cleanup project dir");
    }
}
