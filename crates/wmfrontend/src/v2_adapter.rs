#[cfg(not(target_arch = "wasm32"))]
use eframe::egui;

/// Minimal native egui adapter for a completed WMScript v2 runtime report.
#[cfg(not(target_arch = "wasm32"))]
pub fn show_report(title: &str, messages: Vec<String>) -> Result<(), String> {
    let auto_close = matches!(
        std::env::var("WMS_EGUI_AUTO_CLOSE").ok().as_deref(),
        Some("1") | Some("true")
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([800.0, 480.0]),
        ..Default::default()
    };
    let app = V2ReportApp {
        title: title.to_owned(),
        messages,
        auto_close,
        frames: 0,
    };
    eframe::run_native(title, options, Box::new(move |_| Ok(Box::new(app))))
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn show_report(_title: &str, _messages: Vec<String>) -> Result<(), String> {
    Err("native egui adapter is unavailable on wasm32".to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
struct V2ReportApp {
    title: String,
    messages: Vec<String>,
    auto_close: bool,
    frames: usize,
}
#[cfg(not(target_arch = "wasm32"))]
impl eframe::App for V2ReportApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(context, |ui| {
            ui.heading(&self.title);
            ui.separator();
            if self.messages.is_empty() {
                ui.label("WMScript v2 program completed without UI messages.");
            }
            for message in &self.messages {
                ui.label(message);
            }
        });
        self.frames += 1;
        if self.auto_close && self.frames >= 2 {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
