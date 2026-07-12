use super::*;
use wmplatform::PlatformProfile;
use wmruntime::{Runtime, RuntimeConfig};
use wmvm::Value;

fn toolchainnovel_project() -> GameProject {
    GameProject::new(
        "toolchainnovel",
        "legacy/toolchainnovel.wms",
        r#"export func main() { return "legacy-novel"; }"#,
    )
    .push_asset(GameAsset::new(
        "story/guide",
        10,
        100,
        ResourceType::ScriptData,
        b"legacy guide".to_vec(),
    ))
    .push_asset(
        GameAsset::image("ui/background", 11, 101, b"legacy image".to_vec())
            .with_external_location("assets/uiimage.png", "sha256:toolchainnovel-bg", 1),
    )
}

fn automationrts_project() -> GameProject {
    GameProject::new(
        "automationrts",
        "legacy/automationrts.wms",
        r#"export func main() { return "legacy-rts"; }"#,
    )
}

#[test]
fn build_project_creates_archive_and_program() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
    let project = GameProject::new("sample", "main.wms", r#"export func main() { return 7; }"#)
        .push_asset(GameAsset::new(
            "ui/title",
            10,
            42,
            ResourceType::ScriptData,
            b"title".to_vec(),
        ));

    let build = toolchain.build_project(&project).expect("build");
    assert!(build.archive_size > 64);
    assert_eq!(build.program.entry(), Some(1));
    assert_eq!(build.manifest.resource_map.len(), 1);
}

#[test]
fn run_project_executes_program_and_loads_assets() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
    let project = GameProject::new(
        "sample",
        "main.wms",
        r#"export func main() { return "ok"; }"#,
    )
    .push_asset(GameAsset::new(
        "script/data",
        11,
        77,
        ResourceType::ScriptData,
        b"payload".to_vec(),
    ));
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));

    let report = toolchain.run_project(&mut runtime, &project).expect("run");
    assert_eq!(report.worker_id, 1);
    assert!(matches!(
        report.outcomes.last(),
        Some((_, RunOutcome::Halted { value: Some(_), .. }))
    ));
    assert!(report.loaded_archive.resources_loaded >= 1);
}

#[test]
fn run_archive_reader_executes_compiled_module_from_archive() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::native()));
    let project = GameProject::new(
        "sample",
        "main.wms",
        r#"export func main() { return "archive-ok"; }"#,
    );
    let build = toolchain.build_project(&project).expect("build");
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::native()));

    let report = toolchain
        .run_archive_reader(&mut runtime, std::io::Cursor::new(build.archive.clone()))
        .expect("run archive reader");

    assert_eq!(report.worker_id, 1);
    assert_eq!(report.build.manifest.entry_module_id, 4);
    assert_eq!(report.build.archive_size, build.archive_size);
    assert!(report.build.archive.is_empty());
    assert!(matches!(
        report.outcomes.last(),
        Some((
            _,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "archive-ok"
    ));
}

#[test]
fn build_project_records_package_entries() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::egui()));
    let project = GameProject::new(
        "workers",
        "engine/main.wms",
        r#"export func main() { return "engine"; }"#,
    )
    .push_script(GameScript::new(
        GameWorkerRole::Loader,
        "loader/main.wms",
        r#"export func main() { return "loader"; }"#,
    ))
    .push_script(GameScript::new(
        GameWorkerRole::Ui,
        "ui/main.wms",
        r#"export func main() { return "ui"; }"#,
    ));

    let build = toolchain.build_project(&project).expect("build workers");

    assert_eq!(build.worker_programs.len(), 3);
    assert_eq!(build.manifest.worker_entries.len(), 3);
    assert_eq!(build.manifest.worker_entries[0].role, "ui");
    assert_eq!(build.manifest.worker_entries[1].role, "loader");
    assert_eq!(build.manifest.worker_entries[2].role, "engine");
}

#[test]
fn run_project_spawns_ui_loader_then_engine() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::egui()));
    let project = GameProject::new(
        "workers",
        "engine/main.wms",
        r#"export func main() { return "engine"; }"#,
    )
    .push_script(GameScript::new(
        GameWorkerRole::Loader,
        "loader/main.wms",
        r#"export func main() { return "loader"; }"#,
    ))
    .push_script(GameScript::new(
        GameWorkerRole::Ui,
        "ui/main.wms",
        r#"export func main() { return "ui"; }"#,
    ));
    let mut runtime = Runtime::new(RuntimeConfig::new(PlatformProfile::egui()));

    let report = toolchain
        .run_project(&mut runtime, &project)
        .expect("run workers");

    assert_eq!(report.worker_id, 3);
    assert!(matches!(
        report.outcomes.last(),
        Some((
            3,
            RunOutcome::Halted {
                value: Some(Value::String(text)),
                ..
            }
        )) if text == "engine"
    ));
}

#[test]
fn build_toolchainnovel_sample_with_asset() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::egui()));
    let project = toolchainnovel_project();

    let build = toolchain.build_project(&project).expect("build sample");

    assert!(build.archive_size > 64);
    assert_eq!(build.manifest.package_name, "toolchainnovel");
    assert_eq!(build.manifest.resource_map.len(), 2);
    assert_eq!(build.manifest.resource_map[0].resource_id, 100);
    assert_eq!(build.manifest.resource_map[1].resource_id, 101);
    assert_eq!(
        build.manifest.external_section_locations,
        vec![ManifestSectionLocation::new(
            11,
            "assets/uiimage.png",
            "sha256:toolchainnovel-bg",
            1
        )]
    );
}

#[test]
fn build_automationrts_sample_with_gameplay_extensions() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::egui()));
    let project = automationrts_project();

    let build = toolchain.build_project(&project).expect("build sample");

    assert!(build.archive_size > 64);
    assert_eq!(build.manifest.package_name, "automationrts");
    assert_eq!(build.worker_programs.len(), 1);
    assert_eq!(build.manifest.worker_entries[0].role, "engine");
}

#[test]
fn toolchainnovel_archive_exposes_web_distribution_smoke_manifest() {
    let toolchain = Toolchain::new(ToolchainConfig::new(PlatformProfile::egui()));
    let project = toolchainnovel_project();

    let build = toolchain.build_project(&project).expect("build sample");
    let archive = Archive::decode(&build.archive).expect("archive decode");
    archive.verify_layout().expect("layout");
    archive.verify_manifest_digests().expect("manifest digests");
    let manifest = archive
        .manifest()
        .expect("manifest decode")
        .expect("manifest present");

    assert_eq!(
        manifest.resource_id_by_name_hash(stable_hash64(b"ui/background")),
        Some(101)
    );
    assert_eq!(
        manifest.external_section_locations,
        vec![ManifestSectionLocation::new(
            11,
            "assets/uiimage.png",
            "sha256:toolchainnovel-bg",
            1
        )]
    );

    let section = archive.section(11).expect("background section");
    assert_eq!(section.kind, SectionKind::Asset);
    let payload = archive.section_bytes(11).expect("background payload");
    let digest = manifest
        .section_digests
        .iter()
        .find(|digest| digest.section_id == 11)
        .expect("background digest");
    assert_eq!(digest.section_kind, SectionKind::Asset);
    assert_eq!(digest.flags_canonical, section.flags);
    assert_eq!(digest.unpacked_size, section.unpacked_size);
    assert_eq!(
        digest.digest,
        digest_section(
            section.id,
            section.kind,
            digest.flags_canonical,
            section.unpacked_size,
            payload,
        )
    );
}
