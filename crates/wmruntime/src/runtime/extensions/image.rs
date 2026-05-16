use super::super::*;

impl Runtime {
    pub fn install_image_extension(&mut self) -> Result<ImageExtension, RuntimeError> {
        let load_host_id = 140;
        let info_host_id = 141;
        let status_host_id = 142;
        let release_host_id = 143;
        let draw_host_id = 144;
        let draw_part_host_id = 145;
        let draw_ext_host_id = 146;
        let set_icon_sheet_host_id = 147;
        let draw_icon_host_id = 148;
        let resources = self.resources.clone();

        let _ = self.register_host_function(
            HostFunction::new(load_host_id, 1, 1, CAP_GUI),
            move |args| {
                let resource_id = expect_integer_arg(args, 0, "resource_id")? as u32;
                match resources
                    .borrow_mut()
                    .load_resource(resource_id)
                    .map_err(resource_error_to_host_error)?
                {
                    LoadResult::Ready(handle) => Ok(Value::Handle(handle.into())),
                    LoadResult::Pending(request_id) => Ok(Value::Integer(request_id as i64)),
                }
            },
        );

        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(info_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let resources = resources.borrow();
                let resource_id = resources
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                let entry = resources.entry(resource_id).ok_or_else(|| {
                    HostError::Failed(format!("missing resource entry {resource_id}"))
                })?;
                Ok(make_table(&[
                    (1, Value::Integer(resource_id as i64)),
                    (
                        2,
                        Value::Integer(resource_type_value(
                            entry
                                .data
                                .as_ref()
                                .map(|data| match data {
                                    wmresource::ResourceData::Image(_) => ResourceType::Image,
                                    wmresource::ResourceData::Audio(_) => ResourceType::Audio,
                                    wmresource::ResourceData::Binary(_) => ResourceType::Binary,
                                    wmresource::ResourceData::Font(_) => ResourceType::Font,
                                    wmresource::ResourceData::Video(_) => ResourceType::Video,
                                    wmresource::ResourceData::ScriptData(_) => {
                                        ResourceType::ScriptData
                                    }
                                })
                                .unwrap_or(ResourceType::Unknown(0)),
                        )),
                    ),
                    (
                        3,
                        Value::Integer(
                            entry
                                .data
                                .as_ref()
                                .map_or(0, |data| data.bytes().len() as i64),
                        ),
                    ),
                    (4, Value::Integer(resource_state_code(entry.state))),
                ]))
            },
        );

        let resources = self.resources.clone();
        let _ = self.register_host_function(
            HostFunction::new(status_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let state = resources
                    .borrow()
                    .status(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Integer(resource_state_code(state)))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(release_host_id, 1, 1, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                image_draws
                    .borrow_mut()
                    .retain(|draw| draw.handle != handle.raw());
                icon_sheets.borrow_mut().remove(&handle.raw());
                resources
                    .borrow_mut()
                    .release(handle)
                    .map_err(resource_error_to_host_error)?;
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_host_id, 3, 3, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let x = expect_number_arg(args, 1, "x")? as f32;
                let y = expect_number_arg(args, 2, "y")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x,
                    y,
                    width: None,
                    height: None,
                    source: None,
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_part_host_id, 7, 7, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let sx = expect_number_arg(args, 1, "sx")? as f32;
                let sy = expect_number_arg(args, 2, "sy")? as f32;
                let sw = expect_number_arg(args, 3, "sw")? as f32;
                let sh = expect_number_arg(args, 4, "sh")? as f32;
                let dx = expect_number_arg(args, 5, "dx")? as f32;
                let dy = expect_number_arg(args, 6, "dy")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x: dx,
                    y: dy,
                    width: Some(sw),
                    height: Some(sh),
                    source: Some(ImageSourceRect {
                        x: sx,
                        y: sy,
                        width: sw,
                        height: sh,
                    }),
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_ext_host_id, 11, 11, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let sx = expect_number_arg(args, 1, "sx")? as f32;
                let sy = expect_number_arg(args, 2, "sy")? as f32;
                let sw = expect_number_arg(args, 3, "sw")? as f32;
                let sh = expect_number_arg(args, 4, "sh")? as f32;
                let dx = expect_number_arg(args, 5, "dx")? as f32;
                let dy = expect_number_arg(args, 6, "dy")? as f32;
                let dw = expect_number_arg(args, 7, "dw")? as f32;
                let dh = expect_number_arg(args, 8, "dh")? as f32;
                let rot = expect_number_arg(args, 9, "rot")? as f32;
                let alpha = expect_number_arg(args, 10, "alpha")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x: dx,
                    y: dy,
                    width: Some(dw),
                    height: Some(dh),
                    source: Some(ImageSourceRect {
                        x: sx,
                        y: sy,
                        width: sw,
                        height: sh,
                    }),
                    icon_sheet: None,
                    icon_index: None,
                    rotation_degrees: rot,
                    opacity: alpha.clamp(0.0, 1.0),
                });
                Ok(Value::Bool(true))
            },
        );

        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(set_icon_sheet_host_id, 3, 3, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let cell_w = expect_integer_arg(args, 1, "cell_w")? as u32;
                let cell_h = expect_integer_arg(args, 2, "cell_h")? as u32;
                icon_sheets.borrow_mut().insert(
                    handle.raw(),
                    IconSheetState {
                        cell_width: cell_w,
                        cell_height: cell_h,
                    },
                );
                Ok(Value::Bool(true))
            },
        );

        let resources = self.resources.clone();
        let image_draws = self.image_draws.clone();
        let icon_sheets = self.icon_sheets.clone();
        let _ = self.register_host_function(
            HostFunction::new(draw_icon_host_id, 4, 4, CAP_GUI),
            move |args| {
                let handle = ResourceHandle::from(expect_handle_arg(args, 0, "handle")?);
                let index = expect_integer_arg(args, 1, "index")? as u32;
                let x = expect_number_arg(args, 2, "x")? as f32;
                let y = expect_number_arg(args, 3, "y")? as f32;
                let resource_id = resources
                    .borrow()
                    .resource_id(handle)
                    .map_err(resource_error_to_host_error)?;
                let icon_sheet = icon_sheets
                    .borrow()
                    .get(&handle.raw())
                    .cloned()
                    .ok_or_else(|| {
                        HostError::Failed(format!("missing icon sheet for handle {}", handle.raw()))
                    })?;
                image_draws.borrow_mut().push(ImageDrawState {
                    handle: handle.raw(),
                    resource_id,
                    x,
                    y,
                    width: Some(icon_sheet.cell_width as f32),
                    height: Some(icon_sheet.cell_height as f32),
                    source: None,
                    icon_sheet: Some(icon_sheet),
                    icon_index: Some(index),
                    rotation_degrees: 0.0,
                    opacity: 1.0,
                });
                Ok(Value::Bool(true))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.image",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("info", info_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Unknown),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw", draw_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_part", draw_part_host_id, 7, 7, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_ext", draw_ext_host_id, 11, 11, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("set_icon_sheet", set_icon_sheet_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_icon", draw_icon_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "img",
            &[
                ExtensionFunctionSpec::new("load", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("info", info_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Unknown),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw", draw_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_part", draw_part_host_id, 7, 7, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_ext", draw_ext_host_id, 11, 11, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("set_icon_sheet", set_icon_sheet_host_id, 3, 3, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("draw_icon", draw_icon_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "asset",
            &[
                ExtensionFunctionSpec::new("request", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("preload", load_host_id, 1, 1, CAP_GUI),
                ExtensionFunctionSpec::new("status", status_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Integer),
                ExtensionFunctionSpec::new("release", release_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
            ],
        )?;

        Ok(ImageExtension {
            load_ext_id: ids[0],
            info_ext_id: ids[1],
            status_ext_id: ids[2],
            release_ext_id: ids[3],
            draw_ext_id: ids[4],
            draw_part_ext_id: ids[5],
            draw_ext_ext_id: ids[6],
            set_icon_sheet_ext_id: ids[7],
            draw_icon_ext_id: ids[8],
            load_host_id,
            info_host_id,
            status_host_id,
            release_host_id,
            draw_host_id,
            draw_part_host_id,
            draw_ext_host_id,
            set_icon_sheet_host_id,
            draw_icon_host_id,
        })
    }
}
