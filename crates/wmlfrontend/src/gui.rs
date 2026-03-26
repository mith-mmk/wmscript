#![forbid(unsafe_code)]

use core::fmt;
use std::collections::BTreeMap;

use eframe::{egui, egui::Vec2};

use crate::{FrontendError, FrontendReport};
use wmlui::{UiImageSlot, UiImageSource, UiTheme};

/// Runs the GUI window for a finished frontend report.
pub fn run_gui(report: FrontendReport) -> Result<(), FrontendError> {
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

    let app = ReportApp::new(report, auto_close);
    eframe::run_native(&title, options, Box::new(move |_| Ok(Box::new(app))))
        .map_err(|error| FrontendError::Gui(error.to_string()))
}

struct ReportApp {
    report: FrontendReport,
    textures: BTreeMap<UiImageSlot, egui::TextureHandle>,
    initialized: bool,
    auto_close: bool,
    close_sent: bool,
}

impl ReportApp {
    fn new(report: FrontendReport, auto_close: bool) -> Self {
        Self {
            report,
            textures: BTreeMap::new(),
            initialized: false,
            auto_close,
            close_sent: false,
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
}

impl eframe::App for ReportApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_textures(ctx);
        ctx.set_visuals(Self::theme_visuals(self.report.ui_state.window.theme));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "WML Frontend - {}",
            self.report.build.manifest.package_name
        )));

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
                for choice in &self.report.ui_state.scene.message_window.choices {
                    if choice.enabled && ui.button(&choice.label).clicked() {
                        ui.label(format!("selected: {}", choice.id));
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

#[derive(Debug)]
struct DecodeTextureError(String);

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
