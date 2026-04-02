#![forbid(unsafe_code)]

use core::fmt;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use eframe::{egui, egui::Vec2};

use crate::{FrontendError, FrontendReport, GuiFontPreset};
use wmui::{
    UiChoice, UiColorRgba, UiImageDrawCall, UiImageSlot, UiImageSource, UiKey, UiMouseButton,
    UiPoint, UiRect, UiSceneLayoutState, UiTheme,
};
use wmvm::{Message, Value};

/// Runs the GUI window for a finished frontend report.
pub fn run_gui(
    report: FrontendReport,
    font_preset: GuiFontPreset,
) -> Result<FrontendReport, FrontendError> {
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

    let report_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let app = ReportApp::new(report, auto_close, font_preset, report_slot.clone());
    eframe::run_native(&title, options, Box::new(move |_| Ok(Box::new(app))))
        .map_err(|error| FrontendError::Gui(error.to_string()))?;
    report_slot
        .borrow_mut()
        .take()
        .ok_or_else(|| FrontendError::Gui("GUI exited without a report".to_owned()))
}

struct ReportApp {
    report: FrontendReport,
    report_slot: std::rc::Rc<std::cell::RefCell<Option<FrontendReport>>>,
    textures: BTreeMap<UiImageSlot, egui::TextureHandle>,
    textures_by_resource_id: BTreeMap<u32, TextureEntry>,
    initialized: bool,
    auto_close: bool,
    close_sent: bool,
    input_snapshot: InputSnapshot,
    player_input: String,
    selected_choice: Option<String>,
    message_history_open: bool,
    debug_panel_open: bool,
    runtime_menu_open: bool,
    selected_checkpoint_slot: u32,
    runtime_status_line: Option<String>,
    message_reveal_chars: usize,
    message_signature: Option<String>,
    auto_advance_sent: bool,
    auto_advance_elapsed_seconds: f32,
    font_preset: GuiFontPreset,
    applied_font_preset: Option<GuiFontPreset>,
}

#[derive(Clone)]
struct TextureEntry {
    texture: egui::TextureHandle,
    size: egui::Vec2,
}

impl ReportApp {
    fn new(
        report: FrontendReport,
        auto_close: bool,
        font_preset: GuiFontPreset,
        report_slot: std::rc::Rc<std::cell::RefCell<Option<FrontendReport>>>,
    ) -> Self {
        Self {
            report,
            report_slot,
            textures: BTreeMap::new(),
            textures_by_resource_id: BTreeMap::new(),
            initialized: false,
            auto_close,
            close_sent: false,
            input_snapshot: InputSnapshot::default(),
            player_input: String::new(),
            selected_choice: None,
            message_history_open: false,
            debug_panel_open: false,
            runtime_menu_open: false,
            selected_checkpoint_slot: 1,
            runtime_status_line: None,
            message_reveal_chars: 0,
            message_signature: None,
            auto_advance_sent: false,
            auto_advance_elapsed_seconds: 0.0,
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
                self.textures_by_resource_id.insert(
                    image.resource_id,
                    TextureEntry {
                        size: texture.size_vec2(),
                        texture: texture.clone(),
                    },
                );
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
        self.report
            .runtime
            .set_state_value("ui.last_choice", Value::String(choice.id.clone()));
        self.report
            .runtime
            .set_state_value("ui.last_reply", Value::String(choice.id.clone()));
        self.send_user_reply(Value::String(choice.id.clone()));
    }

    fn submit_player_input(&mut self) {
        let text = self.player_input.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.player_input.clear();
        self.selected_choice = None;
        self.report
            .runtime
            .set_state_value("ui.last_input", Value::String(text.clone()));
        self.report
            .runtime
            .set_state_value("ui.last_reply", Value::String(text.clone()));
        self.send_user_reply(Value::String(text));
    }

    fn can_advance_message(&self) -> bool {
        let message = &self.report.ui_state.scene.message_window;
        message.visible && message.choices.is_empty() && message.input_prompt.is_none()
    }

    fn advance_message(&mut self) -> bool {
        if !self.can_advance_message() {
            return false;
        }
        let text_len = self
            .report
            .ui_state
            .scene
            .message_window
            .text
            .chars()
            .count();
        if self.message_reveal_chars < text_len {
            self.message_reveal_chars = text_len;
            self.auto_advance_sent = false;
            return true;
        }
        if self.report.runtime.waiting_workers().is_empty() {
            return false;
        }
        self.send_user_reply(Value::Nil);
        true
    }

    fn send_user_reply(&mut self, payload: Value) {
        let target_worker_id = {
            let waiting = self.report.runtime.waiting_workers();
            if waiting.len() == 1 {
                waiting[0]
            } else {
                self.report.execution.worker_id
            }
        };
        self.report
            .runtime
            .send_message(Message::new(0, target_worker_id, 0, payload));
        let outcomes = self.report.runtime.run_until_idle(8);
        if !outcomes.is_empty() {
            self.report.execution.outcomes.extend(outcomes);
        }
        self.sync_runtime_state();
    }

    fn reset_message_progress(&mut self) {
        self.message_reveal_chars = 0;
        self.auto_advance_sent = false;
        self.auto_advance_elapsed_seconds = 0.0;
    }

    fn save_runtime_slot(&mut self) {
        let slot = self.selected_checkpoint_slot;
        self.report.runtime.save_checkpoint(slot);
        self.runtime_status_line = Some(format!("saved slot {slot}"));
    }

    fn load_runtime_slot(&mut self) {
        let slot = self.selected_checkpoint_slot;
        match self.report.runtime.load_checkpoint(slot) {
            Ok(true) => {
                self.sync_runtime_state();
                self.reset_message_progress();
                self.runtime_status_line = Some(format!("loaded slot {slot}"));
            }
            Ok(false) => {
                self.runtime_status_line = Some(format!("slot {slot} is empty"));
            }
            Err(error) => {
                self.runtime_status_line = Some(format!("load failed: {error}"));
            }
        }
    }

    fn sync_runtime_state(&mut self) {
        let runtime_message = self.report.runtime.message_window_state();
        self.report.ui_state.scene.message_window =
            crate::to_ui_message_window_state(runtime_message);
        self.report.ui_state.scene.layout = self.report.runtime.scene_layout_state();
        self.report.ui_state.scene.draw_calls = self
            .report
            .runtime
            .image_draws()
            .into_iter()
            .map(crate::to_ui_draw_call)
            .collect();
        self.report.ui_state.scene.audio_playback = self
            .report
            .runtime
            .audio_playback_states()
            .into_iter()
            .map(|(handle, state)| (handle, crate::to_ui_audio_state(state)))
            .collect::<BTreeMap<_, _>>();
        if self.report.ui_state.scene.message_window.choices.is_empty() {
            self.selected_choice = None;
        }
    }

    fn message_signature(message: &wmui::UiMessageWindowState) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            message.visible,
            message.speaker.as_deref().unwrap_or_default(),
            message.text,
            message.choices.len(),
            message.input_prompt.as_deref().unwrap_or_default(),
            message.text_speed.to_bits(),
            message.auto_mode as u8 ^ ((message.skip_mode as u8) << 1),
        )
    }

    fn update_message_reveal(&mut self, ctx: &egui::Context) {
        let signature = {
            let message = &self.report.ui_state.scene.message_window;
            Self::message_signature(message)
        };
        if self.message_signature.as_deref() != Some(signature.as_str()) {
            self.message_signature = Some(signature);
            self.reset_message_progress();
        }

        let message = &self.report.ui_state.scene.message_window;
        let text_len = message.text.chars().count();
        if !message.visible || text_len == 0 {
            self.message_reveal_chars = 0;
            self.auto_advance_elapsed_seconds = 0.0;
            return;
        }

        let dt = ctx.input(|input| input.stable_dt).max(0.0);

        if message.skip_mode || message.text_speed <= 0.0 {
            self.message_reveal_chars = text_len;
        } else {
            let advance = (message.text_speed * dt).ceil() as usize;
            self.message_reveal_chars = self
                .message_reveal_chars
                .saturating_add(advance)
                .min(text_len);
        }

        let reveal_complete = self.message_reveal_chars >= text_len;
        let can_auto_advance = reveal_complete
            && !self.auto_advance_sent
            && message.choices.is_empty()
            && message.input_prompt.is_none()
            && !self.report.runtime.waiting_workers().is_empty()
            && (message.auto_mode || message.skip_mode);

        if can_auto_advance {
            self.auto_advance_elapsed_seconds += dt;
            if self.auto_advance_elapsed_seconds
                >= Self::message_advance_delay_seconds(message, text_len)
            {
                self.auto_advance_sent = true;
                self.send_user_reply(Value::Nil);
            }
        } else if !reveal_complete {
            self.auto_advance_elapsed_seconds = 0.0;
        }

        if !reveal_complete || can_auto_advance {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn message_advance_delay_seconds(message: &wmui::UiMessageWindowState, text_len: usize) -> f32 {
        if message.skip_mode {
            return 0.08;
        }
        if message.auto_mode {
            let chars = text_len as f32;
            return (0.55 + chars * 0.018).clamp(0.55, 2.40);
        }
        f32::INFINITY
    }

    fn revealed_message_text(&self, text: &str) -> String {
        if self.message_reveal_chars == 0 {
            return String::new();
        }
        text.chars().take(self.message_reveal_chars).collect()
    }

    fn egui_color(color: UiColorRgba) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(color.r, color.g, color.b, color.a)
    }

    fn scene_canvas_rect(stage_rect: egui::Rect, layout: &UiSceneLayoutState) -> (egui::Rect, f32) {
        let ref_size = layout.reference_size;
        let scale_x = if ref_size.width > 0.0 {
            stage_rect.width() / ref_size.width
        } else {
            1.0
        };
        let scale_y = if ref_size.height > 0.0 {
            stage_rect.height() / ref_size.height
        } else {
            1.0
        };
        let scale = scale_x.min(scale_y).max(0.1);
        let size = egui::vec2(ref_size.width * scale, ref_size.height * scale);
        let min = stage_rect.center() - size / 2.0;
        (egui::Rect::from_min_size(min, size), scale)
    }

    fn scale_scene_rect(rect: UiRect, canvas_rect: egui::Rect, scale: f32) -> egui::Rect {
        let min = canvas_rect.min + egui::vec2(rect.x * scale, rect.y * scale);
        egui::Rect::from_min_size(min, egui::vec2(rect.width * scale, rect.height * scale))
    }

    fn draw_scene_overlays(&mut self, ctx: &egui::Context, stage_rect: egui::Rect) {
        let layout = self.report.ui_state.scene.layout.clone();
        let message = self.report.ui_state.scene.message_window.clone();
        let choices = message.choices.clone();
        let input_prompt = message.input_prompt.clone();
        let visible = message.visible;
        let speaker = message
            .speaker
            .as_deref()
            .filter(|speaker| !speaker.is_empty())
            .unwrap_or("Narrator")
            .to_owned();
        let revealed_text = self.revealed_message_text(&message.text);
        let text_lines = revealed_text
            .lines()
            .map(|line| line.to_owned())
            .collect::<Vec<_>>();
        let backlog = message.backlog.clone();
        let can_advance = choices.is_empty() && input_prompt.is_none();
        let reveal_complete = self.message_reveal_chars >= message.text.chars().count();
        let style = message.style.clone();
        let panel_fill = Self::egui_color(style.panel_fill);
        let panel_stroke = Self::egui_color(style.panel_stroke);
        let text_color = Self::egui_color(style.text_color);
        let speaker_color = Self::egui_color(style.speaker_color);
        let accent_color = Self::egui_color(style.accent_color);
        let (canvas_rect, scale) = Self::scene_canvas_rect(stage_rect, &layout);
        let body_text_size = style.body_font_size * scale.max(0.75);
        let speaker_text_size = style.speaker_font_size * scale.max(0.75);

        let choice_rect = Self::scale_scene_rect(layout.choice_panel, canvas_rect, scale);
        if visible && !choices.is_empty() {
            egui::Area::new(egui::Id::new("choice_panel"))
                .order(egui::Order::Foreground)
                .fixed_pos(choice_rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(choice_rect.size());
                    egui::Frame::NONE
                        .fill(panel_fill)
                        .stroke(egui::Stroke::new((2.0 * scale).max(1.0), panel_stroke))
                        .show(ui, |ui| {
                            ui.add_space(10.0 * scale);
                            ui.label(
                                egui::RichText::new("Choices")
                                    .size(speaker_text_size)
                                    .color(accent_color),
                            );
                            ui.add_space(8.0 * scale);
                            for choice in &choices {
                                let selected = self
                                    .selected_choice
                                    .as_deref()
                                    .is_some_and(|selected| selected == choice.id);
                                let button = egui::Button::new(
                                    egui::RichText::new(choice.label.clone())
                                        .size(body_text_size)
                                        .color(text_color),
                                )
                                .fill(panel_fill.gamma_multiply(0.45))
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    if selected {
                                        accent_color
                                    } else {
                                        panel_stroke.gamma_multiply(0.6)
                                    },
                                ))
                                .selected(selected);
                                if ui.add_enabled(choice.enabled, button).clicked() {
                                    self.apply_choice(choice);
                                }
                                ui.add_space(4.0 * scale.max(0.75));
                            }
                        });
                });
        }

        let message_rect = Self::scale_scene_rect(layout.message_window, canvas_rect, scale);
        if visible {
            egui::Area::new(egui::Id::new("message_window"))
                .order(egui::Order::Foreground)
                .fixed_pos(message_rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(message_rect.size());
                    let frame_response = egui::Frame::NONE
                        .fill(panel_fill)
                        .stroke(egui::Stroke::new((2.0 * scale).max(1.0), panel_stroke))
                        .show(ui, |ui| {
                            ui.add_space(10.0 * scale);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&speaker)
                                        .size(speaker_text_size)
                                        .color(speaker_color)
                                        .strong(),
                                );
                            });
                            ui.add_space(6.0 * scale);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Speed")
                                        .size(14.0 * scale.max(0.75))
                                        .color(text_color),
                                );
                                let mut speed =
                                    self.report.ui_state.scene.message_window.text_speed;
                                if ui
                                    .add_sized(
                                        [message_rect.width() * 0.35, 18.0 * scale.max(0.75)],
                                        egui::Slider::new(&mut speed, 0.0..=120.0).show_value(true),
                                    )
                                    .changed()
                                {
                                    self.report.runtime.set_message_speed(speed);
                                    self.report.ui_state.scene.message_window.text_speed = speed;
                                }
                                let mut auto_mode =
                                    self.report.ui_state.scene.message_window.auto_mode;
                                if ui.checkbox(&mut auto_mode, "Auto").changed() {
                                    self.report.runtime.set_message_auto_mode(auto_mode);
                                    self.report.ui_state.scene.message_window.auto_mode = auto_mode;
                                    self.auto_advance_sent = false;
                                    self.auto_advance_elapsed_seconds = 0.0;
                                }
                                let mut skip_mode =
                                    self.report.ui_state.scene.message_window.skip_mode;
                                if ui.checkbox(&mut skip_mode, "Skip").changed() {
                                    self.report.runtime.set_message_skip_mode(skip_mode);
                                    self.report.ui_state.scene.message_window.skip_mode = skip_mode;
                                    if skip_mode {
                                        self.message_reveal_chars = message.text.chars().count();
                                    }
                                    self.auto_advance_sent = false;
                                    self.auto_advance_elapsed_seconds = 0.0;
                                }
                            });
                            ui.separator();
                            egui::ScrollArea::vertical()
                                .id_salt("message_window_text")
                                .max_height(message_rect.height() * 0.55)
                                .show(ui, |ui| {
                                    if text_lines.is_empty() {
                                        ui.label(
                                            egui::RichText::new("...")
                                                .size(body_text_size)
                                                .color(text_color),
                                        );
                                    } else {
                                        for line in &text_lines {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .size(body_text_size)
                                                    .color(text_color),
                                            );
                                        }
                                    }
                                });

                            if let Some(prompt) = input_prompt.as_deref() {
                                ui.add_space(8.0 * scale);
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(prompt)
                                        .size((body_text_size - 2.0).max(12.0))
                                        .color(accent_color),
                                );
                                ui.horizontal(|ui| {
                                    let response = ui.add_sized(
                                        [message_rect.width() * 0.55, 28.0 * scale.max(0.75)],
                                        egui::TextEdit::singleline(&mut self.player_input)
                                            .hint_text("Type a line and press Enter"),
                                    );
                                    let enter_pressed = response.lost_focus()
                                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                                    if ui.button("Send").clicked() || enter_pressed {
                                        self.submit_player_input();
                                    }
                                });
                            }

                            ui.add_space(6.0 * scale);
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.message_history_open, "Text Log");
                                ui.label(
                                    egui::RichText::new(
                                        if self.report.ui_state.scene.message_window.skip_mode {
                                            "skip"
                                        } else if self
                                            .report
                                            .ui_state
                                            .scene
                                            .message_window
                                            .auto_mode
                                        {
                                            "auto"
                                        } else {
                                            "manual"
                                        },
                                    )
                                    .size(13.0 * scale.max(0.75))
                                    .color(accent_color),
                                );
                                if choices.is_empty() && input_prompt.is_none() {
                                    let button_label = if self.message_reveal_chars
                                        < message.text.chars().count()
                                    {
                                        "Reveal"
                                    } else {
                                        "Next"
                                    };
                                    if ui.button(button_label).clicked() {
                                        self.advance_message();
                                    }
                                }
                                if can_advance {
                                    let hint = if self
                                        .report
                                        .ui_state
                                        .scene
                                        .message_window
                                        .skip_mode
                                        && reveal_complete
                                    {
                                        "Skip progressing..."
                                    } else if self.report.ui_state.scene.message_window.auto_mode
                                        && reveal_complete
                                    {
                                        "Auto progressing..."
                                    } else if reveal_complete {
                                        "Click / Enter / Next"
                                    } else {
                                        "Click / Enter to reveal"
                                    };
                                    ui.label(
                                        egui::RichText::new(hint)
                                            .size(13.0 * scale.max(0.75))
                                            .color(accent_color.gamma_multiply(0.9)),
                                    );
                                }
                            });
                            if self.message_history_open && !backlog.is_empty() {
                                ui.add_space(4.0 * scale);
                                egui::ScrollArea::vertical()
                                    .id_salt("message_window_backlog")
                                    .max_height(message_rect.height() * 0.22)
                                    .show(ui, |ui| {
                                        for (index, line) in backlog.iter().enumerate() {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{:02}. {}",
                                                    index + 1,
                                                    line
                                                ))
                                                .size(14.0 * scale.max(0.75))
                                                .color(text_color),
                                            );
                                        }
                                    });
                            }
                        });
                    if can_advance {
                        let click_response = ui.interact(
                            frame_response.response.rect,
                            ui.id().with("message_window_click_surface"),
                            egui::Sense::click(),
                        );
                        if click_response.clicked() {
                            self.advance_message();
                        }
                        let pulse_on = ctx.input(|input| ((input.time * 2.0) as i32) % 2 == 0);
                        if pulse_on {
                            let indicator = if reveal_complete { "▼" } else { "…" };
                            let indicator_pos = frame_response.response.rect.right_bottom()
                                - egui::vec2(18.0 * scale.max(0.75), 14.0 * scale.max(0.75));
                            ui.painter().text(
                                indicator_pos,
                                egui::Align2::RIGHT_BOTTOM,
                                indicator,
                                egui::FontId::proportional(18.0 * scale.max(0.75)),
                                accent_color,
                            );
                        }
                    }
                });
        }
    }
}

impl Drop for ReportApp {
    fn drop(&mut self) {
        *self.report_slot.borrow_mut() = Some(self.report.clone());
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
            if !self.advance_message() {
                self.message_history_open = !self.message_history_open;
            }
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
        } else if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
            let _ = self.advance_message();
        }
        self.update_message_reveal(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("WMS Runtime");
                if ui
                    .button(if self.runtime_menu_open {
                        "Hide Menu"
                    } else {
                        "Menu"
                    })
                    .clicked()
                {
                    self.runtime_menu_open = !self.runtime_menu_open;
                }
                if ui
                    .selectable_label(self.message_history_open, "Log")
                    .clicked()
                {
                    self.message_history_open = !self.message_history_open;
                }
                let auto_mode = self.report.ui_state.scene.message_window.auto_mode;
                if ui.selectable_label(auto_mode, "Auto").clicked() {
                    self.report.runtime.set_message_auto_mode(!auto_mode);
                    self.report.ui_state.scene.message_window.auto_mode = !auto_mode;
                    self.auto_advance_sent = false;
                    self.auto_advance_elapsed_seconds = 0.0;
                }
                let skip_mode = self.report.ui_state.scene.message_window.skip_mode;
                if ui.selectable_label(skip_mode, "Skip").clicked() {
                    self.report.runtime.set_message_skip_mode(!skip_mode);
                    self.report.ui_state.scene.message_window.skip_mode = !skip_mode;
                    if !skip_mode {
                        self.message_reveal_chars = self
                            .report
                            .ui_state
                            .scene
                            .message_window
                            .text
                            .chars()
                            .count();
                    }
                    self.auto_advance_sent = false;
                    self.auto_advance_elapsed_seconds = 0.0;
                }
                if ui
                    .selectable_label(self.debug_panel_open, "Debug")
                    .clicked()
                {
                    self.debug_panel_open = !self.debug_panel_open;
                }
                if let Some(status) = &self.runtime_status_line {
                    ui.separator();
                    ui.label(status);
                }
            });
            if self.runtime_menu_open {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "package: {}",
                        self.report.build.manifest.package_name
                    ));
                    ui.label(format!("worker: {}", self.report.execution.worker_id));
                    ui.label(format!("archive: {} bytes", self.report.build.archive_size));
                    ui.separator();
                    ui.label("Slot");
                    ui.add(egui::DragValue::new(&mut self.selected_checkpoint_slot).range(1..=99));
                    if ui.button("Save").clicked() {
                        self.save_runtime_slot();
                    }
                    if ui.button("Load").clicked() {
                        self.load_runtime_slot();
                    }
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
            }
        });

        if self.debug_panel_open {
            egui::SidePanel::right("debug")
                .resizable(true)
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.heading("Debug");
                    ui.separator();
                    ui.label("Images");
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
                    ui.label("Input");
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
                        self.input_snapshot.raw_scroll_delta.x,
                        self.input_snapshot.raw_scroll_delta.y
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
                    ui.separator();
                    ui.label("Draw Calls");
                    let draw_calls = &self.report.ui_state.scene.draw_calls;
                    if draw_calls.is_empty() {
                        ui.label("No draw calls recorded.");
                    } else {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width().max(1.0), 220.0),
                            egui::Sense::hover(),
                        );
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
                        for draw in draw_calls {
                            self.paint_draw_call(&painter, rect.min, 0.25, draw);
                        }
                    }

                    ui.separator();
                    ui.label("Audio Playback");
                    let audio_playback = &self.report.ui_state.scene.audio_playback;
                    if audio_playback.is_empty() {
                        ui.label("No audio playback recorded.");
                    } else {
                        for (handle, state) in audio_playback {
                            ui.monospace(format!(
                            "handle={} resource={} playing={} looped={} position={}ms volume={:.2}",
                            handle,
                            state.resource_id,
                            state.playing,
                            state.looped,
                            state.position_ms,
                            state.volume
                        ));
                        }
                    }

                    ui.separator();
                    ui.label("Summary");
                    ui.label(format!(
                        "events processed: {}",
                        self.report.execution.outcomes.len()
                    ));
                    if let Some((_, outcome)) = self.report.execution.outcomes.last() {
                        ui.monospace(format!("{outcome:?}"));
                    }
                    ui.separator();
                    ui.label("Build Log");
                    if self.report.log_lines.is_empty() {
                        ui.label("No build log lines.");
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("build_log")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for line in &self.report.log_lines {
                                    ui.monospace(line);
                                }
                            });
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
        }

        let mut stage_rect = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
            stage_rect = Some(rect);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(12, 10, 10));
            let layout = self.report.ui_state.scene.layout.clone();
            let (canvas_rect, scale) = Self::scene_canvas_rect(rect, &layout);
            painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_rgb(28, 18, 16));
            let draw_calls = self.report.ui_state.scene.draw_calls.clone();
            for draw in &draw_calls {
                self.paint_draw_call(&painter, canvas_rect.min, scale, draw);
            }
        });
        if let Some(stage_rect) = stage_rect {
            self.draw_scene_overlays(ctx, stage_rect);
        }

        if self.auto_close && !self.close_sent {
            self.close_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl ReportApp {
    fn paint_draw_call(
        &self,
        painter: &egui::Painter,
        origin: egui::Pos2,
        scale: f32,
        draw: &UiImageDrawCall,
    ) {
        let Some(texture_entry) = self.textures_by_resource_id.get(&draw.resource_id) else {
            painter.text(
                origin + egui::vec2(draw.x * scale, draw.y * scale),
                egui::Align2::LEFT_TOP,
                format!("missing texture {}", draw.resource_id),
                egui::TextStyle::Body.resolve(&egui::Style::default()),
                egui::Color32::RED,
            );
            return;
        };

        let natural = texture_entry.size;
        let source = resolve_source_rect(draw, natural);
        let width = draw.width.unwrap_or(source.width()) * scale;
        let height = draw.height.unwrap_or(source.height()) * scale;
        let rect = egui::Rect::from_min_size(
            origin + egui::vec2(draw.x * scale, draw.y * scale),
            egui::vec2(width, height),
        );
        let uv = egui::Rect::from_min_max(
            egui::pos2(source.left() / natural.x, source.top() / natural.y),
            egui::pos2(source.right() / natural.x, source.bottom() / natural.y),
        );
        paint_textured_rect(
            painter,
            texture_entry.texture.id(),
            rect,
            uv,
            egui::Color32::WHITE.linear_multiply(draw.opacity.clamp(0.0, 1.0)),
            draw.rotation_degrees,
        );
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_white_alpha(96)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.left_top() + egui::vec2(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("#{}", draw.resource_id),
            egui::TextStyle::Small.resolve(&egui::Style::default()),
            egui::Color32::WHITE,
        );
    }
}

fn resolve_source_rect(draw: &UiImageDrawCall, natural: egui::Vec2) -> egui::Rect {
    if let Some(source) = draw.source {
        return egui::Rect::from_min_size(
            egui::pos2(source.x, source.y),
            egui::vec2(source.width, source.height),
        );
    }
    if let Some(icon_sheet) = draw.icon_sheet {
        let cell_w = icon_sheet.cell_width as f32;
        let cell_h = icon_sheet.cell_height as f32;
        if cell_w > 0.0 && cell_h > 0.0 {
            let cols = (natural.x / cell_w).floor().max(1.0) as u32;
            let col = icon_sheet.index % cols;
            let row = icon_sheet.index / cols;
            return egui::Rect::from_min_size(
                egui::pos2(col as f32 * cell_w, row as f32 * cell_h),
                egui::vec2(cell_w, cell_h),
            );
        }
    }
    egui::Rect::from_min_size(egui::Pos2::ZERO, natural)
}

fn paint_textured_rect(
    painter: &egui::Painter,
    texture_id: egui::TextureId,
    rect: egui::Rect,
    uv: egui::Rect,
    tint: egui::Color32,
    rotation_degrees: f32,
) {
    if rotation_degrees.abs() <= f32::EPSILON {
        painter.image(texture_id, rect, uv, tint);
        return;
    }

    let center = rect.center();
    let rotation = egui::emath::Rot2::from_angle(rotation_degrees.to_radians());
    let mut mesh = egui::Mesh::with_texture(texture_id);
    let corners = [
        (rect.left_top(), uv.left_top()),
        (rect.right_top(), uv.right_top()),
        (rect.right_bottom(), uv.right_bottom()),
        (rect.left_bottom(), uv.left_bottom()),
    ];
    for (pos, uv) in corners {
        let rotated = center + rotation * (pos - center);
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rotated,
            uv,
            color: tint,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
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

fn map_modifiers(modifiers: egui::Modifiers) -> wmui::UiModifiers {
    wmui::UiModifiers {
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
