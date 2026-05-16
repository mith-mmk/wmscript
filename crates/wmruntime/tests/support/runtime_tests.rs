use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use wmarchive::{
    ArchiveBuilder, ArchiveSection, ManifestBuilder, ManifestResourceEntry, SectionDigest,
    SectionKind, digest_section,
};
use wmresource::ResourceType;
use wmvm::{Function, Program, RunOutcome, Value};

#[derive(Default)]
struct MockNetBackend {
    get_responses: BTreeMap<String, String>,
    post_responses: BTreeMap<(String, String), String>,
    requests: Vec<String>,
}

impl MockNetBackend {
    fn with_get(mut self, url: &str, body: &str) -> Self {
        self.get_responses.insert(url.to_owned(), body.to_owned());
        self
    }

    fn with_post(mut self, url: &str, body: &str, response: &str) -> Self {
        self.post_responses
            .insert((url.to_owned(), body.to_owned()), response.to_owned());
        self
    }
}

#[derive(Default)]
struct MockLlmBackend {
    responses: BTreeMap<String, String>,
    prompts: Vec<String>,
}

impl MockLlmBackend {
    fn with_response(mut self, prompt: &str, response: &str) -> Self {
        self.responses
            .insert(prompt.to_owned(), response.to_owned());
        self
    }
}

impl LlmBackend for MockLlmBackend {
    fn generate(&mut self, prompt: &str) -> Result<String, HostError> {
        self.prompts.push(prompt.to_owned());
        self.responses
            .get(prompt)
            .cloned()
            .ok_or_else(|| HostError::Failed(format!("missing mock response for prompt {prompt}")))
    }
}

impl NetBackend for MockNetBackend {
    fn get(&mut self, url: &str) -> Result<String, HostError> {
        self.requests.push(format!("GET {url}"));
        self.get_responses
            .get(url)
            .cloned()
            .ok_or_else(|| HostError::Failed(format!("missing mock response for GET {url}")))
    }

    fn post(&mut self, url: &str, body: &str) -> Result<String, HostError> {
        self.requests.push(format!("POST {url} {body}"));
        self.post_responses
            .get(&(url.to_owned(), body.to_owned()))
            .cloned()
            .ok_or_else(|| {
                HostError::Failed(format!(
                    "missing mock response for POST {url} with body {body}"
                ))
            })
    }
}

fn build_asset_payload(resource_id: u32, resource_type: ResourceType, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + data.len());
    bytes.extend_from_slice(&resource_id.to_le_bytes());
    bytes.extend_from_slice(&resource_type.as_u16().to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(24u32).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn build_archive(package_name: &str, assets: &[(u32, ResourceType, &[u8])]) -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let mut manifest_builder = ManifestBuilder::new(package_name, 42, 1);
    for (section_id, (resource_id, resource_type, data)) in assets.iter().enumerate() {
        let section_id = section_id as u32 + 2;
        let payload = build_asset_payload(*resource_id, *resource_type, data);
        manifest_builder = manifest_builder.push_resource_mapping(ManifestResourceEntry::new(
            0x1000 + *resource_id as u64,
            *resource_id,
        ));
        manifest_builder = manifest_builder.push_section_digest(SectionDigest {
            section_id,
            section_kind: SectionKind::Asset,
            flags_canonical: 0,
            unpacked_size: payload.len() as u64,
            digest: digest_section(
                section_id,
                SectionKind::Asset,
                0,
                payload.len() as u64,
                &payload,
            ),
        });
        builder =
            builder.push_section(ArchiveSection::new(section_id, SectionKind::Asset, payload));
    }
    builder
        .push_manifest(1, &manifest_builder.build())
        .build()
        .expect("build archive")
}

fn handle_from_value(value: Value) -> u64 {
    match value {
        Value::Handle(handle) => handle,
        other => panic!("expected handle, found {other:?}"),
    }
}

#[test]
fn runtime_can_spawn_and_run_program() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let _ = runtime.register_host_function(HostFunction::new(1, 1, 1, 0), |args| {
        Ok(args.first().cloned().unwrap_or(Value::Nil))
    });

    let mut program = Program::new();
    let idx = program.push_constant(Value::String("hello".to_owned()));
    program.insert_function(Function::new(
        1,
        vec![
            0x10,
            idx as u8,
            (idx >> 8) as u8,
            0x71,
            0x01,
            0x00,
            0x01,
            0x01,
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);
    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
}

#[test]
fn runtime_time_control_api_wakes_sleeping_worker() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let mut program = Program::new();
    let value_idx = program.push_constant(Value::Integer(42));
    program.insert_function(Function::new(
        1,
        vec![
            0xA1, // sleep
            0x10,
            value_idx as u8,
            (value_idx >> 8) as u8,
            0x72, // return
        ],
        0,
        0,
    ));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.tick();
    assert!(matches!(
        outcomes.as_slice(),
        [(_, RunOutcome::Sleeping { .. })]
    ));
    assert_eq!(runtime.sleeping_workers(), vec![worker_id]);

    assert!(runtime.wake_worker(worker_id));
    let outcomes = runtime.tick();
    assert!(matches!(
        outcomes.as_slice(),
        [(_, RunOutcome::Halted { .. })]
    ));
    assert!(runtime.sleeping_workers().is_empty());
    assert!(!runtime.wake_worker(9999));
}

#[test]
fn runtime_installs_and_executes_fs_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_fs_extension().expect("install fs");

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("wml_fs_test_{unique}.txt"));
    let path_text = path.to_string_lossy().to_string();

    let mut program = Program::new();
    let path_idx = program.push_constant(Value::String(path_text.clone()));
    let data_idx = program.push_constant(Value::String("hello fs".to_owned()));
    let code = vec![
        0x10,
        path_idx as u8,
        (path_idx >> 8) as u8,
        0x10,
        data_idx as u8,
        (data_idx >> 8) as u8,
        0x71,
        (extension.write_host_id & 0xFF) as u8,
        (extension.write_host_id >> 8) as u8,
        0x02,
        0x10,
        path_idx as u8,
        (path_idx >> 8) as u8,
        0x71,
        (extension.read_host_id & 0xFF) as u8,
        (extension.read_host_id >> 8) as u8,
        0x01,
        0x72,
    ];
    program.insert_function(Function::new(1, code, 0, 0));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);
    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.fs.read"),
        Ok(extension.read_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.fs.write"),
        Ok(extension.write_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.fs.exists"),
        Ok(extension.exists_ext_id)
    );
    assert_eq!(
        runtime
            .host_registry()
            .function(extension.read_host_id)
            .map(|function| function.required_capabilities),
        Some(CAP_FILE_SYSTEM)
    );
    assert!(matches!(
        outcomes.last(),
        Some((
            _,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "hello fs"
    ));
    let contents = fs::read_to_string(&path).expect("fs write");
    assert_eq!(contents, "hello fs");
    fs::remove_file(&path).ok();
}

#[test]
fn runtime_installs_and_executes_debug_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_debug_extension().expect("install debug");

    let mut program = Program::new();
    let message_idx = program.push_constant(Value::String("debug me".to_owned()));
    let code = vec![
        0x10,
        message_idx as u8,
        (message_idx >> 8) as u8,
        0x71,
        (extension.log_host_id & 0xFF) as u8,
        (extension.log_host_id >> 8) as u8,
        0x01,
        0x10,
        message_idx as u8,
        (message_idx >> 8) as u8,
        0x71,
        (extension.inspect_host_id & 0xFF) as u8,
        (extension.inspect_host_id >> 8) as u8,
        0x01,
        0x72,
    ];
    program.insert_function(Function::new(1, code, 0, 0));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);

    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.debug.log"),
        Ok(extension.log_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.debug.inspect"),
        Ok(extension.inspect_ext_id)
    );
    assert_eq!(runtime.debug_log(), vec!["debug me".to_owned()]);
    assert!(matches!(
        outcomes.last(),
        Some((
            _,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "debug me"
    ));
}

#[test]
fn runtime_installs_and_executes_net_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_net_extension().expect("install net");
    runtime.set_net_backend(
        MockNetBackend::default()
            .with_get("https://example.test/api", "net body")
            .with_post("https://example.test/api", "payload", "posted response"),
    );

    let mut program = Program::new();
    let url_idx = program.push_constant(Value::String("https://example.test/api".to_owned()));
    let body_idx = program.push_constant(Value::String("payload".to_owned()));
    let code = vec![
        0x10,
        url_idx as u8,
        (url_idx >> 8) as u8,
        0x71,
        (extension.get_host_id & 0xFF) as u8,
        (extension.get_host_id >> 8) as u8,
        0x01,
        0x10,
        url_idx as u8,
        (url_idx >> 8) as u8,
        0x10,
        body_idx as u8,
        (body_idx >> 8) as u8,
        0x71,
        (extension.post_host_id & 0xFF) as u8,
        (extension.post_host_id >> 8) as u8,
        0x02,
        0x72,
    ];
    program.insert_function(Function::new(1, code, 0, 0));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);

    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.net.get"),
        Ok(extension.get_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.net.post"),
        Ok(extension.post_ext_id)
    );
    assert!(matches!(
        outcomes.last(),
        Some((
            _,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "posted response"
    ));
}

#[test]
fn runtime_installs_and_executes_llm_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_llm_extension().expect("install llm");
    runtime.set_llm_backend(MockLlmBackend::default().with_response("hello model", "model reply"));

    let mut program = Program::new();
    let prompt_idx = program.push_constant(Value::String("hello model".to_owned()));
    let code = vec![
        0x10,
        prompt_idx as u8,
        (prompt_idx >> 8) as u8,
        0x71,
        (extension.generate_host_id & 0xFF) as u8,
        (extension.generate_host_id >> 8) as u8,
        0x01,
        0x72,
    ];
    program.insert_function(Function::new(1, code, 0, 0));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);

    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.llm.generate"),
        Ok(extension.generate_ext_id)
    );
    assert!(matches!(
        outcomes.last(),
        Some((
            _,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "model reply"
    ));
}

#[test]
fn runtime_installs_and_executes_message_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime
        .install_message_extension()
        .expect("install message");

    let mut program = Program::new();
    let speaker_idx = program.push_constant(Value::String("Narrator".to_owned()));
    let text_idx = program.push_constant(Value::String("Hello world".to_owned()));
    let choice_a_idx = program.push_constant(Value::String("Continue".to_owned()));
    let choice_b_idx = program.push_constant(Value::String("Back".to_owned()));
    let prompt_idx = program.push_constant(Value::String("Your name?".to_owned()));
    let code = vec![
        0x10,
        speaker_idx as u8,
        (speaker_idx >> 8) as u8,
        0x10,
        text_idx as u8,
        (text_idx >> 8) as u8,
        0x71,
        (extension.show_host_id & 0xFF) as u8,
        (extension.show_host_id >> 8) as u8,
        0x02,
        0x10,
        choice_a_idx as u8,
        (choice_a_idx >> 8) as u8,
        0x10,
        choice_b_idx as u8,
        (choice_b_idx >> 8) as u8,
        0x71,
        (extension.choices_host_id & 0xFF) as u8,
        (extension.choices_host_id >> 8) as u8,
        0x02,
        0x10,
        prompt_idx as u8,
        (prompt_idx >> 8) as u8,
        0x71,
        (extension.prompt_host_id & 0xFF) as u8,
        (extension.prompt_host_id >> 8) as u8,
        0x01,
        0x72,
    ];
    program.insert_function(Function::new(1, code, 0, 0));
    program.set_entry(1);

    let worker_id = runtime.spawn_program(program).expect("spawn");
    let outcomes = runtime.run_until_idle(8);

    assert_eq!(worker_id, 1);
    assert!(!outcomes.is_empty());
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.message.show"),
        Ok(extension.show_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.choices"),
        Ok(extension.choices_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.choices_named"),
        Ok(extension.choices_named_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.prompt"),
        Ok(extension.prompt_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.message.speed"),
        Ok(extension.speed_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.message.auto"),
        Ok(extension.auto_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.message.skip"),
        Ok(extension.skip_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.log_clear"),
        Ok(extension.log_clear_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.box_style"),
        Ok(extension.box_style_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.text_color"),
        Ok(extension.text_color_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.speaker_color"),
        Ok(extension.speaker_color_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.accent_color"),
        Ok(extension.accent_color_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.font_size"),
        Ok(extension.font_size_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.reset_style"),
        Ok(extension.reset_style_ext_id)
    );
    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.message.locale"),
        Ok(extension.locale_ext_id)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.speed_host_id, &[Value::Float(24.0)])
            .expect("message speed"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.auto_host_id, &[Value::Bool(true)])
            .expect("message auto"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.skip_host_id, &[Value::Bool(false)])
            .expect("message skip"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.box_style_host_id,
                &[
                    Value::Integer(12),
                    Value::Integer(18),
                    Value::Integer(24),
                    Value::Integer(230),
                    Value::Integer(120),
                    Value::Integer(180),
                    Value::Integer(90),
                    Value::Integer(255),
                ],
            )
            .expect("message box style"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.text_color_host_id,
                &[
                    Value::Integer(240),
                    Value::Integer(245),
                    Value::Integer(250),
                    Value::Integer(255),
                ],
            )
            .expect("message text color"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.speaker_color_host_id,
                &[
                    Value::Integer(255),
                    Value::Integer(232),
                    Value::Integer(160),
                    Value::Integer(255),
                ],
            )
            .expect("message speaker color"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.accent_color_host_id,
                &[
                    Value::Integer(170),
                    Value::Integer(220),
                    Value::Integer(255),
                    Value::Integer(255),
                ],
            )
            .expect("message accent color"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.locale_host_id, &[Value::String("en".to_owned())])
            .expect("message locale"),
        Value::String("en".to_owned())
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.locale_host_id, &[])
            .expect("message locale get"),
        Value::String("en".to_owned())
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.font_size_host_id,
                &[Value::Float(19.0), Value::Float(23.0)],
            )
            .expect("message font size"),
        Value::Bool(true)
    );
    let message = runtime.message_window_state();
    assert!(message.visible);
    assert_eq!(message.speaker.as_deref(), Some("Narrator"));
    assert_eq!(message.text, "Hello world");
    assert_eq!(message.choices.len(), 2);
    assert_eq!(message.choices[0].label, "Continue");
    assert_eq!(message.input_prompt.as_deref(), Some("Your name?"));
    assert_eq!(message.text_speed, 24.0);
    assert!(message.auto_mode);
    assert!(!message.skip_mode);
    assert_eq!(message.style.panel_fill, UiColorRgba::new(12, 18, 24, 230));
    assert_eq!(
        message.style.panel_stroke,
        UiColorRgba::new(120, 180, 90, 255)
    );
    assert_eq!(
        message.style.text_color,
        UiColorRgba::new(240, 245, 250, 255)
    );
    assert_eq!(
        message.style.speaker_color,
        UiColorRgba::new(255, 232, 160, 255)
    );
    assert_eq!(
        message.style.accent_color,
        UiColorRgba::new(170, 220, 255, 255)
    );
    assert_eq!(message.style.body_font_size, 19.0);
    assert_eq!(message.style.speaker_font_size, 23.0);
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.choices_named_host_id,
                &[
                    Value::String("prologue".to_owned()),
                    Value::String("Prologue".to_owned()),
                    Value::String("chapter_1".to_owned()),
                    Value::String("Chapter 1".to_owned()),
                ],
            )
            .expect("message choices named"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.prompt_host_id, &[])
            .expect("message prompt clear"),
        Value::Bool(true)
    );
    let message = runtime.message_window_state();
    assert_eq!(message.choices.len(), 2);
    assert_eq!(message.choices[0].id, "prologue");
    assert_eq!(message.choices[0].label, "Prologue");
    assert!(message.input_prompt.is_none());
    assert!(!message.backlog.is_empty());
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.log_clear_host_id, &[])
            .expect("message log clear"),
        Value::Bool(true)
    );
    let message = runtime.message_window_state();
    assert!(message.backlog.is_empty());
    assert_eq!(message.text, "Hello world");
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.reset_style_host_id, &[])
            .expect("message style reset"),
        Value::Bool(true)
    );
    let message = runtime.message_window_state();
    assert_eq!(message.style, UiMessageWindowStyle::default());
}

#[test]
fn runtime_installs_and_executes_ui_policy_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::egui()));
    let extension = runtime.install_ui_extension().expect("install ui");

    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.ui.context_menu"),
        Ok(extension.context_menu_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.ui.shift_fast"),
        Ok(extension.shift_fast_ext_id)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.context_menu_host_id, &[Value::Bool(true)])
            .expect("context menu"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.shift_fast_host_id, &[Value::Bool(true)])
            .expect("shift fast"),
        Value::Bool(true)
    );

    let policy = runtime.ui_policy_state();
    assert!(policy.context_menu_enabled);
    assert!(policy.shift_fast_enabled);
}

#[test]
fn runtime_installs_and_executes_automation_and_rts_extensions() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let automation = runtime
        .install_automation_extension()
        .expect("install automation");
    let rts = runtime.install_rts_extension().expect("install rts");

    assert_eq!(
        runtime
            .extension_registry()
            .resolve_id("ext.automation.tick"),
        Ok(automation.tick_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.rts.move_unit"),
        Ok(rts.move_unit_ext_id)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                automation.set_resource_host_id,
                &[Value::String("wood".to_owned()), Value::Integer(2)]
            )
            .expect("set wood"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                automation.set_job_host_id,
                &[
                    Value::String("lumber".to_owned()),
                    Value::Bool(true),
                    Value::Integer(3),
                    Value::String("resource.wood".to_owned()),
                ],
            )
            .expect("set lumber job"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(automation.tick_host_id, &[Value::Integer(4)])
            .expect("automation tick"),
        Value::Integer(4)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                automation.resource_host_id,
                &[Value::String("wood".to_owned())]
            )
            .expect("wood amount"),
        Value::Integer(14)
    );
    assert_eq!(runtime.state_value("game.tick"), Some(Value::Integer(4)));
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                rts.set_unit_host_id,
                &[
                    Value::String("worker_1".to_owned()),
                    Value::String("blue".to_owned()),
                    Value::Integer(4),
                    Value::Integer(5),
                    Value::Integer(10),
                ],
            )
            .expect("set unit"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                rts.move_unit_host_id,
                &[
                    Value::String("worker_1".to_owned()),
                    Value::Integer(8),
                    Value::Integer(9),
                ],
            )
            .expect("move unit"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                rts.damage_unit_host_id,
                &[Value::String("worker_1".to_owned()), Value::Integer(3)]
            )
            .expect("damage unit"),
        Value::Integer(7)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(rts.unit_x_host_id, &[Value::String("worker_1".to_owned())])
            .expect("unit x"),
        Value::Integer(8)
    );
    assert_eq!(
        runtime.state_value("rts.units"),
        Some(Value::String("worker_1".to_owned()))
    );
}

#[test]
fn runtime_installs_and_executes_scene_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_scene_extension().expect("install scene");

    assert_eq!(
        runtime.extension_registry().resolve_id("ext.scene.layout"),
        Ok(extension.layout_ext_id)
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.layout_host_id,
                &[
                    Value::Integer(240),
                    Value::Integer(92),
                    Value::Integer(520),
                    Value::Integer(180),
                    Value::Integer(18),
                    Value::Integer(380),
                    Value::Integer(1244),
                    Value::Integer(130),
                ],
            )
            .expect("scene layout"),
        Value::Bool(true)
    );

    let layout = runtime.scene_layout_state();
    assert_eq!(layout.choice_panel.x, 240.0);
    assert_eq!(layout.choice_panel.y, 92.0);
    assert_eq!(layout.message_window.x, 18.0);
    assert_eq!(layout.message_window.height, 130.0);
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.scene.opening"),
        Ok(extension.opening_ext_id)
    );
    assert_eq!(
        runtime.extension_registry().resolve_id("ext.scene.ending"),
        Ok(extension.ending_ext_id)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.opening_host_id,
                &[Value::String("Prologue".to_owned())]
            )
            .expect("scene opening"),
        Value::Bool(true)
    );
    assert_eq!(runtime.message_window_state().text, "Prologue");
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.ending_host_id, &[Value::String("Fin".to_owned())])
            .expect("scene ending"),
        Value::Bool(true)
    );
    assert_eq!(runtime.message_window_state().text, "Fin");

    runtime
        .message_window
        .borrow_mut()
        .text
        .push_str("stale message");
    runtime.image_draws.borrow_mut().push(ImageDrawState {
        handle: 77,
        resource_id: 100,
        x: 12.0,
        y: 16.0,
        ..ImageDrawState::default()
    });
    runtime.icon_sheets.borrow_mut().insert(
        77,
        IconSheetState {
            cell_width: 16,
            cell_height: 16,
        },
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.reset_host_id, &[])
            .expect("scene reset"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime.scene_layout_state(),
        wmui::UiSceneLayoutState::default()
    );
    assert_eq!(
        runtime.message_window_state(),
        MessageWindowState::default()
    );
    assert!(runtime.image_draws().is_empty());
    assert!(runtime.icon_sheets.borrow().is_empty());
}

#[test]
fn runtime_installs_and_executes_image_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_image_extension().expect("install image");
    let archive = build_archive("image-sample", &[(100, ResourceType::Image, b"img")]);
    runtime.load_archive(&archive).expect("load archive");

    let handle = runtime
        .host
        .borrow_mut()
        .call(extension.load_host_id, &[Value::Integer(100)])
        .expect("image load");
    let handle = handle_from_value(handle);

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.draw_host_id,
                &[
                    Value::Handle(handle),
                    Value::Integer(12),
                    Value::Integer(24)
                ]
            )
            .expect("image draw"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.draw_part_host_id,
                &[
                    Value::Handle(handle),
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3),
                    Value::Integer(4),
                    Value::Integer(5),
                    Value::Integer(6),
                ]
            )
            .expect("image draw_part"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.draw_ext_host_id,
                &[
                    Value::Handle(handle),
                    Value::Integer(7),
                    Value::Integer(8),
                    Value::Integer(9),
                    Value::Integer(10),
                    Value::Integer(11),
                    Value::Integer(12),
                    Value::Integer(13),
                    Value::Integer(14),
                    Value::Integer(15),
                    Value::Float(0.5),
                ]
            )
            .expect("image draw_ext"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.set_icon_sheet_host_id,
                &[
                    Value::Handle(handle),
                    Value::Integer(16),
                    Value::Integer(16)
                ]
            )
            .expect("set icon sheet"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.draw_icon_host_id,
                &[
                    Value::Handle(handle),
                    Value::Integer(2),
                    Value::Integer(20),
                    Value::Integer(24)
                ]
            )
            .expect("draw icon"),
        Value::Bool(true)
    );

    let status = runtime
        .host
        .borrow_mut()
        .call(extension.status_host_id, &[Value::Handle(handle)])
        .expect("image status");
    assert_eq!(status, Value::Integer(2));

    let info = runtime
        .host
        .borrow_mut()
        .call(extension.info_host_id, &[Value::Handle(handle)])
        .expect("image info");
    let table = match info {
        Value::Table(table) => table,
        other => panic!("expected table, found {other:?}"),
    };
    assert_eq!(table.get(&1), Some(&Value::Integer(100)));
    assert_eq!(
        table.get(&2),
        Some(&Value::Integer(ResourceType::Image.as_u16() as i64))
    );
    assert_eq!(table.get(&3), Some(&Value::Integer(3)));
    assert_eq!(table.get(&4), Some(&Value::Integer(2)));
    assert_eq!(runtime.image_draws().len(), 4);
    let draw = &runtime.image_draws()[0];
    assert_eq!(draw.handle, handle);
    assert_eq!(draw.resource_id, 100);
    assert_eq!(draw.x, 12.0);
    assert_eq!(draw.y, 24.0);
    assert!(runtime.image_draws()[1].source.is_some());
    assert!(runtime.image_draws()[2].rotation_degrees > 0.0);
    assert!(runtime.image_draws()[3].icon_sheet.is_some());

    let released = runtime
        .host
        .borrow_mut()
        .call(extension.release_host_id, &[Value::Handle(handle)])
        .expect("image release");
    assert_eq!(released, Value::Bool(true));
    assert!(runtime.image_draws().is_empty());
    assert!(!runtime.icon_sheets.borrow().contains_key(&handle));
}

#[test]
fn runtime_installs_and_executes_audio_extension() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let extension = runtime.install_audio_extension().expect("install audio");
    let archive = build_archive("audio-sample", &[(200, ResourceType::Audio, b"audio")]);
    runtime.load_archive(&archive).expect("load archive");

    let handle = runtime
        .host
        .borrow_mut()
        .call(extension.load_host_id, &[Value::Integer(200)])
        .expect("audio load");
    let handle = handle_from_value(handle);

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.play_host_id,
                &[Value::Handle(handle), Value::Bool(true)]
            )
            .expect("audio play"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.playback_host_id,
                &[Value::Handle(handle), Value::Bool(false)]
            )
            .expect("audio playback"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                extension.playback_host_id,
                &[Value::Handle(handle), Value::Bool(false)]
            )
            .expect("audio playback"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.status_host_id, &[Value::Handle(handle)])
            .expect("audio status"),
        Value::Integer(2)
    );
    let audio_state = runtime
        .audio_playback_states()
        .get(&handle)
        .cloned()
        .expect("audio state");
    assert!(audio_state.playing);
    assert!(!audio_state.looped);
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.pause_host_id, &[Value::Handle(handle)])
            .expect("audio pause"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.status_host_id, &[Value::Handle(handle)])
            .expect("audio status"),
        Value::Integer(1)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.stop_host_id, &[Value::Handle(handle)])
            .expect("audio stop"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(extension.release_host_id, &[Value::Handle(handle)])
            .expect("audio release"),
        Value::Bool(true)
    );
}

#[test]
fn runtime_vm_save_and_load_restores_state() {
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));
    let image = runtime.install_image_extension().expect("install image");
    let audio = runtime.install_audio_extension().expect("install audio");
    let vm = runtime.install_vm_extension().expect("install vm");
    let archive = build_archive(
        "checkpoint-sample",
        &[
            (100, ResourceType::Image, b"img"),
            (200, ResourceType::Audio, b"audio"),
        ],
    );
    runtime.load_archive(&archive).expect("load archive");

    let image_handle = handle_from_value(
        runtime
            .host
            .borrow_mut()
            .call(image.load_host_id, &[Value::Integer(100)])
            .expect("image load"),
    );
    let audio_handle = handle_from_value(
        runtime
            .host
            .borrow_mut()
            .call(audio.load_host_id, &[Value::Integer(200)])
            .expect("audio load"),
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                image.draw_host_id,
                &[
                    Value::Handle(image_handle),
                    Value::Integer(4),
                    Value::Integer(8)
                ]
            )
            .expect("image draw before save"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                image.set_icon_sheet_host_id,
                &[
                    Value::Handle(image_handle),
                    Value::Integer(16),
                    Value::Integer(16)
                ]
            )
            .expect("set icon sheet before save"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                image.draw_icon_host_id,
                &[
                    Value::Handle(image_handle),
                    Value::Integer(1),
                    Value::Integer(16),
                    Value::Integer(16)
                ]
            )
            .expect("draw icon before save"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(
                audio.play_host_id,
                &[Value::Handle(audio_handle), Value::Bool(true)]
            )
            .expect("audio play"),
        Value::Bool(true)
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(vm.save_host_id, &[Value::Integer(7)])
            .expect("vm save"),
        Value::Bool(true)
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(audio.pause_host_id, &[Value::Handle(audio_handle)])
            .expect("audio pause"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(image.release_host_id, &[Value::Handle(image_handle)])
            .expect("image release"),
        Value::Bool(true)
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(vm.load_host_id, &[Value::Integer(7)])
            .expect("vm load"),
        Value::Bool(true)
    );

    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(image.status_host_id, &[Value::Handle(image_handle)])
            .expect("image status after load"),
        Value::Integer(2)
    );
    assert_eq!(runtime.image_draws().len(), 2);
    assert_eq!(runtime.image_draws()[0].resource_id, 100);
    assert_eq!(runtime.image_draws()[0].icon_sheet, None);
    assert!(runtime.image_draws()[1].icon_sheet.is_some());
    assert_eq!(
        runtime
            .host
            .borrow_mut()
            .call(audio.status_host_id, &[Value::Handle(audio_handle)])
            .expect("audio status after load"),
        Value::Integer(2)
    );
}
