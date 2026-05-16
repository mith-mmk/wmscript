use super::super::*;

impl Runtime {
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
}
