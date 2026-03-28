#![forbid(unsafe_code)]

pub type ResourceId = u32;
pub type RequestId = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Handle(u64);

impl Handle {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | index as u64)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn index(self) -> u32 {
        self.0 as u32
    }

    pub const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

impl core::fmt::Display for Handle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for Handle {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Handle> for u64 {
    fn from(value: Handle) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceType {
    Image,
    Audio,
    Binary,
    Font,
    Video,
    ScriptData,
    Unknown(u16),
}

impl ResourceType {
    pub const fn from_u16(value: u16) -> Self {
        match value {
            1 => Self::Image,
            2 => Self::Audio,
            3 => Self::Binary,
            4 => Self::Font,
            5 => Self::Video,
            6 => Self::ScriptData,
            other => Self::Unknown(other),
        }
    }

    pub const fn as_u16(self) -> u16 {
        match self {
            Self::Image => 1,
            Self::Audio => 2,
            Self::Binary => 3,
            Self::Font => 4,
            Self::Video => 5,
            Self::ScriptData => 6,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Unloaded,
    Loading,
    Ready,
    Failed,
    Unloading,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceHeader {
    pub resource_id: ResourceId,
    pub resource_type: ResourceType,
    pub flags: u16,
    pub compression: u16,
    pub encoding: u16,
    pub unpacked_size: u32,
    pub packed_size: u32,
    pub data_offset: u32,
}

impl ResourceHeader {
    pub const BYTE_SIZE: usize = 24;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceData {
    Image(Vec<u8>),
    Audio(Vec<u8>),
    Binary(Vec<u8>),
    Font(Vec<u8>),
    Video(Vec<u8>),
    ScriptData(Vec<u8>),
}

impl ResourceData {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Image(bytes)
            | Self::Audio(bytes)
            | Self::Binary(bytes)
            | Self::Font(bytes)
            | Self::Video(bytes)
            | Self::ScriptData(bytes) => bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEntry {
    pub id: ResourceId,
    pub state: ResourceState,
    pub ref_count: u32,
    pub data: Option<ResourceData>,
    pub last_access_frame: u64,
    pub flags: u32,
}

impl ResourceEntry {
    pub fn new(id: ResourceId) -> Self {
        Self {
            id,
            state: ResourceState::Unloaded,
            ref_count: 0,
            data: None,
            last_access_frame: 0,
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleEntry {
    pub resource_id: ResourceId,
    pub generation: u32,
    pub ref_count: u32,
    pub alive: bool,
}

impl HandleEntry {
    pub fn new(resource_id: ResourceId) -> Self {
        Self {
            resource_id,
            generation: 1,
            ref_count: 1,
            alive: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestState {
    Pending,
    InFlight,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRequest {
    pub request_id: RequestId,
    pub resource_id: ResourceId,
    pub worker_id: u32,
    pub state: RequestState,
    pub result_handle: Option<Handle>,
    pub error: Option<String>,
    pub flags: u32,
}

impl ResourceRequest {
    pub fn new(request_id: RequestId, resource_id: ResourceId, worker_id: u32, flags: u32) -> Self {
        Self {
            request_id,
            resource_id,
            worker_id,
            state: RequestState::Pending,
            result_handle: None,
            error: None,
            flags,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadResult {
    Ready(Handle),
    Pending(RequestId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestStatus {
    pub done: bool,
    pub ok: bool,
    pub handle: Option<Handle>,
    pub error: Option<String>,
}
