use wmext::{ExtValueType, ExtensionRegistry};
use wmplatform::PlatformCapabilities;
use wmvm::Value as VmValue;

use super::{
    BinaryOp, Expr, Result, TypeTag, ensure_extension_capabilities, unsupported_expression,
};

pub(super) fn infer_type(
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
