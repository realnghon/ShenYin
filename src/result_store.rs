use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredResult {
    pub token: String,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResultStore {
    base_dir: PathBuf,
    ttl_hours: u64,
}

#[derive(Debug, Error)]
pub enum ResultStoreError {
    #[error("下载内容已不存在。")]
    NotFound,
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredMeta {
    filename: String,
    mime_type: String,
    size: usize,
    created_at: u64,
}

impl ResultStore {
    pub fn new(base_dir: impl Into<PathBuf>, ttl_hours: u64) -> Result<Self, ResultStoreError> {
        let store = Self {
            base_dir: base_dir.into(),
            ttl_hours,
        };
        fs::create_dir_all(&store.base_dir).map_err(io_error)?;
        store.cleanup()?;
        Ok(store)
    }

    pub fn save(
        &self,
        content: &[u8],
        filename: &str,
        mime_type: &str,
    ) -> Result<StoredResult, ResultStoreError> {
        let token = Uuid::new_v4().simple().to_string();
        let token_dir = self.token_dir(&token);
        fs::create_dir_all(&token_dir).map_err(io_error)?;

        let payload_path = self.payload_path(&token);
        fs::write(&payload_path, content).map_err(io_error)?;

        let meta = StoredMeta {
            filename: filename.to_owned(),
            mime_type: mime_type.to_owned(),
            size: content.len(),
            created_at: now_unix_seconds(),
        };
        let meta_bytes =
            serde_json::to_vec(&meta).map_err(|error| ResultStoreError::Io(error.to_string()))?;
        fs::write(self.meta_path(&token), meta_bytes).map_err(io_error)?;

        Ok(StoredResult {
            token,
            filename: filename.to_owned(),
            mime_type: mime_type.to_owned(),
            size: content.len(),
            path: payload_path,
        })
    }

    pub fn get(&self, token: &str) -> Result<StoredResult, ResultStoreError> {
        let meta_path = self.meta_path(token);
        let payload_path = self.payload_path(token);
        if !meta_path.exists() || !payload_path.exists() {
            return Err(ResultStoreError::NotFound);
        }

        let meta_bytes = fs::read(meta_path).map_err(io_error)?;
        let meta: StoredMeta = serde_json::from_slice(&meta_bytes)
            .map_err(|error| ResultStoreError::Io(error.to_string()))?;

        Ok(StoredResult {
            token: token.to_owned(),
            filename: meta.filename,
            mime_type: meta.mime_type,
            size: meta.size,
            path: payload_path,
        })
    }

    pub fn cleanup(&self) -> Result<(), ResultStoreError> {
        let cutoff = now_unix_seconds().saturating_sub(self.ttl_hours.saturating_mul(3600));
        let entries = fs::read_dir(&self.base_dir).map_err(io_error)?;

        for entry in entries {
            let entry = entry.map_err(io_error)?;
            if !entry.path().is_dir() {
                continue;
            }

            let meta_path = entry.path().join("meta.json");
            if !meta_path.exists() {
                let _ = fs::remove_dir_all(entry.path());
                continue;
            }

            let created_at = fs::read(&meta_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<StoredMeta>(&bytes).ok())
                .map(|meta| meta.created_at);

            if created_at.is_none_or(|created_at| created_at < cutoff) {
                let _ = fs::remove_dir_all(entry.path());
            }
        }

        Ok(())
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    fn token_dir(&self, token: &str) -> PathBuf {
        self.base_dir.join(token)
    }

    fn meta_path(&self, token: &str) -> PathBuf {
        self.token_dir(token).join("meta.json")
    }

    fn payload_path(&self, token: &str) -> PathBuf {
        self.token_dir(token).join("payload.bin")
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn io_error(error: std::io::Error) -> ResultStoreError {
    ResultStoreError::Io(error.to_string())
}
