use super::super::*;

impl Runtime {
    pub fn install_message_extension(&mut self) -> Result<MessageExtension, RuntimeError> {
        let show_host_id = 135;
        let append_host_id = 136;
        let choices_host_id = 137;
        let choices_named_host_id = 134;
        let prompt_host_id = 138;
        let hide_host_id = 139;
        let speed_host_id = 131;
        let auto_host_id = 132;
        let skip_host_id = 133;
        let log_clear_host_id = 159;
        let clear_host_id = 149;
        let box_style_host_id = 162;
        let text_color_host_id = 163;
        let speaker_color_host_id = 164;
        let accent_color_host_id = 165;
        let font_size_host_id = 166;
        let reset_style_host_id = 167;
        let frame_host_id = 168;
        let content_inset_host_id = 169;
        let input_box_style_host_id = 220;
        let input_text_color_host_id = 221;
        let input_hint_color_host_id = 222;
        let input_prompt_color_host_id = 223;
        let choice_box_style_host_id = 224;
        let choice_text_color_host_id = 225;
        let choice_accent_color_host_id = 226;
        let choice_selected_style_host_id = 227;
        let locale_host_id = 228;
        let message_window = self.message_window.clone();

        let _ = self.register_host_function(
            HostFunction::new(show_host_id, 1, 2, CAP_GUI),
            move |args| {
                let (speaker, text) = match args.len() {
                    1 => (None, expect_string_arg(args, 0, "text")?),
                    2 => (
                        Some(expect_string_arg(args, 0, "speaker")?),
                        expect_string_arg(args, 1, "text")?,
                    ),
                    other => {
                        return Err(HostError::InvalidArguments(format!(
                            "message.show expected 1..=2 args, got {other}"
                        )));
                    }
                };
                let mut window = message_window.borrow_mut();
                window.visible = true;
                window.speaker = speaker;
                window.text = text.clone();
                window
                    .backlog
                    .extend(text.lines().map(|line| line.to_owned()));
                window.input_prompt = None;
                window.choices.clear();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(append_host_id, 1, 1, CAP_GUI),
            move |args| {
                let line = expect_string_arg(args, 0, "line")?;
                let mut window = message_window.borrow_mut();
                if !window.text.is_empty() {
                    window.text.push('\n');
                }
                window.text.push_str(&line);
                window.backlog.push(line);
                window.visible = true;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choices_host_id, 0, 16, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.visible = true;
                if args.is_empty() {
                    window.choices.clear();
                    return Ok(Value::Bool(true));
                }
                window.choices = args
                    .iter()
                    .enumerate()
                    .map(|(index, value)| MessageChoiceState {
                        id: format!("choice-{}", index + 1),
                        label: render_value(value),
                        enabled: true,
                    })
                    .collect();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choices_named_host_id, 0, 16, CAP_GUI),
            move |args| {
                if !args.len().is_multiple_of(2) {
                    return Err(HostError::InvalidArguments(format!(
                        "message.choices_named expected id/label pairs, got {} args",
                        args.len()
                    )));
                }
                let mut window = message_window.borrow_mut();
                window.visible = true;
                if args.is_empty() {
                    window.choices.clear();
                    return Ok(Value::Bool(true));
                }
                let mut choices = Vec::with_capacity(args.len() / 2);
                for pair in args.chunks(2) {
                    choices.push(MessageChoiceState {
                        id: expect_string_arg(pair, 0, "choice_id")?,
                        label: expect_string_arg(pair, 1, "choice_label")?,
                        enabled: true,
                    });
                }
                window.choices = choices;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(prompt_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.visible = true;
                window.input_prompt = if args.is_empty() {
                    None
                } else {
                    Some(expect_string_arg(args, 0, "prompt")?)
                };
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(hide_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().visible = false;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(speed_host_id, 1, 1, CAP_GUI),
            move |args| {
                let speed = expect_number_arg(args, 0, "speed")? as f32;
                message_window.borrow_mut().text_speed = speed.max(0.0);
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(auto_host_id, 1, 1, CAP_GUI),
            move |args| {
                let enabled = expect_bool_arg(args, 0, "enabled")?;
                message_window.borrow_mut().auto_mode = enabled;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(skip_host_id, 1, 1, CAP_GUI),
            move |args| {
                let enabled = expect_bool_arg(args, 0, "enabled")?;
                message_window.borrow_mut().skip_mode = enabled;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(log_clear_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().backlog.clear();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(clear_host_id, 0, 0, CAP_GUI),
            move |_args| {
                *message_window.borrow_mut() = MessageWindowState::default();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.panel_fill = fill;
                window.style.panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "text")?;
                message_window.borrow_mut().style.text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(speaker_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "speaker")?;
                message_window.borrow_mut().style.speaker_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(accent_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "accent")?;
                message_window.borrow_mut().style.accent_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(font_size_host_id, 2, 2, CAP_GUI),
            move |args| {
                let body = expect_number_arg(args, 0, "body_font_size")? as f32;
                let speaker = expect_number_arg(args, 1, "speaker_font_size")? as f32;
                let mut window = message_window.borrow_mut();
                window.style.body_font_size = body.max(8.0);
                window.style.speaker_font_size = speaker.max(8.0);
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(reset_style_host_id, 0, 0, CAP_GUI),
            move |_args| {
                message_window.borrow_mut().style = UiMessageWindowStyle::default();
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(frame_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                window.style.frame_resource_id = if args.is_empty() {
                    None
                } else {
                    Some(expect_integer_arg(args, 0, "frame_resource_id")? as u32)
                };
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(content_inset_host_id, 4, 4, CAP_GUI),
            move |args| {
                let left = expect_number_arg(args, 0, "left")? as f32;
                let top = expect_number_arg(args, 1, "top")? as f32;
                let right = expect_number_arg(args, 2, "right")? as f32;
                let bottom = expect_number_arg(args, 3, "bottom")? as f32;
                message_window.borrow_mut().style.content_inset =
                    UiInsets::new(left.max(0.0), top.max(0.0), right.max(0.0), bottom.max(0.0));
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.input_panel_fill = fill;
                window.style.input_panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_text")?;
                message_window.borrow_mut().style.input_text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_hint_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_hint")?;
                message_window.borrow_mut().style.input_hint_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(input_prompt_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "input_prompt")?;
                message_window.borrow_mut().style.input_prompt_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_box_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "fill")?;
                let stroke = expect_rgba_args(args, 4, "stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.choice_panel_fill = fill;
                window.style.choice_panel_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_text_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "choice_text")?;
                message_window.borrow_mut().style.choice_text_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_accent_color_host_id, 4, 4, CAP_GUI),
            move |args| {
                let color = expect_rgba_args(args, 0, "choice_accent")?;
                message_window.borrow_mut().style.choice_accent_color = color;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(choice_selected_style_host_id, 8, 8, CAP_GUI),
            move |args| {
                let fill = expect_rgba_args(args, 0, "selected_fill")?;
                let stroke = expect_rgba_args(args, 4, "selected_stroke")?;
                let mut window = message_window.borrow_mut();
                window.style.choice_selected_fill = fill;
                window.style.choice_selected_stroke = stroke;
                Ok(Value::Bool(true))
            },
        );

        let message_window = self.message_window.clone();
        let _ = self.register_host_function(
            HostFunction::new(locale_host_id, 0, 1, CAP_GUI),
            move |args| {
                let mut window = message_window.borrow_mut();
                if args.is_empty() {
                    return Ok(Value::String(window.locale.clone()));
                }
                let locale = expect_string_arg(args, 0, "locale")?;
                let normalized = locale.trim().to_ascii_lowercase();
                window.locale = if normalized.starts_with("ja") {
                    "ja".to_owned()
                } else {
                    "en".to_owned()
                };
                Ok(Value::String(window.locale.clone()))
            },
        );

        let ids = self.extensions.register_extension(
            "ext.message",
            &[
                ExtensionFunctionSpec::new("show", show_host_id, 1, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("append", append_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices", choices_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices_named", choices_named_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("prompt", prompt_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("hide", hide_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speed", speed_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("auto", auto_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("skip", skip_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("log_clear", log_clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("clear", clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("box_style", box_style_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("text_color", text_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speaker_color", speaker_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("accent_color", accent_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("font_size", font_size_host_id, 2, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset_style", reset_style_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("frame", frame_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("content_inset", content_inset_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_box_style",
                    input_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_text_color",
                    input_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_hint_color",
                    input_hint_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_prompt_color",
                    input_prompt_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_box_style",
                    choice_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_text_color",
                    choice_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_accent_color",
                    choice_accent_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_selected_style",
                    choice_selected_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("locale", locale_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::String),
            ],
        )?;
        let _ = self.extensions.register_extension(
            "text",
            &[
                ExtensionFunctionSpec::new("show", show_host_id, 1, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("append", append_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices", choices_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("choices_named", choices_named_host_id, 0, 16, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("prompt", prompt_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("hide", hide_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speed", speed_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("auto", auto_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("skip", skip_host_id, 1, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("log_clear", log_clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("clear", clear_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("box_style", box_style_host_id, 8, 8, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("text_color", text_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("speaker_color", speaker_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("accent_color", accent_color_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("font_size", font_size_host_id, 2, 2, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("reset_style", reset_style_host_id, 0, 0, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("frame", frame_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("content_inset", content_inset_host_id, 4, 4, CAP_GUI)
                    .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_box_style",
                    input_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_text_color",
                    input_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_hint_color",
                    input_hint_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "input_prompt_color",
                    input_prompt_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_box_style",
                    choice_box_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_text_color",
                    choice_text_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_accent_color",
                    choice_accent_color_host_id,
                    4,
                    4,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new(
                    "choice_selected_style",
                    choice_selected_style_host_id,
                    8,
                    8,
                    CAP_GUI,
                )
                .with_return_type(ExtValueType::Bool),
                ExtensionFunctionSpec::new("locale", locale_host_id, 0, 1, CAP_GUI)
                    .with_return_type(ExtValueType::String),
            ],
        )?;

        Ok(MessageExtension {
            show_ext_id: ids[0],
            append_ext_id: ids[1],
            choices_ext_id: ids[2],
            choices_named_ext_id: ids[3],
            prompt_ext_id: ids[4],
            hide_ext_id: ids[5],
            speed_ext_id: ids[6],
            auto_ext_id: ids[7],
            skip_ext_id: ids[8],
            log_clear_ext_id: ids[9],
            clear_ext_id: ids[10],
            box_style_ext_id: ids[11],
            text_color_ext_id: ids[12],
            speaker_color_ext_id: ids[13],
            accent_color_ext_id: ids[14],
            font_size_ext_id: ids[15],
            reset_style_ext_id: ids[16],
            frame_ext_id: ids[17],
            content_inset_ext_id: ids[18],
            input_box_style_ext_id: ids[19],
            input_text_color_ext_id: ids[20],
            input_hint_color_ext_id: ids[21],
            input_prompt_color_ext_id: ids[22],
            choice_box_style_ext_id: ids[23],
            choice_text_color_ext_id: ids[24],
            choice_accent_color_ext_id: ids[25],
            choice_selected_style_ext_id: ids[26],
            locale_ext_id: ids[27],
            show_host_id,
            append_host_id,
            choices_host_id,
            choices_named_host_id,
            prompt_host_id,
            hide_host_id,
            speed_host_id,
            auto_host_id,
            skip_host_id,
            log_clear_host_id,
            clear_host_id,
            box_style_host_id,
            text_color_host_id,
            speaker_color_host_id,
            accent_color_host_id,
            font_size_host_id,
            reset_style_host_id,
            frame_host_id,
            content_inset_host_id,
            input_box_style_host_id,
            input_text_color_host_id,
            input_hint_color_host_id,
            input_prompt_color_host_id,
            choice_box_style_host_id,
            choice_text_color_host_id,
            choice_accent_color_host_id,
            choice_selected_style_host_id,
            locale_host_id,
        })
    }
}
