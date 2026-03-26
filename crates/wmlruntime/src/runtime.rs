#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use wmlarchive::{Archive, ArchiveError, Manifest};
use wmlhost::{CapabilityMask, HostFunction, HostId, HostRegistry};
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
    Resource(ResourceError),
    Verification(VerificationError),
    Host(HostError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(error) => write!(f, "{error}"),
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
    handlers: BTreeMap<HostId, HostHandler>,
}

impl HostDispatcher {
    pub fn new(profile: PlatformProfile) -> Self {
        Self {
            registry: HostRegistry::new(profile),
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
    loaded_archives: Vec<Manifest>,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            scheduler: Scheduler::new(),
            resources: ResourceManager::new(config.memory_limit),
            host: Rc::new(RefCell::new(HostDispatcher::new(config.platform))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use wmlvm::{Function, Program, Value};

    #[test]
    fn runtime_can_spawn_and_run_program() {
        let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
        runtime.register_host_function(HostFunction::new(1, 1, 1, 0), |args| {
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
}
