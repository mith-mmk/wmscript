use wmarchive::Manifest;
use wmresource::ResourceType;
use wmruntime::LoadedArchive;
use wmvm::{Program as VmProgram, RunOutcome, WorkerId};

use crate::util::stable_hash64;

/// Asset bundled into a game project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameAsset {
    pub name: String,
    pub section_id: u32,
    pub resource_id: u32,
    pub resource_type: ResourceType,
    pub payload: Vec<u8>,
    pub flags: u16,
    pub align: u32,
    pub external_location: Option<GameAssetExternalLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum GameWorkerRole {
    Ui,
    Loader,
    Engine,
}

impl GameWorkerRole {
    #[allow(non_upper_case_globals)]
    pub const Frontend: Self = Self::Engine;
    #[allow(non_upper_case_globals)]
    pub const Middleware: Self = Self::Loader;
    #[allow(non_upper_case_globals)]
    pub const Background: Self = Self::Ui;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Loader => "loader",
            Self::Engine => "engine",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ui" | "background" => Some(Self::Ui),
            "loader" | "middleware" => Some(Self::Loader),
            "engine" | "frontend" | "main" | "script" => Some(Self::Engine),
            _ => None,
        }
    }

    pub(crate) const fn module_section_id(self) -> u32 {
        match self {
            Self::Ui => 2,
            Self::Loader => 3,
            Self::Engine => 4,
        }
    }

    pub(crate) const fn spawn_rank(self) -> u8 {
        match self {
            Self::Ui => 0,
            Self::Loader => 1,
            Self::Engine => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameScript {
    pub role: GameWorkerRole,
    pub script_path: String,
    pub source: String,
}

impl GameScript {
    pub fn new(
        role: GameWorkerRole,
        script_path: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            role,
            script_path: script_path.into(),
            source: source.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameAssetExternalLocation {
    pub url: String,
    pub cache_key: String,
    pub flags: u32,
}

impl GameAssetExternalLocation {
    pub fn new(url: impl Into<String>, cache_key: impl Into<String>, flags: u32) -> Self {
        Self {
            url: url.into(),
            cache_key: cache_key.into(),
            flags,
        }
    }
}

impl GameAsset {
    pub fn new(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        resource_type: ResourceType,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            name: name.into(),
            section_id,
            resource_id,
            resource_type,
            payload: payload.into(),
            flags: 0,
            align: 16,
            external_location: None,
        }
    }

    pub fn script_data(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(
            name,
            section_id,
            resource_id,
            ResourceType::ScriptData,
            payload,
        )
    }

    pub fn image(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(name, section_id, resource_id, ResourceType::Image, payload)
    }

    pub fn audio(
        name: impl Into<String>,
        section_id: u32,
        resource_id: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self::new(name, section_id, resource_id, ResourceType::Audio, payload)
    }

    pub fn name_hash(&self) -> u64 {
        stable_hash64(self.name.as_bytes())
    }

    pub fn with_external_location(
        mut self,
        url: impl Into<String>,
        cache_key: impl Into<String>,
        flags: u32,
    ) -> Self {
        self.external_location = Some(GameAssetExternalLocation::new(url, cache_key, flags));
        self
    }
}

/// Source-driven game project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameProject {
    pub package_name: String,
    pub script_path: String,
    pub source: String,
    pub scripts: Vec<GameScript>,
    pub assets: Vec<GameAsset>,
}

impl GameProject {
    pub fn new(
        package_name: impl Into<String>,
        script_path: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            package_name: package_name.into(),
            script_path: script_path.into(),
            source: source.into(),
            scripts: Vec::new(),
            assets: Vec::new(),
        }
    }

    pub fn scripts(&self) -> Vec<GameScript> {
        if self.scripts.is_empty() {
            vec![GameScript::new(
                GameWorkerRole::Engine,
                self.script_path.clone(),
                self.source.clone(),
            )]
        } else {
            self.scripts.clone()
        }
    }

    pub fn push_script(mut self, script: GameScript) -> Self {
        if self.scripts.is_empty() {
            self.scripts.push(GameScript::new(
                GameWorkerRole::Engine,
                self.script_path.clone(),
                self.source.clone(),
            ));
        }
        self.scripts.retain(|existing| existing.role != script.role);
        self.scripts.push(script);
        self.scripts.sort_by_key(|script| script.role);
        self
    }

    pub fn push_asset(mut self, asset: GameAsset) -> Self {
        self.assets.push(asset);
        self
    }

    pub fn with_asset(mut self, asset: GameAsset) -> Self {
        self.assets.push(asset);
        self
    }
}

/// Artifact produced by the build step.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildArtifact {
    pub program: VmProgram,
    pub worker_programs: Vec<WorkerProgram>,
    pub manifest: Manifest,
    pub archive: Vec<u8>,
    pub archive_size: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkerProgram {
    pub role: GameWorkerRole,
    pub section_id: u32,
    pub program: VmProgram,
}

/// Result produced by a full build and run cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionReport {
    pub build: BuildArtifact,
    pub loaded_archive: LoadedArchive,
    pub worker_id: WorkerId,
    pub outcomes: Vec<(WorkerId, RunOutcome)>,
}
