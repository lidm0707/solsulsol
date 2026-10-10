//! Worker-side R2 adapter: the bucket arrives as a native binding (`env.BUCKET`),
//! so this maps the `ObjectStorage` port onto the runtime API instead of S3.

use async_trait::async_trait;
use pos_app::port::object_storage::{
    validate_key, ObjectStorage, ObjectStoreError, StoredObject, DEFAULT_MAX_UPLOAD_BYTES,
};
use worker::HttpMetadata;

/// Name of the R2 binding declared in `wrangler.toml`.
pub const R2_BINDING: &str = "OBJECTS";
/// Adapter-level upload cap; `wrangler.toml` var `R2_MAX_UPLOAD_BYTES` overrides.
pub const MAX_UPLOAD_VAR: &str = "R2_MAX_UPLOAD_BYTES";

pub struct R2BindingStorage {
    bucket: worker::Bucket,
    max_upload_bytes: i64,
}

impl R2BindingStorage {
    pub fn from_env(env: &worker::Env) -> Result<Option<Self>, String> {
        let Ok(bucket) = env.bucket(R2_BINDING) else {
            return Ok(None);
        };
        let max_upload_bytes = match env.var(MAX_UPLOAD_VAR) {
            Ok(v) => v
                .to_string()
                .parse::<i64>()
                .map_err(|e| format!("{MAX_UPLOAD_VAR}: {e}"))?,
            Err(_) => DEFAULT_MAX_UPLOAD_BYTES,
        };
        Ok(Some(Self {
            bucket,
            max_upload_bytes,
        }))
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
}

fn err(e: impl std::fmt::Display) -> ObjectStoreError {
    ObjectStoreError::Provider(e.to_string())
}

#[async_trait]
impl ObjectStorage for R2BindingStorage {
    async fn put_object(
        &self,
        key: &str,
        data: bytes::Bytes,
        content_type: &str,
    ) -> Result<(), ObjectStoreError> {
        self.guard(key, &data)?;
        let metadata = HttpMetadata {
            content_type: Some(content_type.to_owned()),
            ..HttpMetadata::default()
        };
        let put = self.bucket.put(key, data.to_vec()).http_metadata(metadata);
        worker::send::SendFuture::new(put.execute())
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn get_object(&self, key: &str) -> Result<Option<StoredObject>, ObjectStoreError> {
        validate_key(key)?;
        let get = self.bucket.get(key).execute();
        let Some(obj) = worker::send::SendFuture::new(get).await.map_err(err)? else {
            return Ok(None);
        };
        let content_type = obj.http_metadata().content_type.unwrap_or_default();
        let body = obj.body().ok_or(ObjectStoreError::NotFound)?;
        let data = worker::send::SendFuture::new(body.bytes())
            .await
            .map_err(err)?;
        Ok(Some(StoredObject {
            data: data.into(),
            content_type,
        }))
    }

    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError> {
        validate_key(key)?;
        worker::send::SendFuture::new(self.bucket.delete(key))
            .await
            .map_err(err)
    }

    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError> {
        validate_key(key)?;
        // The binding API has no HEAD; a get is the cheap probe.
        let get = self.bucket.get(key).execute();
        Ok(worker::send::SendFuture::new(get)
            .await
            .map_err(err)?
            .is_some())
    }
}
