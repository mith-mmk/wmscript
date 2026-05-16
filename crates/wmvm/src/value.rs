use std::collections::BTreeMap;
use std::rc::Rc;

/// Runtime value stored on the VM stack.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Rc<Vec<Value>>),
    Table(Rc<BTreeMap<u16, Value>>),
    Handle(u64),
}

impl Value {
    pub const fn nil() -> Self {
        Self::Nil
    }

    pub fn truthy(&self) -> bool {
        match self {
            Self::Nil => false,
            Self::Bool(v) => *v,
            Self::Integer(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::String(v) => !v.is_empty(),
            Self::Array(v) => !v.is_empty(),
            Self::Table(v) => !v.is_empty(),
            Self::Handle(_) => true,
        }
    }

    pub(crate) fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(v) => Some(*v),
            Self::Bool(true) => Some(1),
            Self::Bool(false) => Some(0),
            _ => None,
        }
    }

    pub(crate) fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub(crate) fn as_field_id(&self) -> Option<u16> {
        self.as_integer()
            .and_then(|value| u16::try_from(value).ok())
    }
}
