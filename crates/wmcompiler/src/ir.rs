use crate::{FunctionId, ModuleAst, ModuleId, SymbolId, SymbolTable};

/// Resolved import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedImport {
    pub import_id: u32,
    pub module_id: ModuleId,
    pub path: String,
    pub alias: String,
    pub symbol_id: SymbolId,
}

/// Resolved function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunction {
    pub function_id: FunctionId,
    pub symbol_id: SymbolId,
    pub exported: bool,
    pub name: String,
    pub params: Vec<SymbolId>,
    pub locals: SymbolTable,
    pub body: String,
}

/// Resolved global binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGlobal {
    pub symbol_id: SymbolId,
    pub exported: bool,
    pub name: String,
    pub value: String,
}

/// Resolved module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedModule {
    pub module_id: ModuleId,
    pub path: String,
    pub imports: Vec<ResolvedImport>,
    pub symbols: SymbolTable,
    pub functions: Vec<ResolvedFunction>,
    pub globals: Vec<ResolvedGlobal>,
}

/// IR module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrModule {
    pub module_id: ModuleId,
    pub path: String,
    pub imports: Vec<IrImport>,
    pub symbols: SymbolTable,
    pub functions: Vec<IrFunction>,
    pub globals: Vec<IrGlobal>,
}

/// IR import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrImport {
    pub import_id: u32,
    pub module_id: ModuleId,
    pub path: String,
    pub alias: String,
    pub symbol_id: SymbolId,
}

/// IR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrFunction {
    pub function_id: FunctionId,
    pub symbol_id: SymbolId,
    pub exported: bool,
    pub name: String,
    pub params: Vec<SymbolId>,
    pub locals: SymbolTable,
    pub body: String,
}

/// IR global binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrGlobal {
    pub symbol_id: SymbolId,
    pub exported: bool,
    pub name: String,
    pub value: String,
}

/// Complete compiler output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledModule {
    pub ast: ModuleAst,
    pub resolved: ResolvedModule,
    pub ir: IrModule,
}
