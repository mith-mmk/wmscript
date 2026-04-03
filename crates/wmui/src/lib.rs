#![forbid(unsafe_code)]

//! Shared UI abstraction layer for WML frontends.
//!
//! This crate keeps the frontend-agnostic pieces of the UI stack in one
//! place: state, input events, and backend commands. Concrete renderers such as
//! egui or WebGL can translate these commands into pixels later.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use wmplatform::PlatformProfile;

/// 2D point in logical UI coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiPoint {
    pub x: f32,
    pub y: f32,
}

impl UiPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// 2D size in logical UI coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiSize {
    pub width: f32,
    pub height: f32,
}

impl UiSize {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Rectangular region in logical UI coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UiRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Input modifiers carried by keyboard and pointer events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

/// Mouse buttons supported by the abstraction layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum UiMouseButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
}

/// Keys supported by the abstraction layer.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum UiKey {
    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Character(char),
}

/// High-level visual theme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiTheme {
    System,
    Light,
    Dark,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::System
    }
}

/// Cursor shape requested by the frontend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiCursor {
    Default,
    Text,
    Pointer,
    Crosshair,
    Grab,
    Grabbing,
    ResizeHorizontal,
    ResizeVertical,
    Wait,
    Hidden,
}

impl Default for UiCursor {
    fn default() -> Self {
        Self::Default
    }
}

/// Slot used to place an image on screen.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum UiImageSlot {
    Background,
    Portrait,
    Foreground,
    Overlay,
    Named(String),
}

/// Image payload made available to a backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiImageSource {
    pub resource_id: u32,
    pub label: String,
    pub bytes: Vec<u8>,
}

impl UiImageSource {
    pub fn new(resource_id: u32, label: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            resource_id,
            label: label.into(),
            bytes: bytes.into(),
        }
    }
}

/// Rectangular region in image coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiImageRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UiImageRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Icon sheet metadata for sprite-like drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiIconSheet {
    pub cell_width: u32,
    pub cell_height: u32,
    pub index: u32,
}

impl UiIconSheet {
    pub const fn new(cell_width: u32, cell_height: u32, index: u32) -> Self {
        Self {
            cell_width,
            cell_height,
            index,
        }
    }
}

/// A single draw call for an image resource.
#[derive(Clone, Debug, PartialEq)]
pub struct UiImageDrawCall {
    pub resource_id: u32,
    pub x: f32,
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub source: Option<UiImageRect>,
    pub icon_sheet: Option<UiIconSheet>,
    pub rotation_degrees: f32,
    pub opacity: f32,
}

impl UiImageDrawCall {
    pub const fn new(resource_id: u32, x: f32, y: f32) -> Self {
        Self {
            resource_id,
            x,
            y,
            width: None,
            height: None,
            source: None,
            icon_sheet: None,
            rotation_degrees: 0.0,
            opacity: 1.0,
        }
    }
}

/// Playback state for an audio resource.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiAudioPlaybackState {
    pub resource_id: u32,
    pub playing: bool,
    pub looped: bool,
    pub position_ms: u64,
    pub volume: f32,
}

/// Choice shown in a message window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiChoice {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

impl UiChoice {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }
}

/// RGBA color used by the runtime UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl UiColorRgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for UiColorRgba {
    fn default() -> Self {
        Self::new(255, 255, 255, 255)
    }
}

/// Insets from the outer message window frame into the content region.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UiInsets {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}
/// Style used by the message window and related runtime UI panels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiMessageWindowStyle {
    pub panel_fill: UiColorRgba,
    pub panel_stroke: UiColorRgba,
    pub text_color: UiColorRgba,
    pub speaker_color: UiColorRgba,
    pub accent_color: UiColorRgba,
    pub body_font_size: f32,
    pub speaker_font_size: f32,
    pub frame_resource_id: Option<u32>,
    pub content_inset: UiInsets,
}

impl Default for UiMessageWindowStyle {
    fn default() -> Self {
        Self {
            panel_fill: UiColorRgba::new(0, 0, 0, 190),
            panel_stroke: UiColorRgba::new(46, 140, 58, 255),
            text_color: UiColorRgba::new(255, 255, 255, 255),
            speaker_color: UiColorRgba::new(255, 255, 255, 255),
            accent_color: UiColorRgba::new(180, 220, 180, 255),
            body_font_size: 18.0,
            speaker_font_size: 20.0,
            frame_resource_id: None,
            content_inset: UiInsets::new(18.0, 16.0, 18.0, 14.0),
        }
    }
}

/// State of the message window.
#[derive(Clone, Debug, PartialEq)]
pub struct UiMessageWindowState {
    pub visible: bool,
    pub speaker: Option<String>,
    pub text: String,
    pub backlog: Vec<String>,
    pub choices: Vec<UiChoice>,
    pub input_prompt: Option<String>,
    pub text_speed: f32,
    pub auto_mode: bool,
    pub skip_mode: bool,
    pub style: UiMessageWindowStyle,
}

impl Default for UiMessageWindowState {
    fn default() -> Self {
        Self {
            visible: false,
            speaker: None,
            text: String::new(),
            backlog: Vec::new(),
            choices: Vec::new(),
            input_prompt: None,
            text_speed: 48.0,
            auto_mode: false,
            skip_mode: false,
            style: UiMessageWindowStyle::default(),
        }
    }
}

/// State of the active scene.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiSceneState {
    pub images: BTreeMap<UiImageSlot, UiImageSource>,
    pub draw_calls: Vec<UiImageDrawCall>,
    pub audio_playback: BTreeMap<u64, UiAudioPlaybackState>,
    pub layout: UiSceneLayoutState,
    pub message_window: UiMessageWindowState,
}

/// Layout used by the frontend to place in-game panels.
#[derive(Clone, Debug, PartialEq)]
pub struct UiSceneLayoutState {
    pub reference_size: UiSize,
    pub choice_panel: UiRect,
    pub choice_panel_z: i32,
    pub message_window: UiRect,
    pub message_window_z: i32,
    pub input_panel_z: i32,
}
impl Default for UiSceneLayoutState {
    fn default() -> Self {
        Self {
            reference_size: UiSize::new(1280.0, 720.0),
            choice_panel: UiRect::new(270.0, 124.0, 560.0, 210.0),
            choice_panel_z: 10,
            message_window: UiRect::new(20.0, 502.0, 1240.0, 184.0),
            message_window_z: 30,
            input_panel_z: 20,
        }
    }
}

/// State maintained by the outer window shell.
#[derive(Clone, Debug, PartialEq)]
pub struct UiWindowState {
    pub title: String,
    pub size: UiSize,
    pub scale_factor: f32,
    pub focused: bool,
    pub theme: UiTheme,
    pub cursor: UiCursor,
    pub close_requested: bool,
}

impl UiWindowState {
    pub fn new(title: impl Into<String>, size: UiSize) -> Self {
        Self {
            title: title.into(),
            size,
            scale_factor: 1.0,
            focused: true,
            theme: UiTheme::System,
            cursor: UiCursor::Default,
            close_requested: false,
        }
    }
}

impl Default for UiWindowState {
    fn default() -> Self {
        Self::new("WML UI", UiSize::new(1280.0, 720.0))
    }
}

/// State maintained by the input subsystem.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiInputState {
    pub pointer_position: Option<UiPoint>,
    pub pressed_buttons: BTreeSet<UiMouseButton>,
    pub pressed_keys: BTreeSet<UiKey>,
    pub modifiers: UiModifiers,
}

/// Full UI state shared with the frontend-agnostic application layer.
#[derive(Clone, Debug, PartialEq)]
pub struct UiState {
    pub platform: PlatformProfile,
    pub window: UiWindowState,
    pub input: UiInputState,
    pub scene: UiSceneState,
}

impl UiState {
    pub fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            window: UiWindowState::default(),
            input: UiInputState::default(),
            scene: UiSceneState::default(),
        }
    }
}

/// Input and lifecycle events observed by the abstraction layer.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    Resize {
        width: f32,
        height: f32,
        scale_factor: f32,
    },
    FocusChanged(bool),
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerButton {
        button: UiMouseButton,
        pressed: bool,
        position: Option<UiPoint>,
        modifiers: UiModifiers,
    },
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    Key {
        key: UiKey,
        pressed: bool,
        modifiers: UiModifiers,
    },
    TextInput(String),
    ThemeChanged(UiTheme),
    Frame {
        dt_seconds: f32,
    },
    CloseRequested,
}

/// Commands emitted by the UI application layer and consumed by a backend.
#[derive(Clone, Debug, PartialEq)]
pub enum UiCommand {
    SetTitle(String),
    SetCursor(UiCursor),
    SetTheme(UiTheme),
    RequestRepaint,
    ClipboardWrite(String),
    OpenUrl(String),
    Log {
        level: UiLogLevel,
        message: String,
    },
    CloseWindow,
    SetImage {
        slot: UiImageSlot,
        image: UiImageSource,
    },
    ClearImage(UiImageSlot),
    SetSceneLayout(UiSceneLayoutState),
    ShowMessageWindow {
        speaker: Option<String>,
        text: String,
    },
    AppendMessageLine(String),
    SetMessageChoices(Vec<UiChoice>),
    SetInputPrompt(Option<String>),
    SetMessageSpeed(f32),
    SetMessageAuto(bool),
    SetMessageSkip(bool),
    HideMessageWindow,
    ResetScene,
}

/// Logging levels that backends may forward to a host log sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Error returned by UI backends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiError {
    UnsupportedCommand(&'static str),
    BackendFailure(String),
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedCommand(command) => {
                write!(f, "unsupported UI command: {command}")
            }
            Self::BackendFailure(reason) => write!(f, "UI backend failure: {reason}"),
        }
    }
}

impl std::error::Error for UiError {}

/// Mutable application context passed to the UI layer.
pub struct UiContext<'a> {
    state: &'a mut UiState,
    commands: &'a mut Vec<UiCommand>,
}

impl<'a> UiContext<'a> {
    fn new(state: &'a mut UiState, commands: &'a mut Vec<UiCommand>) -> Self {
        Self { state, commands }
    }

    pub fn state(&self) -> &UiState {
        self.state
    }

    pub fn state_mut(&mut self) -> &mut UiState {
        self.state
    }

    pub fn window(&self) -> &UiWindowState {
        &self.state.window
    }

    pub fn input(&self) -> &UiInputState {
        &self.state.input
    }

    pub fn emit(&mut self, command: UiCommand) {
        self.commands.push(command);
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.state.window.title = title.into();
        self.emit(UiCommand::SetTitle(self.state.window.title.clone()));
    }

    pub fn set_cursor(&mut self, cursor: UiCursor) {
        self.state.window.cursor = cursor;
        self.emit(UiCommand::SetCursor(cursor));
    }

    pub fn set_theme(&mut self, theme: UiTheme) {
        self.state.window.theme = theme;
        self.emit(UiCommand::SetTheme(theme));
    }

    pub fn request_repaint(&mut self) {
        self.emit(UiCommand::RequestRepaint);
    }

    pub fn close_window(&mut self) {
        self.state.window.close_requested = true;
        self.emit(UiCommand::CloseWindow);
    }

    pub fn set_image(&mut self, slot: UiImageSlot, image: UiImageSource) {
        self.state.scene.images.insert(slot.clone(), image.clone());
        self.emit(UiCommand::SetImage { slot, image });
    }

    pub fn set_draw_calls(&mut self, draw_calls: Vec<UiImageDrawCall>) {
        self.state.scene.draw_calls = draw_calls;
    }

    pub fn set_audio_playback(&mut self, audio_playback: BTreeMap<u64, UiAudioPlaybackState>) {
        self.state.scene.audio_playback = audio_playback;
    }

    pub fn set_scene_layout(&mut self, layout: UiSceneLayoutState) {
        self.state.scene.layout = layout.clone();
        self.emit(UiCommand::SetSceneLayout(layout));
    }

    pub fn clear_image(&mut self, slot: UiImageSlot) {
        self.state.scene.images.remove(&slot);
        self.emit(UiCommand::ClearImage(slot));
    }

    pub fn show_message_window(&mut self, speaker: Option<String>, text: impl Into<String>) {
        self.state.scene.message_window.visible = true;
        self.state.scene.message_window.speaker = speaker.clone();
        self.state.scene.message_window.text = text.into();
        self.emit(UiCommand::ShowMessageWindow {
            speaker,
            text: self.state.scene.message_window.text.clone(),
        });
    }

    pub fn append_message_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        if !self.state.scene.message_window.text.is_empty() {
            self.state.scene.message_window.text.push('\n');
        }
        self.state.scene.message_window.text.push_str(&line);
        self.state.scene.message_window.backlog.push(line.clone());
        self.emit(UiCommand::AppendMessageLine(line));
    }

    pub fn set_message_choices(&mut self, choices: Vec<UiChoice>) {
        self.state.scene.message_window.choices = choices.clone();
        self.emit(UiCommand::SetMessageChoices(choices));
    }

    pub fn set_input_prompt(&mut self, prompt: Option<String>) {
        self.state.scene.message_window.input_prompt = prompt.clone();
        self.emit(UiCommand::SetInputPrompt(prompt));
    }

    pub fn set_message_speed(&mut self, speed: f32) {
        self.state.scene.message_window.text_speed = speed.max(0.0);
        self.emit(UiCommand::SetMessageSpeed(
            self.state.scene.message_window.text_speed,
        ));
    }

    pub fn set_message_auto(&mut self, enabled: bool) {
        self.state.scene.message_window.auto_mode = enabled;
        self.emit(UiCommand::SetMessageAuto(enabled));
    }

    pub fn set_message_skip(&mut self, enabled: bool) {
        self.state.scene.message_window.skip_mode = enabled;
        self.emit(UiCommand::SetMessageSkip(enabled));
    }

    pub fn hide_message_window(&mut self) {
        self.state.scene.message_window.visible = false;
        self.emit(UiCommand::HideMessageWindow);
    }

    pub fn reset_scene(&mut self) {
        self.state.scene = UiSceneState::default();
        self.emit(UiCommand::ResetScene);
    }

    pub fn log(&mut self, level: UiLogLevel, message: impl Into<String>) {
        self.emit(UiCommand::Log {
            level,
            message: message.into(),
        });
    }
}

/// Frontend-agnostic application interface.
pub trait UiApp {
    fn initialize(&mut self, _ctx: &mut UiContext<'_>) {}

    fn on_event(&mut self, _event: &UiEvent, _ctx: &mut UiContext<'_>) {}

    fn on_frame(&mut self, _ctx: &mut UiContext<'_>) {}
}

/// Backend interface used by egui, WebGL, or test drivers.
pub trait UiBackend {
    fn apply(&mut self, command: &UiCommand) -> Result<(), UiError>;

    fn present(&mut self) -> Result<(), UiError> {
        Ok(())
    }
}

/// Outcome returned after driving a UI frame.
#[derive(Clone, Debug, PartialEq)]
pub struct UiStepOutcome {
    pub state: UiState,
    pub events_processed: usize,
    pub commands: Vec<UiCommand>,
}

/// UI session that owns the application and its shared state.
pub struct UiSession<A> {
    app: A,
    state: UiState,
    pending_events: VecDeque<UiEvent>,
    initialized: bool,
}

impl<A: UiApp> UiSession<A> {
    pub fn new(platform: PlatformProfile, app: A) -> Self {
        Self {
            app,
            state: UiState::new(platform),
            pending_events: VecDeque::new(),
            initialized: false,
        }
    }

    pub fn state(&self) -> &UiState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut UiState {
        &mut self.state
    }

    pub fn push_event(&mut self, event: UiEvent) {
        self.apply_event(&event);
        self.pending_events.push_back(event);
    }

    pub fn step(&mut self) -> UiStepOutcome {
        let mut commands = Vec::new();
        let mut processed = 0usize;
        {
            let mut ctx = UiContext::new(&mut self.state, &mut commands);
            if !self.initialized {
                self.app.initialize(&mut ctx);
                self.initialized = true;
            }
            while let Some(event) = self.pending_events.pop_front() {
                self.app.on_event(&event, &mut ctx);
                processed += 1;
            }
            self.app.on_frame(&mut ctx);
        }
        UiStepOutcome {
            state: self.state.clone(),
            events_processed: processed,
            commands,
        }
    }

    pub fn drive_backend<B: UiBackend>(
        &mut self,
        backend: &mut B,
    ) -> Result<UiStepOutcome, UiError> {
        let outcome = self.step();
        for command in &outcome.commands {
            backend.apply(command)?;
        }
        backend.present()?;
        Ok(outcome)
    }

    fn apply_event(&mut self, event: &UiEvent) {
        match event {
            UiEvent::Resize {
                width,
                height,
                scale_factor,
            } => {
                self.state.window.size = UiSize::new(*width, *height);
                self.state.window.scale_factor = *scale_factor;
            }
            UiEvent::FocusChanged(focused) => {
                self.state.window.focused = *focused;
            }
            UiEvent::PointerMoved { x, y } => {
                self.state.input.pointer_position = Some(UiPoint::new(*x, *y));
            }
            UiEvent::PointerButton {
                button,
                pressed,
                position,
                modifiers,
            } => {
                if let Some(position) = position {
                    self.state.input.pointer_position = Some(*position);
                }
                if *pressed {
                    self.state.input.pressed_buttons.insert(*button);
                } else {
                    self.state.input.pressed_buttons.remove(button);
                }
                self.state.input.modifiers = *modifiers;
            }
            UiEvent::Scroll { .. } => {}
            UiEvent::Key {
                key,
                pressed,
                modifiers,
            } => {
                if *pressed {
                    self.state.input.pressed_keys.insert(key.clone());
                } else {
                    self.state.input.pressed_keys.remove(key);
                }
                self.state.input.modifiers = *modifiers;
            }
            UiEvent::TextInput(_) => {}
            UiEvent::ThemeChanged(theme) => {
                self.state.window.theme = *theme;
            }
            UiEvent::Frame { .. } => {}
            UiEvent::CloseRequested => {
                self.state.window.close_requested = true;
            }
        }
    }
}

/// Helper for applying a UI step to a backend.
pub fn drive_backend<B: UiBackend>(
    backend: &mut B,
    outcome: &UiStepOutcome,
) -> Result<(), UiError> {
    for command in &outcome.commands {
        backend.apply(command)?;
    }
    backend.present()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DemoApp {
        frames: usize,
        last_text: Option<String>,
    }

    impl DemoApp {
        fn new() -> Self {
            Self {
                frames: 0,
                last_text: None,
            }
        }
    }

    impl UiApp for DemoApp {
        fn on_event(&mut self, event: &UiEvent, ctx: &mut UiContext<'_>) {
            if let UiEvent::TextInput(text) = event {
                self.last_text = Some(text.clone());
                ctx.log(UiLogLevel::Info, format!("input: {text}"));
            }
        }

        fn on_frame(&mut self, ctx: &mut UiContext<'_>) {
            self.frames += 1;
            ctx.set_title(format!("frame {}", self.frames));
            ctx.request_repaint();
        }
    }

    #[derive(Default)]
    struct RecordingBackend {
        commands: Vec<UiCommand>,
        presents: usize,
    }

    impl UiBackend for RecordingBackend {
        fn apply(&mut self, command: &UiCommand) -> Result<(), UiError> {
            self.commands.push(command.clone());
            Ok(())
        }

        fn present(&mut self) -> Result<(), UiError> {
            self.presents += 1;
            Ok(())
        }
    }

    struct MessageApp;

    impl UiApp for MessageApp {
        fn initialize(&mut self, ctx: &mut UiContext<'_>) {
            ctx.reset_scene();
            ctx.show_message_window(Some("Narrator".to_owned()), "Hello");
            ctx.set_image(
                UiImageSlot::Background,
                UiImageSource::new(7, "bg", vec![1, 2, 3]),
            );
            ctx.append_message_line("World");
            ctx.set_message_choices(vec![UiChoice::new("next", "Next")]);
        }
    }

    #[test]
    fn session_tracks_input_and_window_state() {
        let mut session = UiSession::new(PlatformProfile::native(), DemoApp::new());
        session.push_event(UiEvent::Resize {
            width: 1920.0,
            height: 1080.0,
            scale_factor: 1.5,
        });
        session.push_event(UiEvent::PointerMoved { x: 10.0, y: 20.0 });
        session.push_event(UiEvent::Key {
            key: UiKey::Enter,
            pressed: true,
            modifiers: UiModifiers {
                shift: true,
                ctrl: false,
                alt: false,
                logo: false,
            },
        });

        let outcome = session.step();
        assert_eq!(outcome.events_processed, 3);
        assert_eq!(outcome.state.window.size, UiSize::new(1920.0, 1080.0));
        assert_eq!(outcome.state.window.scale_factor, 1.5);
        assert_eq!(
            outcome.state.input.pointer_position,
            Some(UiPoint::new(10.0, 20.0))
        );
        assert!(outcome.state.input.pressed_keys.contains(&UiKey::Enter));
        assert_eq!(
            outcome.commands,
            vec![
                UiCommand::SetTitle("frame 1".to_owned()),
                UiCommand::RequestRepaint
            ]
        );
    }

    #[test]
    fn backend_driver_forwards_commands() {
        let mut session = UiSession::new(PlatformProfile::native(), DemoApp::new());
        let mut backend = RecordingBackend::default();

        let outcome = session.drive_backend(&mut backend).expect("drive");

        assert_eq!(backend.presents, 1);
        assert_eq!(backend.commands, outcome.commands);
        assert!(
            backend
                .commands
                .iter()
                .any(|command| matches!(command, UiCommand::SetTitle(title) if title == "frame 1"))
        );
    }

    #[test]
    fn message_window_and_images_are_captured_in_state() {
        let mut session = UiSession::new(PlatformProfile::native(), MessageApp);
        let outcome = session.step();

        assert!(
            outcome
                .commands
                .iter()
                .any(|command| matches!(command, UiCommand::ResetScene))
        );
        assert!(matches!(
            session.state().scene.message_window.speaker.as_deref(),
            Some("Narrator")
        ));
        assert!(session.state().scene.message_window.visible);
        assert!(
            session
                .state()
                .scene
                .images
                .contains_key(&UiImageSlot::Background)
        );
        assert_eq!(
            session.state().scene.message_window.choices[0].label,
            "Next"
        );
    }
}
