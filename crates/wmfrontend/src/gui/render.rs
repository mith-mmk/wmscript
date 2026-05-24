use super::*;

impl ReportApp {
    pub(super) fn draw_runtime_overlay(&mut self, ctx: &egui::Context) {
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

    pub(super) fn draw_runtime_hud(&mut self, ctx: &egui::Context) {
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

    pub(super) fn draw_scene_overlays(&mut self, ctx: &egui::Context, stage_rect: egui::Rect) {
        let layout = self.report.ui_state.scene.layout.clone();
        let message = self.report.ui_state.scene.message_window.clone();
        let rpg = self.report.ui_state.scene.rpg.clone();
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
        let hide_choice_panel = should_hide_choice_panel_for_movement(&choices);
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
        if rpg.map_mode_active() {
            if let Some(hud) = rpg.hud.as_ref().filter(|hud| hud.visible()) {
                let hud_width = (canvas_rect.width() * 0.34).clamp(300.0, 520.0);
                let hud_rect = egui::Rect::from_min_size(
                    canvas_rect.min + egui::vec2(18.0 * scale.max(0.75), 18.0 * scale.max(0.75)),
                    egui::vec2(hud_width, 86.0 * scale.max(0.82)),
                );
                egui::Area::new(egui::Id::new("rpg_hud"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(hud_rect.min)
                    .show(ctx, |ui| {
                        let (panel_rect, _) =
                            ui.allocate_exact_size(hud_rect.size(), egui::Sense::hover());
                        let painter = ui.painter_at(panel_rect);
                        painter.rect_filled(
                            panel_rect,
                            10.0 * scale.max(0.75),
                            egui::Color32::from_rgba_premultiplied(6, 12, 20, 208),
                        );
                        painter.rect_stroke(
                            panel_rect,
                            10.0 * scale.max(0.75),
                            egui::Stroke::new(1.0, choice_panel_stroke),
                            egui::StrokeKind::Inside,
                        );
                        let inner = panel_rect
                            .shrink2(egui::vec2(14.0 * scale.max(0.75), 10.0 * scale.max(0.75)));
                        ui.allocate_ui_at_rect(inner, |ui| {
                            ui.label(
                                egui::RichText::new(&hud.title)
                                    .size((16.0 * scale).max(12.0))
                                    .color(choice_accent_color),
                            );
                            ui.add_space(4.0 * scale.max(0.75));
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&hud.body)
                                        .size((14.0 * scale).max(11.0))
                                        .color(choice_text_color),
                                )
                                .wrap(),
                            );
                        });
                    });
            }

            if !rpg.actions.is_empty() {
                egui::Area::new(egui::Id::new("rpg_action_panel"))
                    .order(choice_order)
                    .fixed_pos(choice_rect.min)
                    .show(ctx, |ui| {
                        let panel_size = egui::vec2(
                            choice_rect.width(),
                            (72.0 + 42.0 * rpg.actions.len() as f32)
                                .min(choice_rect.height().max(120.0)),
                        );
                        let (panel_rect, _) =
                            ui.allocate_exact_size(panel_size, egui::Sense::hover());
                        let painter = ui.painter_at(panel_rect);
                        painter.rect_filled(
                            panel_rect,
                            14.0 * scale.max(0.75),
                            choice_panel_fill.gamma_multiply(0.92),
                        );
                        painter.rect_stroke(
                            panel_rect,
                            14.0 * scale.max(0.75),
                            egui::Stroke::new((1.5 * scale).max(1.0), choice_panel_stroke),
                            egui::StrokeKind::Inside,
                        );
                        let content_rect = panel_rect
                            .shrink2(egui::vec2(24.0 * scale.max(0.75), 18.0 * scale.max(0.75)));
                        ui.allocate_ui_at_rect(content_rect, |ui| {
                            ui.label(
                                egui::RichText::new(if locale_is_ja {
                                    "アクション"
                                } else {
                                    "ACTIONS"
                                })
                                .size((14.0 * scale).max(11.0))
                                .color(choice_accent_color),
                            );
                            ui.add_space(8.0 * scale.max(0.75));
                            egui::ScrollArea::vertical()
                                .id_salt("rpg_action_panel_actions")
                                .auto_shrink([false, false])
                                .max_height((content_rect.height() - 28.0).max(1.0))
                                .show(ui, |ui| {
                                    for (index, action) in rpg.actions.iter().enumerate() {
                                        let label = format!("{}. {}", index + 1, action.label);
                                        let response = ui.add_sized(
                                            [ui.available_width().max(1.0), 34.0 * scale.max(0.82)],
                                            egui::Button::new(
                                                egui::RichText::new(label)
                                                    .size(body_text_size)
                                                    .color(choice_text_color),
                                            ),
                                        );
                                        if response.clicked() && action.enabled {
                                            self.apply_rpg_action(action);
                                        }
                                        ui.add_space(4.0 * scale.max(0.75));
                                    }
                                });
                        });
                    });
            }
        }
        if visible && !choices.is_empty() && !hide_choice_panel {
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
                        egui::ScrollArea::vertical()
                            .id_salt("choice_panel_choices")
                            .auto_shrink([false, false])
                            .max_height(content_rect.height())
                            .show(ui, |ui| {
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
                                        row_rect.left_center()
                                            + egui::vec2(18.0 * scale.max(0.75), 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        if selected { "▸" } else { "  " },
                                        egui::FontId::proportional((20.0 * scale).max(14.0)),
                                        choice_accent_color,
                                    );
                                    let label_rect = egui::Rect::from_min_max(
                                        row_rect.min + egui::vec2(42.0 * scale.max(0.75), 0.0),
                                        row_rect.max - egui::vec2(8.0 * scale.max(0.75), 0.0),
                                    );
                                    ui.put(
                                        label_rect,
                                        egui::Label::new(
                                            egui::RichText::new(&choice.label)
                                                .size(body_text_size)
                                                .color(label_color),
                                        )
                                        .wrap(),
                                    );
                                    if response.clicked() && choice.enabled {
                                        self.apply_choice(choice);
                                    }
                                    ui.add_space(6.0 * scale.max(0.75));
                                }
                            });
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
                            ((speaker.chars().count() as f32 * speaker_text_size * 0.9)
                                + 34.0 * scale.max(0.75))
                            .clamp(96.0 * scale.max(0.75), inner_rect.width());
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
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(line)
                                                    .size(body_text_size)
                                                    .color(text_color),
                                            )
                                            .wrap(),
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
                            if !choices.is_empty() && !hide_choice_panel {
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

impl ReportApp {
    pub(super) fn paint_draw_call(
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
