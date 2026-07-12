use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use wms::{
    Project, Target, build_project, package_project, run, run_archive, run_compiled,
    run_legacy_archive, write_file,
};

fn main() {
    if let Err(error) = execute(env::args().skip(1).collect()) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn execute(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "new" => new_project(args.get(1).map(String::as_str).ok_or_else(usage)?),
        "check" => {
            let project = load_project(args.get(1))?;
            let build = build_project(&project)?;
            println!(
                "checked {} ({} schema types)",
                project.name,
                build.output.schema.len()
            );
            Ok(())
        }
        "build" => {
            let project = load_project(args.get(1))?;
            let build = build_project(&project)?;
            let output = option_path(&args, "--out").unwrap_or_else(|| {
                project
                    .root
                    .join(".test-wms/build")
                    .join(format!("{}.wmp", project.name))
            });
            write_file(&output, &build.output.program.encode_binary())?;
            println!("built {}", output.display());
            Ok(())
        }
        "package" => {
            let project = load_project(args.get(1).filter(|value| !value.starts_with("--")))?;
            let build = build_project(&project)?;
            let bytes = package_project(&project, &build)?;
            let output = option_path(&args, "--out").unwrap_or_else(|| {
                project
                    .root
                    .join(".test-wms/dist")
                    .join(format!("{}.warc", project.name))
            });
            write_file(&output, &bytes)?;
            println!("packaged {}", output.display());
            Ok(())
        }
        "run" => run_command(&args[1..]),
        "test" => test_command(args.get(1)),
        "legacy" if args.get(1).map(String::as_str) == Some("run") => {
            let path = args.get(2).ok_or_else(usage)?;
            run_legacy_archive(Path::new(path))?;
            println!("legacy archive completed");
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn run_command(args: &[String]) -> Result<(), String> {
    let requested_target = option(args, "--target")
        .map(Target::parse)
        .transpose()
        .map_err(|error| error.to_string())?;
    let inputs = option(args, "--inputs")
        .map(|value| {
            value
                .split(',')
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if let Some(path) = args
        .first()
        .filter(|value| !value.starts_with("--") && value.ends_with(".warc"))
    {
        let report = run_archive(
            Path::new(path),
            requested_target.unwrap_or(Target::Headless),
            inputs,
        )?;
        println!("completed in {} rounds: {:?}", report.rounds, report.value);
        return Ok(());
    }
    let project = load_project(args.first().filter(|value| !value.starts_with("--")))?;
    let target = requested_target.unwrap_or(project.default_target);
    let build = build_project(&project)?;
    let report = run_compiled(&build.output, target, inputs, project.seed)?;
    println!(
        "completed {} in {} rounds: {:?}",
        project.name, report.rounds, report.value
    );
    Ok(())
}

fn test_command(path: Option<&String>) -> Result<(), String> {
    let project = load_project(path)?;
    let build = build_project(&project)?;
    for (name, id) in &build.output.test_functions {
        let mut program = build.output.program.clone();
        program.set_entry(*id);
        run(program, Target::Headless, Vec::new())
            .map_err(|error| format!("test `{name}` failed: {error}"))?;
        println!("ok {name}");
    }
    println!("{} tests passed", build.output.test_functions.len());
    Ok(())
}

fn new_project(path: &str) -> Result<(), String> {
    let root = PathBuf::from(path);
    if root.exists()
        && fs::read_dir(&root)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err(format!("{} is not empty", root.display()));
    }
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("game");
    fs::write(root.join("wms.toml"), format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nentry = \"src/main.wms\"\n\n[game]\ntick_hz = 60\nseed = 1\nsave_compat_version = 1\n\n[target]\ndefault = \"headless\"\n\n[capabilities]\nallow = []\n")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/main.wms"), "on start {\n    return;\n}\n")
        .map_err(|error| error.to_string())?;
    println!("created {}", root.display());
    Ok(())
}

fn load_project(path: Option<&String>) -> Result<Project, String> {
    Project::load(
        path.map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".")),
    )
    .map_err(|error| error.to_string())
}
fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|value| value == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}
fn option_path(args: &[String], name: &str) -> Option<PathBuf> {
    option(args, name).map(PathBuf::from)
}
fn usage() -> String {
    "usage: wms new <dir> | check [project] | build [project] [--out file] | run [project|v2.warc] [--target headless|egui] [--inputs a,b] | test [project] | package [project] [--out file] | legacy run <v1.warc>".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn option_parser_is_order_independent() {
        let args = vec!["game".to_owned(), "--target".to_owned(), "egui".to_owned()];
        assert_eq!(option(&args, "--target"), Some("egui"));
    }
}
