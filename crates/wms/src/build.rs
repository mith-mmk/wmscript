use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use wmarchive::{ArchiveV2, AssetV2, ManifestV2};
use wmcompiler::v2::{
    Block, CompileOutput, Expr, Item, SourceModule, Stmt, check_module, lower_module, parse_module,
};

use crate::Project;

pub struct ProjectBuild {
    pub output: CompileOutput,
    pub source: String,
}

pub fn build_project(project: &Project) -> Result<ProjectBuild, String> {
    let path = project.source_path();
    let source =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut items = Vec::new();
    let mut visiting = BTreeSet::new();
    let mut loaded = BTreeSet::new();
    load_module(project, &path, "", &mut visiting, &mut loaded, &mut items)?;
    let module = SourceModule {
        path: path.to_string_lossy().into_owned(),
        items,
    };
    validate_capabilities(project, &module)?;
    let checked = check_module(module).map_err(format_diagnostics)?;
    let output = lower_module(&checked).map_err(format_diagnostics)?;
    Ok(ProjectBuild { output, source })
}

fn validate_capabilities(project: &Project, module: &SourceModule) -> Result<(), String> {
    let mut required = BTreeSet::new();
    for item in &module.items {
        match item {
            Item::Callable(decl) => collect_block_capabilities(&decl.body, &mut required),
            Item::Handler(decl) => collect_block_capabilities(&decl.body, &mut required),
            Item::Record(_) | Item::Enum(_) | Item::Import(_) => {}
        }
    }
    let allowed = project
        .capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let missing = required
        .into_iter()
        .filter(|capability| !allowed.contains(capability))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "project is missing required capabilities: {}",
            missing.join(", ")
        ))
    }
}

fn collect_block_capabilities(block: &Block, output: &mut BTreeSet<&'static str>) {
    for stmt in &block.statements {
        match stmt {
            Stmt::Let { value, .. } | Stmt::Expr(value) | Stmt::Emit(value, _) => {
                collect_expr_capabilities(value, output)
            }
            Stmt::Assign { target, value, .. } => {
                collect_expr_capabilities(target, output);
                collect_expr_capabilities(value, output);
            }
            Stmt::Return(value, _) => {
                if let Some(value) = value {
                    collect_expr_capabilities(value, output);
                }
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                collect_expr_capabilities(condition, output);
                collect_block_capabilities(then_block, output);
                if let Some(block) = else_block {
                    collect_block_capabilities(block, output);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_expr_capabilities(condition, output);
                collect_block_capabilities(body, output);
            }
            Stmt::For { iterable, body, .. } => {
                collect_expr_capabilities(iterable, output);
                collect_block_capabilities(body, output);
            }
            Stmt::Match { value, arms, .. } => {
                collect_expr_capabilities(value, output);
                for arm in arms {
                    collect_block_capabilities(&arm.body, output);
                }
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn collect_expr_capabilities(expr: &Expr, output: &mut BTreeSet<&'static str>) {
    match expr {
        Expr::Field { object, .. } => {
            if let Expr::Name(root, _) = object.as_ref() {
                match root.as_str() {
                    "ui" | "scene" => {
                        output.insert("gui");
                    }
                    "audio" => {
                        output.insert("audio");
                    }
                    "save" => {
                        output.insert("storage");
                    }
                    "asset" => {
                        output.insert("asset");
                    }
                    _ => {}
                }
            }
            collect_expr_capabilities(object, output);
        }
        Expr::Array(values, _) => {
            for value in values {
                collect_expr_capabilities(value, output);
            }
        }
        Expr::Record { fields, .. } => {
            for (_, value) in fields {
                collect_expr_capabilities(value, output);
            }
        }
        Expr::Unary { value, .. } | Expr::Await { value, .. } => {
            collect_expr_capabilities(value, output)
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_capabilities(left, output);
            collect_expr_capabilities(right, output);
        }
        Expr::Index { object, index, .. } => {
            collect_expr_capabilities(object, output);
            collect_expr_capabilities(index, output);
        }
        Expr::Call { callee, args, .. } => {
            collect_expr_capabilities(callee, output);
            for arg in args {
                collect_expr_capabilities(arg, output);
            }
        }
        Expr::Literal(_, _) | Expr::Name(_, _) => {}
    }
}

fn load_module(
    project: &Project,
    path: &Path,
    prefix: &str,
    visiting: &mut BTreeSet<PathBuf>,
    loaded: &mut BTreeSet<(PathBuf, String)>,
    output: &mut Vec<Item>,
) -> Result<(), String> {
    let canonical_root = fs::canonicalize(&project.root)
        .map_err(|error| format!("{}: {error}", project.root.display()))?;
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!("import escapes project root: {}", path.display()));
    }
    if !visiting.insert(canonical.clone()) {
        return Err(format!("cyclic import: {}", path.display()));
    }
    if !loaded.insert((canonical.clone(), prefix.to_owned())) {
        visiting.remove(&canonical);
        return Ok(());
    }
    let source = fs::read_to_string(&canonical)
        .map_err(|error| format!("{}: {error}", canonical.display()))?;
    let module = parse_module(&canonical.to_string_lossy(), &source).map_err(format_diagnostics)?;
    for item in &module.items {
        if let Item::Import(import) = item {
            let alias = import.alias.clone().unwrap_or_else(|| {
                Path::new(&import.path)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("module")
                    .to_owned()
            });
            let child_prefix = if prefix.is_empty() {
                alias
            } else {
                format!("{prefix}.{alias}")
            };
            let child = canonical
                .parent()
                .unwrap_or(&canonical_root)
                .join(&import.path);
            load_module(project, &child, &child_prefix, visiting, loaded, output)?;
        }
    }
    for item in module.items {
        match item {
            Item::Import(_) => {}
            Item::Callable(mut decl) => {
                if !prefix.is_empty() {
                    decl.name = format!("{prefix}.{}", decl.name);
                }
                output.push(Item::Callable(decl));
            }
            Item::Record(mut decl) => {
                if !prefix.is_empty() {
                    decl.name = format!("{prefix}.{}", decl.name);
                }
                output.push(Item::Record(decl));
            }
            Item::Enum(mut decl) => {
                if !prefix.is_empty() {
                    decl.name = format!("{prefix}.{}", decl.name);
                }
                output.push(Item::Enum(decl));
            }
            Item::Handler(decl) if prefix.is_empty() => output.push(Item::Handler(decl)),
            Item::Handler(_) => {
                visiting.remove(&canonical);
                return Err(format!(
                    "imported module cannot declare event handlers: {}",
                    canonical.display()
                ));
            }
        }
    }
    visiting.remove(&canonical);
    Ok(())
}

pub fn package_project(project: &Project, build: &ProjectBuild) -> Result<Vec<u8>, String> {
    let assets = project
        .assets
        .iter()
        .map(|asset| {
            let bytes = fs::read(&asset.path)
                .map_err(|error| format!("{}: {error}", asset.path.display()))?;
            Ok(AssetV2 {
                id: asset.id,
                name: asset.name.clone(),
                kind: asset.kind,
                bytes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let archive = ArchiveV2 {
        manifest: ManifestV2 {
            package: project.name.clone(),
            package_version: project.version.clone(),
            entry_system: "start".to_owned(),
            tick_hz: project.tick_hz,
            seed: project.seed,
            save_compat_version: project.save_compat_version,
            capabilities: project.capabilities.clone(),
        },
        program: build.output.program.encode_binary(),
        schema: encode_schema(&build.output),
        assets,
    };
    archive.encode().map_err(|error| error.to_string())
}

pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn encode_schema(output: &CompileOutput) -> Vec<u8> {
    let mut text = String::from("WMS-SCHEMA-2\n");
    for schema in &output.schema {
        text.push_str(&format!(
            "{:?}\t{}\t{}\n",
            schema.kind, schema.name, schema.persistent
        ));
        for (id, name, ty) in &schema.fields {
            text.push_str(&format!("{id}\t{name}\t{ty}\n"));
        }
    }
    for system in &output.systems {
        text.push_str(&format!(
            "SYSTEM\t{}\t{}\t{}\n",
            system.name,
            system.function_id,
            system.event_type.as_deref().unwrap_or("")
        ));
    }
    text.into_bytes()
}

pub struct PackageMetadata {
    pub schema: Vec<wmcompiler::v2::SchemaType>,
    pub systems: Vec<wmcompiler::v2::SystemEntry>,
}

pub fn decode_metadata(bytes: &[u8]) -> Result<PackageMetadata, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "archive schema is not UTF-8".to_owned())?;
    if !text.starts_with("WMS-SCHEMA-2\n") {
        return Err("unsupported archive schema".to_owned());
    }
    let mut schema = Vec::new();
    let mut systems = Vec::new();
    for line in text.lines().skip(1) {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() == 4 && parts[0] == "SYSTEM" {
            systems.push(wmcompiler::v2::SystemEntry {
                name: parts[1].to_owned(),
                function_id: parts[2]
                    .parse()
                    .map_err(|_| "invalid system function id".to_owned())?,
                event_type: (!parts[3].is_empty()).then(|| parts[3].to_owned()),
            });
            continue;
        }
        if parts.len() != 3 {
            continue;
        }
        let kind = match parts[0] {
            "Struct" => wmcompiler::v2::RecordKind::Struct,
            "Component" => wmcompiler::v2::RecordKind::Component,
            "Resource" => wmcompiler::v2::RecordKind::Resource,
            "Event" => wmcompiler::v2::RecordKind::Event,
            _ => continue,
        };
        let persistent = parts[2]
            .parse::<bool>()
            .map_err(|_| "invalid persistent schema flag".to_owned())?;
        schema.push(wmcompiler::v2::SchemaType {
            name: parts[1].to_owned(),
            kind,
            persistent,
            fields: Vec::new(),
        });
    }
    Ok(PackageMetadata { schema, systems })
}

pub fn decode_schema(bytes: &[u8]) -> Result<Vec<wmcompiler::v2::SchemaType>, String> {
    Ok(decode_metadata(bytes)?.schema)
}

fn format_diagnostics(errors: Vec<wmcompiler::v2::Diagnostic>) -> String {
    errors
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    fn project(capabilities: Vec<String>) -> Project {
        Project {
            root: PathBuf::from("."),
            name: "test".to_owned(),
            version: "0.1.0".to_owned(),
            entry: PathBuf::from("src/main.wms"),
            tick_hz: 60,
            seed: 1,
            save_compat_version: 1,
            default_target: Target::Headless,
            capabilities,
            assets: Vec::new(),
        }
    }

    #[test]
    fn capability_validation_rejects_unlisted_gui_calls() {
        let module =
            parse_module("main.wms", "on start { ui.say(\"a\", \"b\"); return; }").unwrap();
        assert!(
            validate_capabilities(&project(Vec::new()), &module)
                .unwrap_err()
                .contains("gui")
        );
        validate_capabilities(&project(vec!["gui".to_owned()]), &module).unwrap();
    }
}
