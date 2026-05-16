use super::super::*;

impl Runtime {
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
}
