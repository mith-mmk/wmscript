mod ast;
mod check;
mod diagnostic;
mod lexer;
mod lower;
mod parser;
pub mod standard;

pub use ast::*;
pub use check::{CheckedModule, HandlerKindKey};
pub use diagnostic::{Diagnostic, Severity};
pub use lower::{CompileOutput, SchemaType, SystemEntry};

pub fn parse_module(path: &str, source: &str) -> Result<SourceModule, Vec<Diagnostic>> {
    parser::parse(path, source)
}
pub fn check_module(module: SourceModule) -> Result<CheckedModule, Vec<Diagnostic>> {
    check::check(module)
}
pub fn lower_module(module: &CheckedModule) -> Result<CompileOutput, Vec<Diagnostic>> {
    lower::lower(module)
}
pub fn compile_module(path: &str, source: &str) -> Result<CompileOutput, Vec<Diagnostic>> {
    let module = parse_module(path, source)?;
    let checked = check_module(module)?;
    lower_module(&checked)
}
