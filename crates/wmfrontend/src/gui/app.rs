use super::*;

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
        if shortcut_keys_enabled && debug_shortcut_pressed(ctx) {
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
