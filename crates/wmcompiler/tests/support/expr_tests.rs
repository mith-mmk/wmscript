use super::*;
use wmbytecode::Opcode;
use wmext::{ExtValueType, ExtensionFunctionSpec, ExtensionRegistry, NamespacePolicy};
use wmhost::{CAP_FILE_SYSTEM, CAP_GUI};
use wmplatform::PlatformProfile;

#[test]
fn optimizer_folds_constant_return_expression() {
    let mut program = VmProgram::new();
    let (code, type_tag) = compile_return_body(
        "return 1 + 2 * 3;",
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::Integer);
    assert_eq!(program.constant_count(), 1);
    assert_eq!(
        code,
        vec![Opcode::PushConst as u8, 0, 0, Opcode::Return as u8]
    );
}

#[test]
fn type_tag_tracks_string_literal() {
    let mut program = VmProgram::new();
    let (code, type_tag) = compile_return_body(
        r#"return "hello";"#,
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert_eq!(program.constant_count(), 1);
    assert_eq!(
        code,
        vec![Opcode::PushConst as u8, 0, 0, Opcode::Return as u8]
    );
}

#[test]
fn bare_return_emits_empty_frame_return() {
    let mut program = VmProgram::new();
    let (code, type_tag) = compile_return_body(
        "return;",
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::Nil);
    assert_eq!(program.constant_count(), 0);
    assert_eq!(code, vec![Opcode::Return as u8]);
}

#[test]
fn statement_sequence_can_call_extensions_before_return() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension(
            "ext.message",
            &[ExtensionFunctionSpec::new("show", 7, 2, 2, CAP_GUI)],
        )
        .expect("register message extension");

    let mut program = VmProgram::new();
    let body = r#"
            ext.message.show("Narrator", "Hello");
            return "Prologue";
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert_eq!(program.constant_count(), 3);
    assert_eq!(
        code,
        vec![
            Opcode::PushConst as u8,
            0,
            0,
            Opcode::PushConst as u8,
            1,
            0,
            Opcode::CallHost as u8,
            7,
            0,
            2,
            Opcode::Pop as u8,
            Opcode::PushConst as u8,
            2,
            0,
            Opcode::Return as u8,
        ]
    );
}

#[test]
fn extension_return_type_metadata_updates_type_tags() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension(
            "ext.fs",
            &[
                ExtensionFunctionSpec::new("exists", 20, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Bool),
            ],
        )
        .expect("register fs extension");
    assert_eq!(
        registry
            .resolve("ext.fs.exists")
            .unwrap()
            .required_capabilities,
        CAP_FILE_SYSTEM
    );

    let mut program = VmProgram::new();
    let (code, type_tag) = compile_return_body(
        r#"return ext.fs.exists("save.dat");"#,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::Bool);
    assert!(code.contains(&(Opcode::CallHost as u8)));
}

#[test]
fn compiler_rejects_extension_calls_without_platform_capabilities() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension(
            "ext.fs",
            &[
                ExtensionFunctionSpec::new("exists", 20, 1, 1, CAP_FILE_SYSTEM)
                    .with_return_type(ExtValueType::Bool),
            ],
        )
        .expect("register fs extension");

    let mut program = VmProgram::new();
    let error = compile_return_body(
        r#"return ext.fs.exists("save.dat");"#,
        &mut program,
        Some(&registry),
        PlatformProfile::wasm().capabilities,
    )
    .expect_err("compile should reject unsupported extension");
    assert!(matches!(
        error,
        CompileError::UnsupportedExpression { source } if source.contains("unsupported capabilities") && source.contains("file_system")
    ));
}

#[test]
fn if_statement_can_branch_on_state_flags() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension(
            "state",
            &[
                ExtensionFunctionSpec::new("has", 11, 1, 1, 0),
                ExtensionFunctionSpec::new("set", 12, 2, 2, 0),
            ],
        )
        .expect("register state extension");

    let mut program = VmProgram::new();
    let body = r#"
            if state.has("read:chapter_1") {
                return "skip";
            } else {
                state.set("read:chapter_1", true);
                return "show";
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
    assert!(code.contains(&(Opcode::Jump as u8)));
    assert!(code.contains(&(Opcode::Return as u8)));
}

#[test]
fn else_if_chains_compile() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension("state", &[ExtensionFunctionSpec::new("get", 173, 1, 1, 0)])
        .expect("register state extension");

    let mut program = VmProgram::new();
    let body = r#"
            if state.get("ui.last_choice") == "choice-1" {
                return "one";
            } else if state.get("ui.last_choice") == "choice-2" {
                return "two";
            } else {
                return "other";
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
    assert!(code.contains(&(Opcode::Jump as u8)));
    assert!(code.contains(&(Opcode::Return as u8)));
}

#[test]
fn comparison_and_not_operators_compile() {
    let mut program = VmProgram::new();
    let body = r#"
            let flag = recv();
            let limit = recv();
            let threshold = recv();
            if !flag {
                return "no";
            } else if limit < threshold {
                return "lt";
            } else if limit >= threshold {
                return "ge";
            } else {
                return "maybe";
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::Not as u8)));
    assert!(code.contains(&(Opcode::Lt as u8)));
    assert!(code.contains(&(Opcode::Ge as u8)));
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
}

#[test]
fn logical_and_or_short_circuit_compile() {
    let mut program = VmProgram::new();
    let body = r#"
            let left = recv();
            let right = recv();
            if left && right || !left {
                return "ok";
            } else {
                return "no";
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
    assert!(code.contains(&(Opcode::JumpIfTrue as u8)));
    assert!(code.contains(&(Opcode::Not as u8)));
}

#[test]
fn let_bindings_can_drive_branching() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension("state", &[ExtensionFunctionSpec::new("get", 173, 1, 1, 0)])
        .expect("register state extension");

    let mut program = VmProgram::new();
    let body = r#"
            let choice = recv();
            if choice == "choice-1" {
                return "one";
            } else {
                return state.get("ui.last_choice");
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::Unknown);
    assert!(code.contains(&(Opcode::Recv as u8)));
    assert!(code.contains(&(Opcode::StoreLocal as u8)));
    assert!(code.contains(&(Opcode::LoadLocal as u8)));
    assert!(code.contains(&(Opcode::Eq as u8)));
}

#[test]
fn recv_can_be_used_as_a_branch_input() {
    let mut registry = ExtensionRegistry::with_policy(NamespacePolicy::permissive());
    registry
        .register_extension("state", &[ExtensionFunctionSpec::new("get", 173, 1, 1, 0)])
        .expect("register state extension");

    let mut program = VmProgram::new();
    let body = r#"
            recv();
            if state.get("ui.last_choice") == "choice-1" {
                return "prologue";
            } else {
                return "other";
            }
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        Some(&registry),
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::Recv as u8)));
    assert!(code.contains(&(Opcode::Eq as u8)));
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
}

#[test]
fn loop_break_continue_and_recv_compile() {
    let mut program = VmProgram::new();
    let body = r#"
            loop {
                let choice = recv();
                if choice == "skip" {
                    continue;
                } else if choice == "done" {
                    break;
                }
            }
            return "after-loop";
        "#;
    let (code, type_tag) = compile_return_body(
        body,
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect("compile body");
    assert_eq!(type_tag, TypeTag::String);
    assert!(code.contains(&(Opcode::Recv as u8)));
    assert!(code.contains(&(Opcode::Jump as u8)));
    assert!(code.contains(&(Opcode::JumpIfFalse as u8)));
    assert!(code.contains(&(Opcode::Return as u8)));
}

#[test]
fn break_and_continue_require_loop() {
    let mut program = VmProgram::new();
    let break_error = compile_return_body(
        "break;",
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect_err("break outside loop should fail");
    assert!(matches!(
        break_error,
        CompileError::UnsupportedExpression { source } if source.contains("break used outside loop")
    ));

    let mut program = VmProgram::new();
    let continue_error = compile_return_body(
        "continue;",
        &mut program,
        None,
        PlatformProfile::native().capabilities,
    )
    .expect_err("continue outside loop should fail");
    assert!(matches!(
        continue_error,
        CompileError::UnsupportedExpression { source } if source.contains("continue used outside loop")
    ));
}
