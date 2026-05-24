#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use crate::{AudioBackend, SharedAudioBackend, create_disabled_audio_backend};
use wmarchive::{Archive, ArchiveError, ArchiveStreamReader, Manifest};
use wmext::{ExtError, ExtValueType, ExtensionFunctionSpec, ExtensionRegistry, NamespacePolicy};
use wmhost::{
    CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_GUI, CAP_NETWORK, CapabilityMask, HostFunction, HostRegistry,
};
use wmplatform::PlatformProfile;
use wmresource::{
    Handle as ResourceHandle, LoadResult, ResourceData, ResourceError, ResourceManager,
    ResourceType, decode_resource_header,
};
use wmui::{UiInsets, UiMessageWindowStyle, UiRect, UiSceneLayoutState};
use wmverifier::{VerificationError, verify_program};
use wmvm::{
    HostError, Message, Program, ProgramCodecError, RunOutcome, Scheduler, Value, Vm, VmConfig,
    WorkerId, WorkerState,
};

#[cfg(test)]
use wmui::UiColorRgba;

mod archive_loading;
mod backends;
mod checkpoint;
mod extension_ids;
mod extensions;
mod helpers;
mod host_dispatch;
mod state_manager;

use backends::{DisabledLlmBackend, DisabledNetBackend};
pub use backends::{LlmBackend, NetBackend};
use checkpoint::RuntimeCheckpoint;
pub use extension_ids::*;
use helpers::*;
use host_dispatch::{HostDispatcher, SharedHostApi};
use state_manager::StateManager;

/// Runtime configuration shared by the wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub platform: PlatformProfile,
    pub step_limit: usize,
    pub capability_mask: CapabilityMask,
    pub memory_limit: usize,
}

impl RuntimeConfig {
    pub const fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            step_limit: 128,
            capability_mask: CapabilityMask::MAX,
            memory_limit: 64 * 1024 * 1024,
        }
    }

    pub const fn with_step_limit(mut self, step_limit: usize) -> Self {
        self.step_limit = step_limit;
        self
    }

    pub const fn with_capability_mask(mut self, capability_mask: CapabilityMask) -> Self {
        self.capability_mask = capability_mask;
        self
    }

    pub const fn with_memory_limit(mut self, memory_limit: usize) -> Self {
        self.memory_limit = memory_limit;
        self
    }
}

/// Runtime error type.
#[derive(Debug)]
pub enum RuntimeError {
    Archive(ArchiveError),
    Extension(ExtError),
    ProgramCodec(ProgramCodecError),
    Resource(ResourceError),
    Verification(VerificationError),
    Host(HostError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(error) => write!(f, "{error}"),
            Self::Extension(error) => write!(f, "{error}"),
            Self::ProgramCodec(error) => write!(f, "{error}"),
            Self::Resource(error) => write!(f, "{error}"),
            Self::Verification(error) => write!(f, "{error}"),
            Self::Host(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<ArchiveError> for RuntimeError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl From<ExtError> for RuntimeError {
    fn from(value: ExtError) -> Self {
        Self::Extension(value)
    }
}

impl From<ProgramCodecError> for RuntimeError {
    fn from(value: ProgramCodecError) -> Self {
        Self::ProgramCodec(value)
    }
}

impl From<ResourceError> for RuntimeError {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}

impl From<VerificationError> for RuntimeError {
    fn from(value: VerificationError) -> Self {
        Self::Verification(value)
    }
}

impl From<HostError> for RuntimeError {
    fn from(value: HostError) -> Self {
        Self::Host(value)
    }
}

/// Summary returned after loading an archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedArchive {
    pub manifest: Option<Manifest>,
    pub resources_loaded: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioPlaybackState {
    pub resource_id: u32,
    pub playing: bool,
    pub looped: bool,
    pub position_ms: u64,
    pub volume: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageSourceRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IconSheetState {
    pub cell_width: u32,
    pub cell_height: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageChoiceState {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageWindowState {
    pub visible: bool,
    pub speaker: Option<String>,
    pub text: String,
    pub locale: String,
    pub backlog: Vec<String>,
    pub choices: Vec<MessageChoiceState>,
    pub input_prompt: Option<String>,
    pub text_speed: f32,
    pub auto_mode: bool,
    pub skip_mode: bool,
    pub style: UiMessageWindowStyle,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiPolicyState {
    pub context_menu_enabled: bool,
    pub shift_fast_enabled: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RpgActionState {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RpgMapControlsState {
    pub projection: String,
    pub directions: Vec<String>,
}

impl RpgMapControlsState {
    pub fn active(&self) -> bool {
        !self.directions.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RpgHudState {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RpgUiState {
    pub map_controls: RpgMapControlsState,
    pub actions: Vec<RpgActionState>,
    pub hud: Option<RpgHudState>,
}

impl Default for MessageWindowState {
    fn default() -> Self {
        Self {
            visible: false,
            speaker: None,
            text: String::new(),
            locale: "ja".to_owned(),
            backlog: Vec::new(),
            choices: Vec::new(),
            input_prompt: None,
            text_speed: 48.0,
            auto_mode: false,
            skip_mode: false,
            style: UiMessageWindowStyle::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageDrawState {
    pub handle: u64,
    pub resource_id: u32,
    pub x: f32,
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub source: Option<ImageSourceRect>,
    pub icon_sheet: Option<IconSheetState>,
    pub icon_index: Option<u32>,
    pub rotation_degrees: f32,
    pub opacity: f32,
}

/// Headless runtime wrapper.
#[derive(Clone)]
pub struct Runtime {
    config: RuntimeConfig,
    scheduler: Rc<RefCell<Scheduler>>,
    resources: Rc<RefCell<ResourceManager>>,
    host: Rc<RefCell<HostDispatcher>>,
    extensions: ExtensionRegistry,
    audio_backend: Rc<SharedAudioBackend>,
    debug_log: Rc<RefCell<Vec<String>>>,
    net_backend: Rc<RefCell<Box<dyn NetBackend>>>,
    llm_backend: Rc<RefCell<Box<dyn LlmBackend>>>,
    loaded_archives: Rc<RefCell<Vec<Manifest>>>,
    image_draws: Rc<RefCell<Vec<ImageDrawState>>>,
    icon_sheets: Rc<RefCell<BTreeMap<u64, IconSheetState>>>,
    scene_layout: Rc<RefCell<UiSceneLayoutState>>,
    message_window: Rc<RefCell<MessageWindowState>>,
    ui_policy: Rc<RefCell<UiPolicyState>>,
    rpg_ui: Rc<RefCell<RpgUiState>>,
    audio_states: Rc<RefCell<BTreeMap<u64, AudioPlaybackState>>>,
    state_manager: Rc<RefCell<StateManager>>,
    checkpoints: Rc<RefCell<BTreeMap<u32, RuntimeCheckpoint>>>,
    pending_vm_saves: Rc<RefCell<Vec<u32>>>,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            scheduler: Rc::new(RefCell::new(Scheduler::new())),
            resources: Rc::new(RefCell::new(ResourceManager::new(config.memory_limit))),
            host: Rc::new(RefCell::new(HostDispatcher::new(
                config.platform,
                config.capability_mask,
            ))),
            extensions: ExtensionRegistry::with_policy(NamespacePolicy::permissive()),
            audio_backend: Rc::new(SharedAudioBackend::new(create_disabled_audio_backend())),
            debug_log: Rc::new(RefCell::new(Vec::new())),
            net_backend: Rc::new(RefCell::new(Box::new(DisabledNetBackend))),
            llm_backend: Rc::new(RefCell::new(Box::new(DisabledLlmBackend))),
            loaded_archives: Rc::new(RefCell::new(Vec::new())),
            image_draws: Rc::new(RefCell::new(Vec::new())),
            icon_sheets: Rc::new(RefCell::new(BTreeMap::new())),
            scene_layout: Rc::new(RefCell::new(UiSceneLayoutState::default())),
            message_window: Rc::new(RefCell::new(MessageWindowState::default())),
            ui_policy: Rc::new(RefCell::new(UiPolicyState::default())),
            rpg_ui: Rc::new(RefCell::new(RpgUiState::default())),
            audio_states: Rc::new(RefCell::new(BTreeMap::new())),
            state_manager: Rc::new(RefCell::new(StateManager::default())),
            checkpoints: Rc::new(RefCell::new(BTreeMap::new())),
            pending_vm_saves: Rc::new(RefCell::new(Vec::new())),
            config,
        }
    }

    pub fn config(&self) -> RuntimeConfig {
        self.config
    }

    pub fn host_registry(&self) -> HostRegistry {
        self.host.borrow().registry().clone()
    }

    pub fn extension_registry(&self) -> &ExtensionRegistry {
        &self.extensions
    }

    pub fn debug_log(&self) -> Vec<String> {
        self.debug_log.borrow().clone()
    }

    pub fn set_net_backend<B: NetBackend + 'static>(&mut self, backend: B) {
        *self.net_backend.borrow_mut() = Box::new(backend);
    }

    pub fn set_llm_backend<B: LlmBackend + 'static>(&mut self, backend: B) {
        *self.llm_backend.borrow_mut() = Box::new(backend);
    }

    pub fn set_audio_backend(&mut self, backend: Box<dyn AudioBackend>) {
        self.audio_backend.replace(backend);
    }

    pub fn audio_backend_handle(&self) -> Rc<SharedAudioBackend> {
        self.audio_backend.clone()
    }

    pub fn register_host_function(
        &mut self,
        meta: HostFunction,
        callback: impl FnMut(&[Value]) -> Result<Value, HostError> + 'static,
    ) -> Option<HostFunction> {
        self.host.borrow_mut().register(meta, callback)
    }

    pub fn install_standard_extensions(&mut self) -> Result<StandardExtensions, RuntimeError> {
        Ok(StandardExtensions {
            fs: self.install_fs_extension()?,
            debug: self.install_debug_extension()?,
            net: self.install_net_extension()?,
            llm: self.install_llm_extension()?,
            scene: self.install_scene_extension()?,
            message: self.install_message_extension()?,
            image: self.install_image_extension()?,
            audio: self.install_audio_extension()?,
            ui: self.install_ui_extension()?,
            rpg: self.install_rpg_extension()?,
            vm: self.install_vm_extension()?,
            state: self.install_state_extension()?,
            automation: self.install_automation_extension()?,
            rts: self.install_rts_extension()?,
        })
    }

    pub fn spawn_program(&mut self, program: Program) -> Result<WorkerId, RuntimeError> {
        verify_program(&program, &self.host.borrow().registry())?;
        let worker_id = self.scheduler_spawn(program);
        Ok(worker_id)
    }

    fn scheduler_spawn(&mut self, program: Program) -> WorkerId {
        let vm_config = VmConfig::new(
            self.config.platform,
            self.host.borrow().registry().clone(),
            self.config.step_limit,
        )
        .with_capability_mask(self.config.capability_mask)
        .with_worker_id(0);
        let vm = Vm::with_program_and_host_api(
            vm_config,
            program,
            SharedHostApi::new(self.host.clone()),
        );
        self.scheduler.borrow_mut().spawn(vm)
    }

    pub fn tick(&mut self) -> Vec<(WorkerId, RunOutcome)> {
        let outcomes = self
            .scheduler
            .borrow_mut()
            .run_round(self.config.step_limit);
        self.flush_pending_vm_saves();
        outcomes
    }

    pub fn run_until_idle(&mut self, max_rounds: usize) -> Vec<(WorkerId, RunOutcome)> {
        let mut outcomes = Vec::new();
        for _ in 0..max_rounds {
            if self.scheduler.borrow().is_idle() {
                break;
            }
            outcomes.extend(self.tick());
        }
        outcomes
    }

    pub fn resource_manager(&self) -> std::cell::Ref<'_, ResourceManager> {
        self.resources.borrow()
    }

    pub fn resource_manager_mut(&self) -> std::cell::RefMut<'_, ResourceManager> {
        self.resources.borrow_mut()
    }

    pub fn loaded_archives(&self) -> Vec<Manifest> {
        self.loaded_archives.borrow().clone()
    }

    pub fn image_draws(&self) -> Vec<ImageDrawState> {
        self.image_draws.borrow().clone()
    }

    pub fn scene_layout_state(&self) -> UiSceneLayoutState {
        self.scene_layout.borrow().clone()
    }

    pub fn message_window_state(&self) -> MessageWindowState {
        self.message_window.borrow().clone()
    }

    pub fn ui_policy_state(&self) -> UiPolicyState {
        self.ui_policy.borrow().clone()
    }

    pub fn rpg_ui_state(&self) -> RpgUiState {
        self.rpg_ui.borrow().clone()
    }

    pub fn set_message_speed(&self, speed: f32) {
        self.message_window.borrow_mut().text_speed = speed.max(0.0);
    }

    pub fn set_message_auto_mode(&self, enabled: bool) {
        self.message_window.borrow_mut().auto_mode = enabled;
    }

    pub fn set_message_skip_mode(&self, enabled: bool) {
        self.message_window.borrow_mut().skip_mode = enabled;
    }

    pub fn set_message_locale(&self, locale: &str) {
        let normalized = locale.trim().to_ascii_lowercase();
        self.message_window.borrow_mut().locale = if normalized.starts_with("ja") {
            "ja".to_owned()
        } else {
            "en".to_owned()
        };
    }

    pub fn set_state_value(&self, key: impl Into<String>, value: Value) {
        self.state_manager.borrow_mut().set(key.into(), value);
    }

    pub fn state_value(&self, key: &str) -> Option<Value> {
        self.state_manager.borrow().get(key)
    }

    pub fn save_checkpoint(&self, slot: u32) {
        self.checkpoints.borrow_mut().insert(
            slot,
            RuntimeCheckpoint {
                scheduler: self.scheduler.borrow().snapshot(),
                resources: self.resources.borrow().clone(),
                loaded_archives: self.loaded_archives.borrow().clone(),
                image_draws: self.image_draws.borrow().clone(),
                icon_sheets: self.icon_sheets.borrow().clone(),
                scene_layout: self.scene_layout.borrow().clone(),
                message_window: self.message_window.borrow().clone(),
                ui_policy: self.ui_policy.borrow().clone(),
                rpg_ui: self.rpg_ui.borrow().clone(),
                debug_log: self.debug_log.borrow().clone(),
                audio_states: self.audio_states.borrow().clone(),
                state_manager: self.state_manager.borrow().clone(),
            },
        );
    }

    fn flush_pending_vm_saves(&self) {
        let slots = self
            .pending_vm_saves
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        for slot in slots {
            self.save_checkpoint(slot);
        }
    }

    pub fn load_checkpoint(&self, slot: u32) -> Result<bool, RuntimeError> {
        let Some(checkpoint) = self.checkpoints.borrow().get(&slot).cloned() else {
            return Ok(false);
        };
        *self.scheduler.borrow_mut() = Scheduler::from_snapshot(checkpoint.scheduler, |_| {
            Box::new(SharedHostApi::new(self.host.clone()))
        });
        *self.resources.borrow_mut() = checkpoint.resources;
        *self.loaded_archives.borrow_mut() = checkpoint.loaded_archives;
        *self.image_draws.borrow_mut() = checkpoint.image_draws;
        *self.icon_sheets.borrow_mut() = checkpoint.icon_sheets;
        *self.scene_layout.borrow_mut() = checkpoint.scene_layout;
        *self.message_window.borrow_mut() = checkpoint.message_window;
        *self.ui_policy.borrow_mut() = checkpoint.ui_policy;
        *self.rpg_ui.borrow_mut() = checkpoint.rpg_ui;
        *self.debug_log.borrow_mut() = checkpoint.debug_log;
        *self.audio_states.borrow_mut() = checkpoint.audio_states;
        *self.state_manager.borrow_mut() = checkpoint.state_manager;
        {
            let backend = self.audio_backend.clone();
            backend.clear()?;
            let replay_states = self
                .audio_states
                .borrow()
                .iter()
                .filter(|(_, state)| state.playing)
                .map(|(handle, state)| (*handle, state.clone()))
                .collect::<Vec<_>>();
            for (handle, state) in replay_states {
                let bytes = audio_bytes_for_resource_id(&self.resources, state.resource_id)?;
                backend.play(
                    handle,
                    state.resource_id,
                    &bytes,
                    state.looped,
                    state.position_ms,
                    state.volume,
                )?;
            }
        }
        Ok(true)
    }

    pub fn audio_playback_states(&self) -> BTreeMap<u64, AudioPlaybackState> {
        self.audio_states.borrow().clone()
    }

    pub fn worker_state(&self, worker_id: WorkerId) -> Option<WorkerState> {
        self.scheduler.borrow().worker_state(worker_id)
    }

    pub fn waiting_workers(&self) -> Vec<WorkerId> {
        let scheduler = self.scheduler.borrow();
        scheduler
            .worker_ids()
            .filter(|worker_id| {
                matches!(
                    scheduler.worker_state(*worker_id),
                    Some(WorkerState::WaitingMessage)
                )
            })
            .collect()
    }

    pub fn sleeping_workers(&self) -> Vec<WorkerId> {
        let scheduler = self.scheduler.borrow();
        scheduler
            .worker_ids()
            .filter(|worker_id| {
                matches!(
                    scheduler.worker_state(*worker_id),
                    Some(WorkerState::Sleeping)
                )
            })
            .collect()
    }

    pub fn wake_worker(&mut self, worker_id: WorkerId) -> bool {
        self.scheduler.borrow_mut().wake(worker_id)
    }

    /// Stops all running audio sessions.
    pub fn shutdown(&mut self) {
        let _ = self.audio_backend.clear();
    }

    pub fn send_message(&mut self, message: Message) {
        self.scheduler.borrow_mut().deliver(message);
    }
}

fn audio_bytes_for_handle(
    resources: &Rc<RefCell<ResourceManager>>,
    handle: u64,
) -> Result<(u32, Vec<u8>), HostError> {
    let resource_id = resources
        .borrow()
        .resource_id(ResourceHandle::from(handle))
        .map_err(resource_error_to_host_error)?;
    let bytes = audio_bytes_for_resource_id(resources, resource_id)?;
    Ok((resource_id, bytes))
}

fn audio_bytes_for_resource_id(
    resources: &Rc<RefCell<ResourceManager>>,
    resource_id: u32,
) -> Result<Vec<u8>, HostError> {
    let bytes = {
        let resources = resources.borrow();
        let entry = resources
            .entry(resource_id)
            .ok_or_else(|| HostError::Failed(format!("audio resource {resource_id} not found")))?;
        let data = entry.data.as_ref().ok_or_else(|| {
            HostError::Failed(format!("audio resource {resource_id} has no payload"))
        })?;
        data.bytes().to_vec()
    };
    Ok(bytes)
}

#[cfg(test)]
#[path = "../../tests/support/runtime_tests.rs"]
mod tests;
