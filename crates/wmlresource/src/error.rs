#![forbid(unsafe_code)]

use core::fmt;

use crate::{Handle, RequestId, ResourceId};
use wmlarchive::ArchiveError;

pub type Result<T> = core::result::Result<T, ResourceError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceError {
    UnknownResourceId(ResourceId),
    UnknownHandle(Handle),
    HandleExpired(Handle),
    RequestNotFound(RequestId),
    RequestNotReady(RequestId),
    InvalidResourceHeader(String),
    InvalidArchiveSection(String),
    DuplicateResourceId(ResourceId),
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownResourceId(id) => write!(f, "unknown resource id: {id}"),
            Self::UnknownHandle(handle) => write!(f, "unknown handle: {handle}"),
            Self::HandleExpired(handle) => write!(f, "expired handle: {handle}"),
            Self::RequestNotFound(id) => write!(f, "request not found: {id}"),
            Self::RequestNotReady(id) => write!(f, "request not ready: {id}"),
            Self::InvalidResourceHeader(message) => f.write_str(message),
            Self::InvalidArchiveSection(message) => f.write_str(message),
            Self::DuplicateResourceId(id) => write!(f, "duplicate resource id: {id}"),
        }
    }
}

impl std::error::Error for ResourceError {}

impl From<ArchiveError> for ResourceError {
    fn from(value: ArchiveError) -> Self {
        Self::InvalidArchiveSection(value.to_string())
    }
}
