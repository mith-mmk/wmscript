#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crate::{Archive, ArchiveError, Result, SecurityHeader};

const SIGNATURE_ALG_KEYED_DIGEST_V1: u16 = 1;
const HASH_ALG_KEYED_DIGEST_V1: u16 = 1;

fn hash_bytes(parts: &[&[u8]]) -> [u8; 32] {
    let mut state = [
        0x243F_6A88_85A3_08D3_u64,
        0x1319_8A2E_0370_7344_u64,
        0xA409_3822_299F_31D0_u64,
        0x082E_FA98_EC4E_6C89_u64,
    ];
    for part in parts {
        for &byte in *part {
            for lane in 0..4 {
                state[lane] ^= byte as u64;
                state[lane] = state[lane].rotate_left(7 + lane as u32);
                state[lane] = state[lane].wrapping_mul(0x1000_0000_01B3);
                state[lane] ^= state[(lane + 1) % 4].rotate_right(11);
            }
        }
    }
    let mut digest = [0u8; 32];
    digest[0..8].copy_from_slice(&state[0].to_le_bytes());
    digest[8..16].copy_from_slice(&state[1].to_le_bytes());
    digest[16..24].copy_from_slice(&state[2].to_le_bytes());
    digest[24..32].copy_from_slice(&state[3].to_le_bytes());
    digest
}

/// Signing key metadata used by the archive signer and verifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SigningKey {
    key_id: [u8; 16],
    secret: Vec<u8>,
}

impl SigningKey {
    pub fn new(key_id: [u8; 16], secret: impl Into<Vec<u8>>) -> Self {
        Self {
            key_id,
            secret: secret.into(),
        }
    }

    pub const fn key_id(&self) -> [u8; 16] {
        self.key_id
    }

    pub fn security_header(&self) -> SecurityHeader {
        SecurityHeader {
            security_version: 1,
            sig_alg: SIGNATURE_ALG_KEYED_DIGEST_V1,
            hash_alg: HASH_ALG_KEYED_DIGEST_V1,
            enc_alg: 0,
            key_id: self.key_id,
            nonce_len: 0,
            reserved: 0,
            manifest_digest_offset: 0,
            manifest_digest_size: 0,
        }
    }

    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        hash_bytes(&[&self.key_id, &self.secret, data]).to_vec()
    }
}

/// Simple key registry used for verifying archive signatures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeyRing {
    keys: BTreeMap<[u8; 16], SigningKey>,
}

impl KeyRing {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, key: SigningKey) -> Option<SigningKey> {
        self.keys.insert(key.key_id(), key)
    }

    pub fn get(&self, key_id: &[u8; 16]) -> Option<&SigningKey> {
        self.keys.get(key_id)
    }

    pub fn contains(&self, key_id: &[u8; 16]) -> bool {
        self.keys.contains_key(key_id)
    }

    pub fn key_ids(&self) -> impl Iterator<Item = [u8; 16]> + '_ {
        self.keys.keys().copied()
    }
}

/// Archive signer that produces a fixed keyed digest signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveSigner {
    key: SigningKey,
}

impl ArchiveSigner {
    pub fn new(key_id: [u8; 16], secret: impl Into<Vec<u8>>) -> Self {
        Self {
            key: SigningKey::new(key_id, secret),
        }
    }

    pub const fn key_id(&self) -> [u8; 16] {
        self.key.key_id()
    }

    pub fn security_header(&self) -> SecurityHeader {
        self.key.security_header()
    }

    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.key.sign(data)
    }
}

/// Verifier for signed archives.
#[derive(Clone, Debug)]
pub struct ArchiveVerifier<'a> {
    keyring: &'a KeyRing,
}

impl<'a> ArchiveVerifier<'a> {
    pub fn new(keyring: &'a KeyRing) -> Self {
        Self { keyring }
    }

    pub fn verify(&self, archive: &Archive<'_>) -> Result<()> {
        let signature_size = archive.header().signature_size;
        if signature_size == 0 {
            return Ok(());
        }

        let signature = archive
            .signature_bytes()
            .ok_or(ArchiveError::MissingSignature)?;
        let security = archive.security();
        if security.sig_alg != SIGNATURE_ALG_KEYED_DIGEST_V1
            || security.hash_alg != HASH_ALG_KEYED_DIGEST_V1
        {
            return Err(ArchiveError::UnsupportedSignatureAlgorithm(
                security.sig_alg,
            ));
        }
        let key = self
            .keyring
            .get(&security.key_id)
            .ok_or(ArchiveError::UnknownKeyId(security.key_id))?;

        let signed_bytes = normalized_signed_bytes(archive);
        let expected = key.sign(&signed_bytes);
        if expected.as_slice() != signature {
            return Err(ArchiveError::SignatureMismatch);
        }

        Ok(())
    }
}

fn normalized_signed_bytes(archive: &Archive<'_>) -> Vec<u8> {
    let signature_offset = archive.header().signature_offset as usize;
    let mut bytes = archive.bytes()[..signature_offset].to_vec();
    if bytes.len() >= 64 {
        bytes[48..64].fill(0);
    }
    bytes
}
