#![forbid(unsafe_code)]

//! Compiler crate for WML scripts.
//!
//! The crate currently provides a small but coherent pipeline:
//! parse -> resolve imports and symbols -> lower to a declaration IR.

use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;

mod expr;

use wmlbytecode::Opcode;
use wmlplatform::PlatformProfile;
use wmlvm::{Function as VmFunction, Program as VmProgram, Value as VmValue};

/// Compiler configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerConfig {
    /// Target platform profile.
    pub platform: PlatformProfile,
    /// Maximum accepted source size in bytes.
    pub max_source_bytes: usize,
}

impl CompilerConfig {
    /// Creates a new compiler configuration.
    pub const fn new(platform: PlatformProfile) -> Self {
        Self {
            platform,
            max_source_bytes: 1 << 20,
        }
    }

    /// Sets the maximum accepted source size.
    pub const fn with_max_source_bytes(mut self, max_source_bytes: usize) -> Self {
        self.max_source_bytes = max_source_bytes;
        self
    }
}

/// Lightweight module identifier used during import resolution.
pub type ModuleId = u32;

/// Identifier assigned to a function within a module.
pub type FunctionId = u32;

/// Identifier assigned to a symbol.
pub type SymbolId = u32;

/// Result type used by the compiler.
pub type Result<T> = core::result::Result<T, CompileError>;

/// Compiler error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    SourceTooLarge { len: usize, max: usize },
    Parse(ParseError),
    UnknownModule { path: String },
    DuplicateSymbol { name: String },
    UnsupportedExpression { source: String },
    BytecodeOverflow { what: &'static str, value: u32 },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTooLarge { len, max } => {
                write!(f, "source too large: {len} bytes (max {max})")
            }
            Self::Parse(error) => write!(f, "{error}"),
            Self::UnknownModule { path } => write!(f, "unknown module: {path}"),
            Self::DuplicateSymbol { name } => write!(f, "duplicate symbol: {name}"),
            Self::UnsupportedExpression { source } => {
                write!(f, "unsupported expression: {source}")
            }
            Self::BytecodeOverflow { what, value } => {
                write!(f, "{what} does not fit in target bytecode type: {value}")
            }
        }
    }
}

impl std::error::Error for CompileError {}

impl From<ParseError> for CompileError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

/// Parser error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub kind: ParseErrorKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.path, self.line, self.column, self.kind
        )
    }
}

impl std::error::Error for ParseError {}

/// Fine-grained parser error class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnexpectedEof,
    UnexpectedToken { expected: String, found: String },
    InvalidIdentifier(String),
    InvalidStringLiteral,
    InvalidImportSyntax,
    InvalidFunctionSyntax,
    InvalidLetSyntax,
    UnbalancedBraces,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::UnexpectedToken { expected, found } => {
                write!(f, "unexpected token: expected {expected}, found {found}")
            }
            Self::InvalidIdentifier(value) => write!(f, "invalid identifier: {value}"),
            Self::InvalidStringLiteral => f.write_str("invalid string literal"),
            Self::InvalidImportSyntax => f.write_str("invalid import syntax"),
            Self::InvalidFunctionSyntax => f.write_str("invalid function syntax"),
            Self::InvalidLetSyntax => f.write_str("invalid let syntax"),
            Self::UnbalancedBraces => f.write_str("unbalanced braces"),
        }
    }
}

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
    pub const fn config(&self) -> CompilerConfig {
        self.config
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
            let local_count = u8::try_from(function.locals.iter().count()).map_err(|_| {
                CompileError::BytecodeOverflow {
                    what: "local count",
                    value: function.locals.iter().count() as u32,
                }
            })?;
            let code = lower_function_body(&function.body, &mut program)?;
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

/// Module catalog used for import resolution and stable module id assignment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleCatalog {
    modules: BTreeMap<String, ModuleId>,
    next_module_id: ModuleId,
}

impl ModuleCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, path: &str) -> ModuleId {
        if let Some(module_id) = self.modules.get(path) {
            return *module_id;
        }
        let module_id = self.next_module_id.max(1);
        self.next_module_id = module_id.saturating_add(1);
        self.modules.insert(path.to_owned(), module_id);
        module_id
    }

    pub fn resolve(&self, path: &str) -> Option<ModuleId> {
        self.modules.get(path).copied()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.modules.contains_key(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.modules.keys().map(String::as_str)
    }
}

/// Top-level module AST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleAst {
    pub path: String,
    pub items: Vec<ModuleItem>,
}

/// AST item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleItem {
    Import(ImportDecl),
    Function(FunctionDecl),
    Let(LetDecl),
}

/// Import declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDecl {
    pub path: String,
    pub alias: Option<String>,
}

/// Function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub exported: bool,
    pub name: String,
    pub params: Vec<String>,
    pub body: String,
}

/// Top-level global binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LetDecl {
    pub exported: bool,
    pub name: String,
    pub value: String,
}

/// Symbol kinds tracked by the resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Import,
    Function,
    Global,
    Parameter,
}

/// Symbol table entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolEntry {
    pub symbol_id: SymbolId,
    pub kind: SymbolKind,
    pub exported: bool,
}

/// Symbol table used in the resolver and IR.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolTable {
    entries: BTreeMap<String, SymbolEntry>,
    next_symbol_id: SymbolId,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        kind: SymbolKind,
        exported: bool,
    ) -> Result<SymbolId> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(CompileError::DuplicateSymbol { name });
        }
        let symbol_id = self.next_symbol_id.max(1);
        self.next_symbol_id = symbol_id.saturating_add(1);
        self.entries.insert(
            name,
            SymbolEntry {
                symbol_id,
                kind,
                exported,
            },
        );
        Ok(symbol_id)
    }

    pub fn get(&self, name: &str) -> Option<&SymbolEntry> {
        self.entries.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &SymbolEntry)> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.as_str(), entry))
    }
}

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

struct Parser<'a> {
    path: String,
    source: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(path: String, source: &'a str) -> Self {
        Self {
            path,
            source,
            index: 0,
        }
    }

    fn parse_module(mut self) -> std::result::Result<ModuleAst, ParseError> {
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.eof() {
                break;
            }
            if self.consume_keyword("import") {
                items.push(ModuleItem::Import(self.parse_import()?));
                continue;
            }
            let exported = self.consume_keyword("export");
            self.skip_ws_and_comments();
            if self.consume_keyword("func") {
                items.push(ModuleItem::Function(self.parse_function(exported)?));
                continue;
            }
            if self.consume_keyword("let") {
                items.push(ModuleItem::Let(self.parse_let(exported)?));
                continue;
            }
            let found = self.peek_token().unwrap_or_else(|| "<eof>".to_owned());
            return Err(self.error(ParseErrorKind::UnexpectedToken {
                expected: "import, export func, or let".to_owned(),
                found,
            }));
        }

        Ok(ModuleAst {
            path: self.path,
            items,
        })
    }

    fn parse_import(&mut self) -> std::result::Result<ImportDecl, ParseError> {
        self.skip_ws_and_comments();
        let path = self.parse_string_literal()?;
        self.skip_ws_and_comments();
        let alias = if self.consume_keyword("as") {
            self.skip_ws_and_comments();
            Some(self.parse_identifier()?)
        } else {
            None
        };
        self.skip_ws_and_comments();
        self.expect_byte(b';', ";")?;
        Ok(ImportDecl { path, alias })
    }

    fn parse_function(&mut self, exported: bool) -> std::result::Result<FunctionDecl, ParseError> {
        self.skip_ws_and_comments();
        let name = self.parse_identifier()?;
        self.skip_ws_and_comments();
        self.expect_byte(b'(', "(")?;
        let mut params = Vec::new();
        self.skip_ws_and_comments();
        if !self.consume_byte(b')') {
            loop {
                self.skip_ws_and_comments();
                params.push(self.parse_identifier()?);
                self.skip_ws_and_comments();
                if self.consume_byte(b')') {
                    break;
                }
                self.expect_byte(b',', ",")?;
            }
        }
        self.skip_ws_and_comments();
        self.expect_byte(b'{', "{")?;
        let body = self.read_block()?;
        Ok(FunctionDecl {
            exported,
            name,
            params,
            body,
        })
    }

    fn parse_let(&mut self, exported: bool) -> std::result::Result<LetDecl, ParseError> {
        self.skip_ws_and_comments();
        let name = self.parse_identifier()?;
        self.skip_ws_and_comments();
        self.expect_byte(b'=', "=")?;
        self.skip_ws_and_comments();
        let value = self.read_until_semicolon()?;
        Ok(LetDecl {
            exported,
            name,
            value,
        })
    }

    fn read_until_semicolon(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut in_string = false;
        let mut quote = 0u8;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    in_string = false;
                }
                self.index += 1;
                continue;
            }
            if byte == b'\'' || byte == b'"' {
                in_string = true;
                quote = byte;
                self.index += 1;
                continue;
            }
            if byte == b';' {
                let end = self.index;
                self.index += 1;
                return Ok(self.source[start..end].trim().to_owned());
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::UnexpectedEof))
    }

    fn read_block(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut depth = 1usize;
        let mut in_string = false;
        let mut quote = 0u8;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    in_string = false;
                }
                self.index += 1;
                continue;
            }
            if byte == b'/' && bytes.get(self.index + 1) == Some(&b'/') {
                self.index += 2;
                while let Some(&next) = bytes.get(self.index) {
                    self.index += 1;
                    if next == b'\n' {
                        break;
                    }
                }
                continue;
            }
            match byte {
                b'\'' | b'"' => {
                    in_string = true;
                    quote = byte;
                }
                b'{' => {
                    depth += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = self.index;
                        self.index += 1;
                        return Ok(self.source[start..end].to_owned());
                    }
                }
                _ => {}
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::UnbalancedBraces))
    }

    fn parse_identifier(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let first = bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| self.error(ParseErrorKind::UnexpectedEof))?;
        if !is_ident_start(first) {
            return Err(self.error(ParseErrorKind::InvalidIdentifier(
                self.peek_token().unwrap_or_default(),
            )));
        }
        self.index += 1;
        while let Some(&byte) = bytes.get(self.index) {
            if !is_ident_continue(byte) {
                break;
            }
            self.index += 1;
        }
        Ok(self.source[start..self.index].to_owned())
    }

    fn parse_string_literal(&mut self) -> std::result::Result<String, ParseError> {
        self.skip_ws_and_comments();
        if !self.consume_byte(b'"') {
            return Err(self.error(ParseErrorKind::InvalidStringLiteral));
        }
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let end = self.index;
                self.index += 1;
                return Ok(self.source[start..end].to_owned());
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::InvalidStringLiteral))
    }

    fn skip_ws_and_comments(&mut self) -> bool {
        let bytes = self.source.as_bytes();
        let mut advanced = false;
        while let Some(&byte) = bytes.get(self.index) {
            if byte.is_ascii_whitespace() {
                self.index += 1;
                advanced = true;
                continue;
            }
            if byte == b'/' && bytes.get(self.index + 1) == Some(&b'/') {
                self.index += 2;
                while let Some(&next) = bytes.get(self.index) {
                    self.index += 1;
                    if next == b'\n' {
                        break;
                    }
                }
                advanced = true;
                continue;
            }
            break;
        }
        !self.eof() || advanced
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let bytes = self.source.as_bytes();
        let end = self.index.saturating_add(keyword.len());
        if self.source[self.index..].starts_with(keyword)
            && bytes
                .get(end)
                .map_or(true, |byte| !is_ident_continue(*byte))
        {
            self.index = end;
            return true;
        }
        false
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.source.as_bytes().get(self.index) == Some(&byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(
        &mut self,
        byte: u8,
        expected: &'static str,
    ) -> std::result::Result<(), ParseError> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            let found = self.peek_token().unwrap_or_else(|| "<eof>".to_owned());
            Err(self.error(ParseErrorKind::UnexpectedToken {
                expected: expected.to_owned(),
                found,
            }))
        }
    }

    fn peek_token(&self) -> Option<String> {
        self.source[self.index..]
            .chars()
            .next()
            .map(|ch| ch.to_string())
    }

    fn eof(&self) -> bool {
        self.index >= self.source.len()
    }

    fn error(&self, kind: ParseErrorKind) -> ParseError {
        let (line, column) = line_col_at(self.source, self.index);
        ParseError {
            path: self.path.clone(),
            line,
            column,
            kind,
        }
    }
}

fn line_col_at(source: &str, index: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    for byte in source.as_bytes().iter().take(index) {
        if *byte == b'\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_uppercase() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn last_path_segment(path: &str) -> &str {
    path.rsplit(['/', '.']).next().unwrap_or(path)
}

fn lower_function_body(body: &str, program: &mut VmProgram) -> Result<Vec<u8>> {
    let (code, _type_tag) = expr::compile_return_body(body, program)?;
    Ok(code)
}

fn parse_literal_value(source: &str) -> Result<VmValue> {
    let trimmed = source.trim();
    if trimmed.is_empty() || trimmed == "nil" {
        return Ok(VmValue::Nil);
    }
    if trimmed == "true" {
        return Ok(VmValue::Bool(true));
    }
    if trimmed == "false" {
        return Ok(VmValue::Bool(false));
    }
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        return Ok(VmValue::String(unescape_string(inner)?));
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return Ok(VmValue::Integer(value));
    }
    if looks_like_float_literal(trimmed) {
        if let Ok(value) = trimmed.parse::<f64>() {
            return Ok(VmValue::Float(value));
        }
    }

    Err(CompileError::UnsupportedExpression {
        source: trimmed.to_owned(),
    })
}

fn looks_like_float_literal(source: &str) -> bool {
    source.contains('.') || source.contains('e') || source.contains('E')
}

fn unescape_string(source: &str) -> Result<String> {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let escaped = chars
            .next()
            .ok_or_else(|| CompileError::UnsupportedExpression {
                source: source.to_owned(),
            })?;
        match escaped {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            other => {
                return Err(CompileError::UnsupportedExpression {
                    source: format!("\\{other}"),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmlhost::HostRegistry;
    use wmlvm::{RunOutcome, Vm, VmConfig};

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
                value: Some(wmlvm::Value::Integer(42)),
                ..
            }
        ));
    }
}
