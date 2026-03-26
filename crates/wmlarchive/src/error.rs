#![forbid(unsafe_code)]

use core::fmt;

use crate::SectionId;

pub type Result<T> = core::result::Result<T, ArchiveError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveError {
    InvalidMagic,
    UnsupportedVersion(u16),
    UnexpectedEof,
    BrokenLayout,
    DuplicateSectionId(SectionId),
    MissingSignature,
    UnsupportedSignatureAlgorithm(u16),
    UnknownKeyId([u8; 16]),
    SignatureMismatch,
    SectionOutOfRange {
        section_id: SectionId,
        offset: u64,
        size: u64,
        data_len: usize,
    },
    InvalidManifest(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => f.write_str("invalid archive magic"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported archive version: {version}")
            }
            Self::UnexpectedEof => f.write_str("unexpected end of archive"),
            Self::BrokenLayout => f.write_str("broken archive layout"),
            Self::DuplicateSectionId(section_id) => write!(f, "duplicate section id: {section_id}"),
            Self::MissingSignature => f.write_str("missing archive signature"),
            Self::UnsupportedSignatureAlgorithm(alg) => {
                write!(f, "unsupported signature algorithm: {alg}")
            }
            Self::UnknownKeyId(key_id) => write!(f, "unknown key id: {key_id:?}"),
            Self::SignatureMismatch => f.write_str("signature mismatch"),
            Self::SectionOutOfRange {
                section_id,
                offset,
                size,
                data_len,
            } => {
                write!(
                    f,
                    "section {section_id} out of range: offset={offset} size={size} data_len={data_len}"
                )
            }
            Self::InvalidManifest(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ArchiveError {}
