#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "wasm32", allow(dead_code, deprecated))]

use core::fmt;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::{FrontendError, FrontendReport, GuiFontPreset};
use wmui::{
    UiChoice, UiColorRgba, UiImageDrawCall, UiImageSlot, UiImageSource, UiKey, UiMouseButton,
    UiPoint, UiRect, UiSceneLayoutState, UiTheme,
};
use wmvm::{Message, Value};

/// Runs the GUI window for a finished frontend report.
#[cfg(not(target_arch = "wasm32"))]
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
            .with_inner_size(egui::Vec2::new(
                size.width.max(640.0),
                size.height.max(480.0),
            )),
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

/// Reports that the native eframe window path is unavailable on wasm32.
#[cfg(target_arch = "wasm32")]
pub fn run_gui(
    _report: FrontendReport,
    _font_preset: GuiFontPreset,
) -> Result<FrontendReport, FrontendError> {
    Err(FrontendError::Gui(
        "native GUI launch is unavailable on wasm32; use the browser bootstrap path".to_owned(),
    ))
}

#[cfg(target_arch = "wasm32")]
pub async fn run_gui_web(
    report: FrontendReport,
    font_preset: GuiFontPreset,
    canvas_id: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let window =
        web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("missing window"))?;
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("missing document"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("missing canvas element"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let report_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let app = ReportApp::new(report, false, font_preset, report_slot);
    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |_| Ok(Box::new(app))),
        )
        .await?;
    std::mem::forget(runner);
    Ok(())
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
    active_runtime_view: RuntimeView,
    selected_checkpoint_slot: u32,
    runtime_status_line: Option<String>,
    message_reveal_chars: usize,
    message_signature: Option<String>,
    auto_advance_sent: bool,
    auto_advance_elapsed_seconds: f32,
    backlog_effect_progress: f32,
    font_preset: GuiFontPreset,
    applied_font_preset: Option<GuiFontPreset>,
}

#[derive(Clone)]
struct TextureEntry {
    texture: egui::TextureHandle,
    size: egui::Vec2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeView {
    Title,
    Config,
    SaveLoad,
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
            active_runtime_view: RuntimeView::Title,
            selected_checkpoint_slot: 1,
            runtime_status_line: None,
            message_reveal_chars: 0,
            message_signature: None,
            auto_advance_sent: false,
            auto_advance_elapsed_seconds: 0.0,
            backlog_effect_progress: 0.0,
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

    fn locale_code(&self) -> &str {
        if self
            .report
            .ui_state
            .scene
            .message_window
            .locale
            .starts_with("ja")
        {
            "ja"
        } else {
            "en"
        }
    }

    fn tr<'a>(&self, ja: &'a str, en: &'a str) -> &'a str {
        if self.locale_code() == "ja" { ja } else { en }
    }

    fn runtime_view_label(&self, view: RuntimeView) -> &'static str {
        match (self.locale_code(), view) {
            ("ja", RuntimeView::Title) => "タイトル",
            ("ja", RuntimeView::Config) => "設定",
            ("ja", RuntimeView::SaveLoad) => "セーブ/ロード",
            (_, RuntimeView::Title) => "Title",
            (_, RuntimeView::Config) => "Config",
            (_, RuntimeView::SaveLoad) => "Save / Load",
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

    fn selected_or_first_choice(&self) -> Option<UiChoice> {
        let choices = &self.report.ui_state.scene.message_window.choices;
        if let Some(selected) = self.selected_choice.as_deref()
            && let Some(choice) = choices
                .iter()
                .find(|choice| choice.enabled && choice.id == selected)
        {
            return Some(choice.clone());
        }
        choices.iter().find(|choice| choice.enabled).cloned()
    }

    fn select_adjacent_choice(&mut self, delta: i32) {
        let choices = &self.report.ui_state.scene.message_window.choices;
        let enabled = choices
            .iter()
            .filter(|choice| choice.enabled)
            .cloned()
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            self.selected_choice = None;
            return;
        }
        let current_index = self
            .selected_choice
            .as_deref()
            .and_then(|selected| enabled.iter().position(|choice| choice.id == selected))
            .unwrap_or(0) as i32;
        let next_index = (current_index + delta).rem_euclid(enabled.len() as i32) as usize;
        self.selected_choice = Some(enabled[next_index].id.clone());
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

    fn restart_from_beginning(&mut self) {
        match self.report.runtime.load_checkpoint(0) {
            Ok(true) => {
                self.report.execution.outcomes = self.report.runtime.run_until_idle(8);
                self.sync_runtime_state();
                self.reset_message_progress();
                self.selected_choice = self
                    .report
                    .ui_state
                    .scene
                    .message_window
                    .choices
                    .iter()
                    .find(|choice| choice.enabled)
                    .map(|choice| choice.id.clone());
                self.player_input.clear();
                self.runtime_status_line = Some("restarted from beginning".to_owned());
            }
            Ok(false) => {
                self.runtime_status_line = Some("restart point is not available".to_owned());
            }
            Err(error) => {
                self.runtime_status_line = Some(format!("restart failed: {error}"));
            }
        }
    }

    fn open_runtime_view(&mut self, view: RuntimeView) {
        self.runtime_menu_open = true;
        self.active_runtime_view = view;
    }

    fn close_runtime_view(&mut self) {
        self.runtime_menu_open = false;
    }

    fn apply_theme(&mut self, theme: UiTheme) {
        self.report.ui_state.window.theme = theme;
        self.runtime_status_line = Some(format!("theme: {theme:?}"));
    }

    fn toggle_message_history(&mut self) {
        self.message_history_open = !self.message_history_open;
    }

    fn update_backlog_effect(&mut self, ctx: &egui::Context) {
        let target = if self.message_history_open { 1.0 } else { 0.0 };
        let dt = ctx.input(|input| input.stable_dt).max(0.0);
        let speed = 7.5;
        self.backlog_effect_progress += (target - self.backlog_effect_progress) * (speed * dt);
        self.backlog_effect_progress = self.backlog_effect_progress.clamp(0.0, 1.0);
        if (target - self.backlog_effect_progress).abs() > 0.01 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn toggle_auto_mode(&mut self) {
        let enabled = !self.report.ui_state.scene.message_window.auto_mode;
        self.report.runtime.set_message_auto_mode(enabled);
        self.report.ui_state.scene.message_window.auto_mode = enabled;
        self.auto_advance_sent = false;
        self.auto_advance_elapsed_seconds = 0.0;
        self.runtime_status_line = Some(format!("auto: {}", if enabled { "on" } else { "off" }));
    }

    fn toggle_skip_mode(&mut self) {
        let enabled = !self.report.ui_state.scene.message_window.skip_mode;
        self.report.runtime.set_message_skip_mode(enabled);
        self.report.ui_state.scene.message_window.skip_mode = enabled;
        if enabled {
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
        self.runtime_status_line = Some(format!("skip: {}", if enabled { "on" } else { "off" }));
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
        } else if self.selected_choice.is_none() {
            self.selected_choice = self
                .report
                .ui_state
                .scene
                .message_window
                .choices
                .iter()
                .find(|choice| choice.enabled)
                .map(|choice| choice.id.clone());
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

        let policy = self.report.runtime.ui_policy_state();
        let shift_fast = policy.shift_fast_enabled && ctx.input(|input| input.modifiers.shift);

        if shift_fast {
            self.message_reveal_chars = text_len;
        } else if message.skip_mode || message.text_speed <= 0.0 {
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
            && (message.auto_mode || message.skip_mode || shift_fast);

        if can_auto_advance {
            self.auto_advance_elapsed_seconds += dt;
            let delay = if shift_fast {
                0.04
            } else {
                Self::message_advance_delay_seconds(message, text_len)
            };
            if self.auto_advance_elapsed_seconds >= delay {
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

    fn scene_overlay_orders(
        layout: &UiSceneLayoutState,
    ) -> (egui::Order, egui::Order, egui::Order) {
        let mut layers = [
            (layout.choice_panel_z, 0usize),
            (layout.input_panel_z, 1usize),
            (layout.message_window_z, 2usize),
        ];
        layers.sort_by_key(|(z, index)| (*z, *index));

        let mut orders = [egui::Order::Middle; 3];
        let ranked_orders = [
            egui::Order::Middle,
            egui::Order::Foreground,
            egui::Order::Debug,
        ];
        for (rank, (_, index)) in layers.into_iter().enumerate() {
            orders[index] = ranked_orders[rank];
        }

        (orders[0], orders[1], orders[2])
    }
}

mod app;
mod helpers;
mod render;

use helpers::*;

#[cfg(test)]
#[path = "../../tests/support/gui_tests.rs"]
mod tests;
