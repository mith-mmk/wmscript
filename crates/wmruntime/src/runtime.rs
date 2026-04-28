#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use crate::{AudioBackend, SharedAudioBackend, create_disabled_audio_backend};
use wmarchive::{Archive, ArchiveError, ArchiveStreamReader, Manifest};
use wmext::{ExtError, ExtValueType, ExtensionFunctionSpec, ExtensionRegistry, NamespacePolicy};
use wmhost::{
    CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_GUI, CAP_NETWORK, CapabilityMask, HostFunction, HostId,
    HostRegistry,
};
use wmplatform::PlatformProfile;
use wmresource::{
    Handle as ResourceHandle, LoadResult, ResourceData, ResourceError, ResourceManager,
    ResourceState, ResourceType, decode_resource_header,
};
use wmui::{UiColorRgba, UiInsets, UiMessageWindowStyle, UiRect, UiSceneLayoutState};
use wmverifier::{VerificationError, verify_program};
use wmvm::{
    HostApi, HostError, Message, Program, ProgramCodecError, RunOutcome, Scheduler, Value, Vm,
    VmConfig, WorkerId, WorkerState,
};

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

#[derive(Clone, Debug, PartialEq)]
struct RuntimeCheckpoint {
    scheduler: wmvm::SchedulerSnapshot,
    resources: ResourceManager,
    loaded_archives: Vec<Manifest>,
    image_draws: Vec<ImageDrawState>,
    icon_sheets: BTreeMap<u64, IconSheetState>,
    scene_layout: UiSceneLayoutState,
    message_window: MessageWindowState,
    ui_policy: UiPolicyState,
    debug_log: Vec<String>,
    audio_states: BTreeMap<u64, AudioPlaybackState>,
    state_manager: StateManager,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct StateManager {
    current: BTreeMap<String, Value>,
    slots: BTreeMap<u32, BTreeMap<String, Value>>,
}

impl StateManager {
    fn save(&mut self, slot: u32) {
        self.slots.insert(slot, self.current.clone());
    }

    fn load(&mut self, slot: u32) -> bool {
        if let Some(saved) = self.slots.get(&slot).cloned() {
            self.current = saved;
            true
        } else {
            false
        }
    }

    fn has(&self, key: &str) -> bool {
        self.current.contains_key(key)
    }

    fn get(&self, key: &str) -> Option<Value> {
        self.current.get(key).cloned()
    }

    fn set(&mut self, key: String, value: Value) {
        self.current.insert(key, value);
    }

    fn erase(&mut self, key: &str) -> bool {
        self.current.remove(key).is_some()
    }
}

struct HostHandler {
    meta: HostFunction,
    callback: Box<dyn FnMut(&[Value]) -> Result<Value, HostError>>,
}

/// Host bridge used by the runtime wrapper.
pub struct HostDispatcher {
    registry: HostRegistry,
    allowed_capabilities: CapabilityMask,
    handlers: BTreeMap<HostId, HostHandler>,
}

impl HostDispatcher {
    pub fn new(profile: PlatformProfile, allowed_capabilities: CapabilityMask) -> Self {
        Self {
            registry: HostRegistry::new(profile),
            allowed_capabilities,
            handlers: BTreeMap::new(),
        }
    }

    pub fn registry(&self) -> &HostRegistry {
        &self.registry
    }

    pub fn register(
        &mut self,
        meta: HostFunction,
        callback: impl FnMut(&[Value]) -> Result<Value, HostError> + 'static,
    ) -> Option<HostFunction> {
        let previous = self.registry.register(meta);
        self.handlers.insert(
            meta.id,
            HostHandler {
                meta,
                callback: Box::new(callback),
            },
        );
        previous
    }

    fn call(&mut self, host_id: HostId, args: &[Value]) -> Result<Value, HostError> {
        let function = self
            .registry
            .function(host_id)
            .ok_or(HostError::UnknownHostId(host_id))?;
        if function.required_capabilities & !self.allowed_capabilities != 0 {
            return Err(HostError::CapabilityDenied {
                host_id,
                required: function.required_capabilities,
            });
        }
        if args.len() < function.min_args as usize || args.len() > function.max_args as usize {
            return Err(HostError::InvalidArguments(format!(
                "host {host_id} expected {}..={} args, got {}",
                function.min_args,
                function.max_args,
                args.len()
            )));
        }
        let handler = self
            .handlers
            .get_mut(&host_id)
            .ok_or(HostError::UnknownHostId(host_id))?;
        let _ = handler.meta;
        (handler.callback)(args)
    }
}

#[derive(Clone)]
struct SharedHostApi {
    inner: Rc<RefCell<HostDispatcher>>,
}

impl SharedHostApi {
    fn new(inner: Rc<RefCell<HostDispatcher>>) -> Self {
        Self { inner }
    }
}

impl HostApi for SharedHostApi {
    fn call_host(&mut self, host_id: HostId, args: &[Value]) -> Result<Value, HostError> {
        self.inner.borrow_mut().call(host_id, args)
    }
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

    pub fn load_archive(&mut self, bytes: &[u8]) -> Result<LoadedArchive, RuntimeError> {
        let archive = Archive::decode(bytes)?;
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?;
        let resources_loaded = self.resources.borrow_mut().ingest_archive(&archive)?;
        if let Some(manifest) = &manifest {
            self.loaded_archives.borrow_mut().push(manifest.clone());
        }
        Ok(LoadedArchive {
            manifest,
            resources_loaded,
        })
    }

    pub fn load_archive_reader<R: std::io::Read + std::io::Seek>(
        &mut self,
        archive: &mut ArchiveStreamReader<R>,
    ) -> Result<LoadedArchive, RuntimeError> {
        archive.verify_layout()?;
        archive.verify_manifest_digests()?;
        let manifest = archive.manifest()?;
        if let Some(manifest) = &manifest {
            for entry in &manifest.resource_map {
                self.resources
                    .borrow_mut()
                    .catalog_mut()
                    .insert_name_hash(entry.name_hash, entry.resource_id);
            }
            self.loaded_archives.borrow_mut().push(manifest.clone());
        }

        let mut resources_loaded = 0usize;
        let sections = archive.sections().to_vec();
        for section in sections {
            if !matches!(section.kind, wmarchive::SectionKind::Asset) {
                continue;
            }
            let bytes = archive.read_section_entry(&section)?;
            let header = decode_resource_header(&bytes)?;
            let start = header.data_offset as usize;
            let end = start
                .checked_add(header.unpacked_size as usize)
                .ok_or_else(|| {
                    ResourceError::InvalidArchiveSection(format!(
                        "asset section {} payload overflows",
                        section.id
                    ))
                })?;
            let payload = bytes.get(start..end).ok_or_else(|| {
                ResourceError::InvalidArchiveSection(format!(
                    "asset section {} payload missing",
                    section.id
                ))
            })?;
            let data = match header.resource_type {
                ResourceType::Image => ResourceData::Image(payload.to_vec()),
                ResourceType::Audio => ResourceData::Audio(payload.to_vec()),
                ResourceType::Binary => ResourceData::Binary(payload.to_vec()),
                ResourceType::Font => ResourceData::Font(payload.to_vec()),
                ResourceType::Video => ResourceData::Video(payload.to_vec()),
                ResourceType::ScriptData | ResourceType::Unknown(_) => {
                    ResourceData::ScriptData(payload.to_vec())
                }
            };
            self.resources.borrow_mut().register_ready(
                header.resource_id,
                data,
                header.flags as u32,
            )?;
            self.resources.borrow_mut().catalog_mut().insert(
                header.resource_id,
                wmresource::ResourceLocation {
                    section_id: section.id,
                    offset: header.data_offset as u64,
                    size: header.unpacked_size as u64,
                    resource_type: header.resource_type,
                    flags: header.flags,
                },
            )?;
            resources_loaded += 1;
        }

        Ok(LoadedArchive {
            manifest,
            resources_loaded,
        })
    }

    pub fn install_fs_extension(&mut self) -> Result<FsExtension, RuntimeError> {
        let read_host_id = 100;
        let write_host_id = 101;
        let exists_host_id = 102;

        let _ = self.register_host_function(
            HostFunction::new(read_host_id, 1, 1, CAP_FILE_SYSTEM),
            |args| read_text_file(args),
        );
        let _ = self.register_host_function(
            HostFunction::new(write_host_id, 2, 2, CAP_FILE_SYSTEM),
            |args| write_text_file(args),
        );
        let _ = self.register_host_function(
            HostFunction::new(exists_host_id, 1, 1, CAP_FILE_SYSTEM),
            |args| exists_text_file(args),
        );

        let ids = self.extensions.register_extension(
            "ext.fs",
            &[
                ExtensionFunctionSpec::new("read", read_host_id, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::String),
                ExtensionFunctionSpec::new("write", write_host_id, 2, 2, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Nil),
                ExtensionFunctionSpec::new("exists", exists_host_id, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(FsExtension {
            read_ext_id: ids[0],
            write_ext_id: ids[1],
            exists_ext_id: ids[2],
            read_host_id,
            write_host_id,
            exists_host_id,
        })
    }

    pub fn install_debug_extension(&mut self) -> Result<DebugExtension, RuntimeError> {
        let log_host_id = 110;
        let inspect_host_id = 111;
        let log_sink = self.debug_log.clone();

        let _ = self.register_host_function(HostFunction::new(log_host_id, 1, 1, 0), move |args| {
            let message = render_value(args.first().unwrap_or(&Value::Nil));
            log_sink.borrow_mut().push(message);
            Ok(Value::Nil)
        });
        let _ = self.register_host_function(HostFunction::new(inspect_host_id, 1, 1, 0), |args| {
            Ok(Value::String(render_value(
                args.first().unwrap_or(&Value::Nil),
            )))
        });

        let ids = self.extensions.register_extension(
            "ext.debug",
            &[
                ExtensionFunctionSpec::new("log", log_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Nil),
                ExtensionFunctionSpec::new("inspect", inspect_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(DebugExtension {
            log_ext_id: ids[0],
            inspect_ext_id: ids[1],
            log_host_id,
            inspect_host_id,
        })
    }

    pub fn install_state_extension(&mut self) -> Result<StateExtension, RuntimeError> {
        let save_host_id = 170;
        let load_host_id = 171;
        let has_host_id = 172;
        let get_host_id = 173;
        let set_host_id = 174;
        let erase_host_id = 175;
        let state_manager = self.state_manager.clone();

        let _ =
            self.register_host_function(HostFunction::new(save_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                state_manager.borrow_mut().save(slot);
                Ok(Value::Bool(true))
            });

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(load_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                Ok(Value::Bool(state_manager.borrow_mut().load(slot)))
            });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(HostFunction::new(has_host_id, 1, 1, 0), move |args| {
            let key = expect_string_arg(args, 0, "key")?;
            Ok(Value::Bool(state_manager.borrow().has(&key)))
        });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(HostFunction::new(get_host_id, 1, 1, 0), move |args| {
            let key = expect_string_arg(args, 0, "key")?;
            Ok(state_manager.borrow().get(&key).unwrap_or(Value::Nil))
        });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(HostFunction::new(set_host_id, 2, 2, 0), move |args| {
            let key = expect_string_arg(args, 0, "key")?;
            let value = args.get(1).cloned().unwrap_or(Value::Nil);
            state_manager.borrow_mut().set(key, value);
            Ok(Value::Bool(true))
        });

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(erase_host_id, 1, 1, 0), move |args| {
                let key = expect_string_arg(args, 0, "key")?;
                Ok(Value::Bool(state_manager.borrow_mut().erase(&key)))
            });

        let ids = self.extensions.register_extension(
            "state",
            &[
                ExtensionFunctionSpec::new("save", save_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("has", has_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("get", get_host_id, 1, 1, 0),
                ExtensionFunctionSpec::new("set", set_host_id, 2, 2, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("erase", erase_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(StateExtension {
            save_ext_id: ids[0],
            load_ext_id: ids[1],
            has_ext_id: ids[2],
            get_ext_id: ids[3],
            set_ext_id: ids[4],
            erase_ext_id: ids[5],
            save_host_id,
            load_host_id,
            has_host_id,
            get_host_id,
            set_host_id,
            erase_host_id,
        })
    }

    pub fn install_net_extension(&mut self) -> Result<NetExtension, RuntimeError> {
        let get_host_id = 120;
        let post_host_id = 121;
        let net_backend = self.net_backend.clone();

        let _ = self.register_host_function(
            HostFunction::new(get_host_id, 1, 1, CAP_NETWORK),
            move |args| {
                let url = expect_string_arg(args, 0, "url")?;
                net_backend.borrow_mut().get(&url).map(Value::String)
            },
        );

        let net_backend = self.net_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(post_host_id, 2, 2, CAP_NETWORK),
            move |args| {
                let url = expect_string_arg(args, 0, "url")?;
                let body = expect_string_arg(args, 1, "body")?;
                net_backend
                    .borrow_mut()
                    .post(&url, &body)
                    .map(Value::String)
            },
        );

        let ids = self.extensions.register_extension(
            "ext.net",
            &[
                ExtensionFunctionSpec::new("get", get_host_id, 1, 1, CAP_NETWORK)
                    .with_return_type(ExtValueType::String),
                ExtensionFunctionSpec::new("post", post_host_id, 2, 2, CAP_NETWORK)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(NetExtension {
            get_ext_id: ids[0],
            post_ext_id: ids[1],
            get_host_id,
            post_host_id,
        })
    }

    pub fn install_llm_extension(&mut self) -> Result<LlmExtension, RuntimeError> {
        let generate_host_id = 130;
        let llm_backend = self.llm_backend.clone();

        let _ = self.register_host_function(
            HostFunction::new(generate_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let prompt = expect_string_arg(args, 0, "prompt")?;
                llm_backend
                    .borrow_mut()
                    .generate(&prompt)
                    .map(Value::String)
            },
        );

        let ids = self.extensions.register_extension(
            "ext.llm",
            &[
                ExtensionFunctionSpec::new("generate", generate_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(LlmExtension {
            generate_ext_id: ids[0],
            generate_host_id,
        })
    }

    pub fn install_message_extension(&mut self) -> Result<MessageExtension, RuntimeError> {
        let show_host_id = 135;
        let append_host_id = 136;
        let choices_host_id = 137;
        let choices_named_host_id = 134;
        let prompt_host_id = 138;
        let hide_host_id = 139;
        let speed_host_id = 131;
        let auto_host_id = 132;
        let skip_host_id = 133;
        let log_clear_host_id = 159;
        let clear_host_id = 149;
        let box_style_host_id = 162;
        let text_color_host_id = 163;
        let speaker_color_host_id = 164;
        let accent_color_host_id = 165;
        let font_size_host_id = 166;
        let reset_style_host_id = 167;
        let frame_host_id = 168;
        let content_inset_host_id = 169;
        let input_box_style_host_id = 220;
        let input_text_color_host_id = 221;
        let input_hint_color_host_id = 222;
        let input_prompt_color_host_id = 223;
        let choice_box_style_host_id = 224;
        let choice_text_color_host_id = 225;
        let choice_accent_color_host_id = 226;
        let choice_selected_style_host_id = 227;
        let locale_host_id = 228;
        let message_window = self.message_window.clone();

        let _ = self.register_host_function(
            HostFunction::new(show_host_id, 1, 2, CAP_GUI),
            move |args| {
                let (speaker, text) = match args.len() {
                    1 => (None, expect_string_arg(args, 0, "text")?),
                    2 => (
                        Some(expect_string_arg(args, 0, "speaker")?),
                        expect_string_arg(args, 1, "text")?,
                    ),
                    other => {
                        return Err(HostError::InvalidArguments(format!(
                            "message.show expected 1..=2 args, got {other}"
                        )));
                    }
                };
                let mut window = message_window.borrow_mut();
                window.visible = true;
                window.speaker = speaker;
                window.text = text.clone();
                window
                    .backlog
                    .extend(text.lines().map(|line| line.to_owned()));
                window.input_prompt = None;
                window.choices.clear();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(append_host_id, 1, 1, CAP_GUI),
            move |args| {
                let line = expect_string_arg(args, 0, "line")?;
                let mut window = message_window.borrow_mut();
                if !window.text.is_empty() {
                    window.text.push('\n');
                }
                window.text.push_str(&line);
                window.backlog.push(line);
                window.visible = true;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choices_host_id, 0, 16, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.visible = true;
                if args.is_empty() {
                    window.choices.clear();
                    return Ok(Value::Bool(true));
                }
                window.choices = args
                    .iter()
                    .enumerate()
                    .map(|(index, value)| MessageChoiceState {
                        id: format!("choice-{}", index + 1),
                        label: render_value(value),
                        enabled: true,
                    })
                    .collect();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choices_named_host_id, 0, 16, CAP_GUI),
            move |args| {
                if !args.len().is_multiple_of(2) {
                    return Err(HostError::InvalidArguments(format!(
                        "message.choices_named expected id/label pairs, got {} args",
                        args.len()
                    )));
                }
                let mut window = message_window.borrow_mut();
                window.visible = true;
                if args.is_empty() {
                    window.choices.clear();
                    return Ok(Value::Bool(true));
                }
                let mut choices = Vec::with_capacity(args.len() / 2);
                for pair in args.chunks(2) {
                    choices.push(MessageChoiceState {
                        id: expect_string_arg(pair, 0, "choice_id")?,
                        label: expect_string_arg(pair, 1, "choice_label")?,
                        enabled: true,
                    });
                }
                window.choices = choices;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(prompt_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.visible = true;
                window.input_prompt = if args.is_empty() {
                    None
                } else {
                    Some(expect_string_arg(args, 0, "prompt")?)
                };
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(hide_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().visible = false;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(speed_host_id, 1, 1, CAP_GUI),
            move |args| {
                let speed = expect_number_arg(args, 0, "speed")? as f32;
                message_window.borrow_mut().text_speed = speed.max(0.0);
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(auto_host_id, 1, 1, CAP_GUI),
            move |args| {
                let enabled = expect_bool_arg(args, 0, "enabled")?;
                message_window.borrow_mut().auto_mode = enabled;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(skip_host_id, 1, 1, CAP_GUI),
            move |args| {
                let enabled = expect_bool_arg(args, 0, "enabled")?;
                message_window.borrow_mut().skip_mode = enabled;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(log_clear_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().backlog.clear();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(clear_host_id, 0, 0, CAP_GUI),
            move |_args| {
                *message_window.borrow_mut() = MessageWindowState::default();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.panel_fill = fill;
                window.style.panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "text")?;
                message_window.borrow_mut().style.text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(speaker_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "speaker")?;
                message_window.borrow_mut().style.speaker_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(accent_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "accent")?;
                message_window.borrow_mut().style.accent_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(font_size_host_id, 2, 2, CAP_GUI),
            move |args| {
                let body = expect_number_arg(args, 0, "body_font_size")? as f32;
                let speaker = expect_number_arg(args, 1, "speaker_font_size")? as f32;
                let mut window = message_window.borrow_mut();
                window.style.body_font_size = body.max(8.0);
                window.style.speaker_font_size = speaker.max(8.0);
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(reset_style_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().style = UiMessageWindowStyle::default();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(frame_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.style.frame_resource_id = if args.is_empty() {
                    None
                } else {
                    Some(expect_integer_arg(args, 0, "frame_resource_id")? as u32)
                };
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(content_inset_host_id, 4, 4, CAP_GUI),
            move |args| {
                let left = expect_number_arg(args, 0, "left")? as f32;
                let top = expect_number_arg(args, 1, "top")? as f32;
                let right = expect_number_arg(args, 2, "right")? as f32;
                let bottom = expect_number_arg(args, 3, "bottom")? as f32;
                message_window.borrow_mut().style.content_inset =
                    UiInsets::new(left.max(0.0), top.max(0.0), right.max(0.0), bottom.max(0.0));
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.input_panel_fill = fill;
                window.style.input_panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_text")?;
                message_window.borrow_mut().style.input_text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_hint_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_hint")?;
                message_window.borrow_mut().style.input_hint_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_prompt_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_prompt")?;
                message_window.borrow_mut().style.input_prompt_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.choice_panel_fill = fill;
                window.style.choice_panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "choice_text")?;
                message_window.borrow_mut().style.choice_text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_accent_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "choice_accent")?;
                message_window.borrow_mut().style.choice_accent_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_selected_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "selected_fill")?;
                let stroke = expect_rgba_args(args, 4, "selected_stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.choice_selected_fill = fill;
                window.style.choice_selected_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(locale_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                if args.is_empty() {
                    return Ok(Value::String(window.locale.clone()));
                }
                let locale = expect_string_arg(args, 0, "locale")?;
                let normalized = locale.trim().to_ascii_lowercase();
                window.locale = if normalized.starts_with("ja") {
                    "ja".to_owned()
                } else {
                    "en".to_owned()
                };
                Ok(Value::String(window.locale.clone()))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.message",
            &[
                ExtensionFunctionSpec::new("show", show_host_id, 1, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("append", append_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices", choices_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices_named", choices_named_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("prompt", prompt_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("hide", hide_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speed", speed_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("auto", auto_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("skip", skip_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("log_clear", log_clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("clear", clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("box_style", box_style_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("text_color", text_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speaker_color", speaker_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("accent_color", accent_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("font_size", font_size_host_id, 2, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset_style", reset_style_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("frame", frame_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("content_inset", content_inset_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_box_style",
                    input_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_text_color",
                    input_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_hint_color",
                    input_hint_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_prompt_color",
                    input_prompt_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_box_style",
                    choice_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_text_color",
                    choice_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_accent_color",
                    choice_accent_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_selected_style",
                    choice_selected_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("locale", locale_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::String),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "text",
            &[
                ExtensionFunctionSpec::new("show", show_host_id, 1, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("append", append_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices", choices_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices_named", choices_named_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("prompt", prompt_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("hide", hide_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speed", speed_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("auto", auto_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("skip", skip_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("log_clear", log_clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("clear", clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("box_style", box_style_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("text_color", text_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speaker_color", speaker_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("accent_color", accent_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("font_size", font_size_host_id, 2, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset_style", reset_style_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("frame", frame_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("content_inset", content_inset_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_box_style",
                    input_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_text_color",
                    input_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_hint_color",
                    input_hint_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_prompt_color",
                    input_prompt_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_box_style",
                    choice_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_text_color",
                    choice_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_accent_color",
                    choice_accent_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_selected_style",
                    choice_selected_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("locale", locale_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(MessageExtension {
            show_ext_id: ids[0],
            append_ext_id: ids[1],
            choices_ext_id: ids[2],
            choices_named_ext_id: ids[3],
            prompt_ext_id: ids[4],
            hide_ext_id: ids[5],
            speed_ext_id: ids[6],
            auto_ext_id: ids[7],
            skip_ext_id: ids[8],
            log_clear_ext_id: ids[9],
            clear_ext_id: ids[10],
            box_style_ext_id: ids[11],
            text_color_ext_id: ids[12],
            speaker_color_ext_id: ids[13],
            accent_color_ext_id: ids[14],
            font_size_ext_id: ids[15],
            reset_style_ext_id: ids[16],
            frame_ext_id: ids[17],
            content_inset_ext_id: ids[18],
            input_box_style_ext_id: ids[19],
            input_text_color_ext_id: ids[20],
            input_hint_color_ext_id: ids[21],
            input_prompt_color_ext_id: ids[22],
            choice_box_style_ext_id: ids[23],
            choice_text_color_ext_id: ids[24],
            choice_accent_color_ext_id: ids[25],
            choice_selected_style_ext_id: ids[26],
            locale_ext_id: ids[27],
            show_host_id,
            append_host_id,
            choices_host_id,
            choices_named_host_id,
            prompt_host_id,
            hide_host_id,
            speed_host_id,
            auto_host_id,
            skip_host_id,
            log_clear_host_id,
            clear_host_id,
            box_style_host_id,
            text_color_host_id,
            speaker_color_host_id,
            accent_color_host_id,
            font_size_host_id,
            reset_style_host_id,
            frame_host_id,
            content_inset_host_id,
            input_box_style_host_id,
            input_text_color_host_id,
            input_hint_color_host_id,
            input_prompt_color_host_id,
            choice_box_style_host_id,
            choice_text_color_host_id,
            choice_accent_color_host_id,
            choice_selected_style_host_id,
            locale_host_id,
        })
    }
    pub fn install_scene_extension(&mut self) -> Result<SceneExtension, RuntimeError> {
        let layout_host_id = 180;
        let reset_host_id = 181;
        let z_index_host_id = 182;
        let opening_host_id = 183;
        let ending_host_id = 184;
        let background_host_id = 185;
        let scene_layout = self.scene_layout.clone();
        let _ = self.register_host_function(
            HostFunction::new(layout_host_id, 8, 8, CAP_GUI),
            move |args| {
                let mut layout = scene_layout.borrow_mut();
                layout.choice_panel = UiRect::new(
                    expect_number_arg(args, 0, "choice_x")? as f32,
                    expect_number_arg(args, 1, "choice_y")? as f32,
                    expect_number_arg(args, 2, "choice_width")? as f32,
                    expect_number_arg(args, 3, "choice_height")? as f32,
                );
                layout.message_window = UiRect::new(
                    expect_number_arg(args, 4, "message_x")? as f32,
                    expect_number_arg(args, 5, "message_y")? as f32,
                    expect_number_arg(args, 6, "message_width")? as f32,
                    expect_number_arg(args, 7, "message_height")? as f32,
                );
                Ok(Value::Bool(true))
            },
        );
        let scene_layout = self.scene_layout.clone();
        let _ = self.register_host_function(
            HostFunction::new(z_index_host_id, 3, 3, CAP_GUI),
            move |args| {
                let mut layout = scene_layout.borrow_mut();
                layout.choice_panel_z = expect_integer_arg(args, 0, "choice_panel_z")? as i32;
                layout.input_panel_z = expect_integer_arg(args, 1, "input_panel_z")? as i32;
                layout.message_window_z = expect_integer_arg(args, 2, "message_window_z")? as i32;
                Ok(Value::Bool(true))
            },
        );
        let scene_layout = self.scene_layout.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(reset_host_id, 0, 0, CAP_GUI),
            move |_args| {
                *scene_layout.borrow_mut() = UiSceneLayoutState::default();
                image_draws.borrow_mut().clear();
                icon_sheets.borrow_mut().clear();
                *message_window.borrow_mut() = MessageWindowState::default();
                Ok(Value::Bool(true))
            },
        );
        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(opening_host_id, 1, 1, CAP_GUI),
            move |args| {
                let title = expect_string_arg(args, 0, "title")?;
                let mut window = message_window.borrow_mut();
                let speaker = if window.locale.starts_with("ja") {
                    "オープニング"
                } else {
                    "Opening"
                };
                window.visible = true;
                window.speaker = Some(speaker.to_owned());
                window.text = title.clone();
                window
                    .backlog
                    .extend(title.lines().map(|line| line.to_owned()));
                window.input_prompt = None;
                window.choices.clear();
                Ok(Value::Bool(true))
            },
        );
        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(ending_host_id, 1, 1, CAP_GUI),
            move |args| {
                let title = expect_string_arg(args, 0, "title")?;
                let mut window = message_window.borrow_mut();
                let speaker = if window.locale.starts_with("ja") {
                    "エンディング"
                } else {
                    "Ending"
                };
                window.visible = true;
                window.speaker = Some(speaker.to_owned());
                window.text = title.clone();
                window
                    .backlog
                    .extend(title.lines().map(|line| line.to_owned()));
                window.input_prompt = None;
                window.choices.clear();
                Ok(Value::Bool(true))
            },
        );
        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let scene_layout = self.scene_layout.clone();
        let _ = self.register_host_function(
            HostFunction::new(background_host_id, 1, 1, CAP_GUI),
            move |args| {
                let resource_id = expect_integer_arg(args, 0, "resource_id")? as u32;
                let handle = match resources
                    .borrow_mut()
                    .load_resource(resource_id)
                    .map_err(resource_error_to_host_error)?
                {
                    LoadResult::Ready(handle) => handle,
                    LoadResult::Pending(request_id) => {
                        return Ok(Value::Integer(request_id as i64));
                    }
                };
                let layout = scene_layout.borrow();
                let size = layout.reference_size;
                let mut draws = image_draws.borrow_mut();
                draws.retain(|draw| {
                    draw.x != 0.0
                        || draw.y != 0.0
                        || draw.width != Some(size.width)
                        || draw.height != Some(size.height)
                });
                draws.insert(
                    0,
                    ImageDrawState {
                        handle: handle.raw(),
                        resource_id,
                        x: 0.0,
                        y: 0.0,
                        width: Some(size.width),
                        height: Some(size.height),
                        source: None,
                        icon_sheet: None,
                        icon_index: None,
                        rotation_degrees: 0.0,
                        opacity: 1.0,
                    },
                );
                Ok(Value::Bool(true))
            },
        );
        let ids = self.extensions.register_extension(
            "ext.scene",
            &[
                ExtensionFunctionSpec::new("layout", layout_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset", reset_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("z_index", z_index_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("opening", opening_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("ending", ending_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("background", background_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "ui",
            &[
                ExtensionFunctionSpec::new("layout", layout_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset", reset_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("z_index", z_index_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("opening", opening_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("ending", ending_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("background", background_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        Ok(SceneExtension {
            layout_ext_id: ids[0],
            reset_ext_id: ids[1],
            z_index_ext_id: ids[2],
            opening_ext_id: ids[3],
            ending_ext_id: ids[4],
            background_ext_id: ids[5],
            layout_host_id,
            reset_host_id,
            z_index_host_id,
            opening_host_id,
            ending_host_id,
            background_host_id,
        })
    }

    pub fn install_image_extension(&mut self) -> Result<ImageExtension, RuntimeError> {
        let load_host_id = 140;
        let info_host_id = 141;
        let status_host_id = 142;
        let release_host_id = 143;
        let draw_host_id = 144;
        let draw_part_host_id = 145;
        let draw_ext_host_id = 146;
        let set_icon_sheet_host_id = 147;
        let draw_icon_host_id = 148;
        let resources = self.resources.clone();

        let _ = self.register_host_function(
            HostFunction::new(load_host_id, 1, 1, CAP_GUI),
            move |args| {
                let resource_id = expect_integer_arg(args, 0, "resource_id")? as u32;
                match resources
                    .borrow_mut()
                    .load_resource(resource_id)
                    .map_err(resource_error_to_host_error)?
                {
                    LoadResult::Ready(handle) => Ok(Value::Handle(handle.into())),
                    LoadResult::Pending(request_id) => Ok(Value::Integer(request_id as i64)),
                }
            },
        );

        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(info_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let resources = resources.borrow();
                let resource_id = resources
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                let entry = resources.entry(resource_id).ok_or_else(|| {
                    HostError::Failed(format!("missing resource entry {resource_id}"))
                })?;
                Ok(make_table(&[
                    (1, Value::Integer(resource_id as i64)),
                    (
                        2,
                        Value::Integer(resource_type_value(
                            entry
                                .data
                                .as_ref()
                                .map(|data| match data {
                                    wmresource::ResourceData::Image(_) => ResourceType::Image,
                                    wmresource::ResourceData::Audio(_) => ResourceType::Audio,
                                    wmresource::ResourceData::Binary(_) => ResourceType::Binary,
                                    wmresource::ResourceData::Font(_) => ResourceType::Font,
                                    wmresource::ResourceData::Video(_) => ResourceType::Video,
                                    wmresource::ResourceData::ScriptData(_) => {
                                        ResourceType::ScriptData
                                    }
                                })
                                .unwrap_or(ResourceType::Unknown(0)),
                        )),
                    ),
                    (
                        3,
                        Value::Integer(
                            entry
                                .data
                                .as_ref()
                                .map_or(0, |data| data.bytes().len() as i64),
                        ),
                    ),
                    (4, Value::Integer(resource_state_code(entry.state))),
                ]))
            },
        );

        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(status_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let state = resources
                    .borrow()
                    .status(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Integer(resource_state_code(state)))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(release_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                image_draws
                    .borrow_mut()
                    .retain(|draw| draw.handle != handle.raw());
                icon_sheets.borrow_mut().remove(&handle.raw());
                resources
                    .borrow_mut()
                    .release(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_host_id, 3, 3, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let x = expect_number_arg(args, 1, "x")? as f32;
                let y = expect_number_arg(args, 2, "y")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x,
                    y,
                    width: None,
                    height: None,
                    source: None,
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_part_host_id, 7, 7, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let sx = expect_number_arg(args, 1, "sx")? as f32;
                let sy = expect_number_arg(args, 2, "sy")? as f32;
                let sw = expect_number_arg(args, 3, "sw")? as f32;
                let sh = expect_number_arg(args, 4, "sh")? as f32;
                let dx = expect_number_arg(args, 5, "dx")? as f32;
                let dy = expect_number_arg(args, 6, "dy")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x: dx,
                    y: dy,
                    width: Some(sw),
                    height: Some(sh),
                    source: Some(ImageSourceRect {
                        x: sx,
                        y: sy,
                        width: sw,
                        height: sh,
                    }),
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_ext_host_id, 11, 11, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let sx = expect_number_arg(args, 1, "sx")? as f32;
                let sy = expect_number_arg(args, 2, "sy")? as f32;
                let sw = expect_number_arg(args, 3, "sw")? as f32;
                let sh = expect_number_arg(args, 4, "sh")? as f32;
                let dx = expect_number_arg(args, 5, "dx")? as f32;
                let dy = expect_number_arg(args, 6, "dy")? as f32;
                let dw = expect_number_arg(args, 7, "dw")? as f32;
                let dh = expect_number_arg(args, 8, "dh")? as f32;
                let rot = expect_number_arg(args, 9, "rot")? as f32;
                let alpha = expect_number_arg(args, 10, "alpha")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x: dx,
                    y: dy,
                    width: Some(dw),
                    height: Some(dh),
                    source: Some(ImageSourceRect {
                        x: sx,
                        y: sy,
                        width: sw,
                        height: sh,
                    }),
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: rot,
                    opacity: alpha.clamp(0.0, 1.0),
                });
                Ok(Value::Bool(true))
            },
        );

        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(set_icon_sheet_host_id, 3, 3, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let cell_w = expect_integer_arg(args, 1, "cell_w")? as u32;
                let cell_h = expect_integer_arg(args, 2, "cell_h")? as u32;
                icon_sheets.borrow_mut().insert(
                    handle.raw(),
                    IconSheetState {
                        cell_width: cell_w,
                        cell_height: cell_h,
                    },
                );
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_icon_host_id, 4, 4, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let index = expect_integer_arg(args, 1, "index")? as u32;
                let x = expect_number_arg(args, 2, "x")? as f32;
                let y = expect_number_arg(args, 3, "y")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                let icon_sheet = icon_sheets
                    .borrow()
                    .get(&handle.raw())
                    .cloned()
                    .ok_or_else(|| {
                        HostError::Failed(format!("missing icon sheet for handle {}", handle.raw()))
                    })?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x,
                    y,
                    width: Some(icon_sheet.cell_width as f32),
                    height: Some(icon_sheet.cell_height as f32),
                    source: None,
                    icon_sheet: Some(icon_sheet),
                    icon_index: Some(index),
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.image",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("info", info_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Unknown),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw", draw_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_part", draw_part_host_id, 7, 7, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_ext", draw_ext_host_id, 11, 11, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("set_icon_sheet", set_icon_sheet_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_icon", draw_icon_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "img",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("info", info_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Unknown),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw", draw_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_part", draw_part_host_id, 7, 7, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_ext", draw_ext_host_id, 11, 11, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("set_icon_sheet", set_icon_sheet_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_icon", draw_icon_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "asset",
            &[
                ExtensionFunctionSpec::new("request", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("preload", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(ImageExtension {
            load_ext_id: ids[0],
            info_ext_id: ids[1],
            status_ext_id: ids[2],
            release_ext_id: ids[3],
            draw_ext_id: ids[4],
            draw_part_ext_id: ids[5],
            draw_ext_ext_id: ids[6],
            set_icon_sheet_ext_id: ids[7],
            draw_icon_ext_id: ids[8],
            load_host_id,
            info_host_id,
            status_host_id,
            release_host_id,
            draw_host_id,
            draw_part_host_id,
            draw_ext_host_id,
            set_icon_sheet_host_id,
            draw_icon_host_id,
        })
    }

    pub fn install_audio_extension(&mut self) -> Result<AudioExtension, RuntimeError> {
        let load_host_id = 150;
        let play_host_id = 151;
        let pause_host_id = 152;
        let stop_host_id = 153;
        let seek_host_id = 154;
        let volume_host_id = 155;
        let release_host_id = 156;
        let status_host_id = 157;
        let playback_host_id = 158;
        let resources = self.resources.clone();
        let audio_states = self.audio_states.clone();

        let _ = self.register_host_function(
            HostFunction::new(load_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let resource_id = expect_integer_arg(args, 0, "resource_id")? as u32;
                match resources
                    .borrow_mut()
                    .load_resource(resource_id)
                    .map_err(resource_error_to_host_error)?
                {
                    LoadResult::Ready(handle) => {
                        audio_states.borrow_mut().insert(
                            handle.raw(),
                            AudioPlaybackState {
                                resource_id,
                                playing: false,
                                looped: false,
                                position_ms: 0,
                                volume: 1.0,
                            },
                        );
                        Ok(Value::Handle(handle.into()))
                    }
                    LoadResult::Pending(request_id) => Ok(Value::Integer(request_id as i64)),
                }
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(play_host_id, 1, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let looped = args.get(1).map(|value| value.truthy()).unwrap_or(false);
                let (resource_id, bytes) = audio_bytes_for_handle(&resources, handle)?;
                let backend = audio_backend.clone();
                let mut states = audio_states.borrow_mut();
                let state = states.entry(handle).or_insert_with(|| AudioPlaybackState {
                    resource_id,
                    playing: false,
                    looped: false,
                    position_ms: 0,
                    volume: 1.0,
                });
                backend.play(
                    handle,
                    resource_id,
                    &bytes,
                    looped,
                    state.position_ms,
                    state.volume,
                )?;
                state.resource_id = resource_id;
                state.playing = true;
                state.looped = looped;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(playback_host_id, 1, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let looped = args.get(1).map(|value| value.truthy()).unwrap_or(false);
                let (resource_id, bytes) = audio_bytes_for_handle(&resources, handle)?;
                let backend = audio_backend.clone();
                let mut states = audio_states.borrow_mut();
                let state = states.entry(handle).or_insert_with(|| AudioPlaybackState {
                    resource_id,
                    playing: false,
                    looped: false,
                    position_ms: 0,
                    volume: 1.0,
                });
                backend.play(
                    handle,
                    resource_id,
                    &bytes,
                    looped,
                    state.position_ms,
                    state.volume,
                )?;
                state.resource_id = resource_id;
                state.playing = true;
                state.looped = looped;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(pause_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                audio_backend.pause(handle)?;
                if let Some(state) = audio_states.borrow_mut().get_mut(&handle) {
                    state.playing = false;
                }
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(stop_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                audio_backend.stop(handle)?;
                if let Some(state) = audio_states.borrow_mut().get_mut(&handle) {
                    state.playing = false;
                    state.position_ms = 0;
                }
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(seek_host_id, 2, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let position_ms = expect_number_arg(args, 1, "position_ms")?;
                audio_backend.seek(handle, position_ms.max(0.0) as u64)?;
                let mut states = audio_states.borrow_mut();
                let state = states
                    .entry(handle)
                    .or_insert_with(AudioPlaybackState::default);
                state.position_ms = position_ms.max(0.0) as u64;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(volume_host_id, 2, 2, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let volume = expect_number_arg(args, 1, "volume")?;
                audio_backend.volume(handle, volume.clamp(0.0, 1.0) as f32)?;
                let mut states = audio_states.borrow_mut();
                let state = states
                    .entry(handle)
                    .or_insert_with(AudioPlaybackState::default);
                state.volume = volume.clamp(0.0, 1.0) as f32;
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let _ = self.register_host_function(
            HostFunction::new(release_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                audio_backend.release(handle.raw())?;
                audio_states.borrow_mut().remove(&handle.raw());
                resources
                    .borrow_mut()
                    .release(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Bool(true))
            },
        );

        let audio_states = self.audio_states.clone();
        let _ = self.register_host_function(
            HostFunction::new(status_host_id, 1, 1, CAP_ASYNC_IO),
            move |args| {
                let handle = expect_handle_arg(args, 0, "handle")?;
                let status = audio_states
                    .borrow()
                    .get(&handle)
                    .map(|state| if state.playing { 2 } else { 1 })
                    .unwrap_or(0);
                Ok(Value::Integer(status))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.audio",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_ASYNC_IO),
                ExtensionFunctionSpec::new("play", play_host_id, 1, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("playback", playback_host_id, 1, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("pause", pause_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("stop", stop_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("seek", seek_host_id, 2, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("volume", volume_host_id, 2, 2, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_ASYNC_IO)
                    .with_return_type(ExtValueType::Integer),
            ],
        )?;

        Ok(AudioExtension {
            load_ext_id: ids[0],
            play_ext_id: ids[1],
            playback_ext_id: ids[2],
            pause_ext_id: ids[3],
            stop_ext_id: ids[4],
            seek_ext_id: ids[5],
            volume_ext_id: ids[6],
            release_ext_id: ids[7],
            status_ext_id: ids[8],
            load_host_id,
            play_host_id,
            playback_host_id,
            pause_host_id,
            stop_host_id,
            seek_host_id,
            volume_host_id,
            release_host_id,
            status_host_id,
        })
    }

    pub fn install_vm_extension(&mut self) -> Result<VmExtension, RuntimeError> {
        let save_host_id = 160;
        let load_host_id = 161;
        let scheduler = self.scheduler.clone();
        let resources = self.resources.clone();
        let debug_log = self.debug_log.clone();
        let loaded_archives = self.loaded_archives.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let scene_layout = self.scene_layout.clone();
        let message_window = self.message_window.clone();
        let ui_policy = self.ui_policy.clone();
        let audio_states = self.audio_states.clone();
        let state_manager = self.state_manager.clone();
        let checkpoints = self.checkpoints.clone();
        let pending_vm_saves = self.pending_vm_saves.clone();
        let host = self.host.clone();

        let _ =
            self.register_host_function(HostFunction::new(save_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                if let Ok(scheduler) = scheduler.try_borrow() {
                    checkpoints.borrow_mut().insert(
                        slot,
                        RuntimeCheckpoint {
                            scheduler: scheduler.snapshot(),
                            resources: resources.borrow().clone(),
                            loaded_archives: loaded_archives.borrow().clone(),
                            image_draws: image_draws.borrow().clone(),
                            icon_sheets: icon_sheets.borrow().clone(),
                            scene_layout: scene_layout.borrow().clone(),
                            message_window: message_window.borrow().clone(),
                            ui_policy: ui_policy.borrow().clone(),
                            debug_log: debug_log.borrow().clone(),
                            audio_states: audio_states.borrow().clone(),
                            state_manager: state_manager.borrow().clone(),
                        },
                    );
                } else {
                    pending_vm_saves.borrow_mut().push(slot);
                }
                Ok(Value::Bool(true))
            });

        let scheduler = self.scheduler.clone();
        let resources = self.resources.clone();
        let debug_log = self.debug_log.clone();
        let loaded_archives = self.loaded_archives.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let scene_layout = self.scene_layout.clone();
        let message_window = self.message_window.clone();
        let ui_policy = self.ui_policy.clone();
        let audio_states = self.audio_states.clone();
        let audio_backend = self.audio_backend.clone();
        let state_manager = self.state_manager.clone();
        let checkpoints = self.checkpoints.clone();
        let host_for_load = host.clone();
        let _ =
            self.register_host_function(HostFunction::new(load_host_id, 1, 1, 0), move |args| {
                let slot = expect_integer_arg(args, 0, "slot")? as u32;
                let Some(checkpoint) = checkpoints.borrow().get(&slot).cloned() else {
                    return Ok(Value::Bool(false));
                };
                *scheduler.borrow_mut() = Scheduler::from_snapshot(checkpoint.scheduler, |_| {
                    Box::new(SharedHostApi::new(host_for_load.clone()))
                });
                *resources.borrow_mut() = checkpoint.resources;
                *loaded_archives.borrow_mut() = checkpoint.loaded_archives;
                *image_draws.borrow_mut() = checkpoint.image_draws;
                *icon_sheets.borrow_mut() = checkpoint.icon_sheets;
                *scene_layout.borrow_mut() = checkpoint.scene_layout;
                *message_window.borrow_mut() = checkpoint.message_window;
                *ui_policy.borrow_mut() = checkpoint.ui_policy;
                *debug_log.borrow_mut() = checkpoint.debug_log;
                *audio_states.borrow_mut() = checkpoint.audio_states;
                *state_manager.borrow_mut() = checkpoint.state_manager;
                {
                    let backend = audio_backend.clone();
                    backend.clear()?;
                    let replay_states = audio_states
                        .borrow()
                        .iter()
                        .filter(|(_, state)| state.playing)
                        .map(|(handle, state)| (*handle, state.clone()))
                        .collect::<Vec<_>>();
                    for (handle, state) in replay_states {
                        let bytes = audio_bytes_for_resource_id(&resources, state.resource_id)?;
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
                Ok(Value::Bool(true))
            });

        let ids = self.extensions.register_extension(
            "ext.vm",
            &[
                ExtensionFunctionSpec::new("save", save_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(VmExtension {
            save_ext_id: ids[0],
            load_ext_id: ids[1],
            save_host_id,
            load_host_id,
        })
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
            vm: self.install_vm_extension()?,
            state: self.install_state_extension()?,
        })
    }

    fn install_ui_extension(&mut self) -> Result<UiExtension, RuntimeError> {
        let context_menu_host_id = 240;
        let shift_fast_host_id = 241;
        let policy = self.ui_policy.clone();
        let _ = self.register_host_function(
            HostFunction::new(context_menu_host_id, 1, 1, CAP_GUI),
            move |args| {
                policy.borrow_mut().context_menu_enabled = expect_bool_arg(args, 0, "enabled")?;
                Ok(Value::Bool(true))
            },
        );
        let policy = self.ui_policy.clone();
        let _ = self.register_host_function(
            HostFunction::new(shift_fast_host_id, 1, 1, CAP_GUI),
            move |args| {
                policy.borrow_mut().shift_fast_enabled = expect_bool_arg(args, 0, "enabled")?;
                Ok(Value::Bool(true))
            },
        );
        let ids = self.extensions.register_extension(
            "ext.ui",
            &[
                ExtensionFunctionSpec::new("context_menu", context_menu_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("shift_fast", shift_fast_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "ui",
            &[
                ExtensionFunctionSpec::new("context_menu", context_menu_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("shift_fast", shift_fast_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        Ok(UiExtension {
            context_menu_ext_id: ids[0],
            shift_fast_ext_id: ids[1],
            context_menu_host_id,
            shift_fast_host_id,
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

/// Stable ids assigned to the built-in file system extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FsExtension {
    pub read_ext_id: u32,
    pub write_ext_id: u32,
    pub exists_ext_id: u32,
    pub read_host_id: HostId,
    pub write_host_id: HostId,
    pub exists_host_id: HostId,
}

/// Stable ids assigned to the built-in debug extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugExtension {
    pub log_ext_id: u32,
    pub inspect_ext_id: u32,
    pub log_host_id: HostId,
    pub inspect_host_id: HostId,
}

/// Stable ids assigned to the built-in net extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetExtension {
    pub get_ext_id: u32,
    pub post_ext_id: u32,
    pub get_host_id: HostId,
    pub post_host_id: HostId,
}

/// Stable ids assigned to the built-in llm extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LlmExtension {
    pub generate_ext_id: u32,
    pub generate_host_id: HostId,
}

/// Stable ids assigned to the built-in message extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageExtension {
    pub show_ext_id: u32,
    pub append_ext_id: u32,
    pub choices_ext_id: u32,
    pub choices_named_ext_id: u32,
    pub prompt_ext_id: u32,
    pub hide_ext_id: u32,
    pub speed_ext_id: u32,
    pub auto_ext_id: u32,
    pub skip_ext_id: u32,
    pub log_clear_ext_id: u32,
    pub clear_ext_id: u32,
    pub box_style_ext_id: u32,
    pub text_color_ext_id: u32,
    pub speaker_color_ext_id: u32,
    pub accent_color_ext_id: u32,
    pub font_size_ext_id: u32,
    pub reset_style_ext_id: u32,
    pub frame_ext_id: u32,
    pub content_inset_ext_id: u32,
    pub input_box_style_ext_id: u32,
    pub input_text_color_ext_id: u32,
    pub input_hint_color_ext_id: u32,
    pub input_prompt_color_ext_id: u32,
    pub choice_box_style_ext_id: u32,
    pub choice_text_color_ext_id: u32,
    pub choice_accent_color_ext_id: u32,
    pub choice_selected_style_ext_id: u32,
    pub locale_ext_id: u32,
    pub show_host_id: HostId,
    pub append_host_id: HostId,
    pub choices_host_id: HostId,
    pub choices_named_host_id: HostId,
    pub prompt_host_id: HostId,
    pub hide_host_id: HostId,
    pub speed_host_id: HostId,
    pub auto_host_id: HostId,
    pub skip_host_id: HostId,
    pub log_clear_host_id: HostId,
    pub clear_host_id: HostId,
    pub box_style_host_id: HostId,
    pub text_color_host_id: HostId,
    pub speaker_color_host_id: HostId,
    pub accent_color_host_id: HostId,
    pub font_size_host_id: HostId,
    pub reset_style_host_id: HostId,
    pub frame_host_id: HostId,
    pub content_inset_host_id: HostId,
    pub input_box_style_host_id: HostId,
    pub input_text_color_host_id: HostId,
    pub input_hint_color_host_id: HostId,
    pub input_prompt_color_host_id: HostId,
    pub choice_box_style_host_id: HostId,
    pub choice_text_color_host_id: HostId,
    pub choice_accent_color_host_id: HostId,
    pub choice_selected_style_host_id: HostId,
    pub locale_host_id: HostId,
}

/// Stable ids assigned to the built-in scene extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneExtension {
    pub layout_ext_id: u32,
    pub reset_ext_id: u32,
    pub z_index_ext_id: u32,
    pub opening_ext_id: u32,
    pub ending_ext_id: u32,
    pub background_ext_id: u32,
    pub layout_host_id: HostId,
    pub reset_host_id: HostId,
    pub z_index_host_id: HostId,
    pub opening_host_id: HostId,
    pub ending_host_id: HostId,
    pub background_host_id: HostId,
}

/// Stable ids assigned to the built-in image extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageExtension {
    pub load_ext_id: u32,
    pub info_ext_id: u32,
    pub status_ext_id: u32,
    pub release_ext_id: u32,
    pub draw_ext_id: u32,
    pub draw_part_ext_id: u32,
    pub draw_ext_ext_id: u32,
    pub set_icon_sheet_ext_id: u32,
    pub draw_icon_ext_id: u32,
    pub load_host_id: HostId,
    pub info_host_id: HostId,
    pub status_host_id: HostId,
    pub release_host_id: HostId,
    pub draw_host_id: HostId,
    pub draw_part_host_id: HostId,
    pub draw_ext_host_id: HostId,
    pub set_icon_sheet_host_id: HostId,
    pub draw_icon_host_id: HostId,
}

/// Stable ids assigned to the built-in audio extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioExtension {
    pub load_ext_id: u32,
    pub play_ext_id: u32,
    pub playback_ext_id: u32,
    pub pause_ext_id: u32,
    pub stop_ext_id: u32,
    pub seek_ext_id: u32,
    pub volume_ext_id: u32,
    pub release_ext_id: u32,
    pub status_ext_id: u32,
    pub load_host_id: HostId,
    pub play_host_id: HostId,
    pub playback_host_id: HostId,
    pub pause_host_id: HostId,
    pub stop_host_id: HostId,
    pub seek_host_id: HostId,
    pub volume_host_id: HostId,
    pub release_host_id: HostId,
    pub status_host_id: HostId,
}

/// Stable ids assigned to the built-in VM extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmExtension {
    pub save_ext_id: u32,
    pub load_ext_id: u32,
    pub save_host_id: HostId,
    pub load_host_id: HostId,
}

/// Stable ids assigned to the built-in persistent state extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateExtension {
    pub save_ext_id: u32,
    pub load_ext_id: u32,
    pub has_ext_id: u32,
    pub get_ext_id: u32,
    pub set_ext_id: u32,
    pub erase_ext_id: u32,
    pub save_host_id: HostId,
    pub load_host_id: HostId,
    pub has_host_id: HostId,
    pub get_host_id: HostId,
    pub set_host_id: HostId,
    pub erase_host_id: HostId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiExtension {
    pub context_menu_ext_id: u32,
    pub shift_fast_ext_id: u32,
    pub context_menu_host_id: HostId,
    pub shift_fast_host_id: HostId,
}

/// Stable ids for the runtime's standard extension set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardExtensions {
    pub fs: FsExtension,
    pub debug: DebugExtension,
    pub net: NetExtension,
    pub llm: LlmExtension,
    pub scene: SceneExtension,
    pub message: MessageExtension,
    pub image: ImageExtension,
    pub audio: AudioExtension,
    pub ui: UiExtension,
    pub vm: VmExtension,
    pub state: StateExtension,
}

/// Backend interface for `ext.net`.
pub trait NetBackend {
    fn get(&mut self, url: &str) -> Result<String, HostError>;

    fn post(&mut self, url: &str, body: &str) -> Result<String, HostError>;
}

/// Backend interface for `ext.llm`.
pub trait LlmBackend {
    fn generate(&mut self, prompt: &str) -> Result<String, HostError>;
}

struct DisabledNetBackend;

impl NetBackend for DisabledNetBackend {
    fn get(&mut self, url: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "network backend disabled for GET {url}"
        )))
    }

    fn post(&mut self, url: &str, body: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "network backend disabled for POST {url} with {} bytes",
            body.len()
        )))
    }
}

struct DisabledLlmBackend;

impl LlmBackend for DisabledLlmBackend {
    fn generate(&mut self, prompt: &str) -> Result<String, HostError> {
        Err(HostError::Failed(format!(
            "llm backend disabled for prompt length {}",
            prompt.len()
        )))
    }
}

fn read_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| HostError::Failed(format!("read {path}: {error}")))?;
    Ok(Value::String(contents))
}

fn write_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    let contents = expect_string_arg(args, 1, "contents")?;
    std::fs::write(&path, contents.as_bytes())
        .map_err(|error| HostError::Failed(format!("write {path}: {error}")))?;
    Ok(Value::Bool(true))
}

fn exists_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    Ok(Value::Bool(PathBuf::from(path).exists()))
}

fn make_table(fields: &[(u16, Value)]) -> Value {
    let mut map = BTreeMap::new();
    for (key, value) in fields {
        map.insert(*key, value.clone());
    }
    Value::Table(Rc::new(map))
}

fn resource_state_code(state: ResourceState) -> i64 {
    match state {
        ResourceState::Unloaded => 0,
        ResourceState::Loading => 1,
        ResourceState::Ready => 2,
        ResourceState::Failed => 3,
        ResourceState::Unloading => 4,
    }
}

fn resource_type_value(resource_type: ResourceType) -> i64 {
    resource_type.as_u16() as i64
}

fn resource_error_to_host_error(error: wmresource::ResourceError) -> HostError {
    HostError::Failed(error.to_string())
}

fn expect_string_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<String, HostError> {
    match args.get(index) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be string, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

fn expect_integer_arg(args: &[Value], index: usize, name: &'static str) -> Result<i64, HostError> {
    match args.get(index) {
        Some(Value::Integer(value)) => Ok(*value),
        Some(Value::Bool(true)) => Ok(1),
        Some(Value::Bool(false)) => Ok(0),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be integer, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

fn expect_number_arg(args: &[Value], index: usize, name: &'static str) -> Result<f64, HostError> {
    match args.get(index) {
        Some(Value::Integer(value)) => Ok(*value as f64),
        Some(Value::Float(value)) => Ok(*value),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be numeric, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

fn expect_bool_arg(args: &[Value], index: usize, name: &'static str) -> Result<bool, HostError> {
    match args.get(index) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Integer(value)) => Ok(*value != 0),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be bool, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

fn expect_color_component_arg(args: &[Value], index: usize, name: &str) -> Result<u8, HostError> {
    let value = match args.get(index) {
        Some(Value::Integer(value)) => *value as f64,
        Some(Value::Float(value)) => *value,
        Some(found) => {
            return Err(HostError::InvalidArguments(format!(
                "expected {name} argument {index} to be a number, found {found:?}"
            )));
        }
        None => {
            return Err(HostError::InvalidArguments(format!(
                "missing required argument {name} at index {index}"
            )));
        }
    };
    Ok(value.round().clamp(0.0, 255.0) as u8)
}

fn expect_rgba_args(args: &[Value], start: usize, name: &str) -> Result<UiColorRgba, HostError> {
    Ok(UiColorRgba::new(
        expect_color_component_arg(args, start, &format!("{name}_r"))?,
        expect_color_component_arg(args, start + 1, &format!("{name}_g"))?,
        expect_color_component_arg(args, start + 2, &format!("{name}_b"))?,
        expect_color_component_arg(args, start + 3, &format!("{name}_a"))?,
    ))
}

fn expect_handle_arg(args: &[Value], index: usize, name: &'static str) -> Result<u64, HostError> {
    match args.get(index) {
        Some(Value::Handle(value)) => Ok(*value),
        Some(Value::Integer(value)) if *value >= 0 => Ok(*value as u64),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be a handle, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_owned(),
        Value::Bool(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(values) => format!("array(len={})", values.len()),
        Value::Table(values) => format!("table(len={})", values.len()),
        Value::Handle(v) => format!("handle({v})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wmarchive::{
        ArchiveBuilder, ArchiveSection, ManifestBuilder, ManifestResourceEntry, SectionDigest,
        SectionKind, digest_section,
    };
    use wmresource::ResourceType;
    use wmvm::{Function, Program, RunOutcome, Value};

    #[derive(Default)]
    struct MockNetBackend {
        get_responses: BTreeMap<String, String>,
        post_responses: BTreeMap<(String, String), String>,
        requests: Vec<String>,
    }

    impl MockNetBackend {
        fn with_get(mut self, url: &str, body: &str) -> Self {
            self.get_responses.insert(url.to_owned(), body.to_owned());
            self
        }

        fn with_post(mut self, url: &str, body: &str, response: &str) -> Self {
            self.post_responses
                .insert((url.to_owned(), body.to_owned()), response.to_owned());
            self
        }
    }

    #[derive(Default)]
    struct MockLlmBackend {
        responses: BTreeMap<String, String>,
        prompts: Vec<String>,
    }

    impl MockLlmBackend {
        fn with_response(mut self, prompt: &str, response: &str) -> Self {
            self.responses
                .insert(prompt.to_owned(), response.to_owned());
            self
        }
    }

    impl LlmBackend for MockLlmBackend {
        fn generate(&mut self, prompt: &str) -> Result<String, HostError> {
            self.prompts.push(prompt.to_owned());
            self.responses.get(prompt).cloned().ok_or_else(|| {
                HostError::Failed(format!("missing mock response for prompt {prompt}"))
            })
        }
    }

    impl NetBackend for MockNetBackend {
        fn get(&mut self, url: &str) -> Result<String, HostError> {
            self.requests.push(format!("GET {url}"));
            self.get_responses
                .get(url)
                .cloned()
                .ok_or_else(|| HostError::Failed(format!("missing mock response for GET {url}")))
        }

        fn post(&mut self, url: &str, body: &str) -> Result<String, HostError> {
            self.requests.push(format!("POST {url} {body}"));
            self.post_responses
                .get(&(url.to_owned(), body.to_owned()))
                .cloned()
                .ok_or_else(|| {
                    HostError::Failed(format!(
                        "missing mock response for POST {url} with body {body}"
                    ))
                })
        }
    }

    fn build_asset_payload(resource_id: u32, resource_type: ResourceType, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(24 + data.len());
        bytes.extend_from_slice(&resource_id.to_le_bytes());
        bytes.extend_from_slice(&resource_type.as_u16().to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(24u32).to_le_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    fn build_archive(package_name: &str, assets: &[(u32, ResourceType, &[u8])]) -> Vec<u8> {
        let mut builder = ArchiveBuilder::new();
        let mut manifest_builder = ManifestBuilder::new(package_name, 42, 1);
        for (section_id, (resource_id, resource_type, data)) in assets.iter().enumerate() {
            let section_id = section_id as u32 + 2;
            let payload = build_asset_payload(*resource_id, *resource_type, data);
            manifest_builder = manifest_builder.push_resource_mapping(ManifestResourceEntry::new(
                0x1000 + *resource_id as u64,
                *resource_id,
            ));
            manifest_builder = manifest_builder.push_section_digest(SectionDigest {
                section_id,
                section_kind: SectionKind::Asset,
                flags_canonical: 0,
                unpacked_size: payload.len() as u64,
                digest: digest_section(
                    section_id,
                    SectionKind::Asset,
                    0,
                    payload.len() as u64,
                    &payload,
                ),
            });
            builder =
                builder.push_section(ArchiveSection::new(section_id, SectionKind::Asset, payload));
        }
        builder
            .push_manifest(1, &manifest_builder.build())
            .build()
            .expect("build archive")
    }

    fn handle_from_value(value: Value) -> u64 {
        match value {
            Value::Handle(handle) => handle,
            other => panic!("expected handle, found {other:?}"),
        }
    }

    #[test]
    fn runtime_can_spawn_and_run_program() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let _ = runtime.register_host_function(HostFunction::new(1, 1, 1, 0), |args| {
            Ok(args.first().cloned().unwrap_or(Value::Nil))
        });

        let mut program = Program::new();
        let idx = program.push_constant(Value::String("hello".to_owned()));
        program.insert_function(Function::new(
            1,
            vec![
                0x10,
                idx as u8,
                (idx >> 8) as u8,
                0x71,
                0x01,
                0x00,
                0x01,
                0x01,
            ],
            0,
            0,
        ));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);
        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
    }

    #[test]
    fn runtime_time_control_api_wakes_sleeping_worker() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let mut program = Program::new();
        let value_idx = program.push_constant(Value::Integer(42));
        program.insert_function(Function::new(
            1,
            vec![
                0xA1, // sleep
                0x10,
                value_idx as u8,
                (value_idx >> 8) as u8,
                0x72, // return
            ],
            0,
            0,
        ));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.tick();
        assert!(matches!(
            outcomes.as_slice(),
            [(_, RunOutcome::Sleeping { .. })]
        ));
        assert_eq!(runtime.sleeping_workers(), vec![worker_id]);

        assert!(runtime.wake_worker(worker_id));
        let outcomes = runtime.tick();
        assert!(matches!(
            outcomes.as_slice(),
            [(_, RunOutcome::Halted { .. })]
        ));
        assert!(runtime.sleeping_workers().is_empty());
        assert!(!runtime.wake_worker(9999));
    }

    #[test]
    fn runtime_installs_and_executes_fs_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_fs_extension().expect("install fs");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wml_fs_test_{unique}.txt"));
        let path_text = path.to_string_lossy().to_string();

        let mut program = Program::new();
        let path_idx = program.push_constant(Value::String(path_text.clone()));
        let data_idx = program.push_constant(Value::String("hello fs".to_owned()));
        let code = vec![
            0x10,
            path_idx as u8,
            (path_idx >> 8) as u8,
            0x10,
            data_idx as u8,
            (data_idx >> 8) as u8,
            0x71,
            (extension.write_host_id & 0xFF) as u8,
            (extension.write_host_id >> 8) as u8,
            0x02,
            0x10,
            path_idx as u8,
            (path_idx >> 8) as u8,
            0x71,
            (extension.read_host_id & 0xFF) as u8,
            (extension.read_host_id >> 8) as u8,
            0x01,
            0x72,
        ];
        program.insert_function(Function::new(1, code, 0, 0));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);
        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.fs.read"),
            Ok(extension.read_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.fs.write"),
            Ok(extension.write_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.fs.exists"),
            Ok(extension.exists_ext_id)
        );
        assert_eq!(
            runtime
                .host_registry()
                .function(extension.read_host_id)
                .map(|function| function.required_capabilities),
            Some(CAP_FILE_SYSTEM)
        );
        assert!(matches!(
            outcomes.last(),
            Some((
                _,
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                }
            )) if text == "hello fs"
        ));
        let contents = fs::read_to_string(&path).expect("fs write");
        assert_eq!(contents, "hello fs");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn runtime_installs_and_executes_debug_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_debug_extension().expect("install debug");

        let mut program = Program::new();
        let message_idx = program.push_constant(Value::String("debug me".to_owned()));
        let code = vec![
            0x10,
            message_idx as u8,
            (message_idx >> 8) as u8,
            0x71,
            (extension.log_host_id & 0xFF) as u8,
            (extension.log_host_id >> 8) as u8,
            0x01,
            0x10,
            message_idx as u8,
            (message_idx >> 8) as u8,
            0x71,
            (extension.inspect_host_id & 0xFF) as u8,
            (extension.inspect_host_id >> 8) as u8,
            0x01,
            0x72,
        ];
        program.insert_function(Function::new(1, code, 0, 0));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);

        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.debug.log"),
            Ok(extension.log_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.debug.inspect"),
            Ok(extension.inspect_ext_id)
        );
        assert_eq!(runtime.debug_log(), vec!["debug me".to_owned()]);
        assert!(matches!(
            outcomes.last(),
            Some((
                _,
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                }
            )) if text == "debug me"
        ));
    }

    #[test]
    fn runtime_installs_and_executes_net_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_net_extension().expect("install net");
        runtime.set_net_backend(
            MockNetBackend::default()
                .with_get("https://example.test/api", "net body")
                .with_post("https://example.test/api", "payload", "posted response"),
        );

        let mut program = Program::new();
        let url_idx = program.push_constant(Value::String("https://example.test/api".to_owned()));
        let body_idx = program.push_constant(Value::String("payload".to_owned()));
        let code = vec![
            0x10,
            url_idx as u8,
            (url_idx >> 8) as u8,
            0x71,
            (extension.get_host_id & 0xFF) as u8,
            (extension.get_host_id >> 8) as u8,
            0x01,
            0x10,
            url_idx as u8,
            (url_idx >> 8) as u8,
            0x10,
            body_idx as u8,
            (body_idx >> 8) as u8,
            0x71,
            (extension.post_host_id & 0xFF) as u8,
            (extension.post_host_id >> 8) as u8,
            0x02,
            0x72,
        ];
        program.insert_function(Function::new(1, code, 0, 0));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);

        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.net.get"),
            Ok(extension.get_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.net.post"),
            Ok(extension.post_ext_id)
        );
        assert!(matches!(
            outcomes.last(),
            Some((
                _,
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                }
            )) if text == "posted response"
        ));
    }

    #[test]
    fn runtime_installs_and_executes_llm_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_llm_extension().expect("install llm");
        runtime
            .set_llm_backend(MockLlmBackend::default().with_response("hello model", "model reply"));

        let mut program = Program::new();
        let prompt_idx = program.push_constant(Value::String("hello model".to_owned()));
        let code = vec![
            0x10,
            prompt_idx as u8,
            (prompt_idx >> 8) as u8,
            0x71,
            (extension.generate_host_id & 0xFF) as u8,
            (extension.generate_host_id >> 8) as u8,
            0x01,
            0x72,
        ];
        program.insert_function(Function::new(1, code, 0, 0));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);

        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.llm.generate"),
            Ok(extension.generate_ext_id)
        );
        assert!(matches!(
            outcomes.last(),
            Some((
                _,
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                }
            )) if text == "model reply"
        ));
    }

    #[test]
    fn runtime_installs_and_executes_message_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime
            .install_message_extension()
            .expect("install message");

        let mut program = Program::new();
        let speaker_idx = program.push_constant(Value::String("Narrator".to_owned()));
        let text_idx = program.push_constant(Value::String("Hello world".to_owned()));
        let choice_a_idx = program.push_constant(Value::String("Continue".to_owned()));
        let choice_b_idx = program.push_constant(Value::String("Back".to_owned()));
        let prompt_idx = program.push_constant(Value::String("Your name?".to_owned()));
        let code = vec![
            0x10,
            speaker_idx as u8,
            (speaker_idx >> 8) as u8,
            0x10,
            text_idx as u8,
            (text_idx >> 8) as u8,
            0x71,
            (extension.show_host_id & 0xFF) as u8,
            (extension.show_host_id >> 8) as u8,
            0x02,
            0x10,
            choice_a_idx as u8,
            (choice_a_idx >> 8) as u8,
            0x10,
            choice_b_idx as u8,
            (choice_b_idx >> 8) as u8,
            0x71,
            (extension.choices_host_id & 0xFF) as u8,
            (extension.choices_host_id >> 8) as u8,
            0x02,
            0x10,
            prompt_idx as u8,
            (prompt_idx >> 8) as u8,
            0x71,
            (extension.prompt_host_id & 0xFF) as u8,
            (extension.prompt_host_id >> 8) as u8,
            0x01,
            0x72,
        ];
        program.insert_function(Function::new(1, code, 0, 0));
        program.set_entry(1);

        let worker_id = runtime.spawn_program(program).expect("spawn");
        let outcomes = runtime.run_until_idle(8);

        assert_eq!(worker_id, 1);
        assert!(!outcomes.is_empty());
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.message.show"),
            Ok(extension.show_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.choices"),
            Ok(extension.choices_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.choices_named"),
            Ok(extension.choices_named_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.prompt"),
            Ok(extension.prompt_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.message.speed"),
            Ok(extension.speed_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.message.auto"),
            Ok(extension.auto_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.message.skip"),
            Ok(extension.skip_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.log_clear"),
            Ok(extension.log_clear_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.box_style"),
            Ok(extension.box_style_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.text_color"),
            Ok(extension.text_color_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.speaker_color"),
            Ok(extension.speaker_color_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.accent_color"),
            Ok(extension.accent_color_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.font_size"),
            Ok(extension.font_size_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.reset_style"),
            Ok(extension.reset_style_ext_id)
        );
        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.message.locale"),
            Ok(extension.locale_ext_id)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.speed_host_id, &[Value::Float(24.0)])
                .expect("message speed"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.auto_host_id, &[Value::Bool(true)])
                .expect("message auto"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.skip_host_id, &[Value::Bool(false)])
                .expect("message skip"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.box_style_host_id,
                    &[
                        Value::Integer(12),
                        Value::Integer(18),
                        Value::Integer(24),
                        Value::Integer(230),
                        Value::Integer(120),
                        Value::Integer(180),
                        Value::Integer(90),
                        Value::Integer(255),
                    ],
                )
                .expect("message box style"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.text_color_host_id,
                    &[
                        Value::Integer(240),
                        Value::Integer(245),
                        Value::Integer(250),
                        Value::Integer(255),
                    ],
                )
                .expect("message text color"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.speaker_color_host_id,
                    &[
                        Value::Integer(255),
                        Value::Integer(232),
                        Value::Integer(160),
                        Value::Integer(255),
                    ],
                )
                .expect("message speaker color"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.accent_color_host_id,
                    &[
                        Value::Integer(170),
                        Value::Integer(220),
                        Value::Integer(255),
                        Value::Integer(255),
                    ],
                )
                .expect("message accent color"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.locale_host_id, &[Value::String("en".to_owned())])
                .expect("message locale"),
            Value::String("en".to_owned())
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.locale_host_id, &[])
                .expect("message locale get"),
            Value::String("en".to_owned())
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.font_size_host_id,
                    &[Value::Float(19.0), Value::Float(23.0)],
                )
                .expect("message font size"),
            Value::Bool(true)
        );
        let message = runtime.message_window_state();
        assert!(message.visible);
        assert_eq!(message.speaker.as_deref(), Some("Narrator"));
        assert_eq!(message.text, "Hello world");
        assert_eq!(message.choices.len(), 2);
        assert_eq!(message.choices[0].label, "Continue");
        assert_eq!(message.input_prompt.as_deref(), Some("Your name?"));
        assert_eq!(message.text_speed, 24.0);
        assert!(message.auto_mode);
        assert!(!message.skip_mode);
        assert_eq!(message.style.panel_fill, UiColorRgba::new(12, 18, 24, 230));
        assert_eq!(
            message.style.panel_stroke,
            UiColorRgba::new(120, 180, 90, 255)
        );
        assert_eq!(
            message.style.text_color,
            UiColorRgba::new(240, 245, 250, 255)
        );
        assert_eq!(
            message.style.speaker_color,
            UiColorRgba::new(255, 232, 160, 255)
        );
        assert_eq!(
            message.style.accent_color,
            UiColorRgba::new(170, 220, 255, 255)
        );
        assert_eq!(message.style.body_font_size, 19.0);
        assert_eq!(message.style.speaker_font_size, 23.0);
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.choices_named_host_id,
                    &[
                        Value::String("prologue".to_owned()),
                        Value::String("Prologue".to_owned()),
                        Value::String("chapter_1".to_owned()),
                        Value::String("Chapter 1".to_owned()),
                    ],
                )
                .expect("message choices named"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.prompt_host_id, &[])
                .expect("message prompt clear"),
            Value::Bool(true)
        );
        let message = runtime.message_window_state();
        assert_eq!(message.choices.len(), 2);
        assert_eq!(message.choices[0].id, "prologue");
        assert_eq!(message.choices[0].label, "Prologue");
        assert!(message.input_prompt.is_none());
        assert!(!message.backlog.is_empty());
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.log_clear_host_id, &[])
                .expect("message log clear"),
            Value::Bool(true)
        );
        let message = runtime.message_window_state();
        assert!(message.backlog.is_empty());
        assert_eq!(message.text, "Hello world");
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.reset_style_host_id, &[])
                .expect("message style reset"),
            Value::Bool(true)
        );
        let message = runtime.message_window_state();
        assert_eq!(message.style, UiMessageWindowStyle::default());
    }

    #[test]
    fn runtime_installs_and_executes_ui_policy_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::egui()));
        let extension = runtime.install_ui_extension().expect("install ui");

        assert_eq!(
            runtime
                .extension_registry()
                .resolve_id("ext.ui.context_menu"),
            Ok(extension.context_menu_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.ui.shift_fast"),
            Ok(extension.shift_fast_ext_id)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.context_menu_host_id, &[Value::Bool(true)])
                .expect("context menu"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.shift_fast_host_id, &[Value::Bool(true)])
                .expect("shift fast"),
            Value::Bool(true)
        );

        let policy = runtime.ui_policy_state();
        assert!(policy.context_menu_enabled);
        assert!(policy.shift_fast_enabled);
    }
    #[test]
    fn runtime_installs_and_executes_scene_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_scene_extension().expect("install scene");

        assert_eq!(
            runtime.extension_registry().resolve_id("ext.scene.layout"),
            Ok(extension.layout_ext_id)
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.layout_host_id,
                    &[
                        Value::Integer(240),
                        Value::Integer(92),
                        Value::Integer(520),
                        Value::Integer(180),
                        Value::Integer(18),
                        Value::Integer(380),
                        Value::Integer(1244),
                        Value::Integer(130),
                    ],
                )
                .expect("scene layout"),
            Value::Bool(true)
        );

        let layout = runtime.scene_layout_state();
        assert_eq!(layout.choice_panel.x, 240.0);
        assert_eq!(layout.choice_panel.y, 92.0);
        assert_eq!(layout.message_window.x, 18.0);
        assert_eq!(layout.message_window.height, 130.0);
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.scene.opening"),
            Ok(extension.opening_ext_id)
        );
        assert_eq!(
            runtime.extension_registry().resolve_id("ext.scene.ending"),
            Ok(extension.ending_ext_id)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.opening_host_id,
                    &[Value::String("Prologue".to_owned())]
                )
                .expect("scene opening"),
            Value::Bool(true)
        );
        assert_eq!(runtime.message_window_state().text, "Prologue");
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.ending_host_id, &[Value::String("Fin".to_owned())])
                .expect("scene ending"),
            Value::Bool(true)
        );
        assert_eq!(runtime.message_window_state().text, "Fin");

        runtime
            .message_window
            .borrow_mut()
            .text
            .push_str("stale message");
        runtime.image_draws.borrow_mut().push(ImageDrawState {
            handle: 77,
            resource_id: 100,
            x: 12.0,
            y: 16.0,
            ..ImageDrawState::default()
        });
        runtime.icon_sheets.borrow_mut().insert(
            77,
            IconSheetState {
                cell_width: 16,
                cell_height: 16,
            },
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.reset_host_id, &[])
                .expect("scene reset"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime.scene_layout_state(),
            wmui::UiSceneLayoutState::default()
        );
        assert_eq!(
            runtime.message_window_state(),
            MessageWindowState::default()
        );
        assert!(runtime.image_draws().is_empty());
        assert!(runtime.icon_sheets.borrow().is_empty());
    }

    #[test]
    fn runtime_installs_and_executes_image_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_image_extension().expect("install image");
        let archive = build_archive("image-sample", &[(100, ResourceType::Image, b"img")]);
        runtime.load_archive(&archive).expect("load archive");

        let handle = runtime
            .host
            .borrow_mut()
            .call(extension.load_host_id, &[Value::Integer(100)])
            .expect("image load");
        let handle = handle_from_value(handle);

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.draw_host_id,
                    &[
                        Value::Handle(handle),
                        Value::Integer(12),
                        Value::Integer(24)
                    ]
                )
                .expect("image draw"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.draw_part_host_id,
                    &[
                        Value::Handle(handle),
                        Value::Integer(1),
                        Value::Integer(2),
                        Value::Integer(3),
                        Value::Integer(4),
                        Value::Integer(5),
                        Value::Integer(6),
                    ]
                )
                .expect("image draw_part"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.draw_ext_host_id,
                    &[
                        Value::Handle(handle),
                        Value::Integer(7),
                        Value::Integer(8),
                        Value::Integer(9),
                        Value::Integer(10),
                        Value::Integer(11),
                        Value::Integer(12),
                        Value::Integer(13),
                        Value::Integer(14),
                        Value::Integer(15),
                        Value::Float(0.5),
                    ]
                )
                .expect("image draw_ext"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.set_icon_sheet_host_id,
                    &[
                        Value::Handle(handle),
                        Value::Integer(16),
                        Value::Integer(16)
                    ]
                )
                .expect("set icon sheet"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.draw_icon_host_id,
                    &[
                        Value::Handle(handle),
                        Value::Integer(2),
                        Value::Integer(20),
                        Value::Integer(24)
                    ]
                )
                .expect("draw icon"),
            Value::Bool(true)
        );

        let status = runtime
            .host
            .borrow_mut()
            .call(extension.status_host_id, &[Value::Handle(handle)])
            .expect("image status");
        assert_eq!(status, Value::Integer(2));

        let info = runtime
            .host
            .borrow_mut()
            .call(extension.info_host_id, &[Value::Handle(handle)])
            .expect("image info");
        let table = match info {
            Value::Table(table) => table,
            other => panic!("expected table, found {other:?}"),
        };
        assert_eq!(table.get(&1), Some(&Value::Integer(100)));
        assert_eq!(
            table.get(&2),
            Some(&Value::Integer(ResourceType::Image.as_u16() as i64))
        );
        assert_eq!(table.get(&3), Some(&Value::Integer(3)));
        assert_eq!(table.get(&4), Some(&Value::Integer(2)));
        assert_eq!(runtime.image_draws().len(), 4);
        let draw = &runtime.image_draws()[0];
        assert_eq!(draw.handle, handle);
        assert_eq!(draw.resource_id, 100);
        assert_eq!(draw.x, 12.0);
        assert_eq!(draw.y, 24.0);
        assert!(runtime.image_draws()[1].source.is_some());
        assert!(runtime.image_draws()[2].rotation_degrees > 0.0);
        assert!(runtime.image_draws()[3].icon_sheet.is_some());

        let released = runtime
            .host
            .borrow_mut()
            .call(extension.release_host_id, &[Value::Handle(handle)])
            .expect("image release");
        assert_eq!(released, Value::Bool(true));
        assert!(runtime.image_draws().is_empty());
        assert!(!runtime.icon_sheets.borrow().contains_key(&handle));
    }

    #[test]
    fn runtime_installs_and_executes_audio_extension() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let extension = runtime.install_audio_extension().expect("install audio");
        let archive = build_archive("audio-sample", &[(200, ResourceType::Audio, b"audio")]);
        runtime.load_archive(&archive).expect("load archive");

        let handle = runtime
            .host
            .borrow_mut()
            .call(extension.load_host_id, &[Value::Integer(200)])
            .expect("audio load");
        let handle = handle_from_value(handle);

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.play_host_id,
                    &[Value::Handle(handle), Value::Bool(true)]
                )
                .expect("audio play"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.playback_host_id,
                    &[Value::Handle(handle), Value::Bool(false)]
                )
                .expect("audio playback"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    extension.playback_host_id,
                    &[Value::Handle(handle), Value::Bool(false)]
                )
                .expect("audio playback"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.status_host_id, &[Value::Handle(handle)])
                .expect("audio status"),
            Value::Integer(2)
        );
        let audio_state = runtime
            .audio_playback_states()
            .get(&handle)
            .cloned()
            .expect("audio state");
        assert!(audio_state.playing);
        assert!(!audio_state.looped);
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.pause_host_id, &[Value::Handle(handle)])
                .expect("audio pause"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.status_host_id, &[Value::Handle(handle)])
                .expect("audio status"),
            Value::Integer(1)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.stop_host_id, &[Value::Handle(handle)])
                .expect("audio stop"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(extension.release_host_id, &[Value::Handle(handle)])
                .expect("audio release"),
            Value::Bool(true)
        );
    }

    #[test]
    fn runtime_vm_save_and_load_restores_state() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        let image = runtime.install_image_extension().expect("install image");
        let audio = runtime.install_audio_extension().expect("install audio");
        let vm = runtime.install_vm_extension().expect("install vm");
        let archive = build_archive(
            "checkpoint-sample",
            &[
                (100, ResourceType::Image, b"img"),
                (200, ResourceType::Audio, b"audio"),
            ],
        );
        runtime.load_archive(&archive).expect("load archive");

        let image_handle = handle_from_value(
            runtime
                .host
                .borrow_mut()
                .call(image.load_host_id, &[Value::Integer(100)])
                .expect("image load"),
        );
        let audio_handle = handle_from_value(
            runtime
                .host
                .borrow_mut()
                .call(audio.load_host_id, &[Value::Integer(200)])
                .expect("audio load"),
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    image.draw_host_id,
                    &[
                        Value::Handle(image_handle),
                        Value::Integer(4),
                        Value::Integer(8)
                    ]
                )
                .expect("image draw before save"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    image.set_icon_sheet_host_id,
                    &[
                        Value::Handle(image_handle),
                        Value::Integer(16),
                        Value::Integer(16)
                    ]
                )
                .expect("set icon sheet before save"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    image.draw_icon_host_id,
                    &[
                        Value::Handle(image_handle),
                        Value::Integer(1),
                        Value::Integer(16),
                        Value::Integer(16)
                    ]
                )
                .expect("draw icon before save"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(
                    audio.play_host_id,
                    &[Value::Handle(audio_handle), Value::Bool(true)]
                )
                .expect("audio play"),
            Value::Bool(true)
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(vm.save_host_id, &[Value::Integer(7)])
                .expect("vm save"),
            Value::Bool(true)
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(audio.pause_host_id, &[Value::Handle(audio_handle)])
                .expect("audio pause"),
            Value::Bool(true)
        );
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(image.release_host_id, &[Value::Handle(image_handle)])
                .expect("image release"),
            Value::Bool(true)
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(vm.load_host_id, &[Value::Integer(7)])
                .expect("vm load"),
            Value::Bool(true)
        );

        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(image.status_host_id, &[Value::Handle(image_handle)])
                .expect("image status after load"),
            Value::Integer(2)
        );
        assert_eq!(runtime.image_draws().len(), 2);
        assert_eq!(runtime.image_draws()[0].resource_id, 100);
        assert_eq!(runtime.image_draws()[0].icon_sheet, None);
        assert!(runtime.image_draws()[1].icon_sheet.is_some());
        assert_eq!(
            runtime
                .host
                .borrow_mut()
                .call(audio.status_host_id, &[Value::Handle(audio_handle)])
                .expect("audio status after load"),
            Value::Integer(2)
        );
    }
}
