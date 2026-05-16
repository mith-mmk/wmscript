use wmvm::Value as VmValue;

use super::{BinaryOp, Expr, Result, unsupported_expression};

pub(super) fn fold_unary_neg(expr: &Expr) -> Result<Option<VmValue>> {
    match expr {
        Expr::Literal(VmValue::Integer(value)) => Ok(Some(VmValue::Integer(-value))),
        Expr::Literal(VmValue::Float(value)) => Ok(Some(VmValue::Float(-value))),
        Expr::Literal(other) => Err(unsupported_expression(format!(
            "unary negation is not supported for {other:?}"
        ))),
        _ => Ok(None),
    }
}

pub(super) fn fold_unary_not(expr: &Expr) -> Result<Option<VmValue>> {
    match expr {
        Expr::Literal(value) => Ok(Some(VmValue::Bool(!value.truthy()))),
        _ => Ok(None),
    }
}

pub(super) fn fold_binary(op: BinaryOp, left: &Expr, right: &Expr) -> Result<Option<VmValue>> {
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
