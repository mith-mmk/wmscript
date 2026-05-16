#![forbid(unsafe_code)]

use wmbytecode::{Op, encode_op};
use wmext::{ExtFunction, ExtensionRegistry};
use wmhost::{CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_GUI, CAP_NETWORK, CAP_WEB_COMPAT, CapabilityMask};
use wmplatform::PlatformCapabilities;
use wmvm::{Program as VmProgram, Value as VmValue};

use super::lowering::parse_literal_value;
use super::{CompileError, Result};

mod fold;
mod infer;
mod parser;
mod scope;

use fold::*;
use infer::*;
use parser::ExprParser;
use scope::LocalScope;

/// Type tags used by the compiler's expression analyzer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeTag {
    Unknown,
    Nil,
    Bool,
    Integer,
    Float,
    String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Literal(VmValue),
    Variable(String),
    UnaryNeg(Box<Expr>),
    UnaryNot(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        path: Vec<String>,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum Stmt {
    Expr(Expr),
    Let {
        name: String,
        value: Option<Expr>,
    },
    Return(Option<Expr>),
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Vec<Stmt>,
    },
    Loop {
        body: Vec<Stmt>,
    },
    Break,
    Continue,
}

/// Compiles a `return` statement body into bytecode and a type tag.
#[cfg(test)]
pub(crate) fn compile_return_body(
    body: &str,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
) -> Result<(Vec<u8>, TypeTag)> {
    let (code, type_tag, _) = compile_body(
        body,
        program,
        extension_registry,
        platform_capabilities,
        &[],
    )?;
    Ok((code, type_tag))
}

pub(crate) fn compile_function_body(
    body: &str,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    initial_locals: &[String],
) -> Result<(Vec<u8>, TypeTag, usize)> {
    compile_body(
        body,
        program,
        extension_registry,
        platform_capabilities,
        initial_locals,
    )
}

fn compile_body(
    body: &str,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    initial_locals: &[String],
) -> Result<(Vec<u8>, TypeTag, usize)> {
    let mut parser = ExprParser::new(body);
    let statements = parser.parse_statements(None)?;
    let mut locals = LocalScope::new(initial_locals)?;
    let mut code = Vec::new();
    let mut type_tag = None;
    let emitted = emit_statements(
        &statements,
        program,
        extension_registry,
        platform_capabilities,
        &mut locals,
        &mut code,
        &mut type_tag,
        None,
    )?;
    let saw_return = emitted.saw_return;
    let type_tag = type_tag.or(emitted.return_type).unwrap_or(TypeTag::Nil);
    let local_count = locals.local_count();

    if !saw_return {
        encode_op(&Op::Return, &mut code);
    }

    Ok((code, type_tag, local_count))
}

fn optimize_expr(expr: Expr) -> Result<Expr> {
    match expr {
        Expr::UnaryNeg(inner) => {
            let inner = optimize_expr(*inner)?;
            if let Some(value) = fold_unary_neg(&inner)? {
                return Ok(Expr::Literal(value));
            }
            Ok(Expr::UnaryNeg(Box::new(inner)))
        }
        Expr::UnaryNot(inner) => {
            let inner = optimize_expr(*inner)?;
            if let Some(value) = fold_unary_not(&inner)? {
                return Ok(Expr::Literal(value));
            }
            Ok(Expr::UnaryNot(Box::new(inner)))
        }
        Expr::Binary { op, left, right } => {
            let left = optimize_expr(*left)?;
            let right = optimize_expr(*right)?;
            if let Some(value) = fold_binary(op, &left, &right)? {
                return Ok(Expr::Literal(value));
            }
            Ok(Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        Expr::Call { path, args } => Ok(Expr::Call {
            path,
            args: args
                .into_iter()
                .map(optimize_expr)
                .collect::<Result<Vec<_>>>()?,
        }),
        literal => Ok(literal),
    }
}

fn emit_statements(
    statements: &[Stmt],
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    locals: &mut LocalScope,
    out: &mut Vec<u8>,
    type_tag: &mut Option<TypeTag>,
    mut loop_ctx: Option<&mut LoopContext>,
) -> Result<EmitResult> {
    let mut summary = EmitResult::default();
    for stmt in statements {
        let emitted = emit_statement(
            stmt,
            program,
            extension_registry,
            platform_capabilities,
            locals,
            out,
            type_tag,
            loop_ctx.as_deref_mut(),
        )?;
        summary.saw_return |= emitted.saw_return;
        if let Some(return_type) = emitted.return_type {
            *type_tag = Some(match type_tag.take() {
                Some(existing) => merge_type_tags(existing, return_type),
                None => return_type,
            });
        }
        summary.return_type = match (summary.return_type, emitted.return_type) {
            (Some(existing), Some(next)) => Some(merge_type_tags(existing, next)),
            (None, Some(next)) => Some(next),
            (current, None) => current,
        };
    }
    Ok(summary)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EmitResult {
    saw_return: bool,
    return_type: Option<TypeTag>,
}

#[derive(Debug)]
struct LoopContext {
    start: usize,
    breaks: Vec<usize>,
    continues: Vec<usize>,
}

impl LoopContext {
    fn new(start: usize) -> Self {
        Self {
            start,
            breaks: Vec::new(),
            continues: Vec::new(),
        }
    }
}

fn emit_statement(
    stmt: &Stmt,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    locals: &mut LocalScope,
    out: &mut Vec<u8>,
    type_tag: &mut Option<TypeTag>,
    mut loop_ctx: Option<&mut LoopContext>,
) -> Result<EmitResult> {
    match stmt {
        Stmt::Expr(expr) => {
            let optimized = optimize_expr(expr.clone())?;
            emit_expr_into(
                &optimized,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?;
            encode_op(&Op::Pop, out);
            Ok(EmitResult::default())
        }
        Stmt::Let { name, value } => {
            if let Some(expr) = value.clone() {
                let optimized = optimize_expr(expr)?;
                emit_expr_into(
                    &optimized,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
            } else {
                encode_op(&Op::PushNil, out);
            }
            let slot = locals.declare(name.clone())?;
            encode_op(&Op::StoreLocal(slot), out);
            Ok(EmitResult::default())
        }
        Stmt::Return(expr) => {
            let return_type = if let Some(expr) = expr.clone() {
                let optimized = optimize_expr(expr)?;
                let return_type =
                    infer_type(&optimized, extension_registry, platform_capabilities)?;
                emit_expr_into(
                    &optimized,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                *type_tag = Some(match type_tag.take() {
                    Some(existing) => merge_type_tags(existing, return_type),
                    None => return_type,
                });
                Some(return_type)
            } else {
                Some(TypeTag::Nil)
            };
            encode_op(&Op::Return, out);
            Ok(EmitResult {
                saw_return: true,
                return_type,
            })
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let optimized = optimize_expr(condition.clone())?;
            emit_expr_into(
                &optimized,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?;
            let jump_false_pos = emit_jump_placeholder(Op::JumpIfFalse(0), out);
            let mut then_locals = locals.clone();
            let then_result = emit_statements(
                then_branch,
                program,
                extension_registry,
                platform_capabilities,
                &mut then_locals,
                out,
                type_tag,
                loop_ctx.as_deref_mut(),
            )?;
            locals.merge_max(then_locals.local_count());
            let mut else_result = EmitResult::default();
            if else_branch.is_empty() {
                let target = out.len();
                patch_jump_target(out, jump_false_pos, target)?;
            } else {
                let jump_end_pos = emit_jump_placeholder(Op::Jump(0), out);
                let target = out.len();
                patch_jump_target(out, jump_false_pos, target)?;
                let mut else_locals = locals.clone();
                else_result = emit_statements(
                    else_branch,
                    program,
                    extension_registry,
                    platform_capabilities,
                    &mut else_locals,
                    out,
                    type_tag,
                    loop_ctx.as_deref_mut(),
                )?;
                locals.merge_max(else_locals.local_count());
                let target = out.len();
                patch_jump_target(out, jump_end_pos, target)?;
            }
            let return_type = match (then_result, else_result) {
                (
                    EmitResult {
                        saw_return: true,
                        return_type: Some(left),
                    },
                    EmitResult {
                        saw_return: true,
                        return_type: Some(right),
                    },
                ) if left == right => Some(left),
                _ => None,
            };
            let saw_return =
                then_result.saw_return && !else_branch.is_empty() && else_result.saw_return;
            Ok(EmitResult {
                saw_return,
                return_type,
            })
        }
        Stmt::Loop { body } => {
            let loop_start = out.len();
            let mut local_loop_ctx = LoopContext::new(loop_start);
            let mut body_locals = locals.clone();
            let body_result = emit_statements(
                body,
                program,
                extension_registry,
                platform_capabilities,
                &mut body_locals,
                out,
                type_tag,
                Some(&mut local_loop_ctx),
            )?;
            locals.merge_max(body_locals.local_count());
            let jump_back_pos = emit_jump_placeholder(Op::Jump(0), out);
            patch_jump_target(out, jump_back_pos, loop_start)?;
            let loop_end = out.len();
            let has_break = !local_loop_ctx.breaks.is_empty();
            for break_pos in local_loop_ctx.breaks {
                patch_jump_target(out, break_pos, loop_end)?;
            }
            for continue_pos in local_loop_ctx.continues {
                patch_jump_target(out, continue_pos, local_loop_ctx.start)?;
            }
            Ok(EmitResult {
                saw_return: body_result.saw_return && !has_break,
                return_type: if has_break {
                    None
                } else {
                    body_result.return_type
                },
            })
        }
        Stmt::Break => {
            let Some(loop_ctx) = loop_ctx.as_deref_mut() else {
                return Err(unsupported_expression("break used outside loop"));
            };
            let pos = emit_jump_placeholder(Op::Jump(0), out);
            loop_ctx.breaks.push(pos);
            Ok(EmitResult::default())
        }
        Stmt::Continue => {
            let Some(loop_ctx) = loop_ctx.as_deref_mut() else {
                return Err(unsupported_expression("continue used outside loop"));
            };
            let pos = emit_jump_placeholder(Op::Jump(0), out);
            loop_ctx.continues.push(pos);
            Ok(EmitResult::default())
        }
    }
}

fn merge_type_tags(left: TypeTag, right: TypeTag) -> TypeTag {
    match (left, right) {
        (TypeTag::Unknown, _) | (_, TypeTag::Unknown) => TypeTag::Unknown,
        (TypeTag::Nil, other) => other,
        (other, TypeTag::Nil) => other,
        (left, right) if left == right => left,
        _ => TypeTag::Unknown,
    }
}

fn emit_jump_placeholder(opcode: Op, out: &mut Vec<u8>) -> usize {
    let pos = out.len();
    match opcode {
        Op::Jump(_) => encode_op(&Op::Jump(0), out),
        Op::JumpIfFalse(_) => encode_op(&Op::JumpIfFalse(0), out),
        Op::JumpIfTrue(_) => encode_op(&Op::JumpIfTrue(0), out),
        other => encode_op(&other, out),
    }
    pos
}

fn patch_jump_target(out: &mut [u8], opcode_pos: usize, target: usize) -> Result<()> {
    let target = u32::try_from(target).map_err(|_| CompileError::BytecodeOverflow {
        what: "jump target",
        value: target as u32,
    })?;
    let operand_pos = opcode_pos + 1;
    let bytes = target.to_le_bytes();
    let end = operand_pos + bytes.len();
    if end > out.len() {
        return Err(unsupported_expression("jump target patch out of range"));
    }
    out[operand_pos..end].copy_from_slice(&bytes);
    Ok(())
}

fn emit_expr_into(
    expr: &Expr,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    locals: &LocalScope,
    out: &mut Vec<u8>,
) -> Result<()> {
    match expr {
        Expr::Literal(value) => {
            let const_id = program.push_constant(value.clone());
            encode_op(&Op::PushConst(const_id), out);
        }
        Expr::Variable(name) => {
            let slot = locals.lookup(name)?;
            encode_op(&Op::LoadLocal(slot), out);
        }
        Expr::UnaryNeg(inner) => {
            emit_expr_into(
                inner,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?;
            encode_op(&Op::Neg, out);
        }
        Expr::UnaryNot(inner) => {
            emit_expr_into(
                inner,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?;
            encode_op(&Op::Not, out);
        }
        Expr::Binary { op, left, right } => match op {
            BinaryOp::And => emit_short_circuit_and(
                left,
                right,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?,
            BinaryOp::Or => emit_short_circuit_or(
                left,
                right,
                program,
                extension_registry,
                platform_capabilities,
                locals,
                out,
            )?,
            BinaryOp::Add => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Add, out);
            }
            BinaryOp::Sub => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Sub, out);
            }
            BinaryOp::Mul => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Mul, out);
            }
            BinaryOp::Div => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Div, out);
            }
            BinaryOp::Eq => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Eq, out);
            }
            BinaryOp::Ne => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Ne, out);
            }
            BinaryOp::Lt => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Lt, out);
            }
            BinaryOp::Le => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Le, out);
            }
            BinaryOp::Gt => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Gt, out);
            }
            BinaryOp::Ge => {
                emit_expr_into(
                    left,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                emit_expr_into(
                    right,
                    program,
                    extension_registry,
                    platform_capabilities,
                    locals,
                    out,
                )?;
                encode_op(&Op::Ge, out);
            }
        },
        Expr::Call { path, args } => {
            let full_name = path.join(".");
            if path.len() == 1 {
                match path[0].as_str() {
                    "recv" => {
                        if !args.is_empty() {
                            return Err(unsupported_expression(
                                "recv() does not take any arguments",
                            ));
                        }
                        encode_op(&Op::Recv, out);
                        return Ok(());
                    }
                    "try_recv" => {
                        if !args.is_empty() {
                            return Err(unsupported_expression(
                                "try_recv() does not take any arguments",
                            ));
                        }
                        encode_op(&Op::TryRecv, out);
                        return Ok(());
                    }
                    "yield" => {
                        if !args.is_empty() {
                            return Err(unsupported_expression(
                                "yield() does not take any arguments",
                            ));
                        }
                        encode_op(&Op::Yield, out);
                        return Ok(());
                    }
                    "sleep" => {
                        if !args.is_empty() {
                            return Err(unsupported_expression(
                                "sleep() does not take any arguments",
                            ));
                        }
                        encode_op(&Op::Sleep, out);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            let Some(extension_registry) = extension_registry else {
                return Err(unsupported_expression(
                    "extension calls require an extension registry",
                ));
            };
            let ext = extension_registry.resolve(&full_name).map_err(|error| {
                unsupported_expression(format!("unknown extension call `{full_name}`: {error}"))
            })?;
            ensure_extension_capabilities(ext, platform_capabilities, &full_name)?;
            if args.len() < ext.min_args as usize || args.len() > ext.max_args as usize {
                return Err(unsupported_expression(format!(
                    "extension call `{full_name}` expected {}..={} args, got {}",
                    ext.min_args,
                    ext.max_args,
                    args.len()
                )));
            }
            for arg in args {
                emit_expr_into(
                    arg,
                    program,
                    Some(extension_registry),
                    platform_capabilities,
                    locals,
                    out,
                )?;
            }
            encode_op(&Op::CallHost(ext.host_id, args.len() as u8), out);
        }
    }
    Ok(())
}

fn emit_short_circuit_and(
    left: &Expr,
    right: &Expr,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    locals: &LocalScope,
    out: &mut Vec<u8>,
) -> Result<()> {
    emit_expr_into(
        left,
        program,
        extension_registry,
        platform_capabilities,
        locals,
        out,
    )?;
    let jump_false_pos = emit_jump_placeholder(Op::JumpIfFalse(0), out);
    emit_expr_into(
        right,
        program,
        extension_registry,
        platform_capabilities,
        locals,
        out,
    )?;
    let jump_false_pos_right = emit_jump_placeholder(Op::JumpIfFalse(0), out);
    encode_op(&Op::PushTrue, out);
    let jump_end_pos = emit_jump_placeholder(Op::Jump(0), out);
    let false_target = out.len();
    patch_jump_target(out, jump_false_pos, false_target)?;
    patch_jump_target(out, jump_false_pos_right, false_target)?;
    encode_op(&Op::PushFalse, out);
    let end_target = out.len();
    patch_jump_target(out, jump_end_pos, end_target)?;
    Ok(())
}

fn emit_short_circuit_or(
    left: &Expr,
    right: &Expr,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
    locals: &LocalScope,
    out: &mut Vec<u8>,
) -> Result<()> {
    emit_expr_into(
        left,
        program,
        extension_registry,
        platform_capabilities,
        locals,
        out,
    )?;
    let jump_true_pos = emit_jump_placeholder(Op::JumpIfTrue(0), out);
    emit_expr_into(
        right,
        program,
        extension_registry,
        platform_capabilities,
        locals,
        out,
    )?;
    let jump_true_pos_right = emit_jump_placeholder(Op::JumpIfTrue(0), out);
    encode_op(&Op::PushFalse, out);
    let jump_end_pos = emit_jump_placeholder(Op::Jump(0), out);
    let true_target = out.len();
    patch_jump_target(out, jump_true_pos, true_target)?;
    patch_jump_target(out, jump_true_pos_right, true_target)?;
    encode_op(&Op::PushTrue, out);
    let end_target = out.len();
    patch_jump_target(out, jump_end_pos, end_target)?;
    Ok(())
}

fn unsupported_expression(message: impl Into<String>) -> CompileError {
    CompileError::UnsupportedExpression {
        source: message.into(),
    }
}

fn ensure_extension_capabilities(
    ext: &ExtFunction,
    platform_capabilities: PlatformCapabilities,
    full_name: &str,
) -> Result<()> {
    let supported = platform_capability_mask(platform_capabilities);
    let missing = ext.required_capabilities & !supported;
    if missing == 0 {
        return Ok(());
    }
    let missing = missing_capability_names(missing);
    Err(unsupported_expression(format!(
        "extension call `{full_name}` requires unsupported capabilities: {missing}"
    )))
}

fn platform_capability_mask(capabilities: PlatformCapabilities) -> CapabilityMask {
    let mut mask = 0;
    if capabilities.file_system {
        mask |= CAP_FILE_SYSTEM;
    }
    if capabilities.async_io {
        mask |= CAP_ASYNC_IO;
    }
    if capabilities.gui {
        mask |= CAP_GUI;
    }
    if capabilities.network {
        mask |= CAP_NETWORK;
    }
    if capabilities.web_compat {
        mask |= CAP_WEB_COMPAT;
    }
    mask
}

fn missing_capability_names(mask: CapabilityMask) -> String {
    let mut names = Vec::new();
    if mask & CAP_FILE_SYSTEM != 0 {
        names.push("file_system");
    }
    if mask & CAP_ASYNC_IO != 0 {
        names.push("async_io");
    }
    if mask & CAP_GUI != 0 {
        names.push("gui");
    }
    if mask & CAP_NETWORK != 0 {
        names.push("network");
    }
    if mask & CAP_WEB_COMPAT != 0 {
        names.push("web_compat");
    }
    if names.is_empty() {
        format!("0x{mask:08x}")
    } else {
        names.join(", ")
    }
}

#[cfg(test)]
#[path = "../tests/support/expr_tests.rs"]
mod tests;
