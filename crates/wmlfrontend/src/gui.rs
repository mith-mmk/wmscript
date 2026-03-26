#![forbid(unsafe_code)]

use core::fmt;
use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::{egui, egui::Vec2};

use crate::{FrontendError, FrontendReport, GuiFontPreset};
use wmlui::{UiChoice, UiImageSlot, UiImageSource, UiKey, UiMouseButton, UiPoint, UiTheme};

/// Runs the GUI window for a finished frontend report.
pub fn run_gui(report: FrontendReport, font_preset: GuiFontPreset) -> Result<(), FrontendError> {
    let title = format!("WML Frontend - {}", report.build.manifest.package_name);
    let size = report.ui_state.window.size;
    let auto_close = matches!(
        std::env::var("WML_FRONTEND_AUTO_CLOSE").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title.clone())
            .with_inner_size(Vec2::new(size.width.max(640.0), size.height.max(480.0))),
        ..Default::default()
    };

    let app = ReportApp::new(report, auto_close, font_preset);
    eframe::run_native(&title, options, Box::new(move |_| Ok(Box::new(app))))
        .map_err(|error| FrontendError::Gui(error.to_string()))
}

struct ReportApp {
    report: FrontendReport,
    textures: BTreeMap<UiImageSlot, egui::TextureHandle>,
    initialized: bool,
    auto_close: bool,
    close_sent: bool,
    input_snapshot: InputSnapshot,
    selected_choice: Option<String>,
    font_preset: GuiFontPreset,
    applied_font_preset: Option<GuiFontPreset>,
}

impl ReportApp {
    fn new(report: FrontendReport, auto_close: bool, font_preset: GuiFontPreset) -> Self {
        Self {
            report,
            textures: BTreeMap::new(),
            initialized: false,
            auto_close,
            close_sent: false,
            input_snapshot: InputSnapshot::default(),
            selected_choice: None,
            font_preset,
            applied_font_preset: None,
        }
    }

    fn theme_visuals(theme: UiTheme) -> egui::Visuals {
        match theme {
            UiTheme::Dark => egui::Visuals::dark(),
            UiTheme::Light => egui::Visuals::light(),
            UiTheme::System => egui::Visuals::dark(),
        }
    }

    fn ensure_textures(&mut self, ctx: &egui::Context) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        for (slot, image) in &self.report.ui_state.scene.images {
            if let Ok(texture) = decode_texture(ctx, image) {
                self.textures.insert(slot.clone(), texture);
            }
        }
    }

    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        if self.applied_font_preset == Some(self.font_preset) {
            return;
        }
        ctx.set_fonts(font_definitions(self.font_preset));
        self.applied_font_preset = Some(self.font_preset);
    }

    fn sync_input_state(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            self.report.ui_state.input.pointer_position = input
                .pointer
                .hover_pos()
                .map(|pos| UiPoint::new(pos.x, pos.y));
            self.report.ui_state.input.modifiers = map_modifiers(input.modifiers);
            self.report.ui_state.input.pressed_buttons = map_pressed_buttons(&input.pointer);
            self.report.ui_state.input.pressed_keys = input
                .keys_down
                .iter()
                .filter_map(|key| map_key_to_ui(*key))
                .collect();

            self.input_snapshot.pointer_position = input.pointer.hover_pos();
            self.input_snapshot.raw_scroll_delta = input.raw_scroll_delta;
            self.input_snapshot.modifiers = input.modifiers;
            self.input_snapshot.pressed_keys =
                input.keys_down.iter().copied().collect::<Vec<egui::Key>>();
            self.input_snapshot.text_input.clear();
            self.input_snapshot.recent_events.clear();
            for event in &input.events {
                match event {
                    egui::Event::Text(text) => {
                        self.input_snapshot.text_input.push_str(text);
                        self.input_snapshot
                            .recent_events
                            .push(format!("text: {text}"));
                    }
                    egui::Event::Key {
                        key,
                        pressed,
                        repeat,
                        modifiers,
                        ..
                    } => {
                        self.input_snapshot.recent_events.push(format!(
                            "key: {key:?} pressed={pressed} repeat={repeat} mods={modifiers:?}"
                        ));
                    }
                    egui::Event::PointerMoved(pos) => {
                        self.input_snapshot
                            .recent_events
                            .push(format!("pointer: {:.1},{:.1}", pos.x, pos.y));
                    }
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        ..
                    } => {
                        self.input_snapshot.recent_events.push(format!(
                            "button: {button:?} pressed={pressed} at {:.1},{:.1}",
                            pos.x, pos.y
                        ));
                    }
                    egui::Event::MouseWheel { delta, .. } => {
                        self.input_snapshot
                            .recent_events
                            .push(format!("scroll: {:.1},{:.1}", delta.x, delta.y));
                    }
                    _ => {}
                }
            }
        });
    }

    fn apply_choice(&mut self, choice: &UiChoice) {
        self.selected_choice = Some(choice.id.clone());
        self.report.ui_state.scene.message_window.visible = true;
        self.report.ui_state.scene.message_window.speaker = Some("Choice".to_owned());
        self.report.ui_state.scene.message_window.text = format!("selected: {}", choice.label);
        self.report
            .ui_state
            .scene
            .message_window
            .backlog
            .push(format!("selected: {}", choice.label));
    }
}

impl eframe::App for ReportApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_textures(ctx);
        self.ensure_fonts(ctx);
        self.sync_input_state(ctx);
        ctx.set_visuals(Self::theme_visuals(self.report.ui_state.window.theme));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "WML Frontend - {}",
            self.report.build.manifest.package_name
        )));

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            self.report.ui_state.scene.message_window.visible =
                !self.report.ui_state.scene.message_window.visible;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Enter))
            && let Some(choice) = self
                .report
                .ui_state
                .scene
                .message_window
                .choices
                .iter()
                .find(|choice| choice.enabled)
                .cloned()
        {
            self.apply_choice(&choice);
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("WML Frontend");
                ui.separator();
                ui.label(format!(
                    "package: {}",
                    self.report.build.manifest.package_name
                ));
                ui.label(format!(
                    "archive: {} bytes",
                    self.report.build.archive.len()
                ));
                ui.label(format!("worker: {}", self.report.execution.worker_id));
                ui.separator();
                egui::ComboBox::from_label("Font")
                    .selected_text(self.font_preset.label())
                    .show_ui(ui, |ui| {
                        for preset in [
                            GuiFontPreset::NotoSans,
                            GuiFontPreset::EguiDefault,
                            GuiFontPreset::Monospace,
                        ] {
                            ui.selectable_value(&mut self.font_preset, preset, preset.label());
                        }
                    });
            });
        });

        egui::SidePanel::left("assets")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Images");
                ui.separator();
                if self.textures.is_empty() {
                    ui.label("No images loaded.");
                } else {
                    for (slot, texture) in &self.textures {
                        ui.group(|ui| {
                            ui.label(slot_name(slot));
                            let size = texture.size_vec2();
                            let max_width = ui.available_width().max(1.0);
                            let scale = (max_width / size.x).min(1.0);
                            let display_size = if scale < 1.0 { size * scale } else { size };
                            ui.image((texture.id(), display_size));
                        });
                        ui.add_space(8.0);
                    }
                }
                ui.separator();
                ui.heading("Choices");
                for choice in self.report.ui_state.scene.message_window.choices.clone() {
                    let selected = self
                        .selected_choice
                        .as_deref()
                        .is_some_and(|selected| selected == choice.id);
                    let response = ui.add_enabled(
                        choice.enabled,
                        egui::Button::new(choice.label.clone()).selected(selected),
                    );
                    if response.clicked() {
                        self.apply_choice(&choice);
                    }
                }
                ui.separator();
                ui.heading("Input");
                ui.label(format!(
                    "pointer: {}",
                    self.input_snapshot
                        .pointer_position
                        .map(|pos| format!("{:.1}, {:.1}", pos.x, pos.y))
                        .unwrap_or_else(|| "none".to_owned())
                ));
                ui.label(format!("modifiers: {:?}", self.input_snapshot.modifiers));
                ui.label(format!(
                    "scroll: {:.1}, {:.1}",
                    self.input_snapshot.raw_scroll_delta.x, self.input_snapshot.raw_scroll_delta.y
                ));
                ui.label(format!(
                    "keys down: {}",
                    self.input_snapshot
                        .pressed_keys
                        .iter()
                        .map(|key| format!("{key:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                if !self.input_snapshot.text_input.is_empty() {
                    ui.label(format!("text: {}", self.input_snapshot.text_input));
                }
                if !self.input_snapshot.recent_events.is_empty() {
                    ui.separator();
                    ui.label("recent events:");
                    for line in self.input_snapshot.recent_events.iter().rev().take(6) {
                        ui.monospace(line);
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Message Window");
            });
            ui.add_space(8.0);

            let window = &self.report.ui_state.scene.message_window;
            egui::Frame::group(ui.style())
                .fill(ui.visuals().panel_fill)
                .show(ui, |ui| {
                    ui.set_min_height(180.0);
                    ui.vertical(|ui| {
                        if let Some(speaker) = &window.speaker {
                            ui.label(egui::RichText::new(speaker).strong());
                        } else {
                            ui.label(egui::RichText::new("Narrator").strong());
                        }
                        ui.separator();
                        if window.visible {
                            for line in window.text.lines() {
                                ui.label(line);
                            }
                        } else {
                            ui.label("Message window hidden.");
                        }
                    });
                });

            ui.add_space(12.0);
            ui.heading("Summary");
            ui.label(format!(
                "events processed: {}",
                self.report.execution.outcomes.len()
            ));
            if let Some((_, outcome)) = self.report.execution.outcomes.last() {
                ui.monospace(format!("{outcome:?}"));
            }

            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ui.add_space(8.0);
            if self.report.ui_state.window.close_requested {
                ui.label("Frontend run completed.");
            }
        });

        if self.auto_close && !self.close_sent {
            self.close_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn font_definitions(preset: GuiFontPreset) -> egui::FontDefinitions {
    match preset {
        GuiFontPreset::NotoSans => {
            let mut fonts = egui::FontDefinitions::default();
            if let Some(bytes) = load_noto_sans_bytes() {
                fonts.font_data.insert(
                    "noto_sans_jp".to_owned(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "noto_sans_jp".to_owned());
            }
            fonts
        }
        GuiFontPreset::EguiDefault => egui::FontDefinitions::default(),
        GuiFontPreset::Monospace => {
            let mut fonts = egui::FontDefinitions::default();
            if let Some(monospace) = fonts.families.get(&egui::FontFamily::Monospace).cloned() {
                fonts
                    .families
                    .insert(egui::FontFamily::Proportional, monospace);
            }
            fonts
        }
    }
}

fn load_noto_sans_bytes() -> Option<Vec<u8>> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("WML_FRONTEND_FONT_PATH") {
        candidates.push(path);
    }
    candidates.extend([
        r"C:\Windows\Fonts\NotoSansJP-VF.ttf".to_owned(),
        r"C:\Windows\Fonts\NotoSansCJKjp-Regular.otf".to_owned(),
        r"C:\Windows\Fonts\NotoSansJP-Regular.otf".to_owned(),
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_owned(),
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".to_owned(),
        "/usr/share/fonts/truetype/noto/NotoSansJP-Regular.ttf".to_owned(),
        "/System/Library/Fonts/Supplemental/NotoSansCJK.ttc".to_owned(),
        "/Library/Fonts/NotoSansJP-Regular.otf".to_owned(),
    ]);
    for path in candidates {
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

fn decode_texture(
    ctx: &egui::Context,
    image: &UiImageSource,
) -> Result<egui::TextureHandle, DecodeTextureError> {
    let decoded = image::load_from_memory(&image.bytes)
        .map_err(|error| DecodeTextureError::new(format!("{}: {}", image.label, error)))?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Ok(ctx.load_texture(
        image.label.clone(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn slot_name(slot: &UiImageSlot) -> String {
    match slot {
        UiImageSlot::Background => "background".to_owned(),
        UiImageSlot::Portrait => "portrait".to_owned(),
        UiImageSlot::Foreground => "foreground".to_owned(),
        UiImageSlot::Overlay => "overlay".to_owned(),
        UiImageSlot::Named(name) => name.clone(),
    }
}

fn map_modifiers(modifiers: egui::Modifiers) -> wmlui::UiModifiers {
    wmlui::UiModifiers {
        shift: modifiers.shift,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        logo: modifiers.mac_cmd || modifiers.command,
    }
}

fn map_pressed_buttons(pointer: &egui::PointerState) -> std::collections::BTreeSet<UiMouseButton> {
    let mut buttons = std::collections::BTreeSet::new();
    if pointer.button_down(egui::PointerButton::Primary) {
        buttons.insert(UiMouseButton::Primary);
    }
    if pointer.button_down(egui::PointerButton::Secondary) {
        buttons.insert(UiMouseButton::Secondary);
    }
    if pointer.button_down(egui::PointerButton::Middle) {
        buttons.insert(UiMouseButton::Middle);
    }
    if pointer.button_down(egui::PointerButton::Extra1) {
        buttons.insert(UiMouseButton::Back);
    }
    if pointer.button_down(egui::PointerButton::Extra2) {
        buttons.insert(UiMouseButton::Forward);
    }
    buttons
}

fn map_key_to_ui(key: egui::Key) -> Option<UiKey> {
    Some(match key {
        egui::Key::Enter => UiKey::Enter,
        egui::Key::Escape => UiKey::Escape,
        egui::Key::Backspace => UiKey::Backspace,
        egui::Key::Tab => UiKey::Tab,
        egui::Key::Space => UiKey::Space,
        egui::Key::ArrowUp => UiKey::ArrowUp,
        egui::Key::ArrowDown => UiKey::ArrowDown,
        egui::Key::ArrowLeft => UiKey::ArrowLeft,
        egui::Key::ArrowRight => UiKey::ArrowRight,
        egui::Key::A => UiKey::Character('a'),
        egui::Key::B => UiKey::Character('b'),
        egui::Key::C => UiKey::Character('c'),
        egui::Key::D => UiKey::Character('d'),
        egui::Key::E => UiKey::Character('e'),
        egui::Key::F => UiKey::Character('f'),
        egui::Key::G => UiKey::Character('g'),
        egui::Key::H => UiKey::Character('h'),
        egui::Key::I => UiKey::Character('i'),
        egui::Key::J => UiKey::Character('j'),
        egui::Key::K => UiKey::Character('k'),
        egui::Key::L => UiKey::Character('l'),
        egui::Key::M => UiKey::Character('m'),
        egui::Key::N => UiKey::Character('n'),
        egui::Key::O => UiKey::Character('o'),
        egui::Key::P => UiKey::Character('p'),
        egui::Key::Q => UiKey::Character('q'),
        egui::Key::R => UiKey::Character('r'),
        egui::Key::S => UiKey::Character('s'),
        egui::Key::T => UiKey::Character('t'),
        egui::Key::U => UiKey::Character('u'),
        egui::Key::V => UiKey::Character('v'),
        egui::Key::W => UiKey::Character('w'),
        egui::Key::X => UiKey::Character('x'),
        egui::Key::Y => UiKey::Character('y'),
        egui::Key::Z => UiKey::Character('z'),
        _ => return None,
    })
}

#[derive(Debug)]
struct DecodeTextureError(String);

#[derive(Default)]
struct InputSnapshot {
    pointer_position: Option<egui::Pos2>,
    raw_scroll_delta: egui::Vec2,
    modifiers: egui::Modifiers,
    pressed_keys: Vec<egui::Key>,
    text_input: String,
    recent_events: Vec<String>,
}

impl DecodeTextureError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for DecodeTextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeTextureError {}
