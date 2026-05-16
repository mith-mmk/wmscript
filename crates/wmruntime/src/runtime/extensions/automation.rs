use super::super::*;

impl Runtime {
    pub fn install_automation_extension(&mut self) -> Result<AutomationExtension, RuntimeError> {
        let resource_host_id = 250;
        let set_resource_host_id = 251;
        let add_resource_host_id = 252;
        let set_job_host_id = 253;
        let enable_job_host_id = 254;
        let tick_host_id = 255;
        let job_progress_host_id = 256;
        let state_manager = self.state_manager.clone();

        let _ = self.register_host_function(
            HostFunction::new(resource_host_id, 1, 1, 0),
            move |args| {
                let name = expect_string_arg(args, 0, "resource")?;
                let key = automation_resource_key(&name);
                Ok(Value::Integer(state_integer(&state_manager.borrow(), &key)))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(set_resource_host_id, 2, 2, 0),
            move |args| {
                let name = expect_string_arg(args, 0, "resource")?;
                let amount = expect_integer_arg(args, 1, "amount")?;
                let key = automation_resource_key(&name);
                state_manager.borrow_mut().set(key, Value::Integer(amount));
                Ok(Value::Bool(true))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(add_resource_host_id, 2, 2, 0),
            move |args| {
                let name = expect_string_arg(args, 0, "resource")?;
                let delta = expect_integer_arg(args, 1, "delta")?;
                let key = automation_resource_key(&name);
                let mut state = state_manager.borrow_mut();
                let next = state_integer(&state, &key).saturating_add(delta);
                state.set(key, Value::Integer(next));
                Ok(Value::Integer(next))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(set_job_host_id, 4, 4, 0), move |args| {
                let id = expect_state_id_arg(args, 0, "job")?;
                let enabled = expect_bool_arg(args, 1, "enabled")?;
                let rate = expect_integer_arg(args, 2, "rate")?;
                let output = expect_string_arg(args, 3, "output")?;
                let mut state = state_manager.borrow_mut();
                append_state_list_value(&mut state, "automation.jobs", &id);
                state.set(format!("job.{id}.enabled"), Value::Bool(enabled));
                state.set(format!("job.{id}.rate"), Value::Integer(rate.max(0)));
                if !state.has(&format!("job.{id}.progress")) {
                    state.set(format!("job.{id}.progress"), Value::Integer(0));
                }
                state.set(format!("job.{id}.output"), Value::String(output));
                Ok(Value::Bool(true))
            });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(enable_job_host_id, 2, 2, 0),
            move |args| {
                let id = expect_state_id_arg(args, 0, "job")?;
                let enabled = expect_bool_arg(args, 1, "enabled")?;
                state_manager
                    .borrow_mut()
                    .set(format!("job.{id}.enabled"), Value::Bool(enabled));
                Ok(Value::Bool(true))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(tick_host_id, 1, 1, 0), move |args| {
                let steps = expect_integer_arg(args, 0, "steps")?.max(0);
                let mut state = state_manager.borrow_mut();
                let current_tick = state_integer(&state, "game.tick");
                let next_tick = current_tick.saturating_add(steps);
                state.set("game.tick".to_owned(), Value::Integer(next_tick));

                for job_id in state_list_value(&state, "automation.jobs") {
                    if !state_bool(&state, &format!("job.{job_id}.enabled")) {
                        continue;
                    }
                    let rate = state_integer(&state, &format!("job.{job_id}.rate")).max(0);
                    let output = match state.get(&format!("job.{job_id}.output")) {
                        Some(Value::String(value)) if !value.is_empty() => value,
                        _ => continue,
                    };
                    let progress_key = format!("job.{job_id}.progress");
                    let progress = state_integer(&state, &progress_key)
                        .saturating_add(rate.saturating_mul(steps));
                    let produced = progress.max(0);
                    state.set(progress_key, Value::Integer(0));
                    if produced > 0 {
                        let output_key = automation_resource_key(&output);
                        let next = state_integer(&state, &output_key).saturating_add(produced);
                        state.set(output_key, Value::Integer(next));
                    }
                }

                Ok(Value::Integer(next_tick))
            });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(job_progress_host_id, 1, 1, 0),
            move |args| {
                let id = expect_state_id_arg(args, 0, "job")?;
                Ok(Value::Integer(state_integer(
                    &state_manager.borrow(),
                    &format!("job.{id}.progress"),
                )))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.automation",
            &[
                ExtensionFunctionSpec::new("resource", resource_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("set_resource", set_resource_host_id, 2, 2, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("add_resource", add_resource_host_id, 2, 2, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("set_job", set_job_host_id, 4, 4, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("enable_job", enable_job_host_id, 2, 2, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("tick", tick_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("job_progress", job_progress_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
            ],
        )?;

        Ok(AutomationExtension {
            resource_ext_id: ids[0],
            set_resource_ext_id: ids[1],
            add_resource_ext_id: ids[2],
            set_job_ext_id: ids[3],
            enable_job_ext_id: ids[4],
            tick_ext_id: ids[5],
            job_progress_ext_id: ids[6],
            resource_host_id,
            set_resource_host_id,
            add_resource_host_id,
            set_job_host_id,
            enable_job_host_id,
            tick_host_id,
            job_progress_host_id,
        })
    }
}
