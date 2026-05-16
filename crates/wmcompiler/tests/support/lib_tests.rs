use super::*;
use wmhost::HostRegistry;
use wmvm::{RunOutcome, Vm, VmConfig};

#[test]
fn compiler_keeps_config() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    assert!(compiler.config().platform.capabilities.file_system);
}

#[test]
fn parser_extracts_module_items() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            import "math/util" as m;
            export func add(a, b) {
                return a + b;
            }
            export let version = 1;
        "#;
    let module = compiler.parse_module("main", source).expect("parse module");
    assert_eq!(module.items.len(), 3);
    assert!(matches!(module.items[0], ModuleItem::Import(_)));
    assert!(matches!(module.items[1], ModuleItem::Function(_)));
    assert!(matches!(module.items[2], ModuleItem::Let(_)));
}

#[test]
fn resolver_assigns_ids() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            import "math/util" as m;
            export func add(a, b) {
                return a + b;
            }
            let version = 1;
        "#;
    let module = compiler.parse_module("main", source).expect("parse module");
    let mut catalog = ModuleCatalog::new();
    let imported_module_id = catalog.register("math/util");
    let resolved = compiler
        .resolve_module(module, &mut catalog)
        .expect("resolve module");
    assert_eq!(resolved.module_id, 2);
    assert_eq!(resolved.imports[0].module_id, imported_module_id);
    assert_eq!(resolved.functions[0].function_id, 1);
    assert!(resolved.symbols.get("m").is_some());
}

#[test]
fn compiler_builds_ir() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            import "math/util";
            func init() {
                return;
            }
        "#;
    let module = compiler.parse_module("main", source).expect("parse module");
    let mut catalog = ModuleCatalog::new();
    catalog.register("math/util");
    let compiled = compiler
        .compile("main", source, &mut catalog)
        .expect("compile module");
    assert_eq!(compiled.ast.items.len(), 2);
    assert_eq!(compiled.resolved.imports.len(), 1);
    assert_eq!(compiled.ir.functions.len(), 1);
    assert_eq!(
        compiled.ir.symbols.get("util").unwrap().kind,
        SymbolKind::Import
    );
    assert!(Compiler::supports_opcode(Opcode::PushConst));
    let _ = module;
}

#[test]
fn compiler_emits_program_for_literal_return() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            export func main() {
                return 42;
            }
        "#;
    let mut catalog = ModuleCatalog::new();
    let program = compiler
        .compile_program("main", source, &mut catalog)
        .expect("compile program");
    assert_eq!(program.entry(), Some(1));
    assert_eq!(program.constant_count(), 1);
    let function = program.function(1).expect("function");
    assert_eq!(
        function.code,
        vec![Opcode::PushConst as u8, 0, 0, Opcode::Return as u8]
    );

    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            32,
        ),
        program,
    );
    let outcome = vm.run_frame(32);
    assert!(matches!(
        outcome,
        RunOutcome::Halted {
            value: Some(wmvm::Value::Integer(42)),
            ..
        }
    ));
}

#[test]
fn compiled_logical_short_circuit_runs_without_stack_underflow() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            export func main() {
                if true && false {
                    return "bad-and";
                } else if false || true {
                    return "ok";
                } else {
                    return "bad-or";
                }
            }
        "#;
    let mut catalog = ModuleCatalog::new();
    let program = compiler
        .compile_program("main", source, &mut catalog)
        .expect("compile program");
    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            32,
        ),
        program,
    );
    let outcome = vm.run_frame(64);
    assert!(matches!(
        outcome,
        RunOutcome::Halted {
            value: Some(wmvm::Value::String(text)),
            ..
        } if text == "ok"
    ));
}

#[test]
fn compiler_keeps_implicit_return_after_partial_if_return() {
    let compiler = Compiler::new(CompilerConfig::new(PlatformProfile::native()));
    let source = r#"
            export func main() {
                if false {
                    return "unreachable";
                }
            }
        "#;
    let mut catalog = ModuleCatalog::new();
    let program = compiler
        .compile_program("main", source, &mut catalog)
        .expect("compile program");
    let mut vm = Vm::with_program(
        VmConfig::new(
            PlatformProfile::native(),
            HostRegistry::new(PlatformProfile::native()),
            32,
        ),
        program,
    );
    let outcome = vm.run_frame(64);
    assert!(matches!(
        outcome,
        RunOutcome::Halted {
            value: None | Some(wmvm::Value::Nil),
            ..
        }
    ));
}
