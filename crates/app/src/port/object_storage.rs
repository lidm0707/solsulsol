//! Object storage port: S3-compatible blob storage behind a provider-agnostic
//! trait. Adapters must map provider errors into [`ObjectStoreError`].

use async_trait::async_trait;
use bytes::Bytes;

/// Hard cap on object key length (S3 allows 1024; we stay conservative).
pub const MAX_KEY_LEN: usize = 512;
/// Default adapter-level upload limit; overridable via `POS_MAX_UPLOAD_BYTES`.
pub const DEFAULT_MAX_UPLOAD_BYTES: i64 = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub data: Bytes,
    pub content_type: String,
}

#[derive(Debug)]
pub enum ObjectStoreError {
    NotFound,
    InvalidKey(String),
    TooLarge { limit: i64 },
    Config(String),
    Provider(String),
}

impl std::fmt::Display for ObjectStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "object not found"),
            Self::InvalidKey(k) => write!(f, "invalid object key: {k}"),
            Self::TooLarge { limit } => write!(f, "object exceeds size limit of {limit} bytes"),
            Self::Config(e) => write!(f, "storage configuration error: {e}"),
            Self::Provider(e) => write!(f, "storage provider error: {e}"),
        }
    }
}

impl std::error::Error for ObjectStoreError {}

/// Rejects keys that could escape the intended prefix/namespace: path
/// traversal (`..`), absolute paths, empty segments, control chars and
/// non-ASCII are all refused before any provider call.
pub fn validate_key(key: &str) -> Result<(), ObjectStoreError> {
    if key.is_empty() || key.len() > MAX_KEY_LEN {
        return Err(ObjectStoreError::InvalidKey(key.to_owned()));
    }
    let charset_ok = key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
    if !charset_ok {
        return Err(ObjectStoreError::InvalidKey(key.to_owned()));
    }
    let segments_ok = key
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != "..");
    if key.starts_with('/') || !segments_ok {
        return Err(ObjectStoreError::InvalidKey(key.to_owned()));
    }
    Ok(())
}

#[async_trait]
pub trait ObjectStorage: Send + Sync {
    /// Stores `data` under `key`. Fails with [`ObjectStoreError::TooLarge`]
    /// when the payload exceeds the adapter's configured upload limit.
    async fn put_object(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), ObjectStoreError>;

    /// Returns `Ok(None)` for missing objects instead of an error.
    async fn get_object(&self, key: &str) -> Result<Option<StoredObject>, ObjectStoreError>;

    /// Deleting a missing object is not an error (idempotent).
    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError>;

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError>;
}
