use crate::engine::{self, EngineError};
use crate::transport;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CryptoResult {
    pub kind: String,
    pub filename: String,
    pub content: Vec<u8>,
    pub mime_type: String,
    pub inline_text: Option<String>,
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone)]
pub struct EncryptRequest {
    pub input_type: String,
    pub armor: bool,
    pub compression_name: String,
    pub passphrase: String,
    pub text_value: String,
    pub file_name: Option<String>,
    pub file_bytes: Option<Vec<u8>>,
}

pub fn encrypt_content(request: EncryptRequest) -> Result<CryptoResult, CryptoError> {
    let (payload, source_name) = match request.input_type.as_str() {
        "text" => {
            if request.text_value.is_empty() {
                return Err(CryptoError::Message("请输入要处理的纯文本。".into()));
            }
            (request.text_value.into_bytes(), None)
        }
        "file" => {
            let file_name = request
                .file_name
                .clone()
                .ok_or_else(|| CryptoError::Message("文件模式下必须提供文件名和内容。".into()))?;
            let file_bytes = request
                .file_bytes
                .clone()
                .ok_or_else(|| CryptoError::Message("文件模式下必须提供文件名和内容。".into()))?;
            if file_bytes.is_empty() {
                return Err(CryptoError::Message("文件内容为空，无法继续。".into()));
            }
            (file_bytes, Some(safe_filename(&file_name, "payload.bin")))
        }
        _ => return Err(CryptoError::Message("不支持的输入类型。".into())),
    };

    if request.passphrase.is_empty() {
        return Err(CryptoError::Message("必须输入访问码。".into()));
    }

    let output_bytes = engine::encrypt_bytes(
        &payload,
        &request.passphrase,
        &request.compression_name,
        source_name.as_deref(),
    )
    .map_err(map_engine_error("加密失败"))?;

    if request.armor {
        let inline_text = transport::encode_text_output(&output_bytes);
        let filename = download_name(&request.input_type, source_name.as_deref(), true);
        return Ok(CryptoResult {
            kind: "text".into(),
            filename,
            content: inline_text.as_bytes().to_vec(),
            mime_type: "text/plain; charset=utf-8".into(),
            inline_text: Some(inline_text),
        });
    }

    Ok(CryptoResult {
        kind: "download".into(),
        filename: download_name(&request.input_type, source_name.as_deref(), false),
        content: output_bytes,
        mime_type: "application/octet-stream".into(),
        inline_text: None,
    })
}

pub fn decrypt_content(encrypted_blob: &[u8], passphrase: &str) -> Result<CryptoResult, CryptoError> {
    if encrypted_blob.is_empty() {
        return Err(CryptoError::Message("请提供要解密的内容。".into()));
    }
    if passphrase.is_empty() {
        return Err(CryptoError::Message("必须输入访问码。".into()));
    }

    let normalized_blob = transport::normalize_transport_blob(encrypted_blob);
    let decrypted = engine::decrypt_bytes(&normalized_blob, passphrase).map_err(map_engine_error("解密失败"))?;

    if let Some(filename) = decrypted.filename {
        let filename = safe_filename(&filename, "decrypted.bin");
        return Ok(CryptoResult {
            kind: "download".into(),
            filename,
            content: decrypted.content,
            mime_type: "application/octet-stream".into(),
            inline_text: None,
        });
    }

    match String::from_utf8(decrypted.content.clone()) {
        Ok(text_value) => Ok(CryptoResult {
            kind: "text".into(),
            filename: "decrypted.txt".into(),
            content: text_value.as_bytes().to_vec(),
            mime_type: "text/plain; charset=utf-8".into(),
            inline_text: Some(text_value),
        }),
        Err(_) => Ok(CryptoResult {
            kind: "download".into(),
            filename: "decrypted.bin".into(),
            content: decrypted.content,
            mime_type: "application/octet-stream".into(),
            inline_text: None,
        }),
    }
}

fn safe_filename(name: &str, fallback: &str) -> String {
    let file_name = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    let cleaned = file_name
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            ' ' => '_',
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('.')
        .trim_matches('_')
        .to_owned();

    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned
    }
}

fn download_name(input_type: &str, source_name: Option<&str>, text_mode: bool) -> String {
    let base_name = if input_type == "file" {
        source_name.unwrap_or("payload.bin")
    } else {
        "message"
    };
    let extension = if text_mode { "txt" } else { "bin" };
    format!("{base_name}.{extension}")
}

fn map_engine_error(prefix: &'static str) -> impl FnOnce(EngineError) -> CryptoError {
    move |error| match error {
        EngineError::BadCiphertext => CryptoError::Message("解密失败：访问码不正确或数据已损坏。".into()),
        EngineError::UnsupportedVersion(version) => {
            CryptoError::Message(format!("不支持的数据版本：{version}"))
        }
        EngineError::InvalidData => CryptoError::Message("数据格式无效或已损坏。".into()),
        EngineError::DecompressionFailed => CryptoError::Message("解压缩失败：数据可能已损坏。".into()),
        EngineError::Internal(inner) => CryptoError::Message(format!("{prefix}：{inner}")),
    }
}
