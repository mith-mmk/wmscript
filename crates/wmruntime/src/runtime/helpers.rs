use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use wmresource::{ResourceState, ResourceType};
use wmui::UiColorRgba;
use wmvm::{HostError, Value};

use super::StateManager;

pub(super) fn read_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| HostError::Failed(format!("read {path}: {error}")))?;
    Ok(Value::String(contents))
}

pub(super) fn write_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    let contents = expect_string_arg(args, 1, "contents")?;
    std::fs::write(&path, contents.as_bytes())
        .map_err(|error| HostError::Failed(format!("write {path}: {error}")))?;
    Ok(Value::Bool(true))
}

pub(super) fn exists_text_file(args: &[Value]) -> Result<Value, HostError> {
    let path = expect_string_arg(args, 0, "path")?;
    Ok(Value::Bool(PathBuf::from(path).exists()))
}

pub(super) fn make_table(fields: &[(u16, Value)]) -> Value {
    let mut map = BTreeMap::new();
    for (key, value) in fields {
        map.insert(*key, value.clone());
    }
    Value::Table(Rc::new(map))
}

pub(super) fn resource_state_code(state: ResourceState) -> i64 {
    match state {
        ResourceState::Unloaded => 0,
        ResourceState::Loading => 1,
        ResourceState::Ready => 2,
        ResourceState::Failed => 3,
        ResourceState::Unloading => 4,
    }
}

pub(super) fn resource_type_value(resource_type: ResourceType) -> i64 {
    resource_type.as_u16() as i64
}

pub(super) fn resource_error_to_host_error(error: wmresource::ResourceError) -> HostError {
    HostError::Failed(error.to_string())
}

pub(super) fn expect_string_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<String, HostError> {
    match args.get(index) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be string, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

pub(super) fn expect_integer_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<i64, HostError> {
    match args.get(index) {
        Some(Value::Integer(value)) => Ok(*value),
        Some(Value::Bool(true)) => Ok(1),
        Some(Value::Bool(false)) => Ok(0),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be integer, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

pub(super) fn expect_number_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<f64, HostError> {
    match args.get(index) {
        Some(Value::Integer(value)) => Ok(*value as f64),
        Some(Value::Float(value)) => Ok(*value),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be numeric, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

pub(super) fn expect_bool_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<bool, HostError> {
    match args.get(index) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Integer(value)) => Ok(*value != 0),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be bool, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

pub(super) fn expect_state_id_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<String, HostError> {
    let value = expect_string_arg(args, index, name)?;
    if value.is_empty()
        || value.contains('|')
        || value.contains(char::is_whitespace)
        || value.contains("..")
    {
        return Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be a simple state id, found {value:?}"
        )));
    }
    Ok(value)
}

pub(super) fn automation_resource_key(name: &str) -> String {
    if name.starts_with("resource.") || name.starts_with("inventory.") {
        name.to_owned()
    } else {
        format!("resource.{name}")
    }
}

pub(super) fn state_integer(state: &StateManager, key: &str) -> i64 {
    match state.get(key) {
        Some(Value::Integer(value)) => value,
        Some(Value::Bool(true)) => 1,
        Some(Value::Bool(false)) | None => 0,
        Some(Value::Float(value)) => value as i64,
        _ => 0,
    }
}

pub(super) fn state_bool(state: &StateManager, key: &str) -> bool {
    match state.get(key) {
        Some(value) => value.truthy(),
        None => false,
    }
}

pub(super) fn state_list_value(state: &StateManager, key: &str) -> Vec<String> {
    match state.get(key) {
        Some(Value::String(value)) => value
            .split('|')
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn append_state_list_value(state: &mut StateManager, key: &str, value: &str) {
    let mut values = state_list_value(state, key);
    if values.iter().any(|existing| existing == value) {
        return;
    }
    values.push(value.to_owned());
    state.set(key.to_owned(), Value::String(values.join("|")));
}

pub(super) fn expect_color_component_arg(
    args: &[Value],
    index: usize,
    name: &str,
) -> Result<u8, HostError> {
    let value = match args.get(index) {
        Some(Value::Integer(value)) => *value as f64,
        Some(Value::Float(value)) => *value,
        Some(found) => {
            return Err(HostError::InvalidArguments(format!(
                "expected {name} argument {index} to be a number, found {found:?}"
            )));
        }
        None => {
            return Err(HostError::InvalidArguments(format!(
                "missing required argument {name} at index {index}"
            )));
        }
    };
    Ok(value.round().clamp(0.0, 255.0) as u8)
}

pub(super) fn expect_rgba_args(
    args: &[Value],
    start: usize,
    name: &str,
) -> Result<UiColorRgba, HostError> {
    Ok(UiColorRgba::new(
        expect_color_component_arg(args, start, &format!("{name}_r"))?,
        expect_color_component_arg(args, start + 1, &format!("{name}_g"))?,
        expect_color_component_arg(args, start + 2, &format!("{name}_b"))?,
        expect_color_component_arg(args, start + 3, &format!("{name}_a"))?,
    ))
}

pub(super) fn expect_handle_arg(
    args: &[Value],
    index: usize,
    name: &'static str,
) -> Result<u64, HostError> {
    match args.get(index) {
        Some(Value::Handle(value)) => Ok(*value),
        Some(Value::Integer(value)) if *value >= 0 => Ok(*value as u64),
        Some(found) => Err(HostError::InvalidArguments(format!(
            "expected {name} argument {index} to be a handle, found {found:?}"
        ))),
        None => Err(HostError::InvalidArguments(format!(
            "missing required argument {name} at index {index}"
        ))),
    }
}

pub(super) fn render_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_owned(),
        Value::Bool(v) => v.to_string(),
        Value::Integer(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(values) => format!("array(len={})", values.len()),
        Value::Table(values) => format!("table(len={})", values.len()),
        Value::Handle(v) => format!("handle({v})"),
    }
}
