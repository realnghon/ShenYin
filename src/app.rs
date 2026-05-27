use crate::text;
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const OUTER_VERSION: u8 = 1;
const INNER_VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const ARGON_MEMORY_KIB: u32 = 65_536;
const ARGON_ITERATIONS: u32 = 3;
const ARGON_LANES: u32 = 1;

#[derive(Debug, Clone)]
pub struct PackRequest<'a> {
    pub input_name: &'a str,
    pub input_bytes: Vec<u8>,
    pub passphrase: &'a str,
    pub confirm_passphrase: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackResult {
    pub file_name: String,
    pub file_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCommand {
    pub action: Action,
    pub input_path: PathBuf,
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Pack,
    Unpack,
}

#[derive(Debug, Error)]
pub enum FoldBoxError {
    #[error("missing command")]
    MissingCommand,
    #[error("unsupported command")]
    UnknownCommand,
    #[error("missing input file path")]
    MissingInputPath,
    #[error("passphrase cannot be empty")]
    EmptyPassphrase,
    #[error("passphrases do not match")]
    MismatchedPassphrase,
    #[error("invalid input file name")]
    InvalidInputName,
    #[error("operation failed")]
    ProcessingFailed,
    #[error("failed to read file: {0}")]
    ReadFailed(String),
    #[error("failed to write file: {0}")]
    WriteFailed(String),
}

pub fn pack_bytes(request: PackRequest<'_>) -> Result<String, FoldBoxError> {
    if request.passphrase.is_empty() {
        return Err(FoldBoxError::EmptyPassphrase);
    }
    if let Some(confirm_passphrase) = request.confirm_passphrase {
        if request.passphrase != confirm_passphrase {
            return Err(FoldBoxError::MismatchedPassphrase);
        }
    }

    let file_name = safe_file_name(request.input_name)?;
    let plaintext = build_plaintext_blob(&file_name, &request.input_bytes)?;

    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    getrandom::fill(&mut salt).map_err(|_| FoldBoxError::ProcessingFailed)?;
    getrandom::fill(&mut nonce).map_err(|_| FoldBoxError::ProcessingFailed)?;

    let key = derive_key(request.passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext.as_slice())
        .map_err(|_| FoldBoxError::ProcessingFailed)?;

    let mut payload = Vec::with_capacity(1 + SALT_LEN + NONCE_LEN + ciphertext.len());
    payload.push(OUTER_VERSION);
    payload.extend_from_slice(&salt);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);

    text::encode_wrapped(&payload).map_err(|_| FoldBoxError::ProcessingFailed)
}

pub fn unpack_text(encoded_text: &str, passphrase: &str) -> Result<UnpackResult, FoldBoxError> {
    if passphrase.is_empty() {
        return Err(FoldBoxError::EmptyPassphrase);
    }

    let payload = text::decode_relaxed(encoded_text).map_err(|_| FoldBoxError::ProcessingFailed)?;
    if payload.len() < 1 + SALT_LEN + NONCE_LEN {
        return Err(FoldBoxError::ProcessingFailed);
    }
    if payload[0] != OUTER_VERSION {
        return Err(FoldBoxError::ProcessingFailed);
    }

    let salt = &payload[1..1 + SALT_LEN];
    let nonce_start = 1 + SALT_LEN;
    let nonce_end = nonce_start + NONCE_LEN;
    let nonce = &payload[nonce_start..nonce_end];
    let ciphertext = &payload[nonce_end..];

    let key = derive_key(passphrase, salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| FoldBoxError::ProcessingFailed)?;

    parse_plaintext_blob(&plaintext)
}

pub fn default_pack_output_path(input_path: &Path) -> PathBuf {
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = input_path
        .file_name()
        .unwrap_or_else(|| OsStr::new("output.bin"));
    parent.join(format!("{}.box.txt", file_name.to_string_lossy()))
}

pub fn default_unpack_output_path(input_path: &Path, original_file_name: &str) -> PathBuf {
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(sanitized_output_name(original_file_name))
}

pub fn parse_cli(args: impl IntoIterator<Item = String>) -> Result<CliCommand, FoldBoxError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Err(FoldBoxError::MissingCommand);
    };

    if matches!(command.as_str(), "--help" | "-h") {
        return Ok(CliCommand {
            action: Action::Pack,
            input_path: PathBuf::new(),
            output_path: None,
        });
    }

    let action = match command.as_str() {
        "pack" => Action::Pack,
        "unpack" => Action::Unpack,
        _ => return Err(FoldBoxError::UnknownCommand),
    };

    let input_path = args
        .next()
        .map(|value| cli_path_from_arg(&value))
        .ok_or(FoldBoxError::MissingInputPath)?;
    let output_path = args.next().map(|value| cli_path_from_arg(&value));
    if args.next().is_some() {
        return Err(FoldBoxError::UnknownCommand);
    }

    Ok(CliCommand {
        action,
        input_path,
        output_path,
    })
}

pub fn pack_file(
    input_path: &Path,
    output_path: Option<&Path>,
    passphrase: &str,
    confirm_passphrase: &str,
) -> Result<PathBuf, FoldBoxError> {
    let input_bytes =
        fs::read(input_path).map_err(|error| FoldBoxError::ReadFailed(error.to_string()))?;
    let input_name = input_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(FoldBoxError::InvalidInputName)?;
    let encoded = pack_bytes(PackRequest {
        input_name,
        input_bytes,
        passphrase,
        confirm_passphrase: Some(confirm_passphrase),
    })?;

    let output_path = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_pack_output_path(input_path));
    fs::write(&output_path, encoded)
        .map_err(|error| FoldBoxError::WriteFailed(error.to_string()))?;
    Ok(output_path)
}

pub fn unpack_file(
    input_path: &Path,
    output_path: Option<&Path>,
    passphrase: &str,
) -> Result<PathBuf, FoldBoxError> {
    let encoded = fs::read_to_string(input_path)
        .map_err(|error| FoldBoxError::ReadFailed(error.to_string()))?;
    let unpacked = unpack_text(&encoded, passphrase)?;
    let output_path = output_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_unpack_output_path(input_path, &unpacked.file_name));
    fs::write(&output_path, unpacked.file_bytes)
        .map_err(|error| FoldBoxError::WriteFailed(error.to_string()))?;
    Ok(output_path)
}

pub fn help_text(program_name: &str) -> String {
    format!(
        "Usage:\n  {program_name} pack <input-file> [output-file]\n  {program_name} unpack <input-file> [output-file]\n  {program_name} --help\n"
    )
}

fn build_plaintext_blob(file_name: &str, file_bytes: &[u8]) -> Result<Vec<u8>, FoldBoxError> {
    let name_bytes = file_name.as_bytes();
    let name_len = u16::try_from(name_bytes.len()).map_err(|_| FoldBoxError::InvalidInputName)?;
    let mut payload = Vec::with_capacity(1 + 2 + name_bytes.len() + file_bytes.len());
    payload.push(INNER_VERSION);
    payload.extend_from_slice(&name_len.to_be_bytes());
    payload.extend_from_slice(name_bytes);
    payload.extend_from_slice(file_bytes);
    Ok(payload)
}

fn parse_plaintext_blob(plaintext: &[u8]) -> Result<UnpackResult, FoldBoxError> {
    if plaintext.len() < 3 || plaintext[0] != INNER_VERSION {
        return Err(FoldBoxError::ProcessingFailed);
    }

    let name_len = u16::from_be_bytes([plaintext[1], plaintext[2]]) as usize;
    let name_start = 3;
    let name_end = name_start + name_len;
    if plaintext.len() < name_end {
        return Err(FoldBoxError::ProcessingFailed);
    }

    let file_name = std::str::from_utf8(&plaintext[name_start..name_end])
        .map_err(|_| FoldBoxError::ProcessingFailed)?;
    Ok(UnpackResult {
        file_name: sanitized_output_name(file_name),
        file_bytes: plaintext[name_end..].to_vec(),
    })
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], FoldBoxError> {
    let params = Params::new(
        ARGON_MEMORY_KIB,
        ARGON_ITERATIONS,
        ARGON_LANES,
        Some(KEY_LEN),
    )
    .map_err(|_| FoldBoxError::ProcessingFailed)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|_| FoldBoxError::ProcessingFailed)?;
    Ok(key)
}

fn safe_file_name(input_name: &str) -> Result<String, FoldBoxError> {
    let file_name = Path::new(input_name)
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(FoldBoxError::InvalidInputName)?;
    let cleaned = sanitized_output_name(file_name);
    if cleaned.is_empty() {
        return Err(FoldBoxError::InvalidInputName);
    }
    Ok(cleaned)
}

fn sanitized_output_name(input_name: &str) -> String {
    let file_name = Path::new(input_name)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("restored.bin");
    let cleaned = file_name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect::<String>()
        .trim_matches(['.', ' '])
        .to_owned();

    if cleaned.is_empty() {
        "restored.bin".to_owned()
    } else {
        cleaned
    }
}

fn cli_path_from_arg(raw: &str) -> PathBuf {
    let trimmed = trim_matching_quotes(raw.trim());
    let expanded = expand_home_prefix(trimmed);
    PathBuf::from(expanded)
}

fn trim_matching_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }

    value
}

fn expand_home_prefix(value: &str) -> String {
    if value == "~" {
        return home_dir_string().unwrap_or_else(|| value.to_owned());
    }

    if let Some(remainder) = value.strip_prefix("~/").or_else(|| value.strip_prefix("~\\")) {
        if let Some(home_dir) = home_dir_string() {
            return PathBuf::from(home_dir)
                .join(remainder)
                .to_string_lossy()
                .into_owned();
        }
    }

    value.to_owned()
}

fn home_dir_string() -> Option<String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|value| PathBuf::from(value).to_string_lossy().into_owned())
}
