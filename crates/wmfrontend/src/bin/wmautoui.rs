use std::env;
use std::fs;
use std::path::PathBuf;

use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmtoolchain::{GameProject, Toolchain, ToolchainConfig, spawn_worker_programs};
use wmvm::{Message, RunOutcome, Value, WorkerState};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = CliArgs::parse(env::args().skip(1))?;

    let toolchain =
        Toolchain::new(ToolchainConfig::new(args.platform).with_step_limit(args.step_limit));
    let mut runtime =
        Runtime::new(RuntimeConfig::new(args.platform).with_step_limit(args.step_limit));
    toolchain.bootstrap_runtime(&mut runtime)?;

    let (build, worker_id) = if let Some(archive_path) = &args.archive_path {
        let archive_bytes = fs::read(archive_path)?;
        let build = toolchain.load_archive(&archive_bytes)?;
        runtime.load_archive(&archive_bytes)?;
        let worker_id = spawn_worker_programs(&mut runtime, &build.worker_programs)?;
        (build, worker_id)
    } else {
        let script_path = args.script_path.clone().ok_or("missing script path")?;
        let source = fs::read_to_string(&script_path)?;
        let package_name = args.package_name.clone().unwrap_or_else(|| {
            script_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("wml-game")
                .to_owned()
        });
        let project = GameProject::new(
            package_name,
            script_path.to_string_lossy().to_string(),
            source,
        );
        let build = toolchain.build_project(&project)?;
        runtime.load_archive(&build.archive)?;
        let worker_id = spawn_worker_programs(&mut runtime, &build.worker_programs)?;
        (build, worker_id)
    };

    let mut all_outcomes = Vec::new();
    let mut sent_messages = 0usize;
    let mut sent_choices = 0usize;
    let mut sent_inputs = 0usize;

    for _round in 0..args.max_rounds {
        let outcomes = runtime.tick();
        all_outcomes.extend(outcomes);

        let worker_state = runtime.worker_state(worker_id);
        if matches!(
            worker_state,
            Some(WorkerState::Halted) | Some(WorkerState::Error(_))
        ) {
            break;
        }

        let waiting_workers = runtime.waiting_workers();
        if waiting_workers.is_empty() {
            continue;
        }

        let message_state = runtime.message_window_state();
        let rpg_state = runtime.rpg_ui_state();
        let payload = match select_auto_reply(&mut args, &message_state, &rpg_state)? {
            AutoReply::Choice(choice) => {
                if !args.quiet {
                    println!(
                        "[auto-ui] choice id={:?} label={:?}",
                        choice.id, choice.label
                    );
                }
                runtime.set_state_value("ui.last_choice", Value::String(choice.id.clone()));
                runtime.set_state_value("ui.last_reply", Value::String(choice.id.clone()));
                sent_choices += 1;
                Value::String(choice.id)
            }
            AutoReply::RpgAction { id, label } => {
                if !args.quiet {
                    println!("[auto-ui] rpg id={id:?} label={label:?}");
                }
                runtime.set_state_value("ui.last_choice", Value::String(id.clone()));
                runtime.set_state_value("ui.last_reply", Value::String(id.clone()));
                sent_choices += 1;
                Value::String(id)
            }
            AutoReply::Input { prompt, value } => {
                if !args.quiet {
                    println!("[auto-ui] input prompt={prompt:?} -> {value:?}");
                }
                runtime.set_state_value("ui.last_input", Value::String(value.clone()));
                runtime.set_state_value("ui.last_reply", Value::String(value.clone()));
                sent_inputs += 1;
                Value::String(value)
            }
            AutoReply::Advance => {
                if !args.quiet {
                    println!("[auto-ui] advance (nil)");
                }
                Value::Nil
            }
        };

        for waiting_worker in waiting_workers {
            runtime.send_message(Message::new(0, waiting_worker, 0, payload.clone()));
            sent_messages += 1;
        }
    }

    let worker_state = runtime.worker_state(worker_id);
    if !matches!(
        worker_state,
        Some(WorkerState::Halted) | Some(WorkerState::Error(_))
    ) {
        return Err(format!(
            "auto-ui run did not finish within max rounds (state={worker_state:?})"
        )
        .into());
    }

    if let Some(expected) = &args.expect_string {
        let actual = all_outcomes
            .iter()
            .rev()
            .find_map(|(_, outcome)| match outcome {
                RunOutcome::Halted {
                    value: Some(Value::String(text)),
                    ..
                } => Some(text.clone()),
                _ => None,
            });
        if actual.as_deref() != Some(expected.as_str()) {
            return Err(format!("expected final string {:?}, got {:?}", expected, actual).into());
        }
    }
    if let Some(expected) = args.expect_image_resource {
        let actual = runtime.image_draws().first().map(|draw| draw.resource_id);
        if actual != Some(expected) {
            return Err(format!(
                "expected first image resource {:?}, got {:?}",
                expected, actual
            )
            .into());
        }
    }
    if let Some(expected) = args.expect_audio_resource {
        let found = runtime
            .audio_playback_states()
            .values()
            .any(|state| state.resource_id == expected);
        if !found {
            let actual = runtime
                .audio_playback_states()
                .values()
                .map(|state| state.resource_id)
                .collect::<Vec<_>>();
            return Err(format!(
                "expected audio resource {:?} in playback state, got {:?}",
                expected, actual
            )
            .into());
        }
    }

    println!("=== auto-ui summary ===");
    println!("package: {}", build.manifest.package_name);
    println!("archive bytes: {}", build.archive_size);
    println!("worker: {}", worker_id);
    println!("messages sent: {}", sent_messages);
    println!("choice replies: {}", sent_choices);
    println!("input replies: {}", sent_inputs);
    if let Some((_, outcome)) = all_outcomes.last() {
        println!("final outcome: {outcome:?}");
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum AutoReply {
    Choice(wmruntime::MessageChoiceState),
    RpgAction { id: String, label: String },
    Input { prompt: String, value: String },
    Advance,
}

fn select_auto_reply(
    args: &mut CliArgs,
    message_state: &wmruntime::MessageWindowState,
    rpg_state: &wmruntime::RpgUiState,
) -> Result<AutoReply, Box<dyn std::error::Error>> {
    if rpg_state.map_controls.active()
        && let Some(reply) = select_rpg_reply(args, rpg_state)
    {
        return Ok(reply);
    }

    if !message_state.choices.is_empty() {
        let choice = select_choice(args, &message_state.choices)
            .ok_or("no enabled choice available for auto-ui")?;
        return Ok(AutoReply::Choice(choice));
    }

    if let Some(prompt) = message_state.input_prompt.as_deref() {
        let value = args
            .input
            .clone()
            .unwrap_or_else(|| "auto-input".to_owned());
        return Ok(AutoReply::Input {
            prompt: prompt.to_owned(),
            value,
        });
    }

    Ok(AutoReply::Advance)
}

fn select_rpg_reply(args: &mut CliArgs, rpg_state: &wmruntime::RpgUiState) -> Option<AutoReply> {
    if let Some(wanted) = args.next_sequence_choice() {
        if rpg_state
            .map_controls
            .directions
            .iter()
            .any(|direction| direction == &wanted)
        {
            return Some(AutoReply::RpgAction {
                id: wanted.clone(),
                label: wanted,
            });
        }
        if let Some(action) = rpg_state
            .actions
            .iter()
            .find(|action| action.enabled && (action.id == wanted || action.label == wanted))
        {
            return Some(AutoReply::RpgAction {
                id: action.id.clone(),
                label: action.label.clone(),
            });
        }
        return Some(AutoReply::RpgAction {
            id: wanted.clone(),
            label: wanted,
        });
    }
    if let Some(wanted) = args.choice.as_deref() {
        if rpg_state
            .map_controls
            .directions
            .iter()
            .any(|direction| direction == wanted)
        {
            return Some(AutoReply::RpgAction {
                id: wanted.to_owned(),
                label: wanted.to_owned(),
            });
        }
        if let Some(action) = rpg_state
            .actions
            .iter()
            .find(|action| action.enabled && (action.id == wanted || action.label == wanted))
        {
            return Some(AutoReply::RpgAction {
                id: action.id.clone(),
                label: action.label.clone(),
            });
        }
    }
    rpg_state
        .actions
        .iter()
        .find(|action| action.enabled)
        .map(|action| AutoReply::RpgAction {
            id: action.id.clone(),
            label: action.label.clone(),
        })
        .or_else(|| {
            rpg_state
                .map_controls
                .directions
                .first()
                .map(|direction| AutoReply::RpgAction {
                    id: direction.clone(),
                    label: direction.clone(),
                })
        })
}

fn select_choice(
    args: &mut CliArgs,
    choices: &[wmruntime::MessageChoiceState],
) -> Option<wmruntime::MessageChoiceState> {
    let mut enabled = choices.iter().filter(|choice| choice.enabled);
    if let Some(wanted) = args.next_sequence_choice() {
        if let Some(choice) = enabled.clone().find(|choice| choice.id == wanted) {
            return Some(choice.clone());
        }
        if let Some(choice) = enabled.clone().find(|choice| choice.label == wanted) {
            return Some(choice.clone());
        }
    }
    if let Some(wanted) = args.choice.as_deref() {
        if let Some(choice) = enabled.clone().find(|choice| choice.id == wanted) {
            return Some(choice.clone());
        }
        if let Some(choice) = enabled.clone().find(|choice| choice.label == wanted) {
            return Some(choice.clone());
        }
    }
    enabled.next().cloned()
}

#[derive(Debug)]
struct CliArgs {
    script_path: Option<PathBuf>,
    archive_path: Option<PathBuf>,
    package_name: Option<String>,
    step_limit: usize,
    max_rounds: usize,
    platform: PlatformProfile,
    input: Option<String>,
    choice: Option<String>,
    choice_sequence: Vec<String>,
    choice_sequence_index: usize,
    expect_string: Option<String>,
    expect_image_resource: Option<u32>,
    expect_audio_resource: Option<u32>,
    quiet: bool,
}

impl CliArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut script_path = None;
        let mut archive_path = None;
        let mut package_name = None;
        let mut step_limit = 128usize;
        let mut max_rounds = 512usize;
        let mut platform = PlatformProfile::native();
        let mut input = None;
        let mut choice = None;
        let mut choice_sequence = Vec::new();
        let mut expect_string = None;
        let mut expect_image_resource = None;
        let mut expect_audio_resource = None;
        let mut quiet = false;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--archive" => {
                    archive_path = Some(PathBuf::from(next_value(&mut args, "--archive")?))
                }
                "--package" => package_name = Some(next_value(&mut args, "--package")?),
                "--platform" => platform = parse_platform(&next_value(&mut args, "--platform")?)?,
                "--step-limit" => step_limit = next_value(&mut args, "--step-limit")?.parse()?,
                "--max-rounds" => max_rounds = next_value(&mut args, "--max-rounds")?.parse()?,
                "--input" => input = Some(next_value(&mut args, "--input")?),
                "--choice" => choice = Some(next_value(&mut args, "--choice")?),
                "--choices" => {
                    choice_sequence = parse_choice_sequence(&next_value(&mut args, "--choices")?);
                }
                "--expect" => expect_string = Some(next_value(&mut args, "--expect")?),
                "--expect-image-resource" => {
                    expect_image_resource =
                        Some(next_value(&mut args, "--expect-image-resource")?.parse()?)
                }
                "--expect-audio-resource" => {
                    expect_audio_resource =
                        Some(next_value(&mut args, "--expect-audio-resource")?.parse()?)
                }
                "--quiet" => quiet = true,
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}").into());
                }
                value => {
                    if script_path.is_some() || archive_path.is_some() {
                        return Err(format!("unexpected extra positional argument: {value}").into());
                    }
                    let path = PathBuf::from(value);
                    if path.extension().and_then(|ext| ext.to_str()) == Some("warc") {
                        archive_path = Some(path);
                    } else {
                        script_path = Some(path);
                    }
                }
            }
        }

        if script_path.is_none() && archive_path.is_none() {
            return Err("missing script path or --archive path".into());
        }
        if script_path.is_some() && archive_path.is_some() {
            return Err("script path and --archive cannot be combined".into());
        }

        Ok(Self {
            script_path,
            archive_path,
            package_name,
            step_limit,
            max_rounds,
            platform,
            input,
            choice,
            choice_sequence,
            choice_sequence_index: 0,
            expect_string,
            expect_image_resource,
            expect_audio_resource,
            quiet,
        })
    }

    fn next_sequence_choice(&mut self) -> Option<String> {
        let value = self
            .choice_sequence
            .get(self.choice_sequence_index)
            .cloned()?;
        self.choice_sequence_index += 1;
        Some(value)
    }
}

fn next_value(
    args: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn parse_platform(value: &str) -> Result<PlatformProfile, Box<dyn std::error::Error>> {
    match value {
        "native" => Ok(PlatformProfile::native()),
        "wasm" => Ok(PlatformProfile::wasm()),
        "egui" => Ok(PlatformProfile::egui()),
        other => Err(format!("unknown platform: {other}").into()),
    }
}

fn parse_choice_sequence(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|choice| !choice.is_empty())
        .map(str::to_owned)
        .collect()
}

fn print_usage() {
    eprintln!(
        "usage: wmautoui <script.wms|archive.warc> [--archive FILE] [--package NAME] [--platform native|wasm|egui] [--step-limit N] [--max-rounds N] [--choice ID_OR_LABEL] [--choices ID_OR_LABEL,...] [--input TEXT] [--expect TEXT] [--expect-image-resource ID] [--expect-audio-resource ID] [--quiet]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with_input_and_choice() -> CliArgs {
        CliArgs {
            script_path: Some(PathBuf::from("sample.wms")),
            archive_path: None,
            package_name: None,
            step_limit: 128,
            max_rounds: 512,
            platform: PlatformProfile::native(),
            input: Some("lumen".to_owned()),
            choice: Some("repair".to_owned()),
            choice_sequence: Vec::new(),
            choice_sequence_index: 0,
            expect_string: None,
            expect_image_resource: None,
            expect_audio_resource: None,
            quiet: false,
        }
    }

    #[test]
    fn auto_reply_prefers_choice_over_input_argument() {
        let mut args = args_with_input_and_choice();
        let mut state = wmruntime::MessageWindowState::default();
        state.input_prompt = Some("合言葉".to_owned());
        state.choices = vec![wmruntime::MessageChoiceState {
            id: "repair".to_owned(),
            label: "継電器を直す".to_owned(),
            enabled: true,
        }];

        let reply = select_auto_reply(&mut args, &state, &wmruntime::RpgUiState::default())
            .expect("auto reply");

        assert!(matches!(
            reply,
            AutoReply::Choice(wmruntime::MessageChoiceState { id, .. }) if id == "repair"
        ));
    }

    #[test]
    fn auto_reply_uses_input_only_when_prompt_has_no_choices() {
        let mut args = args_with_input_and_choice();
        let mut state = wmruntime::MessageWindowState::default();
        state.input_prompt = Some("合言葉".to_owned());

        let reply = select_auto_reply(&mut args, &state, &wmruntime::RpgUiState::default())
            .expect("auto reply");

        assert_eq!(
            reply,
            AutoReply::Input {
                prompt: "合言葉".to_owned(),
                value: "lumen".to_owned()
            }
        );
    }

    #[test]
    fn auto_reply_advances_plain_message() {
        let mut args = args_with_input_and_choice();
        let state = wmruntime::MessageWindowState::default();

        let reply = select_auto_reply(&mut args, &state, &wmruntime::RpgUiState::default())
            .expect("auto reply");

        assert_eq!(reply, AutoReply::Advance);
    }

    #[test]
    fn parse_choices_sequence_ignores_empty_items() {
        assert_eq!(
            parse_choice_sequence("forest, stone,, attack "),
            vec!["forest".to_owned(), "stone".to_owned(), "attack".to_owned()]
        );
    }

    #[test]
    fn parse_expect_audio_resource() {
        let args = CliArgs::parse(
            [
                "sample.warc".to_owned(),
                "--expect-audio-resource".to_owned(),
                "204".to_owned(),
            ]
            .into_iter(),
        )
        .expect("parse args");

        assert_eq!(args.expect_audio_resource, Some(204));
    }

    #[test]
    fn auto_reply_consumes_choice_sequence_before_single_choice() {
        let mut args = args_with_input_and_choice();
        args.choice = Some("fallback".to_owned());
        args.choice_sequence = vec!["forest".to_owned(), "attack".to_owned()];
        let mut state = wmruntime::MessageWindowState::default();
        state.choices = vec![
            wmruntime::MessageChoiceState {
                id: "forest".to_owned(),
                label: "森へ行く".to_owned(),
                enabled: true,
            },
            wmruntime::MessageChoiceState {
                id: "attack".to_owned(),
                label: "攻撃".to_owned(),
                enabled: true,
            },
        ];

        let first = select_auto_reply(&mut args, &state, &wmruntime::RpgUiState::default())
            .expect("first auto reply");
        let second = select_auto_reply(&mut args, &state, &wmruntime::RpgUiState::default())
            .expect("second auto reply");

        assert!(matches!(
            first,
            AutoReply::Choice(wmruntime::MessageChoiceState { id, .. }) if id == "forest"
        ));
        assert!(matches!(
            second,
            AutoReply::Choice(wmruntime::MessageChoiceState { id, .. }) if id == "attack"
        ));
    }

    #[test]
    fn auto_reply_uses_rpg_map_sequence_before_message_choices() {
        let mut args = args_with_input_and_choice();
        args.choice_sequence = vec!["east".to_owned(), "status".to_owned()];
        let mut rpg = wmruntime::RpgUiState::default();
        rpg.map_controls = wmruntime::RpgMapControlsState {
            projection: "tile2d".to_owned(),
            directions: vec!["north".to_owned(), "east".to_owned()],
        };
        rpg.actions = vec![wmruntime::RpgActionState {
            id: "status".to_owned(),
            label: "ステータス".to_owned(),
            enabled: true,
        }];
        let mut state = wmruntime::MessageWindowState::default();
        state.choices = vec![wmruntime::MessageChoiceState {
            id: "battle".to_owned(),
            label: "戦う".to_owned(),
            enabled: true,
        }];

        let first = select_auto_reply(&mut args, &state, &rpg).expect("first auto reply");
        let second = select_auto_reply(&mut args, &state, &rpg).expect("second auto reply");

        assert_eq!(
            first,
            AutoReply::RpgAction {
                id: "east".to_owned(),
                label: "east".to_owned()
            }
        );
        assert_eq!(
            second,
            AutoReply::RpgAction {
                id: "status".to_owned(),
                label: "ステータス".to_owned()
            }
        );
    }
}
