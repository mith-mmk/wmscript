#![forbid(unsafe_code)]

//! Console frontend that drives WML projects through the toolchain and runtime.

use core::fmt;
use std::cell::RefCell;
use std::rc::Rc;

use wmlplatform::PlatformProfile;
use wmlruntime::Runtime;
use wmltoolchain::{
    BuildArtifact, ExecutionReport, GameProject, Toolchain, ToolchainConfig, ToolchainError,
};
use wmlui::{
    UiApp, UiBackend, UiCommand, UiContext, UiError, UiEvent, UiLogLevel, UiSession, UiTheme,
};

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
    MissingReport,
}

impl fmt::Display for FrontendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => write!(f, "{error}"),
            Self::Ui(error) => write!(f, "{error}"),
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

/// Summary returned after running the frontend.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontendReport {
    pub build: BuildArtifact,
    pub execution: ExecutionReport,
    pub log_lines: Vec<String>,
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
        }
        Ok(())
    }
}

struct FrontendApp {
    config: FrontendConfig,
    toolchain: Toolchain,
    runtime: Runtime,
    report_slot: Rc<RefCell<Option<FrontendReport>>>,
    log_lines: Rc<RefCell<Vec<String>>>,
    started: bool,
}

impl FrontendApp {
    fn new(
        config: FrontendConfig,
        report_slot: Rc<RefCell<Option<FrontendReport>>>,
        log_lines: Rc<RefCell<Vec<String>>>,
    ) -> Self {
        let toolchain = Toolchain::new(
            ToolchainConfig::new(config.platform).with_step_limit(config.step_limit),
        );
        let runtime = Runtime::new(
            wmlruntime::RuntimeConfig::new(config.platform).with_step_limit(config.step_limit),
        );
        Self {
            config,
            toolchain,
            runtime,
            report_slot,
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
                *self.log_lines.borrow_mut() = log_lines.clone();
                *self.report_slot.borrow_mut() = Some(FrontendReport {
                    build,
                    execution,
                    log_lines,
                });
                for line in self.log_lines.borrow().iter() {
                    ctx.log(UiLogLevel::Info, line.clone());
                }
                ctx.set_title(format!(
                    "WML Frontend - done ({})",
                    self.config.project.package_name
                ));
                ctx.close_window();
            }
            Err(error) => {
                ctx.log(UiLogLevel::Error, error.to_string());
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
    lines.push(format!("worker {} finished", execution.worker_id));
    if let Some((_, outcome)) = execution.outcomes.last() {
        lines.push(format!("final outcome: {outcome:?}"));
    }
    lines
}

/// Runs a project through the frontend shell and returns the execution report.
pub fn run_frontend(config: FrontendConfig) -> Result<FrontendReport, FrontendError> {
    let report_slot = Rc::new(RefCell::new(None));
    let log_lines = Rc::new(RefCell::new(Vec::new()));
    let app = FrontendApp::new(config.clone(), report_slot.clone(), log_lines);
    let mut session = UiSession::new(config.platform, app);
    session.push_event(UiEvent::Frame { dt_seconds: 0.0 });
    session.push_event(UiEvent::TextInput(config.project.script_path.clone()));
    let mut backend = ConsoleBackend::default();
    let _ = session.drive_backend(&mut backend)?;
    report_slot
        .borrow_mut()
        .take()
        .ok_or(FrontendError::MissingReport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmlplatform::PlatformProfile;
    use wmlresource::ResourceType;
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
        .push_asset(GameAsset::new(
            "ui/title",
            10,
            42,
            ResourceType::ScriptData,
            b"title".to_vec(),
        ));
        let config = FrontendConfig::new(PlatformProfile::native(), project);

        let report = run_frontend(config).expect("frontend run");
        assert_eq!(report.execution.worker_id, 1);
        assert!(!report.log_lines.is_empty());
    }
}
