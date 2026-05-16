use super::super::*;

impl Runtime {
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
}
