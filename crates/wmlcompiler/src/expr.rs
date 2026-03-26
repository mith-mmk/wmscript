#![forbid(unsafe_code)]

use wmlbytecode::{Op, Opcode, encode_op};
use wmlext::ExtensionRegistry;
use wmlvm::{Program as VmProgram, Value as VmValue};

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
}

#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Literal(VmValue),
    UnaryNeg(Box<Expr>),
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

/// Compiles a `return` statement body into bytecode and a type tag.
pub(crate) fn compile_return_body(
    body: &str,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
) -> Result<(Vec<u8>, TypeTag)> {
    let mut parser = ExprParser::new(body);
    let expr = match parser.parse_return_statement()? {
        Some(expr) => expr,
        None => {
            parser.finish()?;
            return Ok((vec![Opcode::Return as u8], TypeTag::Nil));
        }
    };
    parser.finish()?;

    let optimized = optimize_expr(expr)?;
    let type_tag = infer_type(&optimized)?;
    let code = emit_expr(&optimized, program, extension_registry)?;
    Ok((code, type_tag))
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

fn infer_type(expr: &Expr) -> Result<TypeTag> {
    match expr {
        Expr::Literal(value) => Ok(type_of_value(value)),
        Expr::UnaryNeg(inner) => match infer_type(inner)? {
            TypeTag::Integer | TypeTag::Float => Ok(infer_type(inner)?),
            other => Err(unsupported_expression(format!(
                "unary negation requires a numeric type, found {other:?}"
            ))),
        },
        Expr::Binary { op, left, right } => {
            let left = infer_type(left)?;
            let right = infer_type(right)?;
            infer_binary_type(*op, left, right)
        }
        Expr::Call { .. } => Ok(TypeTag::Unknown),
    }
}

fn infer_binary_type(op: BinaryOp, left: TypeTag, right: TypeTag) -> Result<TypeTag> {
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

fn emit_expr(
    expr: &Expr,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
) -> Result<Vec<u8>> {
    let mut code = Vec::new();
    emit_expr_into(expr, program, extension_registry, &mut code)?;
    encode_op(&Op::Return, &mut code);
    Ok(code)
}

fn emit_expr_into(
    expr: &Expr,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    out: &mut Vec<u8>,
) -> Result<()> {
    match expr {
        Expr::Literal(value) => {
            let const_id = program.push_constant(value.clone());
            encode_op(&Op::PushConst(const_id), out);
        }
        Expr::UnaryNeg(inner) => {
            emit_expr_into(inner, program, extension_registry, out)?;
            encode_op(&Op::Neg, out);
        }
        Expr::Binary { op, left, right } => {
            emit_expr_into(left, program, extension_registry, out)?;
            emit_expr_into(right, program, extension_registry, out)?;
            match op {
                BinaryOp::Add => encode_op(&Op::Add, out),
                BinaryOp::Sub => encode_op(&Op::Sub, out),
                BinaryOp::Mul => encode_op(&Op::Mul, out),
                BinaryOp::Div => encode_op(&Op::Div, out),
            }
        }
        Expr::Call { path, args } => {
            let Some(extension_registry) = extension_registry else {
                return Err(unsupported_expression(
                    "extension calls require an extension registry",
                ));
            };
            let full_name = path.join(".");
            let ext = extension_registry.resolve(&full_name).map_err(|error| {
                unsupported_expression(format!("unknown extension call `{full_name}`: {error}"))
            })?;
            if args.len() < ext.min_args as usize || args.len() > ext.max_args as usize {
                return Err(unsupported_expression(format!(
                    "extension call `{full_name}` expected {}..={} args, got {}",
                    ext.min_args,
                    ext.max_args,
                    args.len()
                )));
            }
            for arg in args {
                emit_expr_into(arg, program, Some(extension_registry), out)?;
            }
            encode_op(&Op::CallHost(ext.host_id, args.len() as u8), out);
        }
    }
    Ok(())
}

fn unsupported_expression(message: impl Into<String>) -> CompileError {
    CompileError::UnsupportedExpression {
        source: message.into(),
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

    fn parse_return_statement(&mut self) -> Result<Option<Expr>> {
        self.skip_ws_and_comments();
        if self.eof() {
            return Ok(None);
        }
        self.expect_keyword("return")?;
        self.skip_ws_and_comments();
        if self.consume_byte(b';') {
            return Ok(None);
        }
        let expr = self.parse_expression()?;
        self.skip_ws_and_comments();
        self.expect_byte(b';')?;
        Ok(Some(expr))
    }

    fn finish(&mut self) -> Result<()> {
        self.skip_ws_and_comments();
        if self.eof() {
            Ok(())
        } else {
            Err(unsupported_expression("unexpected trailing tokens"))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_additive()
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
                    Ok(Expr::Literal(parse_literal_value(&path[0])?))
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
                || matches!(byte, b'+' | b'-' | b'*' | b'/' | b'(' | b')' | b';')
                || byte == b','
                || byte == b'.'
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

    fn expect_keyword(&mut self, keyword: &str) -> Result<()> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(unsupported_expression(format!(
                "expected keyword `{keyword}`"
            )))
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

    #[test]
    fn optimizer_folds_constant_return_expression() {
        let mut program = VmProgram::new();
        let (code, type_tag) =
            compile_return_body("return 1 + 2 * 3;", &mut program, None).expect("compile body");
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
        let (code, type_tag) =
            compile_return_body(r#"return "hello";"#, &mut program, None).expect("compile body");
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
        let (code, type_tag) =
            compile_return_body("return;", &mut program, None).expect("compile body");
        assert_eq!(type_tag, TypeTag::Nil);
        assert_eq!(program.constant_count(), 0);
        assert_eq!(code, vec![Opcode::Return as u8]);
    }
}
