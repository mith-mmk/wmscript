use super::super::*;

impl Runtime {
    pub fn install_rpg_extension(&mut self) -> Result<RpgExtension, RuntimeError> {
        let map_controls_host_id = 270;
        let actions_host_id = 271;
        let hud_host_id = 272;
        let clear_host_id = 273;

        let rpg_ui = self.rpg_ui.clone();
        let _ = self.register_host_function(
            HostFunction::new(map_controls_host_id, 1, 17, CAP_GUI),
            move |args| {
                let projection = expect_string_arg(args, 0, "projection")?;
                let mut directions = Vec::with_capacity(args.len().saturating_sub(1));
                for index in 1..args.len() {
                    let id = expect_string_arg(args, index, "direction")?;
                    if !id.is_empty() {
                        directions.push(id);
                    }
                }
                rpg_ui.borrow_mut().map_controls = RpgMapControlsState {
                    projection,
                    directions,
                };
                Ok(Value::Bool(true))
            },
        );

        let rpg_ui = self.rpg_ui.clone();
        let _ = self.register_host_function(
            HostFunction::new(actions_host_id, 0, 32, CAP_GUI),
            move |args| {
                if !args.len().is_multiple_of(2) {
                    return Err(HostError::InvalidArguments(format!(
                        "rpg.actions expected id/label pairs, got {} args",
                        args.len()
                    )));
                }
                let mut actions = Vec::with_capacity(args.len() / 2);
                for pair in args.chunks(2) {
                    actions.push(RpgActionState {
                        id: expect_string_arg(pair, 0, "action_id")?,
                        label: expect_string_arg(pair, 1, "action_label")?,
                        enabled: true,
                    });
                }
                rpg_ui.borrow_mut().actions = actions;
                Ok(Value::Bool(true))
            },
        );

        let rpg_ui = self.rpg_ui.clone();
        let _ = self.register_host_function(
            HostFunction::new(hud_host_id, 2, 2, CAP_GUI),
            move |args| {
                rpg_ui.borrow_mut().hud = Some(RpgHudState {
                    title: expect_string_arg(args, 0, "title")?,
                    body: expect_string_arg(args, 1, "body")?,
                });
                Ok(Value::Bool(true))
            },
        );

        let rpg_ui = self.rpg_ui.clone();
        let _ = self.register_host_function(
            HostFunction::new(clear_host_id, 0, 0, CAP_GUI),
            move |_args| {
                *rpg_ui.borrow_mut() = RpgUiState::default();
                Ok(Value::Bool(true))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.rpg",
            &[
                ExtensionFunctionSpec::new("map_controls", map_controls_host_id, 1, 17, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("actions", actions_host_id, 0, 32, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("hud", hud_host_id, 2, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("clear", clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(RpgExtension {
            map_controls_ext_id: ids[0],
            actions_ext_id: ids[1],
            hud_ext_id: ids[2],
            clear_ext_id: ids[3],
            map_controls_host_id,
            actions_host_id,
            hud_host_id,
            clear_host_id,
        })
    }
}
