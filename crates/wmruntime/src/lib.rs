#![forbid(unsafe_code)]

//! Headless runtime wrapper for WML programs and archives.

mod audio_backend;
pub mod game;
mod runtime;

pub use audio_backend::{
    AudioBackend, SharedAudioBackend, create_default_audio_backend, create_disabled_audio_backend,
};
pub use runtime::*;
