use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use bzip2::{Compression as BzCompression, read::BzDecoder, write::BzEncoder};
use flate2::{Compression as FlateCompression, read::ZlibDecoder, write::ZlibEncoder};
use pbkdf2::pbkdf2_hmac_array;
use serde_json::Value;
use sha2::Sha256;
use std::io::{Read, Write};
use thiserror::Error;

const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const ITERATIONS: u32 = 600_000;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptResult {
    pub content: Vec<u8>,
    pub filename: Option<String>,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("数据格式无效或已损坏。")]
    InvalidData,
    #[error("不支持的数据版本：{0}")]
    UnsupportedVersion(u8),
    #[error("解密失败：访问码不正确或数据已损坏。")]
    BadCiphertext,
    #[error("解压缩失败：数据可能已损坏。")]
    DecompressionFailed,
    #[error("{0}")]
    Internal(String),
}

pub fn encrypt_bytes(
    payload: &[u8],
    passphrase: &str,
    compression_name: &str,
    source_filename: Option<&str>,
) -> Result<Vec<u8>, EngineError> {
    let mut meta = serde_json::Map::new();
    if let Some(source_filename) = source_filename {
        meta.insert("f".into(), Value::String(source_filename.to_owned()));
    }
    if !compression_name.is_empty() && compression_name != "none" {
        meta.insert("c".into(), Value::String(compression_name.to_owned()));
    }

    let meta_bytes = serde_json::to_vec(&Value::Object(meta))
        .map_err(|error| EngineError::Internal(error.to_string()))?;
    let compressed = compress(payload, compression_name)?;

    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    getrandom::fill(&mut salt).map_err(|error| EngineError::Internal(error.to_string()))?;
    getrandom::fill(&mut nonce).map_err(|error| EngineError::Internal(error.to_string()))?;

    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|error| EngineError::Internal(error.to_string()))?;
    let ciphertext = cipher
        .encrypt(
            nonce.as_slice().into(),
            Payload {
                msg: &compressed,
                aad: &meta_bytes,
            },
        )
        .map_err(|_| EngineError::BadCiphertext)?;

    let mut output =
        Vec::with_capacity(1 + SALT_LEN + NONCE_LEN + 4 + meta_bytes.len() + ciphertext.len());
    output.push(VERSION);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&(meta_bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(&meta_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_bytes(
    encrypted_blob: &[u8],
    passphrase: &str,
) -> Result<DecryptResult, EngineError> {
    let min_len = 1 + SALT_LEN + NONCE_LEN + 4;
    if encrypted_blob.len() < min_len {
        return Err(EngineError::InvalidData);
    }

    let version = encrypted_blob[0];
    if version != VERSION {
        return Err(EngineError::UnsupportedVersion(version));
    }

    let salt_start = 1;
    let nonce_start = salt_start + SALT_LEN;
    let meta_len_start = nonce_start + NONCE_LEN;
    let meta_start = meta_len_start + 4;

    let salt = &encrypted_blob[salt_start..nonce_start];
    let nonce = &encrypted_blob[nonce_start..meta_len_start];
    let meta_len = u32::from_be_bytes(
        encrypted_blob[meta_len_start..meta_start]
            .try_into()
            .map_err(|_| EngineError::InvalidData)?,
    ) as usize;

    if encrypted_blob.len() < meta_start + meta_len {
        return Err(EngineError::InvalidData);
    }

    let meta_bytes = &encrypted_blob[meta_start..meta_start + meta_len];
    let ciphertext = &encrypted_blob[meta_start + meta_len..];

    let key = derive_key(passphrase, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|error| EngineError::Internal(error.to_string()))?;
    let compressed = cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: ciphertext,
                aad: meta_bytes,
            },
        )
        .map_err(|_| EngineError::BadCiphertext)?;

    let meta: serde_json::Map<String, Value> =
        serde_json::from_slice(meta_bytes).unwrap_or_default();
    let compression = meta.get("c").and_then(Value::as_str).unwrap_or("none");
    let plaintext = decompress(&compressed, compression)?;
    let filename = meta.get("f").and_then(Value::as_str).map(str::to_owned);

    Ok(DecryptResult {
        content: plaintext,
        filename,
    })
}

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    pbkdf2_hmac_array::<Sha256, KEY_LEN>(passphrase.as_bytes(), salt, ITERATIONS)
}

fn compress(data: &[u8], algo: &str) -> Result<Vec<u8>, EngineError> {
    match algo {
        "zlib" | "zip" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), FlateCompression::new(6));
            encoder
                .write_all(data)
                .map_err(|error| EngineError::Internal(error.to_string()))?;
            encoder
                .finish()
                .map_err(|error| EngineError::Internal(error.to_string()))
        }
        "bz2" => {
            let mut encoder = BzEncoder::new(Vec::new(), BzCompression::new(6));
            encoder
                .write_all(data)
                .map_err(|error| EngineError::Internal(error.to_string()))?;
            encoder
                .finish()
                .map_err(|error| EngineError::Internal(error.to_string()))
        }
        _ => Ok(data.to_vec()),
    }
}

fn decompress(data: &[u8], algo: &str) -> Result<Vec<u8>, EngineError> {
    match algo {
        "zlib" | "zip" => {
            let mut decoder = ZlibDecoder::new(data);
            let mut output = Vec::new();
            decoder
                .read_to_end(&mut output)
                .map_err(|_| EngineError::DecompressionFailed)?;
            Ok(output)
        }
        "bz2" => {
            let mut decoder = BzDecoder::new(data);
            let mut output = Vec::new();
            decoder
                .read_to_end(&mut output)
                .map_err(|_| EngineError::DecompressionFailed)?;
            Ok(output)
        }
        _ => Ok(data.to_vec()),
    }
}
