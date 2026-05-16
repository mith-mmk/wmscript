#![forbid(unsafe_code)]

//! Compiler crate for WML scripts.
//!
//! The crate currently provides a small but coherent pipeline:
//! parse -> resolve imports and symbols -> lower to a declaration IR.

mod ast;
mod compiler;
mod config;
mod error;
mod expr;
mod ir;
mod lowering;
mod parser;
mod symbol;

pub use ast::{FunctionDecl, ImportDecl, LetDecl, ModuleAst, ModuleItem};
pub use compiler::Compiler;
pub use config::{CompilerConfig, FunctionId, ModuleId, SymbolId};
pub use error::{CompileError, ParseError, ParseErrorKind, Result};
pub use ir::{
    CompiledModule, IrFunction, IrGlobal, IrImport, IrModule, ResolvedFunction, ResolvedGlobal,
    ResolvedImport, ResolvedModule,
};
pub use symbol::{ModuleCatalog, SymbolEntry, SymbolKind, SymbolTable};

#[cfg(test)]
use wmbytecode::Opcode;
#[cfg(test)]
use wmplatform::PlatformProfile;

#[cfg(test)]
#[path = "../tests/support/lib_tests.rs"]
mod tests;
