use std::convert::TryFrom;

use wmbytecode::Opcode;
use wmvm::{Function as VmFunction, Program as VmProgram};

use crate::lowering::{lower_function_body, ordered_local_names, parse_literal_value};
use crate::parser::{Parser, last_path_segment};
use crate::{
    CompileError, CompiledModule, CompilerConfig, IrFunction, IrGlobal, IrImport, IrModule,
    ModuleAst, ModuleCatalog, ModuleItem, ResolvedFunction, ResolvedGlobal, ResolvedImport,
    ResolvedModule, Result, SymbolKind, SymbolTable,
};

/// Compiler front end.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compiler {
    config: CompilerConfig,
}

impl Compiler {
    /// Creates a compiler for the given platform.
    pub const fn new(config: CompilerConfig) -> Self {
        Self { config }
    }

    /// Returns the compiler configuration.
    pub fn config(&self) -> CompilerConfig {
        self.config.clone()
    }

    /// Reports whether a bytecode opcode is in the current bootstrap set.
    pub const fn supports_opcode(opcode: Opcode) -> bool {
        matches!(opcode, Opcode::Nop | Opcode::Halt | Opcode::PushConst)
    }

    /// Parses a module from source text.
    pub fn parse_module(
        &self,
        path: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<ModuleAst> {
        let path = path.into();
        let source = source.into();
        if source.len() > self.config.max_source_bytes {
            return Err(CompileError::SourceTooLarge {
                len: source.len(),
                max: self.config.max_source_bytes,
            });
        }
        Parser::new(path, &source)
            .parse_module()
            .map_err(CompileError::from)
    }

    /// Resolves module imports and symbols.
    pub fn resolve_module(
        &self,
        module: ModuleAst,
        catalog: &mut ModuleCatalog,
    ) -> Result<ResolvedModule> {
        let module_id = catalog.register(&module.path);
        let mut symbol_table = SymbolTable::new();
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut globals = Vec::new();

        for item in module.items {
            match item {
                ModuleItem::Import(import_decl) => {
                    let imported_module_id =
                        catalog.resolve(&import_decl.path).ok_or_else(|| {
                            CompileError::UnknownModule {
                                path: import_decl.path.clone(),
                            }
                        })?;
                    let alias = import_decl
                        .alias
                        .unwrap_or_else(|| last_path_segment(&import_decl.path).to_owned());
                    let symbol_id =
                        symbol_table.insert(alias.clone(), SymbolKind::Import, false)?;
                    imports.push(ResolvedImport {
                        import_id: imports.len() as u32 + 1,
                        module_id: imported_module_id,
                        path: import_decl.path,
                        alias,
                        symbol_id,
                    });
                }
                ModuleItem::Function(function_decl) => {
                    let symbol_id = symbol_table.insert(
                        function_decl.name.clone(),
                        SymbolKind::Function,
                        function_decl.exported,
                    )?;
                    let mut locals = SymbolTable::new();
                    let mut param_ids = Vec::with_capacity(function_decl.params.len());
                    for param in function_decl.params {
                        param_ids.push(locals.insert(param, SymbolKind::Parameter, false)?);
                    }
                    functions.push(ResolvedFunction {
                        function_id: functions.len() as u32 + 1,
                        symbol_id,
                        exported: function_decl.exported,
                        name: function_decl.name,
                        params: param_ids,
                        locals,
                        body: function_decl.body,
                    });
                }
                ModuleItem::Let(let_decl) => {
                    let symbol_id = symbol_table.insert(
                        let_decl.name.clone(),
                        SymbolKind::Global,
                        let_decl.exported,
                    )?;
                    globals.push(ResolvedGlobal {
                        symbol_id,
                        exported: let_decl.exported,
                        name: let_decl.name,
                        value: let_decl.value,
                    });
                }
            }
        }

        Ok(ResolvedModule {
            module_id,
            path: module.path,
            imports,
            symbols: symbol_table,
            functions,
            globals,
        })
    }

    /// Lowers a resolved module into a declaration-style IR.
    pub fn lower_to_ir(&self, resolved: ResolvedModule) -> IrModule {
        let imports = resolved
            .imports
            .into_iter()
            .map(|import| IrImport {
                import_id: import.import_id,
                module_id: import.module_id,
                path: import.path,
                alias: import.alias,
                symbol_id: import.symbol_id,
            })
            .collect();
        let functions = resolved
            .functions
            .into_iter()
            .map(|function| IrFunction {
                function_id: function.function_id,
                symbol_id: function.symbol_id,
                exported: function.exported,
                name: function.name,
                params: function.params,
                locals: function.locals,
                body: function.body,
            })
            .collect();
        let globals = resolved
            .globals
            .into_iter()
            .map(|global| IrGlobal {
                symbol_id: global.symbol_id,
                exported: global.exported,
                name: global.name,
                value: global.value,
            })
            .collect();

        IrModule {
            module_id: resolved.module_id,
            path: resolved.path,
            imports,
            symbols: resolved.symbols,
            functions,
            globals,
        }
    }

    /// Parses, resolves, and lowers a module in one call.
    pub fn compile(
        &self,
        path: impl Into<String>,
        source: impl Into<String>,
        catalog: &mut ModuleCatalog,
    ) -> Result<CompiledModule> {
        let ast = self.parse_module(path, source)?;
        let resolved = self.resolve_module(ast.clone(), catalog)?;
        let ir = self.lower_to_ir(resolved.clone());
        Ok(CompiledModule { ast, resolved, ir })
    }

    /// Lowers an IR module to a runnable VM program.
    pub fn lower_to_program(&self, ir: &IrModule) -> Result<VmProgram> {
        let mut program = VmProgram::new();

        for global in &ir.globals {
            let value = parse_literal_value(&global.value)?;
            let _ = program.push_constant(value);
        }

        let mut entry = None;
        for function in &ir.functions {
            let func_id = u16::try_from(function.function_id).map_err(|_| {
                CompileError::BytecodeOverflow {
                    what: "function id",
                    value: function.function_id,
                }
            })?;
            let arg_count = u8::try_from(function.params.len()).map_err(|_| {
                CompileError::BytecodeOverflow {
                    what: "argument count",
                    value: function.params.len() as u32,
                }
            })?;
            let initial_locals = ordered_local_names(&function.locals);
            let (code, local_count) = lower_function_body(
                &function.body,
                &mut program,
                self.config.extension_registry(),
                self.config.platform.capabilities,
                &initial_locals,
            )?;
            let local_count =
                u8::try_from(local_count).map_err(|_| CompileError::BytecodeOverflow {
                    what: "local count",
                    value: local_count as u32,
                })?;
            program.insert_function(VmFunction::new(func_id, code, arg_count, local_count));
            if entry.is_none() && function.name == "main" {
                entry = Some(func_id);
            }
        }

        if entry.is_none() {
            entry = program.function_ids().next();
        }
        if let Some(entry) = entry {
            program.set_entry(entry);
        }

        Ok(program)
    }

    /// Compiles source text all the way down to a runnable VM program.
    pub fn compile_program(
        &self,
        path: impl Into<String>,
        source: impl Into<String>,
        catalog: &mut ModuleCatalog,
    ) -> Result<VmProgram> {
        let compiled = self.compile(path, source, catalog)?;
        self.lower_to_program(&compiled.ir)
    }
}
