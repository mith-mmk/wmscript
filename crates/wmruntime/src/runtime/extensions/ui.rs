use super::super::*;

impl Runtime {
    pub(in crate::runtime) fn install_ui_extension(&mut self) -> Result<UiExtension, RuntimeError> {
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
}
