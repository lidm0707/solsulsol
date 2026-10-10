//! Local filesystem object storage adapter. Objects live as files under a
//! fixed root directory; the content type is kept in a sibling `.meta` file.
//! All provider specifics stay behind [`ObjectStorage`].

use async_trait::async_trait;
use bytes::Bytes;
use pos_app::port::object_storage::{
    DEFAULT_MAX_UPLOAD_BYTES, ObjectStorage, ObjectStoreError, StoredObject, validate_key,
};

pub const META_EXT: &str = ".meta";

pub struct FsObjectStorage {
    root: std::path::PathBuf,
    max_upload_bytes: i64,
}

impl FsObjectStorage {
    pub fn new(root: &str, max_upload_bytes: Option<i64>) -> Self {
        Self {
            root: std::path::PathBuf::from(root),
            max_upload_bytes: max_upload_bytes.unwrap_or(DEFAULT_MAX_UPLOAD_BYTES),
        }
    }

    fn guard(&self, key: &str, data: &[u8]) -> Result<(), ObjectStoreError> {
        validate_key(key)?;
        if data.len() as i64 > self.max_upload_bytes {
            return Err(ObjectStoreError::TooLarge {
                limit: self.max_upload_bytes,
            });
        }
        Ok(())
    }

    fn path(&self, key: &str) -> std::path::PathBuf {
        self.root.join(key)
    }

    fn meta_path(&self, key: &str) -> std::path::PathBuf {
        self.root.join(format!("{key}{META_EXT}"))
    }

    fn err(e: std::io::Error) -> ObjectStoreError {
        ObjectStoreError::Provider(e.to_string())
    }
}

#[async_trait]
impl ObjectStorage for FsObjectStorage {
    async fn put_object(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), ObjectStoreError> {
        self.guard(key, &data)?;
        let path = self.path(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(Self::err)?;
        }
        tokio::fs::write(&path, &data).await.map_err(Self::err)?;
        tokio::fs::write(self.meta_path(key), content_type)
            .await
            .map_err(Self::err)?;
        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<Option<StoredObject>, ObjectStoreError> {
        validate_key(key)?;
        let path = self.path(key);
        if !tokio::fs::try_exists(&path).await.map_err(Self::err)? {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await.map_err(Self::err)?;
        let content_type = tokio::fs::read_to_string(self.meta_path(key))
            .await
            .unwrap_or_default();
        Ok(Some(StoredObject {
            data: data.into(),
            content_type,
        }))
    }

    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError> {
        validate_key(key)?;
        tokio::fs::remove_file(self.path(key)).await.ok();
        tokio::fs::remove_file(self.meta_path(key)).await.ok();
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError> {
        validate_key(key)?;
        tokio::fs::try_exists(self.path(key))
            .await
            .map_err(Self::err)
    }
}
