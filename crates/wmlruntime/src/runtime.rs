#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use wmlarchive::{Archive, ArchiveError, Manifest};
use wmlext::{ExtError, ExtensionFunctionSpec, ExtensionRegistry};
use wmlhost::{
    CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_NETWORK, CapabilityMask, HostFunction, HostId, HostRegistry,
};
use wmlplatform::PlatformProfile;
use wmlresource::{ResourceError, ResourceManager};
use wmlverifier::{VerificationError, verify_program};
use wmlvm::{
    HostApi, HostError, Message, Program, RunOutcome, Scheduler, Value, Vm, VmConfig, WorkerId,
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
    Resource(ResourceError),
    Verification(VerificationError),
    Host(HostError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(error) => write!(f, "{error}"),
            Self::Extension(error) => write!(f, "{error}"),
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
pub struct Runtime {
    config: RuntimeConfig,
    scheduler: Scheduler,
    resources: ResourceManager,
    host: Rc<RefCell<HostDispatcher>>,
    extensions: ExtensionRegistry,
    debug_log: Rc<RefCell<Vec<String>>>,
    net_backend: Rc<RefCell<Box<dyn NetBackend>>>,
    llm_backend: Rc<RefCell<Box<dyn LlmBackend>>>,
    loaded_archives: Vec<Manifest>,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            scheduler: Scheduler::new(),
            resources: ResourceManager::new(config.memory_limit),
            host: Rc::new(RefCell::new(HostDispatcher::new(
                config.platform,
                config.capability_mask,
            ))),
            extensions: ExtensionRegistry::new(),
            debug_log: Rc::new(RefCell::new(Vec::new())),
            net_backend: Rc::new(RefCell::new(Box::new(DisabledNetBackend))),
            llm_backend: Rc::new(RefCell::new(Box::new(DisabledLlmBackend))),
            loaded_archives: Vec::new(),
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
        let resources_loaded = self.resources.ingest_archive(&archive)?;
        if let Some(manifest) = &manifest {
            self.loaded_archives.push(manifest.clone());
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
                ExtensionFunctionSpec::new("read", read_host_id, 1, 1, CAP_FILE_SYSTEM),
                ExtensionFunctionSpec::new("write", write_host_id, 2, 2, CAP_FILE_SYSTEM),
                ExtensionFunctionSpec::new("exists", exists_host_id, 1, 1, CAP_FILE_SYSTEM),
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
                ExtensionFunctionSpec::new("log", log_host_id, 1, 1, 0),
                ExtensionFunctionSpec::new("inspect", inspect_host_id, 1, 1, 0),
            ],
        )?;

        Ok(DebugExtension {
            log_ext_id: ids[0],
            inspect_ext_id: ids[1],
            log_host_id,
            inspect_host_id,
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
                ExtensionFunctionSpec::new("get", get_host_id, 1, 1, CAP_NETWORK),
                ExtensionFunctionSpec::new("post", post_host_id, 2, 2, CAP_NETWORK),
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
            &[ExtensionFunctionSpec::new(
                "generate",
                generate_host_id,
                1,
                1,
                CAP_ASYNC_IO,
            )],
        )?;

        Ok(LlmExtension {
            generate_ext_id: ids[0],
            generate_host_id,
        })
    }

    pub fn install_standard_extensions(&mut self) -> Result<StandardExtensions, RuntimeError> {
        Ok(StandardExtensions {
            fs: self.install_fs_extension()?,
            debug: self.install_debug_extension()?,
            net: self.install_net_extension()?,
            llm: self.install_llm_extension()?,
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
        self.scheduler.spawn(vm)
    }

    pub fn tick(&mut self) -> Vec<(WorkerId, RunOutcome)> {
        self.scheduler.run_round(self.config.step_limit)
    }

    pub fn run_until_idle(&mut self, max_rounds: usize) -> Vec<(WorkerId, RunOutcome)> {
        let mut outcomes = Vec::new();
        for _ in 0..max_rounds {
            if self.scheduler.is_idle() {
                break;
            }
            outcomes.extend(self.tick());
        }
        outcomes
    }

    pub fn resource_manager(&self) -> &ResourceManager {
        &self.resources
    }

    pub fn resource_manager_mut(&mut self) -> &mut ResourceManager {
        &mut self.resources
    }

    pub fn loaded_archives(&self) -> &[Manifest] {
        &self.loaded_archives
    }

    pub fn send_message(&mut self, message: Message) {
        self.scheduler.deliver(message);
    }
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

/// Stable ids for the runtime's standard extension set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandardExtensions {
    pub fs: FsExtension,
    pub debug: DebugExtension,
    pub net: NetExtension,
    pub llm: LlmExtension,
}

/// Stable ids assigned to the built-in llm extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LlmExtension {
    pub generate_ext_id: u32,
    pub generate_host_id: HostId,
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
    use wmlvm::{Function, Program, RunOutcome, Value};

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
}
