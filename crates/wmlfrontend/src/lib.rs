#![forbid(unsafe_code)]

//! Console frontend that drives WML projects through the toolchain and runtime.

use core::fmt;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub mod demo;
mod gui;

use wmlplatform::PlatformProfile;
use wmlruntime::{
    AudioPlaybackState as RuntimeAudioPlaybackState, IconSheetState, ImageDrawState,
    ImageSourceRect, MessageChoiceState as RuntimeMessageChoiceState,
    MessageWindowState as RuntimeMessageWindowState, Runtime, SharedAudioBackend,
    create_default_audio_backend,
};
use wmltoolchain::{
    BuildArtifact, ExecutionReport, GameProject, Toolchain, ToolchainConfig, ToolchainError,
};
use wmlui::{
    UiApp, UiAudioPlaybackState, UiBackend, UiChoice, UiCommand, UiContext, UiError, UiEvent,
    UiIconSheet, UiImageDrawCall, UiImageRect, UiImageSlot, UiImageSource, UiLogLevel, UiSession,
    UiState, UiTheme,
};
use wmlvm::{RunOutcome, Value};

/// Configuration for the frontend shell.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontendConfig {
    pub platform: PlatformProfile,
    pub project: GameProject,
    pub step_limit: usize,
    pub auto_run: bool,
}

impl FrontendConfig {
    pub fn new(platform: PlatformProfile, project: GameProject) -> Self {
        Self {
            platform,
            project,
            step_limit: 128,
            auto_run: true,
        }
    }

    pub fn with_step_limit(mut self, step_limit: usize) -> Self {
        self.step_limit = step_limit;
        self
    }

    pub fn with_auto_run(mut self, auto_run: bool) -> Self {
        self.auto_run = auto_run;
        self
    }
}

/// Frontend error.
#[derive(Debug)]
pub enum FrontendError {
    Toolchain(ToolchainError),
    Ui(UiError),
    Gui(String),
    MissingReport,
}

impl fmt::Display for FrontendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => write!(f, "{error}"),
            Self::Ui(error) => write!(f, "{error}"),
            Self::Gui(message) => write!(f, "{message}"),
            Self::MissingReport => f.write_str("frontend finished without producing a report"),
        }
    }
}

impl std::error::Error for FrontendError {}

impl From<ToolchainError> for FrontendError {
    fn from(value: ToolchainError) -> Self {
        Self::Toolchain(value)
    }
}

impl From<UiError> for FrontendError {
    fn from(value: UiError) -> Self {
        Self::Ui(value)
    }
}

/// Font preset used by the egui frontend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiFontPreset {
    NotoSans,
    EguiDefault,
    Monospace,
}

impl GuiFontPreset {
    pub const fn default_preset() -> Self {
        Self::NotoSans
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NotoSans => "Noto Sans",
            Self::EguiDefault => "Egui Default",
            Self::Monospace => "Monospace",
        }
    }
}

/// Launches the GUI frontend for a finished report.
pub fn launch_frontend_gui(
    report: FrontendReport,
    font_preset: GuiFontPreset,
) -> Result<FrontendReport, FrontendError> {
    gui::run_gui(report, font_preset)
}

/// Summary returned after running the frontend.
#[derive(Clone)]
pub struct FrontendReport {
    pub build: BuildArtifact,
    pub execution: ExecutionReport,
    pub log_lines: Vec<String>,
    pub ui_state: UiState,
    pub runtime: Runtime,
    pub audio_backend: Rc<SharedAudioBackend>,
}

impl PartialEq for FrontendReport {
    fn eq(&self, other: &Self) -> bool {
        self.build == other.build
            && self.execution == other.execution
            && self.log_lines == other.log_lines
            && self.ui_state == other.ui_state
    }
}

impl fmt::Debug for FrontendReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrontendReport")
            .field("build", &self.build)
            .field("execution", &self.execution)
            .field("log_lines", &self.log_lines)
            .field("ui_state", &self.ui_state)
            .field("audio_backend", &self.audio_backend)
            .finish()
    }
}

/// Console backend that prints the frontend's UI commands.
#[derive(Default)]
pub struct ConsoleBackend {
    closed: bool,
}

impl ConsoleBackend {
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl UiBackend for ConsoleBackend {
    fn apply(&mut self, command: &UiCommand) -> Result<(), UiError> {
        match command {
            UiCommand::SetTitle(title) => println!("[ui] title: {title}"),
            UiCommand::SetCursor(cursor) => println!("[ui] cursor: {cursor:?}"),
            UiCommand::SetTheme(theme) => println!("[ui] theme: {theme:?}"),
            UiCommand::RequestRepaint => println!("[ui] repaint"),
            UiCommand::ClipboardWrite(text) => println!("[ui] clipboard: {text}"),
            UiCommand::OpenUrl(url) => println!("[ui] open url: {url}"),
            UiCommand::Log { level, message } => println!("[ui][{level:?}] {message}"),
            UiCommand::CloseWindow => {
                self.closed = true;
                println!("[ui] close requested");
            }
            UiCommand::SetImage { slot, image } => println!(
                "[ui] image: slot={slot:?} resource={} bytes={} label={}",
                image.resource_id,
                image.bytes.len(),
                image.label
            ),
            UiCommand::ClearImage(slot) => println!("[ui] clear image: slot={slot:?}"),
            UiCommand::SetSceneLayout(layout) => println!(
                "[ui] scene layout: choice=({}, {}, {}, {}), message=({}, {}, {}, {})",
                layout.choice_panel.x,
                layout.choice_panel.y,
                layout.choice_panel.width,
                layout.choice_panel.height,
                layout.message_window.x,
                layout.message_window.y,
                layout.message_window.width,
                layout.message_window.height
            ),
            UiCommand::ShowMessageWindow { speaker, text } => {
                println!("[ui] message window: speaker={speaker:?} text={text}")
            }
            UiCommand::AppendMessageLine(line) => println!("[ui] message line: {line}"),
            UiCommand::SetMessageChoices(choices) => println!(
                "[ui] message choices: {}",
                choices
                    .iter()
                    .map(|choice| choice.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            UiCommand::SetInputPrompt(prompt) => println!("[ui] input prompt: {prompt:?}"),
            UiCommand::HideMessageWindow => println!("[ui] hide message window"),
            UiCommand::ResetScene => println!("[ui] reset scene"),
        }
        Ok(())
    }
}

struct FrontendApp {
    config: FrontendConfig,
    toolchain: Toolchain,
    runtime: Runtime,
    report_slot: Rc<RefCell<Option<FrontendReport>>>,
    error_slot: Rc<RefCell<Option<FrontendError>>>,
    log_lines: Rc<RefCell<Vec<String>>>,
    started: bool,
}

impl FrontendApp {
    fn new(
        config: FrontendConfig,
        report_slot: Rc<RefCell<Option<FrontendReport>>>,
        error_slot: Rc<RefCell<Option<FrontendError>>>,
        log_lines: Rc<RefCell<Vec<String>>>,
    ) -> Self {
        let toolchain = Toolchain::new(
            ToolchainConfig::new(config.platform).with_step_limit(config.step_limit),
        );
        let mut runtime = Runtime::new(
            wmlruntime::RuntimeConfig::new(config.platform).with_step_limit(config.step_limit),
        );
        runtime.set_audio_backend(create_default_audio_backend());
        Self {
            config,
            toolchain,
            runtime,
            report_slot,
            error_slot,
            log_lines,
            started: false,
        }
    }
}

impl UiApp for FrontendApp {
    fn initialize(&mut self, ctx: &mut UiContext<'_>) {
        ctx.set_title(format!(
            "WML Frontend - {}",
            self.config.project.package_name
        ));
        ctx.set_theme(UiTheme::System);
        ctx.reset_scene();
        ctx.log(
            UiLogLevel::Info,
            format!("loaded project {}", self.config.project.script_path),
        );
        ctx.request_repaint();
    }

    fn on_event(&mut self, event: &UiEvent, ctx: &mut UiContext<'_>) {
        if matches!(event, UiEvent::Key { pressed: true, .. }) {
            ctx.request_repaint();
        }
    }

    fn on_frame(&mut self, ctx: &mut UiContext<'_>) {
        if self.started {
            return;
        }
        self.started = true;

        ctx.log(UiLogLevel::Info, "bootstrapping runtime");
        match self
            .toolchain
            .bootstrap_runtime(&mut self.runtime)
            .map_err(ToolchainError::from)
            .and_then(|_| {
                self.toolchain
                    .run_project(&mut self.runtime, &self.config.project)
            }) {
            Ok(execution) => {
                let build = execution.build.clone();
                let log_lines = collect_lines(&build, &execution);
                let story_text = final_story_text(&execution);
                *self.log_lines.borrow_mut() = log_lines.clone();
                let runtime_message = self.runtime.message_window_state();
                let ui_message = to_ui_message_window_state(runtime_message);
                ctx.set_scene_layout(self.runtime.scene_layout_state());
                if ui_message.visible
                    || ui_message.speaker.is_some()
                    || !ui_message.text.is_empty()
                    || !ui_message.choices.is_empty()
                    || ui_message.input_prompt.is_some()
                {
                    if ui_message.visible {
                        ctx.show_message_window(
                            ui_message.speaker.clone(),
                            ui_message.text.clone(),
                        );
                    } else {
                        ctx.hide_message_window();
                        ctx.state_mut().scene.message_window.speaker = ui_message.speaker.clone();
                        ctx.state_mut().scene.message_window.text = ui_message.text.clone();
                    }
                    ctx.state_mut().scene.message_window.backlog = ui_message.backlog.clone();
                    ctx.state_mut().scene.message_window.choices = ui_message.choices.clone();
                    ctx.set_message_choices(ui_message.choices.clone());
                    ctx.set_input_prompt(ui_message.input_prompt.clone());
                } else {
                    ctx.show_message_window(
                        Some(self.config.project.package_name.clone()),
                        story_text
                            .clone()
                            .unwrap_or_else(|| "runtime completed".to_owned()),
                    );
                    ctx.state_mut().scene.message_window.backlog = story_text
                        .as_deref()
                        .map(|text| text.lines().map(|line| line.to_owned()).collect())
                        .unwrap_or_default();
                }
                for line in &log_lines {
                    ctx.log(UiLogLevel::Info, line.clone());
                }
                for asset in &self.config.project.assets {
                    if matches!(asset.resource_type, wmlresource::ResourceType::Image) {
                        ctx.set_image(
                            UiImageSlot::Named(asset.name.clone()),
                            UiImageSource::new(
                                asset.resource_id,
                                asset.name.clone(),
                                asset.payload.clone(),
                            ),
                        );
                    }
                }
                let draw_calls = self
                    .runtime
                    .image_draws()
                    .into_iter()
                    .map(to_ui_draw_call)
                    .collect::<Vec<_>>();
                ctx.set_draw_calls(draw_calls);
                let audio_playback = self
                    .runtime
                    .audio_playback_states()
                    .into_iter()
                    .map(|(handle, state)| (handle, to_ui_audio_state(state)))
                    .collect::<BTreeMap<_, _>>();
                ctx.set_audio_playback(audio_playback);
                ctx.set_title(format!(
                    "WML Frontend - done ({})",
                    self.config.project.package_name
                ));
                if ctx.state().scene.message_window.choices.is_empty() {
                    ctx.set_message_choices(vec![UiChoice::new("close", "Close")]);
                }
                ctx.close_window();
                *self.report_slot.borrow_mut() = Some(FrontendReport {
                    build,
                    execution,
                    log_lines,
                    ui_state: ctx.state().clone(),
                    runtime: self.runtime.clone(),
                    audio_backend: self.runtime.audio_backend_handle(),
                });
            }
            Err(error) => {
                ctx.log(UiLogLevel::Error, error.to_string());
                *self.error_slot.borrow_mut() = Some(FrontendError::from(error));
                ctx.close_window();
            }
        }
    }
}

fn collect_lines(build: &BuildArtifact, execution: &ExecutionReport) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "build: {} sections, archive {} bytes",
        build.manifest.section_digests.len() + 1,
        build.archive.len()
    ));
    lines.push(format!(
        "loaded resources: {}",
        execution.loaded_archive.resources_loaded
    ));
    let image_count = execution.build.manifest.resource_map.iter().count();
    lines.push(format!("resource map entries: {}", image_count));
    lines.push(format!("worker {} finished", execution.worker_id));
    if let Some((_, outcome)) = execution.outcomes.last() {
        lines.push(format!("final outcome: {outcome:?}"));
    }
    lines
}

fn final_story_text(execution: &ExecutionReport) -> Option<String> {
    let (_, outcome) = execution.outcomes.last()?;
    match outcome {
        RunOutcome::Halted {
            value: Some(Value::String(text)),
            ..
        } => Some(text.clone()),
        _ => None,
    }
}

fn to_ui_draw_call(draw: ImageDrawState) -> UiImageDrawCall {
    UiImageDrawCall {
        resource_id: draw.resource_id,
        x: draw.x,
        y: draw.y,
        width: draw.width,
        height: draw.height,
        source: draw.source.map(to_ui_image_rect),
        icon_sheet: draw
            .icon_sheet
            .map(|sheet| to_ui_icon_sheet(sheet, draw.icon_index.unwrap_or(0))),
        rotation_degrees: draw.rotation_degrees,
        opacity: draw.opacity,
    }
}

fn to_ui_image_rect(rect: ImageSourceRect) -> UiImageRect {
    UiImageRect::new(rect.x, rect.y, rect.width, rect.height)
}

fn to_ui_icon_sheet(sheet: IconSheetState, index: u32) -> UiIconSheet {
    UiIconSheet::new(sheet.cell_width, sheet.cell_height, index)
}

fn to_ui_audio_state(state: RuntimeAudioPlaybackState) -> UiAudioPlaybackState {
    UiAudioPlaybackState {
        resource_id: state.resource_id,
        playing: state.playing,
        looped: state.looped,
        position_ms: state.position_ms,
        volume: state.volume,
    }
}

fn to_ui_message_window_state(state: RuntimeMessageWindowState) -> wmlui::UiMessageWindowState {
    wmlui::UiMessageWindowState {
        visible: state.visible,
        speaker: state.speaker,
        text: state.text,
        backlog: state.backlog,
        choices: state.choices.into_iter().map(to_ui_choice).collect(),
        input_prompt: state.input_prompt,
    }
}

fn to_ui_choice(choice: RuntimeMessageChoiceState) -> UiChoice {
    UiChoice {
        id: choice.id,
        label: choice.label,
        enabled: choice.enabled,
    }
}

/// Runs a project through the frontend shell and returns the execution report.
pub fn run_frontend(config: FrontendConfig) -> Result<FrontendReport, FrontendError> {
    let report_slot = Rc::new(RefCell::new(None));
    let error_slot = Rc::new(RefCell::new(None));
    let log_lines = Rc::new(RefCell::new(Vec::new()));
    let app = FrontendApp::new(
        config.clone(),
        report_slot.clone(),
        error_slot.clone(),
        log_lines,
    );
    let mut session = UiSession::new(config.platform, app);
    session.push_event(UiEvent::Frame { dt_seconds: 0.0 });
    session.push_event(UiEvent::TextInput(config.project.script_path.clone()));
    let mut backend = ConsoleBackend::default();
    let _ = session.drive_backend(&mut backend)?;
    if let Some(error) = error_slot.borrow_mut().take() {
        return Err(error);
    }
    report_slot
        .borrow_mut()
        .take()
        .ok_or(FrontendError::MissingReport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmlplatform::PlatformProfile;
    use wmltoolchain::{GameAsset, GameProject};

    #[test]
    fn frontend_runs_game_project_end_to_end() {
        let project = GameProject::new(
            "demo-game",
            "samples/easynovel/main.wml",
            r#"
export func main() {
    return "Prologue";
}
"#,
        )
        .push_asset(GameAsset::image("ui/title", 10, 42, b"title".to_vec()));
        let config = FrontendConfig::new(PlatformProfile::native(), project);

        let report = run_frontend(config).expect("frontend run");
        assert_eq!(report.execution.worker_id, 1);
        assert!(!report.log_lines.is_empty());
        assert_eq!(
            report.ui_state.scene.message_window.speaker.as_deref(),
            Some("demo-game")
        );
        assert!(
            report
                .ui_state
                .scene
                .images
                .contains_key(&UiImageSlot::Named("ui/title".to_owned()))
        );
    }
}
