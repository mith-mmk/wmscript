use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use wmhost::{CapabilityMask, HostFunction, HostId, HostRegistry};
use wmplatform::PlatformProfile;
use wmvm::{HostApi, HostError, Value};

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

    pub(in crate::runtime) fn call(
        &mut self,
        host_id: HostId,
        args: &[Value],
    ) -> Result<Value, HostError> {
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
pub(super) struct SharedHostApi {
    inner: Rc<RefCell<HostDispatcher>>,
}

impl SharedHostApi {
    pub(super) fn new(inner: Rc<RefCell<HostDispatcher>>) -> Self {
        Self { inner }
    }
}

impl HostApi for SharedHostApi {
    fn call_host(&mut self, host_id: HostId, args: &[Value]) -> Result<Value, HostError> {
        self.inner.borrow_mut().call(host_id, args)
    }
}
