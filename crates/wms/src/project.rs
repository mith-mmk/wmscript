use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use wmarchive::AssetKindV2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Headless,
    Egui,
}

impl Target {
    pub fn parse(value: &str) -> Result<Self, ProjectError> {
        match value {
            "headless" => Ok(Self::Headless),
            "egui" => Ok(Self::Egui),
            _ => Err(ProjectError::InvalidValue(format!(
                "unknown target `{value}`"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub id: u32,
    pub name: String,
    pub kind: AssetKindV2,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub version: String,
    pub entry: PathBuf,
    pub tick_hz: u32,
    pub seed: u64,
    pub save_compat_version: u32,
    pub default_target: Target,
    pub capabilities: Vec<String>,
    pub assets: Vec<Asset>,
}

impl Project {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = path.as_ref();
        let manifest = if path.is_dir() {
            path.join("wms.toml")
        } else {
            path.to_owned()
        };
        let root = manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let source = fs::read_to_string(&manifest).map_err(|error| ProjectError::Io {
            path: manifest.clone(),
            message: error.to_string(),
        })?;
        parse(&root, &source)
    }
    pub fn source_path(&self) -> PathBuf {
        self.root.join(&self.entry)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectError {
    Io { path: PathBuf, message: String },
    Syntax { line: usize, message: String },
    UnknownKey { line: usize, key: String },
    Missing(&'static str),
    InvalidValue(String),
    UnsafePath(PathBuf),
    DuplicateAssetId(u32),
    DuplicateAssetName(String),
}
impl core::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Syntax { line, message } => write!(f, "wms.toml:{line}: {message}"),
            Self::UnknownKey { line, key } => write!(f, "wms.toml:{line}: unknown key `{key}`"),
            Self::Missing(key) => write!(f, "wms.toml is missing `{key}`"),
            Self::InvalidValue(message) => f.write_str(message),
            Self::UnsafePath(path) => write!(
                f,
                "project path must stay below the project root: {}",
                path.display()
            ),
            Self::DuplicateAssetId(id) => write!(f, "duplicate asset id: {id}"),
            Self::DuplicateAssetName(name) => write!(f, "duplicate asset name: {name}"),
        }
    }
}
impl std::error::Error for ProjectError {}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Section {
    Root,
    Package,
    Game,
    Target,
    Capabilities,
    Asset,
}
#[derive(Default)]
struct AssetBuilder {
    id: Option<u32>,
    name: Option<String>,
    kind: Option<AssetKindV2>,
    path: Option<PathBuf>,
}

fn parse(root: &Path, source: &str) -> Result<Project, ProjectError> {
    let mut section = Section::Root;
    let mut name = None;
    let mut version = "0.1.0".to_owned();
    let mut entry = None;
    let mut tick_hz = 60u32;
    let mut seed = 1u64;
    let mut save_compat_version = 1u32;
    let mut default_target = Target::Headless;
    let mut capabilities = Vec::new();
    let mut assets = Vec::new();
    let mut pending = None;
    for (index, raw) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            finish_asset(root, pending.take(), &mut assets)?;
            section = match line {
                "[package]" => Section::Package,
                "[game]" => Section::Game,
                "[target]" => Section::Target,
                "[capabilities]" => Section::Capabilities,
                "[[asset]]" => {
                    pending = Some(AssetBuilder::default());
                    Section::Asset
                }
                _ => {
                    return Err(ProjectError::Syntax {
                        line: line_number,
                        message: format!("unknown section `{line}`"),
                    });
                }
            };
            continue;
        }
        let (key, raw_value) = line.split_once('=').ok_or_else(|| ProjectError::Syntax {
            line: line_number,
            message: "expected `key = value`".to_owned(),
        })?;
        let key = key.trim();
        let value = raw_value.trim();
        match section {
            Section::Package => match key {
                "name" => name = Some(string(value, line_number)?),
                "version" => version = string(value, line_number)?,
                "entry" => entry = Some(safe_relative(&string(value, line_number)?)?),
                _ => return unknown(line_number, key),
            },
            Section::Game => match key {
                "tick_hz" => tick_hz = integer(value, line_number)?,
                "seed" => seed = integer(value, line_number)?,
                "save_compat_version" => save_compat_version = integer(value, line_number)?,
                _ => return unknown(line_number, key),
            },
            Section::Target => match key {
                "default" => default_target = Target::parse(&string(value, line_number)?)?,
                _ => return unknown(line_number, key),
            },
            Section::Capabilities => match key {
                "allow" => capabilities = string_array(value, line_number)?,
                _ => return unknown(line_number, key),
            },
            Section::Asset => {
                let asset = pending.as_mut().ok_or_else(|| ProjectError::Syntax {
                    line: line_number,
                    message: "asset key is outside [[asset]]".to_owned(),
                })?;
                match key {
                    "id" => asset.id = Some(integer(value, line_number)?),
                    "name" => asset.name = Some(string(value, line_number)?),
                    "kind" => {
                        asset.kind = Some(match string(value, line_number)?.as_str() {
                            "data" => AssetKindV2::Data,
                            "image" => AssetKindV2::Image,
                            "audio" => AssetKindV2::Audio,
                            other => {
                                return Err(ProjectError::InvalidValue(format!(
                                    "unknown asset kind `{other}`"
                                )));
                            }
                        })
                    }
                    "path" => asset.path = Some(safe_relative(&string(value, line_number)?)?),
                    _ => return unknown(line_number, key),
                }
            }
            Section::Root => {
                return Err(ProjectError::UnknownKey {
                    line: line_number,
                    key: key.to_owned(),
                });
            }
        }
    }
    finish_asset(root, pending, &mut assets)?;
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for asset in &assets {
        if !ids.insert(asset.id) {
            return Err(ProjectError::DuplicateAssetId(asset.id));
        }
        if !names.insert(asset.name.clone()) {
            return Err(ProjectError::DuplicateAssetName(asset.name.clone()));
        }
    }
    Ok(Project {
        root: root.to_path_buf(),
        name: name.ok_or(ProjectError::Missing("package.name"))?,
        version,
        entry: entry.ok_or(ProjectError::Missing("package.entry"))?,
        tick_hz,
        seed,
        save_compat_version,
        default_target,
        capabilities,
        assets,
    })
}

fn finish_asset(
    root: &Path,
    pending: Option<AssetBuilder>,
    assets: &mut Vec<Asset>,
) -> Result<(), ProjectError> {
    let Some(asset) = pending else {
        return Ok(());
    };
    let path = asset.path.ok_or(ProjectError::Missing("asset.path"))?;
    assets.push(Asset {
        id: asset.id.ok_or(ProjectError::Missing("asset.id"))?,
        name: asset.name.ok_or(ProjectError::Missing("asset.name"))?,
        kind: asset.kind.ok_or(ProjectError::Missing("asset.kind"))?,
        path: root.join(path),
    });
    Ok(())
}
fn unknown<T>(line: usize, key: &str) -> Result<T, ProjectError> {
    Err(ProjectError::UnknownKey {
        line,
        key: key.to_owned(),
    })
}
fn safe_relative(value: &str) -> Result<PathBuf, ProjectError> {
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(ProjectError::UnsafePath(path));
    }
    Ok(path)
}
fn string(value: &str, line: usize) -> Result<String, ProjectError> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Ok(value[1..value.len() - 1].to_owned())
    } else {
        Err(ProjectError::Syntax {
            line,
            message: "expected quoted string".to_owned(),
        })
    }
}
fn integer<T: core::str::FromStr>(value: &str, line: usize) -> Result<T, ProjectError> {
    value.parse().map_err(|_| ProjectError::Syntax {
        line,
        message: "expected integer".to_owned(),
    })
}
fn string_array(value: &str, line: usize) -> Result<Vec<String>, ProjectError> {
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| ProjectError::Syntax {
            line,
            message: "expected string array".to_owned(),
        })?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|part| string(part.trim(), line))
        .collect()
}
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quoted {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
        } else if ch == '#' && !quoted {
            return &line[..index];
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_strict_manifest() {
        let project = parse(
            Path::new("game"),
            r#"[package]
name = "demo"
version = "1.0.0"
entry = "src/main.wms"
[game]
tick_hz = 30
seed = 7
save_compat_version = 2
[target]
default = "headless"
[capabilities]
allow = ["gui"]
[[asset]]
id = 1
name = "title"
kind = "image"
path = "assets/title.png"
"#,
        )
        .unwrap();
        assert_eq!(project.name, "demo");
        assert_eq!(project.assets.len(), 1);
    }
    #[test]
    fn rejects_parent_path() {
        assert!(matches!(
            safe_relative("../secret"),
            Err(ProjectError::UnsafePath(_))
        ));
    }
}
