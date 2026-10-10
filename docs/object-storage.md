# Object storage port

## Architecture

Hexagonal: services depend only on the `ObjectStorage` port
(`crates/app/src/port/object_storage.rs`); the provider lives in an
infrastructure adapter. The self-hosted backend stores objects on the **local
filesystem** (`FsObjectStorage`); the Cloudflare Worker maps the port onto its
native **R2 binding** (`R2BindingStorage`). The in-memory mock used in tests
can likewise be substituted without touching business logic.

```
pos-app  port/object_storage.rs   ObjectStorage trait + StoredObject,
                                  ObjectStoreError, validate_key()
backend  infra/storage_adapter/local.rs   FsObjectStorage (filesystem)
worker   src/infra/r2.rs                  R2BindingStorage (R2 binding)
tests    common::MemObjectStorage         in-memory mock
```

## Port

```rust
#[async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put_object(&self, key: &str, data: Bytes, content_type: &str)
        -> Result<(), ObjectStoreError>;
    async fn get_object(&self, key: &str)
        -> Result<Option<StoredObject>, ObjectStoreError>;  // None = missing
    async fn delete_object(&self, key: &str) -> Result<(), ObjectStoreError>;
    async fn exists(&self, key: &str) -> Result<bool, ObjectStoreError>;
}
```

Error mapping is the adapter's job; provider errors never leak upward.
`get_object` returns `Ok(None)` for missing objects; `delete_object` is
idempotent. Keys are validated centrally (`validate_key`): no `..` traversal,
no absolute paths, no empty segments, ASCII-safe charset, ≤ 512 bytes.

## Configuration

Backend (env):

| Variable | Meaning |
|---|---|
| `POS_STORAGE_DIR` | root directory for stored objects; unset = storage disabled |
| `POS_MAX_UPLOAD_BYTES` | adapter upload cap, default 10 MiB |

Objects are written as files under the root, with the content type kept in a
sibling `<key>.meta` file. Keys are centrally validated, so a key can never
escape the root.

Worker (Cloudflare):

| Binding / var | Meaning |
|---|---|
| `OBJECTS` (R2 binding) | target bucket, declared in `wrangler.toml` |
| `R2_MAX_UPLOAD_BYTES` (var) | adapter upload cap, default 10 MiB |

- See `.env.example` for the backend template. Worker config lives in
  `wrangler.toml [vars]`/bindings; secrets go via `wrangler secret put`.

## Dependency injection

`AppServices.objects: Option<Arc<dyn ObjectStorage>>` — set via
`.objects(...)` in the builder. Absent storage is a valid deployment state;
object endpoints should treat `svc.objects == None` as "feature disabled".

## Security notes

- An object key alone never proves ownership; authorization must be enforced
  in the service layer (derive the tenant prefix from the authenticated
  user/shop, not from a user-supplied key) before any port call.
- Uploads are capped at the adapter level; payloads are buffered `Bytes`
  (sized for the ≤10 MiB shop quota). Larger workloads should switch to
  streaming — the port signature stays the same.

## Testing

- Unit + round-trip tests: `crates/backend/tests/object_storage_test.rs`
  (upload/retrieve/delete/exists, missing object, invalid keys, filesystem
  adapter round-trip via temp dir, DI substitution with the mock).
