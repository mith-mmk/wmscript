use wmext::ExtensionRegistry;
use wmvm::{Program as VmProgram, Value as VmValue};

use crate::{CompileError, Result, SymbolKind, SymbolTable, expr};

pub(crate) fn lower_function_body(
    body: &str,
    program: &mut VmProgram,
    extension_registry: Option<&ExtensionRegistry>,
    platform_capabilities: wmplatform::PlatformCapabilities,
    initial_locals: &[String],
) -> Result<(Vec<u8>, usize)> {
    let (code, _type_tag, local_count) = expr::compile_function_body(
        body,
        program,
        extension_registry,
        platform_capabilities,
        initial_locals,
    )?;
    Ok((code, local_count))
}

pub(crate) fn ordered_local_names(locals: &SymbolTable) -> Vec<String> {
    let mut entries = locals
        .iter()
        .filter(|(_, entry)| matches!(entry.kind, SymbolKind::Parameter))
        .map(|(name, entry)| (entry.symbol_id, name.to_owned()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|(symbol_id, _)| *symbol_id);
    entries.into_iter().map(|(_, name)| name).collect()
}

pub(crate) fn parse_literal_value(source: &str) -> Result<VmValue> {
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
