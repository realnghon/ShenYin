use thiserror::Error;

pub const INLINE_TEXT_THRESHOLD: usize = 100_000;
const LINE_WIDTH: usize = 76;
const ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("文本模式数据格式无效。")]
    InvalidEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPayload {
    pub text_available: bool,
    pub text_too_large: bool,
    pub text_length: usize,
    pub text: Option<String>,
}

pub fn encode_text_output(raw_bytes: &[u8]) -> String {
    let mut encoded = String::new();

    for chunk in raw_bytes.chunks(4) {
        let mut buffer = [0_u8; 4];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let mut value = u32::from_be_bytes(buffer);
        let mut digits = [0_u8; 5];

        for index in (0..5).rev() {
            digits[index] = ALPHABET[(value % 85) as usize];
            value /= 85;
        }

        let take = if chunk.len() < 4 { chunk.len() + 1 } else { 5 };
        encoded.push_str(std::str::from_utf8(&digits[..take]).expect("alphabet is ASCII"));
    }

    wrap_lines(&encoded)
}

pub fn decode_text_input(text: &str) -> Result<Vec<u8>, TransportError> {
    let stripped = text.replace('\n', "").replace('\r', "").trim().to_owned();
    if stripped.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::new();
    let bytes = stripped.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let chunk_len = (bytes.len() - index).min(5);
        let chunk = &bytes[index..index + chunk_len];
        index += chunk_len;

        if chunk_len == 1 {
            return Err(TransportError::InvalidEncoding);
        }

        let mut value = 0_u64;
        for byte in chunk {
            value = value
                .checked_mul(85)
                .ok_or(TransportError::InvalidEncoding)?
                + alphabet_index(*byte)? as u64;
        }

        if chunk_len < 5 {
            for _ in chunk_len..5 {
                value = value
                    .checked_mul(85)
                    .ok_or(TransportError::InvalidEncoding)?
                    + 84;
            }
        }

        let block = (value as u32).to_be_bytes();
        let take = if chunk_len < 5 { chunk_len - 1 } else { 4 };
        output.extend_from_slice(&block[..take]);
    }

    Ok(output)
}

pub fn extract_text_payload(text: &str) -> TextPayload {
    if text.len() > INLINE_TEXT_THRESHOLD {
        return TextPayload {
            text_available: false,
            text_too_large: true,
            text_length: text.len(),
            text: None,
        };
    }

    TextPayload {
        text_available: true,
        text_too_large: false,
        text_length: text.len(),
        text: Some(text.to_owned()),
    }
}

pub fn normalize_transport_blob(blob: &[u8]) -> Vec<u8> {
    if let Ok(text) = std::str::from_utf8(blob) {
        let trimmed = text.trim();
        if let Ok(decoded) = decode_text_input(trimmed) {
            return decoded;
        }
        return trimmed.as_bytes().to_vec();
    }

    blob.to_vec()
}

fn wrap_lines(encoded: &str) -> String {
    if encoded.len() <= LINE_WIDTH {
        return encoded.to_owned();
    }

    encoded
        .as_bytes()
        .chunks(LINE_WIDTH)
        .map(|chunk| std::str::from_utf8(chunk).expect("encoded Base85 is ASCII"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn alphabet_index(byte: u8) -> Result<u32, TransportError> {
    ALPHABET
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|index| index as u32)
        .ok_or(TransportError::InvalidEncoding)
}
