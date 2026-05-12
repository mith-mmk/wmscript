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

    fn draw_runtime_overlay(&mut self, ctx: &egui::Context) {
        if !self.runtime_menu_open {
            return;
        }

        egui::Window::new(self.runtime_view_label(self.active_runtime_view))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .order(egui::Order::Debug)
            .show(ctx, |ui| {
                let title_label = self.runtime_view_label(RuntimeView::Title);
                let config_label = self.runtime_view_label(RuntimeView::Config);
                let save_load_label = self.runtime_view_label(RuntimeView::SaveLoad);
                ui.horizontal_wrapped(|ui| {
                    for view in [
                        RuntimeView::Title,
                        RuntimeView::Config,
                        RuntimeView::SaveLoad,
                    ] {
                        let label = match view {
                            RuntimeView::Title => title_label,
                            RuntimeView::Config => config_label,
                            RuntimeView::SaveLoad => save_load_label,
                        };
                        ui.selectable_value(&mut self.active_runtime_view, view, label);
                    }
                    ui.separator();
                    if ui.button(self.tr("閉じる", "Close")).clicked() {
                        self.close_runtime_view();
                    }
                });
                ui.separator();

                match self.active_runtime_view {
                    RuntimeView::Title => {
                        ui.heading(&self.report.build.manifest.package_name);
                        ui.label(format!("worker: {}", self.report.execution.worker_id));
                        ui.label(format!("archive: {} bytes", self.report.build.archive_size));
                        ui.add_space(8.0);
                        ui.label(self.tr("ランタイム操作", "Runtime actions"));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(self.tr("設定", "Config")).clicked() {
                                self.active_runtime_view = RuntimeView::Config;
                            }
                            if ui.button(self.tr("セーブ/ロード", "Save / Load")).clicked() {
                                self.active_runtime_view = RuntimeView::SaveLoad;
                            }
                            if ui.button(self.tr("リスタート", "Restart")).clicked() {
                                self.restart_from_beginning();
                            }
                            if ui.button(self.tr("ログ表示切替", "Toggle Log")).clicked() {
                                self.message_history_open = !self.message_history_open;
                            }
                            if ui
                                .button(self.tr("デバッグ表示切替", "Toggle Debug"))
                                .clicked()
                            {
                                self.debug_panel_open = !self.debug_panel_open;
                            }
                            if ui.button(self.tr("ゲームを閉じる", "Close Game")).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(self.tr("現在のメッセージ", "Current message"));
                        let message = &self.report.ui_state.scene.message_window;
                        ui.monospace(format!(
                            "visible={} auto={} skip={} choices={} prompt={}",
                            message.visible,
                            message.auto_mode,
                            message.skip_mode,
                            message.choices.len(),
                            message.input_prompt.is_some()
                        ));
                        if let Some(status) = &self.runtime_status_line {
                            ui.separator();
                            ui.label(status);
                        }
                    }
                    RuntimeView::Config => {
                        ui.label(self.tr("テーマ", "Theme"));
                        ui.horizontal(|ui| {
                            for theme in [UiTheme::System, UiTheme::Dark, UiTheme::Light] {
                                if ui
                                    .selectable_label(
                                        self.report.ui_state.window.theme == theme,
                                        format!("{theme:?}"),
                                    )
                                    .clicked()
                                {
                                    self.apply_theme(theme);
                                }
                            }
                        });
                        ui.add_space(8.0);
                        egui::ComboBox::from_label(self.tr("フォント", "Font"))
                            .selected_text(self.font_preset.label())
                            .show_ui(ui, |ui| {
                                for preset in [
                                    GuiFontPreset::NotoSans,
                                    GuiFontPreset::EguiDefault,
                                    GuiFontPreset::Monospace,
                                ] {
                                    ui.selectable_value(
                                        &mut self.font_preset,
                                        preset,
                                        preset.label(),
                                    );
                                }
                            });
                        ui.add_space(8.0);
                        ui.label(self.tr("言語", "Language"));
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(self.locale_code() == "ja", "日本語")
                                .clicked()
                            {
                                self.report.runtime.set_message_locale("ja");
                                self.sync_runtime_state();
                            }
                            if ui
                                .selectable_label(self.locale_code() == "en", "English")
                                .clicked()
                            {
                                self.report.runtime.set_message_locale("en");
                                self.sync_runtime_state();
                            }
                        });
                        ui.add_space(8.0);
                        let mut speed = self.report.ui_state.scene.message_window.text_speed;
                        if ui
                            .add(
                                egui::Slider::new(&mut speed, 0.0..=120.0)
                                    .text(self.tr("文字送り速度", "Text Speed")),
                            )
                            .changed()
                        {
                            self.report.runtime.set_message_speed(speed);
                            self.report.ui_state.scene.message_window.text_speed = speed;
                        }
                        let mut auto_mode = self.report.ui_state.scene.message_window.auto_mode;
                        if ui
                            .checkbox(&mut auto_mode, self.tr("オート進行", "Auto Mode"))
                            .changed()
                        {
                            self.report.runtime.set_message_auto_mode(auto_mode);
                            self.report.ui_state.scene.message_window.auto_mode = auto_mode;
                            self.auto_advance_sent = false;
                            self.auto_advance_elapsed_seconds = 0.0;
                        }
                        let mut skip_mode = self.report.ui_state.scene.message_window.skip_mode;
                        if ui
                            .checkbox(&mut skip_mode, self.tr("スキップ", "Skip Mode"))
                            .changed()
                        {
                            self.report.runtime.set_message_skip_mode(skip_mode);
                            self.report.ui_state.scene.message_window.skip_mode = skip_mode;
                            if skip_mode {
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
                        if let Some(status) = &self.runtime_status_line {
                            ui.separator();
                            ui.label(status);
                        }
                    }
                    RuntimeView::SaveLoad => {
                        ui.label(self.tr("チェックポイントスロット", "Checkpoint Slot"));
                        ui.add(
                            egui::DragValue::new(&mut self.selected_checkpoint_slot).range(1..=99),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("保存", "Save")).clicked() {
                                self.save_runtime_slot();
                            }
                            if ui.button(self.tr("読み込み", "Load")).clicked() {
                                self.load_runtime_slot();
                            }
                            if ui.button(self.tr("リスタート", "Restart")).clicked() {
                                self.restart_from_beginning();
                            }
                        });
                        ui.add_space(8.0);
                        ui.label(self.tr(
                            "Save はランタイムのメモリ内チェックポイントを保存します。",
                            "Save stores an in-memory runtime checkpoint.",
                        ));
                        ui.label(self.tr(
                            "Load はそのスロットから VM / scene / resource / audio を復元します。",
                            "Load restores VM, scene, resource, and audio state from that slot.",
                        ));
                        if let Some(status) = &self.runtime_status_line {
                            ui.separator();
                            ui.label(status);
                        }
                    }
                }
            });
    }

    fn draw_runtime_hud(&mut self, ctx: &egui::Context) {
        let mut chips = Vec::new();
        if self.report.ui_state.scene.message_window.auto_mode {
            chips.push(self.tr("オート", "AUTO").to_owned());
        }
        if self.report.ui_state.scene.message_window.skip_mode {
            chips.push(self.tr("スキップ", "SKIP").to_owned());
        }
        if self.message_history_open {
            chips.push(self.tr("ログ", "LOG").to_owned());
        }
        if self.debug_panel_open {
            chips.push(self.tr("デバッグ", "DEBUG").to_owned());
        }
        if let Some(status) = &self.runtime_status_line {
            chips.push(status.clone());
        }
        if chips.is_empty() {
            return;
        }

        egui::Area::new(egui::Id::new("runtime_hud"))
            .order(egui::Order::Debug)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(14.0, 14.0))
            .show(ctx, |ui| {
                let label = chips.join("   ");
                let font = egui::FontId::proportional(13.0);
                let galley = ui.painter().layout_no_wrap(
                    label.clone(),
                    font.clone(),
                    egui::Color32::from_rgba_premultiplied(218, 234, 244, 220),
                );
                let size = galley.size() + egui::vec2(22.0, 12.0);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    rect,
                    10.0,
                    egui::Color32::from_rgba_premultiplied(6, 10, 18, 164),
                );
                painter.rect_stroke(
                    rect,
                    10.0,
                    egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_premultiplied(84, 164, 202, 150),
                    ),
                    egui::StrokeKind::Inside,
                );
                painter.galley(
                    rect.min + egui::vec2(11.0, 6.0),
                    galley,
                    egui::Color32::WHITE,
                );
            });
    }

    fn draw_scene_overlays(&mut self, ctx: &egui::Context, stage_rect: egui::Rect) {
        let layout = self.report.ui_state.scene.layout.clone();
        let message = self.report.ui_state.scene.message_window.clone();
        let choices = message.choices.clone();
        let input_prompt = message.input_prompt.clone();
        let visible = message.visible;
        let locale_is_ja = message.locale.starts_with("ja");
        let speaker = message
            .speaker
            .as_deref()
            .filter(|speaker| !speaker.is_empty())
            .unwrap_or(if locale_is_ja {
                "語り手"
            } else {
                "Narrator"
            })
            .to_owned();
        let revealed_text = self.revealed_message_text(&message.text);
        let text_lines = revealed_text
            .lines()
            .map(|line| line.to_owned())
            .collect::<Vec<_>>();
        let backlog = message.backlog.clone();
        let can_advance = choices.is_empty() && input_prompt.is_none();
        let reveal_complete = self.message_reveal_chars >= message.text.chars().count();
        let backlog_effect = self.backlog_effect_progress.clamp(0.0, 1.0);
        let style = message.style.clone();
        let panel_stroke = Self::egui_color(style.panel_stroke);
        let text_color = Self::egui_color(style.text_color);
        let speaker_color = Self::egui_color(style.speaker_color);
        let accent_color = Self::egui_color(style.accent_color);
        let choice_panel_fill = Self::egui_color(style.choice_panel_fill);
        let choice_panel_stroke = Self::egui_color(style.choice_panel_stroke);
        let choice_text_color = Self::egui_color(style.choice_text_color);
        let choice_accent_color = Self::egui_color(style.choice_accent_color);
        let choice_selected_fill = Self::egui_color(style.choice_selected_fill);
        let choice_selected_stroke = Self::egui_color(style.choice_selected_stroke);
        let input_panel_fill = Self::egui_color(style.input_panel_fill);
        let input_panel_stroke = Self::egui_color(style.input_panel_stroke);
        let input_text_color = Self::egui_color(style.input_text_color);
        let input_hint_color = Self::egui_color(style.input_hint_color);
        let input_prompt_color = Self::egui_color(style.input_prompt_color);
        let (canvas_rect, scale) = Self::scene_canvas_rect(stage_rect, &layout);
        let body_text_size = style.body_font_size * scale.max(0.75);
        let speaker_text_size = style.speaker_font_size * scale.max(0.75);

        let (choice_order, input_order, message_order) = Self::scene_overlay_orders(&layout);

        let choice_rect = Self::scale_scene_rect(layout.choice_panel, canvas_rect, scale);
        if visible && !choices.is_empty() {
            egui::Area::new(egui::Id::new("choice_panel"))
                .order(choice_order)
                .fixed_pos(choice_rect.min)
                .show(ctx, |ui| {
                    let (panel_rect, _) =
                        ui.allocate_exact_size(choice_rect.size(), egui::Sense::hover());
                    let painter = ui.painter_at(panel_rect);
                    let shadow_offset = 10.0 * scale.max(0.75);
                    painter.rect_filled(
                        panel_rect.translate(egui::vec2(0.0, shadow_offset)),
                        18.0 * scale.max(0.75),
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 52),
                    );
                    painter.rect_filled(
                        panel_rect,
                        18.0 * scale.max(0.75),
                        choice_panel_fill.gamma_multiply(0.92),
                    );
                    painter.rect_stroke(
                        panel_rect,
                        18.0 * scale.max(0.75),
                        egui::Stroke::new(
                            (2.0 * scale).max(1.0),
                            choice_panel_stroke.gamma_multiply(0.95),
                        ),
                        egui::StrokeKind::Inside,
                    );
                    painter.line_segment(
                        [
                            panel_rect.left_top()
                                + egui::vec2(28.0 * scale.max(0.75), 18.0 * scale.max(0.75)),
                            panel_rect.right_top()
                                - egui::vec2(28.0 * scale.max(0.75), -18.0 * scale.max(0.75)),
                        ],
                        egui::Stroke::new(
                            (1.5 * scale).max(1.0),
                            choice_accent_color.gamma_multiply(0.8),
                        ),
                    );
                    painter.text(
                        panel_rect.left_top()
                            + egui::vec2(26.0 * scale.max(0.75), 18.0 * scale.max(0.75)),
                        egui::Align2::LEFT_TOP,
                        if locale_is_ja {
                            "選択肢"
                        } else {
                            "SELECTION"
                        },
                        egui::FontId::proportional((14.0 * scale).max(11.0)),
                        choice_accent_color,
                    );

                    let content_rect = egui::Rect::from_min_max(
                        panel_rect.min + egui::vec2(28.0 * scale.max(0.75), 50.0 * scale.max(0.75)),
                        panel_rect.max - egui::vec2(28.0 * scale.max(0.75), 24.0 * scale.max(0.75)),
                    );
                    ui.allocate_ui_at_rect(content_rect, |ui| {
                        ui.set_clip_rect(content_rect);
                        let row_height = 38.0 * scale.max(0.82);
                        for choice in &choices {
                            let (row_rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width().max(1.0), row_height),
                                egui::Sense::click(),
                            );
                            let selected = self
                                .selected_choice
                                .as_deref()
                                .is_some_and(|selected| selected == choice.id);
                            let row_painter = ui.painter_at(row_rect);
                            if selected {
                                row_painter.rect_filled(
                                    row_rect,
                                    10.0 * scale.max(0.75),
                                    choice_selected_fill,
                                );
                                row_painter.rect_stroke(
                                    row_rect,
                                    10.0 * scale.max(0.75),
                                    egui::Stroke::new(1.0, choice_selected_stroke),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let label_color = if choice.enabled {
                                choice_text_color
                            } else {
                                choice_text_color.gamma_multiply(0.35)
                            };
                            row_painter.text(
                                row_rect.left_center() + egui::vec2(18.0 * scale.max(0.75), 0.0),
                                egui::Align2::LEFT_CENTER,
                                if selected { "▸" } else { "  " },
                                egui::FontId::proportional((20.0 * scale).max(14.0)),
                                choice_accent_color,
                            );
                            row_painter.text(
                                row_rect.left_center() + egui::vec2(42.0 * scale.max(0.75), 0.0),
                                egui::Align2::LEFT_CENTER,
                                &choice.label,
                                egui::FontId::proportional(body_text_size),
                                label_color,
                            );
                            if response.clicked() && choice.enabled {
                                self.apply_choice(choice);
                            }
                            ui.add_space(6.0 * scale.max(0.75));
                        }
                    });
                });
        }
        let message_rect = Self::scale_scene_rect(layout.message_window, canvas_rect, scale);
        let input_rect = input_prompt.as_ref().map(|_| {
            let width =
                (canvas_rect.width() * 0.44).clamp(420.0 * scale.max(0.75), 680.0 * scale.max(0.9));
            let height = 86.0 * scale.max(0.78);
            let x = canvas_rect.center().x - (width * 0.5);
            let y = (message_rect.min.y - height - 18.0 * scale.max(0.75))
                .max(canvas_rect.min.y + 16.0 * scale.max(0.75));
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
        });
        if let (true, Some(prompt), Some(input_rect)) =
            (visible, input_prompt.as_deref(), input_rect)
        {
            egui::Area::new(egui::Id::new("message_input_window"))
                .order(input_order)
                .fixed_pos(input_rect.min)
                .show(ctx, |ui| {
                    let (panel_rect, _) =
                        ui.allocate_exact_size(input_rect.size(), egui::Sense::hover());
                    let painter = ui.painter_at(panel_rect);
                    painter.rect_filled(
                        panel_rect.translate(egui::vec2(0.0, 10.0 * scale.max(0.75))),
                        14.0 * scale.max(0.75),
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 54),
                    );
                    painter.rect_filled(panel_rect, 14.0 * scale.max(0.75), input_panel_fill);
                    painter.rect_stroke(
                        panel_rect,
                        14.0 * scale.max(0.75),
                        egui::Stroke::new((1.6 * scale).max(1.0), input_panel_stroke),
                        egui::StrokeKind::Inside,
                    );
                    let content_rect = egui::Rect::from_min_max(
                        panel_rect.min + egui::vec2(18.0 * scale.max(0.75), 12.0 * scale.max(0.75)),
                        panel_rect.max - egui::vec2(18.0 * scale.max(0.75), 12.0 * scale.max(0.75)),
                    );
                    ui.allocate_ui_at_rect(content_rect, |ui| {
                        ui.set_clip_rect(content_rect);
                        ui.label(
                            egui::RichText::new(prompt)
                                .size((body_text_size - 1.0).max(13.0))
                                .color(input_prompt_color),
                        );
                        ui.add_space(8.0 * scale.max(0.75));
                        let response = ui
                            .scope(|ui| {
                                ui.visuals_mut().override_text_color = Some(input_text_color);
                                ui.visuals_mut().widgets.inactive.fg_stroke.color =
                                    input_text_color;
                                ui.visuals_mut().widgets.hovered.fg_stroke.color = input_text_color;
                                ui.visuals_mut().widgets.active.fg_stroke.color = input_text_color;
                                ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                                    input_text_color;
                                ui.add_sized(
                                    [content_rect.width().max(1.0), 32.0 * scale.max(0.75)],
                                    egui::TextEdit::singleline(&mut self.player_input)
                                        .hint_text(
                                            egui::RichText::new(if locale_is_ja {
                                                "Enter で送信"
                                            } else {
                                                "Enter to send"
                                            })
                                            .color(input_hint_color),
                                        )
                                        .text_color(input_text_color)
                                        .frame(false),
                                )
                            })
                            .inner;
                        let response_rect = response.rect.expand2(egui::vec2(8.0, 6.0));
                        let response_painter = ui.painter_at(response_rect);
                        response_painter.rect_filled(
                            response_rect,
                            8.0 * scale.max(0.75),
                            egui::Color32::from_rgba_premultiplied(8, 14, 24, 208),
                        );
                        response_painter.rect_stroke(
                            response_rect,
                            8.0 * scale.max(0.75),
                            egui::Stroke::new(1.0, input_panel_stroke),
                            egui::StrokeKind::Inside,
                        );
                        if response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                        {
                            self.submit_player_input();
                        }
                    });
                });
        }
        if visible {
            egui::Area::new(egui::Id::new("message_window"))
                .order(message_order)
                .fixed_pos(message_rect.min)
                .show(ctx, |ui| {
                    let (frame_rect, _) =
                        ui.allocate_exact_size(message_rect.size(), egui::Sense::hover());
                    let painter = ui.painter_at(frame_rect);
                    let shadow_offset = 14.0 * scale.max(0.75);
                    painter.rect_filled(
                        frame_rect.translate(egui::vec2(0.0, shadow_offset)),
                        18.0 * scale.max(0.75),
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 68),
                    );
                    if let Some(frame_resource_id) = message.style.frame_resource_id {
                        if let Some(texture_entry) =
                            self.textures_by_resource_id.get(&frame_resource_id)
                        {
                            painter.image(
                                texture_entry.texture.id(),
                                frame_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else {
                            painter.rect_filled(
                                frame_rect,
                                16.0 * scale.max(0.75),
                                choice_panel_fill.gamma_multiply(0.92),
                            );
                            painter.rect_stroke(
                                frame_rect,
                                16.0 * scale.max(0.75),
                                egui::Stroke::new((2.0 * scale).max(1.0), panel_stroke),
                                egui::StrokeKind::Inside,
                            );
                        }
                    } else {
                        painter.rect_filled(
                            frame_rect,
                            16.0 * scale.max(0.75),
                            choice_panel_fill.gamma_multiply(0.92),
                        );
                        painter.rect_stroke(
                            frame_rect,
                            16.0 * scale.max(0.75),
                            egui::Stroke::new((2.0 * scale).max(1.0), panel_stroke),
                            egui::StrokeKind::Inside,
                        );
                        painter.line_segment(
                            [
                                frame_rect.left_top()
                                    + egui::vec2(34.0 * scale.max(0.75), 18.0 * scale.max(0.75)),
                                frame_rect.right_top()
                                    - egui::vec2(34.0 * scale.max(0.75), -18.0 * scale.max(0.75)),
                            ],
                            egui::Stroke::new(
                                (1.5 * scale).max(1.0),
                                accent_color.gamma_multiply(0.75),
                            ),
                        );
                    }

                    let inset = message.style.content_inset;
                    let left = (inset.left * scale.max(0.75)).min(frame_rect.width() * 0.45);
                    let right = (inset.right * scale.max(0.75)).min(frame_rect.width() * 0.45);
                    let top = (inset.top * scale.max(0.75)).min(frame_rect.height() * 0.45);
                    let bottom = (inset.bottom * scale.max(0.75)).min(frame_rect.height() * 0.45);
                    let inner_rect = egui::Rect::from_min_max(
                        frame_rect.min + egui::vec2(left, top),
                        frame_rect.max - egui::vec2(right, bottom),
                    );
                    ui.allocate_ui_at_rect(inner_rect, |ui| {
                        ui.set_clip_rect(inner_rect);

                        let badge_height = 28.0 * scale.max(0.8);
                        let speaker_width =
                            ((speaker.chars().count() as f32 * speaker_text_size * 0.7)
                                + 30.0 * scale.max(0.75))
                            .clamp(96.0 * scale.max(0.75), inner_rect.width() * 0.55);
                        let (speaker_rect, _) = ui.allocate_exact_size(
                            egui::vec2(speaker_width, badge_height),
                            egui::Sense::hover(),
                        );
                        let speaker_painter = ui.painter_at(speaker_rect);
                        speaker_painter.rect_filled(
                            speaker_rect,
                            8.0 * scale.max(0.75),
                            accent_color.gamma_multiply(0.14),
                        );
                        speaker_painter.rect_stroke(
                            speaker_rect,
                            8.0 * scale.max(0.75),
                            egui::Stroke::new(1.0, accent_color.gamma_multiply(0.75)),
                            egui::StrokeKind::Inside,
                        );
                        speaker_painter.text(
                            speaker_rect.left_center() + egui::vec2(14.0 * scale.max(0.75), 0.0),
                            egui::Align2::LEFT_CENTER,
                            &speaker,
                            egui::FontId::proportional(speaker_text_size),
                            speaker_color,
                        );
                        ui.add_space(10.0 * scale);

                        let reserved_height = badge_height + (52.0 * scale.max(0.75));
                        let max_text_height =
                            (inner_rect.height() - reserved_height).max(48.0 * scale.max(0.75));
                        let text_area_height = if backlog_effect > 0.01 && !backlog.is_empty() {
                            (max_text_height * (0.72 - 0.22 * backlog_effect)).max(42.0)
                        } else {
                            max_text_height
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("message_window_text")
                            .max_height(text_area_height)
                            .show(ui, |ui| {
                                if text_lines.is_empty() {
                                    ui.label(
                                        egui::RichText::new("...")
                                            .size(body_text_size)
                                            .color(text_color.gamma_multiply(0.7)),
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

                        ui.add_space(10.0 * scale);
                        ui.horizontal_wrapped(|ui| {
                            let mut chips = Vec::new();
                            if self.report.ui_state.scene.message_window.skip_mode {
                                chips.push(
                                    if locale_is_ja { "スキップ" } else { "SKIP" }.to_owned(),
                                );
                            } else if self.report.ui_state.scene.message_window.auto_mode {
                                chips.push(if locale_is_ja { "オート" } else { "AUTO" }.to_owned());
                            } else {
                                chips.push(if locale_is_ja { "手動" } else { "MANUAL" }.to_owned());
                            }
                            if !choices.is_empty() {
                                chips.push(if locale_is_ja {
                                    format!("選択肢 {}", choices.len())
                                } else {
                                    format!("CHOICE {}", choices.len())
                                });
                            }
                            if input_prompt.is_some() {
                                chips.push(if locale_is_ja { "入力" } else { "INPUT" }.to_owned());
                            }
                            if self.message_history_open {
                                chips.push(if locale_is_ja { "ログ" } else { "LOG" }.to_owned());
                            }
                            for chip in chips {
                                ui.label(
                                    egui::RichText::new(chip)
                                        .size(12.0 * scale.max(0.8))
                                        .color(accent_color),
                                );
                            }
                        });

                        if backlog_effect > 0.01 && !backlog.is_empty() {
                            ui.add_space(8.0 * scale);
                            egui::ScrollArea::vertical()
                                .id_salt("message_window_backlog")
                                .max_height(message_rect.height() * (0.08 + 0.12 * backlog_effect))
                                .show(ui, |ui| {
                                    for (index, line) in backlog.iter().enumerate() {
                                        let depth = (backlog.len().saturating_sub(index)) as f32;
                                        let depth_alpha = (1.0 - depth * 0.012).clamp(0.62, 1.0);
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{:02}. {}",
                                                index + 1,
                                                line
                                            ))
                                            .size(14.0 * scale.max(0.75))
                                            .color(
                                                text_color.gamma_multiply(
                                                    (0.45 + 0.55 * backlog_effect) * depth_alpha,
                                                ),
                                            ),
                                        );
                                    }
                                });
                        }
                    });
                    if can_advance {
                        let click_response = ui.interact(
                            frame_rect,
                            ui.id().with("message_window_click_surface"),
                            egui::Sense::click(),
                        );
                        if click_response.clicked() {
                            self.advance_message();
                        }
                        let pulse_on = ctx.input(|input| ((input.time * 2.0) as i32) % 2 == 0);
                        if pulse_on {
                            let indicator = if reveal_complete { "▼" } else { "…" };
                            let indicator_pos = frame_rect.right_bottom()
                                - egui::vec2(18.0 * scale.max(0.75), 14.0 * scale.max(0.75));
                            ui.painter().text(
                                indicator_pos,
                                egui::Align2::RIGHT_BOTTOM,
                                indicator,
                                egui::FontId::proportional(18.0 * scale.max(0.75)),
                                choice_accent_color,
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
            if self.runtime_menu_open {
                self.close_runtime_view();
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
            let _ = self.advance_message();
        }
        if self.report.runtime.ui_policy_state().context_menu_enabled
            && ctx.input(|input| input.pointer.secondary_clicked())
        {
            self.open_runtime_view(RuntimeView::Title);
        }
        let shortcut_keys_enabled = !ctx.wants_keyboard_input();
        let direction_shortcut_consumed = shortcut_keys_enabled
            && direction_choice_for_pressed_key(
                &self.report.ui_state.scene.message_window.choices,
                ctx,
            )
            .map(|choice| {
                self.apply_choice(&choice);
            })
            .is_some();
        if !direction_shortcut_consumed
            && shortcut_keys_enabled
            && ctx.input(|input| input.key_pressed(egui::Key::ArrowUp))
        {
            self.select_adjacent_choice(-1);
        }
        if !direction_shortcut_consumed
            && shortcut_keys_enabled
            && ctx.input(|input| input.key_pressed(egui::Key::ArrowDown))
        {
            self.select_adjacent_choice(1);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Enter))
            && let Some(choice) = self.selected_or_first_choice()
        {
            self.apply_choice(&choice);
        } else if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
            let _ = self.advance_message();
        }
        if !direction_shortcut_consumed
            && shortcut_keys_enabled
            && ctx.input(|input| input.key_pressed(egui::Key::A))
        {
            self.toggle_auto_mode();
        }
        if !direction_shortcut_consumed
            && shortcut_keys_enabled
            && ctx.input(|input| input.key_pressed(egui::Key::S))
        {
            self.toggle_skip_mode();
        }
        if shortcut_keys_enabled && ctx.input(|input| input.key_pressed(egui::Key::L)) {
            self.toggle_message_history();
        }
        if shortcut_keys_enabled && ctx.input(|input| input.key_pressed(egui::Key::M)) {
            self.open_runtime_view(RuntimeView::Title);
        }
        if !direction_shortcut_consumed
            && shortcut_keys_enabled
            && ctx.input(|input| input.key_pressed(egui::Key::D))
        {
            self.debug_panel_open = !self.debug_panel_open;
        }
        if shortcut_keys_enabled && ctx.input(|input| input.key_pressed(egui::Key::R)) {
            self.restart_from_beginning();
        }
        self.update_message_reveal(ctx);
        self.update_backlog_effect(ctx);

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

        self.draw_runtime_hud(ctx);
        self.draw_runtime_overlay(ctx);

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

fn direction_choice_for_pressed_key(choices: &[UiChoice], ctx: &egui::Context) -> Option<UiChoice> {
    for key in [
        egui::Key::ArrowUp,
        egui::Key::W,
        egui::Key::ArrowDown,
        egui::Key::S,
        egui::Key::ArrowLeft,
        egui::Key::A,
        egui::Key::ArrowRight,
        egui::Key::D,
    ] {
        if ctx.input(|input| input.key_pressed(key))
            && let Some(choice) = direction_choice_for_key(choices, key)
        {
            return Some(choice);
        }
    }
    None
}

fn direction_choice_for_key(choices: &[UiChoice], key: egui::Key) -> Option<UiChoice> {
    let ids: &[&str] = match key {
        egui::Key::ArrowUp | egui::Key::W => &["north", "forward"],
        egui::Key::ArrowDown | egui::Key::S => &["south", "back"],
        egui::Key::ArrowLeft | egui::Key::A => &["west", "turn_left"],
        egui::Key::ArrowRight | egui::Key::D => &["east", "turn_right"],
        _ => return None,
    };
    ids.iter().find_map(|id| {
        choices
            .iter()
            .find(|choice| choice.enabled && choice.id == *id)
            .cloned()
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(id: &str) -> UiChoice {
        UiChoice::new(id, id)
    }

    #[test]
    fn direction_keys_map_to_2d_choices() {
        let choices = vec![
            choice("north"),
            choice("south"),
            choice("west"),
            choice("east"),
        ];
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::ArrowUp)
                .map(|choice| choice.id)
                .as_deref(),
            Some("north")
        );
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::W)
                .map(|choice| choice.id)
                .as_deref(),
            Some("north")
        );
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::S)
                .map(|choice| choice.id)
                .as_deref(),
            Some("south")
        );
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::D)
                .map(|choice| choice.id)
                .as_deref(),
            Some("east")
        );
    }

    #[test]
    fn direction_keys_map_to_grid3d_choices() {
        let choices = vec![
            choice("forward"),
            choice("back"),
            choice("turn_left"),
            choice("turn_right"),
        ];
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::ArrowUp)
                .map(|choice| choice.id)
                .as_deref(),
            Some("forward")
        );
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::ArrowLeft)
                .map(|choice| choice.id)
                .as_deref(),
            Some("turn_left")
        );
        assert_eq!(
            direction_choice_for_key(&choices, egui::Key::A)
                .map(|choice| choice.id)
                .as_deref(),
            Some("turn_left")
        );
    }

    #[test]
    fn non_map_choices_do_not_consume_shortcut_keys() {
        let choices = vec![choice("status"), choice("inventory")];
        assert!(direction_choice_for_key(&choices, egui::Key::A).is_none());
        assert!(direction_choice_for_key(&choices, egui::Key::S).is_none());
        assert!(direction_choice_for_key(&choices, egui::Key::ArrowDown).is_none());
    }
}
