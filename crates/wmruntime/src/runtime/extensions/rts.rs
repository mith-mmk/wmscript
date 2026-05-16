use super::super::*;

impl Runtime {
    pub fn install_rts_extension(&mut self) -> Result<RtsExtension, RuntimeError> {
        let set_unit_host_id = 260;
        let move_unit_host_id = 261;
        let unit_x_host_id = 262;
        let unit_y_host_id = 263;
        let unit_hp_host_id = 264;
        let damage_unit_host_id = 265;
        let state_manager = self.state_manager.clone();

        let _ = self.register_host_function(
            HostFunction::new(set_unit_host_id, 5, 5, 0),
            move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                let team = expect_string_arg(args, 1, "team")?;
                let x = expect_integer_arg(args, 2, "x")?;
                let y = expect_integer_arg(args, 3, "y")?;
                let hp = expect_integer_arg(args, 4, "hp")?;
                let mut state = state_manager.borrow_mut();
                append_state_list_value(&mut state, "rts.units", &id);
                state.set(format!("unit.{id}.team"), Value::String(team));
                state.set(format!("unit.{id}.x"), Value::Integer(x));
                state.set(format!("unit.{id}.y"), Value::Integer(y));
                state.set(format!("unit.{id}.target_x"), Value::Integer(x));
                state.set(format!("unit.{id}.target_y"), Value::Integer(y));
                state.set(format!("unit.{id}.hp"), Value::Integer(hp.max(0)));
                state.set(
                    format!("unit.{id}.last_order"),
                    Value::String("spawn".to_owned()),
                );
                Ok(Value::Bool(true))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(move_unit_host_id, 3, 3, 0),
            move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                let x = expect_integer_arg(args, 1, "x")?;
                let y = expect_integer_arg(args, 2, "y")?;
                let mut state = state_manager.borrow_mut();
                state.set(format!("unit.{id}.x"), Value::Integer(x));
                state.set(format!("unit.{id}.y"), Value::Integer(y));
                state.set(format!("unit.{id}.target_x"), Value::Integer(x));
                state.set(format!("unit.{id}.target_y"), Value::Integer(y));
                state.set(
                    format!("unit.{id}.last_order"),
                    Value::String("move".to_owned()),
                );
                Ok(Value::Bool(true))
            },
        );

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(unit_x_host_id, 1, 1, 0), move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                Ok(Value::Integer(state_integer(
                    &state_manager.borrow(),
                    &format!("unit.{id}.x"),
                )))
            });

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(unit_y_host_id, 1, 1, 0), move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                Ok(Value::Integer(state_integer(
                    &state_manager.borrow(),
                    &format!("unit.{id}.y"),
                )))
            });

        let state_manager = self.state_manager.clone();
        let _ =
            self.register_host_function(HostFunction::new(unit_hp_host_id, 1, 1, 0), move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                Ok(Value::Integer(state_integer(
                    &state_manager.borrow(),
                    &format!("unit.{id}.hp"),
                )))
            });

        let state_manager = self.state_manager.clone();
        let _ = self.register_host_function(
            HostFunction::new(damage_unit_host_id, 2, 2, 0),
            move |args| {
                let id = expect_state_id_arg(args, 0, "unit")?;
                let damage = expect_integer_arg(args, 1, "damage")?.max(0);
                let key = format!("unit.{id}.hp");
                let mut state = state_manager.borrow_mut();
                let hp = state_integer(&state, &key).saturating_sub(damage).max(0);
                state.set(key, Value::Integer(hp));
                Ok(Value::Integer(hp))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.rts",
            &[
                ExtensionFunctionSpec::new("set_unit", set_unit_host_id, 5, 5, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("move_unit", move_unit_host_id, 3, 3, 0)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("unit_x", unit_x_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("unit_y", unit_y_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("unit_hp", unit_hp_host_id, 1, 1, 0)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("damage_unit", damage_unit_host_id, 2, 2, 0)
                    .with_return_type(ExtValueType::Integer),
            ],
        )?;

        Ok(RtsExtension {
            set_unit_ext_id: ids[0],
            move_unit_ext_id: ids[1],
            unit_x_ext_id: ids[2],
            unit_y_ext_id: ids[3],
            unit_hp_ext_id: ids[4],
            damage_unit_ext_id: ids[5],
            set_unit_host_id,
            move_unit_host_id,
            unit_x_host_id,
            unit_y_host_id,
            unit_hp_host_id,
            damage_unit_host_id,
        })
    }
}
