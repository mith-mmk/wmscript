#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use wmbytecode::{Op, encode_op};
use wmext::{ExtFunction, ExtValueType, ExtensionRegistry};
use wmhost::{CAP_ASYNC_IO, CAP_FILE_SYSTEM, CAP_GUI, CAP_NETWORK, CAP_WEB_COMPAT, CapabilityMask};
use wmplatform::PlatformCapabilities;
use wmvm::{Program as VmProgram, Value as VmValue};

use super::{CompileError, Result, parse_literal_value};

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

fn parse_statements_until(
    parser: &mut ExprParser<'_>,
    terminator: Option<u8>,
) -> Result<Vec<Stmt>> {
    let mut statements = Vec::new();
    loop {
        parser.skip_ws_and_comments();
        if parser.eof() {
            if terminator.is_some() {
                return Err(unsupported_expression("unexpected end of block"));
            }
            break;
        }
        if let Some(terminator) = terminator {
            if parser.peek_byte() == Some(terminator) {
                break;
            }
        }
        statements.push(parser.parse_statement()?);
    }
    Ok(statements)
}

fn fold_unary_neg(expr: &Expr) -> Result<Option<VmValue>> {
    match expr {
        Expr::Literal(VmValue::Integer(value)) => Ok(Some(VmValue::Integer(-value))),
        Expr::Literal(VmValue::Float(value)) => Ok(Some(VmValue::Float(-value))),
        Expr::Literal(other) => Err(unsupported_expression(format!(
            "unary negation is not supported for {other:?}"
        ))),
        _ => Ok(None),
    }
}

fn fold_unary_not(expr: &Expr) -> Result<Option<VmValue>> {
    match expr {
        Expr::Literal(value) => Ok(Some(VmValue::Bool(!value.truthy()))),
        _ => Ok(None),
    }
}

fn fold_binary(op: BinaryOp, left: &Expr, right: &Expr) -> Result<Option<VmValue>> {
    let Some(left) = literal_value(left) else {
        return Ok(None);
    };
    let Some(right) = literal_value(right) else {
        return Ok(None);
    };

    let value = match op {
        BinaryOp::Add => fold_add(left, right)?,
        BinaryOp::Sub => fold_sub(left, right)?,
        BinaryOp::Mul => fold_mul(left, right)?,
        BinaryOp::Div => fold_div(left, right)?,
        BinaryOp::Eq => VmValue::Bool(left == right),
        BinaryOp::Ne => VmValue::Bool(left != right),
        BinaryOp::Lt => VmValue::Bool(fold_ordering(&left, &right, |a, b| a < b)?),
        BinaryOp::Le => VmValue::Bool(fold_ordering(&left, &right, |a, b| a <= b)?),
        BinaryOp::Gt => VmValue::Bool(fold_ordering(&left, &right, |a, b| a > b)?),
        BinaryOp::Ge => VmValue::Bool(fold_ordering(&left, &right, |a, b| a >= b)?),
        BinaryOp::And => VmValue::Bool(left.truthy() && right.truthy()),
        BinaryOp::Or => VmValue::Bool(left.truthy() || right.truthy()),
    };
    Ok(Some(value))
}

fn fold_add(left: VmValue, right: VmValue) -> Result<VmValue> {
    match (left, right) {
        (VmValue::Integer(a), VmValue::Integer(b)) => Ok(VmValue::Integer(a + b)),
        (left, right) => Ok(VmValue::Float(as_number(&left)? + as_number(&right)?)),
    }
}

fn fold_sub(left: VmValue, right: VmValue) -> Result<VmValue> {
    match (left, right) {
        (VmValue::Integer(a), VmValue::Integer(b)) => Ok(VmValue::Integer(a - b)),
        (left, right) => Ok(VmValue::Float(as_number(&left)? - as_number(&right)?)),
    }
}

fn fold_mul(left: VmValue, right: VmValue) -> Result<VmValue> {
    match (left, right) {
        (VmValue::Integer(a), VmValue::Integer(b)) => Ok(VmValue::Integer(a * b)),
        (left, right) => Ok(VmValue::Float(as_number(&left)? * as_number(&right)?)),
    }
}

fn fold_div(left: VmValue, right: VmValue) -> Result<VmValue> {
    match (left, right) {
        (VmValue::Integer(_), VmValue::Integer(0)) => Err(unsupported_expression(
            "division by zero in constant expression",
        )),
        (VmValue::Integer(a), VmValue::Integer(b)) => Ok(VmValue::Integer(a / b)),
        (left, right) => {
            let right = as_number(&right)?;
            if right == 0.0 {
                return Err(unsupported_expression(
                    "division by zero in constant expression",
                ));
            }
            Ok(VmValue::Float(as_number(&left)? / right))
        }
    }
}

fn fold_ordering<F>(left: &VmValue, right: &VmValue, predicate: F) -> Result<bool>
where
    F: FnOnce(f64, f64) -> bool,
{
    let left = as_number(left)?;
    let right = as_number(right)?;
    Ok(predicate(left, right))
}

fn as_number(value: &VmValue) -> Result<f64> {
    match value {
        VmValue::Integer(value) => Ok(*value as f64),
        VmValue::Float(value) => Ok(*value),
        other => Err(unsupported_expression(format!(
            "expected numeric literal, found {other:?}"
        ))),
    }
}

fn literal_value(expr: &Expr) -> Option<VmValue> {
    match expr {
        Expr::Literal(value) => Some(value.clone()),
        _ => None,
    }
}

fn infer_type(
    expr: &Expr,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
) -> Result<TypeTag> {
    match expr {
        Expr::Literal(value) => Ok(type_of_value(value)),
        Expr::Variable(_) => Ok(TypeTag::Unknown),
        Expr::UnaryNeg(inner) => {
            match infer_type(inner, extension_registry, platform_capabilities)? {
                TypeTag::Integer | TypeTag::Float => Ok(infer_type(
                    inner,
                    extension_registry,
                    platform_capabilities,
                )?),
                other => Err(unsupported_expression(format!(
                    "unary negation requires a numeric type, found {other:?}"
                ))),
            }
        }
        Expr::UnaryNot(_) => Ok(TypeTag::Bool),
        Expr::Binary { op, left, right } => {
            let left = infer_type(left, extension_registry, platform_capabilities)?;
            let right = infer_type(right, extension_registry, platform_capabilities)?;
            infer_binary_type(*op, left, right)
        }
        Expr::Call { path, .. } => infer_call_type(path, extension_registry, platform_capabilities),
    }
}

fn infer_call_type(
    path: &[String],
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: PlatformCapabilities,
) -> Result<TypeTag> {
    let Some(extension_registry) = extension_registry else {
        return Ok(TypeTag::Unknown);
    };
    let full_name = path.join(".");
    let ext = match extension_registry.resolve(&full_name) {
        Ok(ext) => ext,
        Err(_) => return Ok(TypeTag::Unknown),
    };
    ensure_extension_capabilities(ext, platform_capabilities, &full_name)?;
    Ok(match ext.return_type {
        Some(return_type) => type_tag_from_ext_value_type(return_type),
        None => TypeTag::Unknown,
    })
}

fn type_tag_from_ext_value_type(value_type: ExtValueType) -> TypeTag {
    match value_type {
        ExtValueType::Unknown => TypeTag::Unknown,
        ExtValueType::Nil => TypeTag::Nil,
        ExtValueType::Bool => TypeTag::Bool,
        ExtValueType::Integer => TypeTag::Integer,
        ExtValueType::Float => TypeTag::Float,
        ExtValueType::String => TypeTag::String,
    }
}

fn infer_binary_type(op: BinaryOp, left: TypeTag, right: TypeTag) -> Result<TypeTag> {
    if matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::And
            | BinaryOp::Or
    ) {
        return Ok(TypeTag::Bool);
    }
    match (left, right) {
        (TypeTag::Unknown, _) | (_, TypeTag::Unknown) => Err(unsupported_expression(
            "binary operator requires statically known numeric operands",
        )),
        (TypeTag::Integer, TypeTag::Integer) if matches!(op, BinaryOp::Div) => Ok(TypeTag::Integer),
        (TypeTag::Integer, TypeTag::Integer) => Ok(TypeTag::Integer),
        (TypeTag::Integer, TypeTag::Float)
        | (TypeTag::Float, TypeTag::Integer)
        | (TypeTag::Float, TypeTag::Float) => Ok(TypeTag::Float),
        (left, right) => Err(unsupported_expression(format!(
            "binary operator requires numeric operands, found {left:?} and {right:?}"
        ))),
    }
}

fn type_of_value(value: &VmValue) -> TypeTag {
    match value {
        VmValue::Array(_) | VmValue::Table(_) | VmValue::Handle(_) => TypeTag::Unknown,
        VmValue::Nil => TypeTag::Nil,
        VmValue::Bool(_) => TypeTag::Bool,
        VmValue::Integer(_) => TypeTag::Integer,
        VmValue::Float(_) => TypeTag::Float,
        VmValue::String(_) => TypeTag::String,
    }
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

#[derive(Clone, Debug, Default)]
struct LocalScope {
    slots: BTreeMap<String, u8>,
    next_slot: u8,
    max_slot: u8,
}

impl LocalScope {
    fn new(initial_locals: &[String]) -> Result<Self> {
        let mut scope = Self::default();
        for name in initial_locals {
            scope.declare(name.clone())?;
        }
        Ok(scope)
    }

    fn lookup(&self, name: &str) -> Result<u8> {
        self.slots
            .get(name)
            .copied()
            .ok_or_else(|| unsupported_expression(format!("unknown local `{name}`")))
    }

    fn declare(&mut self, name: String) -> Result<u8> {
        if self.slots.contains_key(&name) {
            return Err(unsupported_expression(format!("duplicate local `{name}`")));
        }
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or_else(|| unsupported_expression("too many local variables"))?;
        self.max_slot = self.max_slot.max(self.next_slot);
        self.slots.insert(name, slot);
        Ok(slot)
    }

    fn local_count(&self) -> usize {
        self.max_slot as usize
    }

    fn merge_max(&mut self, other_local_count: usize) {
        self.max_slot = self
            .max_slot
            .max(other_local_count.min(u8::MAX as usize) as u8);
    }
}

struct ExprParser<'a> {
    source: &'a str,
    index: usize,
}

impl<'a> ExprParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, index: 0 }
    }

    fn parse_statements(&mut self, terminator: Option<u8>) -> Result<Vec<Stmt>> {
        parse_statements_until(self, terminator)
    }

    fn parse_statement(&mut self) -> Result<Stmt> {
        self.skip_ws_and_comments();
        if self.consume_keyword("return") {
            self.skip_ws_and_comments();
            if self.consume_byte(b';') {
                return Ok(Stmt::Return(None));
            }
            let expr = self.parse_expression()?;
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.consume_keyword("if") {
            return self.parse_if_statement();
        }
        if self.consume_keyword("loop") {
            let body = self.parse_block()?;
            return Ok(Stmt::Loop { body });
        }
        if self.consume_keyword("break") {
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Break);
        }
        if self.consume_keyword("continue") {
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Continue);
        }
        if self.consume_keyword("let") {
            self.skip_ws_and_comments();
            let name = self.read_identifier_source()?;
            self.skip_ws_and_comments();
            let value = if self.consume_byte(b'=') {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Let { name, value });
        }

        let expr = self.parse_expression()?;
        self.skip_ws_and_comments();
        self.expect_byte(b';')?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt> {
        let condition = self.parse_expression()?;
        self.skip_ws_and_comments();
        let then_branch = self.parse_block()?;
        self.skip_ws_and_comments();
        let else_branch = if self.consume_keyword("else") {
            self.skip_ws_and_comments();
            if self.consume_keyword("if") {
                vec![self.parse_if_statement()?]
            } else {
                self.parse_block()?
            }
        } else {
            Vec::new()
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>> {
        self.skip_ws_and_comments();
        self.expect_byte(b'{')?;
        let statements = self.parse_statements(Some(b'}'))?;
        self.skip_ws_and_comments();
        self.expect_byte(b'}')?;
        Ok(statements)
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut expr = self.parse_and()?;
        loop {
            self.skip_ws_and_comments();
            if !self.source[self.index..].starts_with("||") {
                break;
            }
            self.index += 2;
            let rhs = self.parse_and()?;
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut expr = self.parse_equality()?;
        loop {
            self.skip_ws_and_comments();
            if !self.source[self.index..].starts_with("&&") {
                break;
            }
            self.index += 2;
            let rhs = self.parse_equality()?;
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.source[self.index..].starts_with("==") {
                self.index += 2;
                Some(BinaryOp::Eq)
            } else if self.source[self.index..].starts_with("!=") {
                self.index += 2;
                Some(BinaryOp::Ne)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_comparison()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut expr = self.parse_additive()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.source[self.index..].starts_with("<=") {
                self.index += 2;
                Some(BinaryOp::Le)
            } else if self.source[self.index..].starts_with(">=") {
                self.index += 2;
                Some(BinaryOp::Ge)
            } else if self.consume_byte(b'<') {
                Some(BinaryOp::Lt)
            } else if self.consume_byte(b'>') {
                Some(BinaryOp::Gt)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_additive()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.consume_byte(b'+') {
                Some(BinaryOp::Add)
            } else if self.consume_byte(b'-') {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_multiplicative()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut expr = self.parse_unary()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.consume_byte(b'*') {
                Some(BinaryOp::Mul)
            } else if self.consume_byte(b'/') {
                Some(BinaryOp::Div)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        self.skip_ws_and_comments();
        if self.consume_byte(b'-') {
            return Ok(Expr::UnaryNeg(Box::new(self.parse_unary()?)));
        }
        if self.consume_byte(b'!') {
            return Ok(Expr::UnaryNot(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        self.skip_ws_and_comments();
        match self.peek_byte() {
            Some(b'(') => {
                self.index += 1;
                let expr = self.parse_expression()?;
                self.skip_ws_and_comments();
                self.expect_byte(b')')?;
                Ok(expr)
            }
            Some(b'"') => {
                let literal = self.read_string_literal_source()?;
                Ok(Expr::Literal(parse_literal_value(&literal)?))
            }
            Some(byte) if is_literal_start(byte) => {
                let ident = self.read_identifier_source()?;
                let mut path = vec![ident];
                while self.consume_byte(b'.') {
                    path.push(self.read_identifier_source()?);
                }
                self.skip_ws_and_comments();
                if self.consume_byte(b'(') {
                    let mut args = Vec::new();
                    self.skip_ws_and_comments();
                    if !self.consume_byte(b')') {
                        loop {
                            args.push(self.parse_expression()?);
                            self.skip_ws_and_comments();
                            if self.consume_byte(b')') {
                                break;
                            }
                            self.expect_byte(b',')?;
                        }
                    }
                    Ok(Expr::Call { path, args })
                } else if path.len() == 1 {
                    if let Ok(value) = parse_literal_value(&path[0]) {
                        Ok(Expr::Literal(value))
                    } else {
                        Ok(Expr::Variable(path[0].clone()))
                    }
                } else {
                    Err(unsupported_expression(format!(
                        "unexpected path expression `{}`",
                        path.join(".")
                    )))
                }
            }
            Some(_) => Err(unsupported_expression("unexpected token in expression")),
            None => Err(unsupported_expression("unexpected end of expression")),
        }
    }

    fn read_identifier_source(&mut self) -> Result<String> {
        let start = self.index;
        while let Some(byte) = self.peek_byte() {
            if byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'+' | b'-'
                        | b'*'
                        | b'/'
                        | b'('
                        | b')'
                        | b';'
                        | b','
                        | b'.'
                        | b'='
                        | b'<'
                        | b'>'
                        | b'!'
                        | b'&'
                        | b'|'
                )
            {
                break;
            }
            self.index += 1;
        }
        if start == self.index {
            return Err(unsupported_expression("expected literal"));
        }
        Ok(self.source[start..self.index].to_owned())
    }

    fn read_string_literal_source(&mut self) -> Result<String> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let quote = bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| unsupported_expression("unexpected end of string literal"))?;
        self.index += 1;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            self.index += 1;
            if escaped {
                escaped = false;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                continue;
            }
            if byte == quote {
                return Ok(self.source[start..self.index].to_owned());
            }
        }
        Err(unsupported_expression("unterminated string literal"))
    }

    fn skip_ws_and_comments(&mut self) {
        let bytes = self.source.as_bytes();
        while let Some(&byte) = bytes.get(self.index) {
            if byte.is_ascii_whitespace() {
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
            break;
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let end = self.index.saturating_add(keyword.len());
        if self.source[self.index..].starts_with(keyword)
            && self
                .source
                .as_bytes()
                .get(end)
                .map_or(true, |byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        {
            self.index = end;
            true
        } else {
            false
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.peek_byte() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(&mut self, byte: u8) -> Result<()> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            Err(unsupported_expression(format!(
                "expected `{}`",
                byte as char
            )))
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.index).copied()
    }

    fn eof(&self) -> bool {
        self.index >= self.source.len()
    }
}

fn is_literal_start(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
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
}
