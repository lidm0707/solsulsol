//! Hugging Face model fetch: resolve → cache → local path.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::error::HfError;

const HF_RESOLVE_BASE: &str = "https://huggingface.co";
const HF_REVISION: &str = "main";
const CACHE_DIR_NAME: &str = "voice-over-demo";
const MIN_MODEL_BYTES: u64 = 1024;
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;

/// `owner/repo/path/to/file.onnx` inside the Hugging Face hub.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    pub owner: String,
    pub repo: String,
    pub path: String,
}

impl ModelSpec {
    pub fn parse(raw: &str) -> Result<Self, HfError> {
        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() < 3 || parts.iter().any(|p| p.is_empty()) {
            return Err(HfError::BadSpec(raw.to_owned()));
        }
        Ok(Self {
            owner: parts[0].to_owned(),
            repo: parts[1].to_owned(),
            path: parts[2..].join("/"),
        })
    }

    pub fn resolve_url(&self) -> String {
        format!(
            "{HF_RESOLVE_BASE}/{}/{}/resolve/{HF_REVISION}/{}",
            self.owner, self.repo, self.path
        )
    }

    pub fn cache_path(&self, cache_root: &std::path::Path) -> PathBuf {
        let file_name = self.path.rsplit('/').next().unwrap_or(self.path.as_str());
        cache_root
            .join(&self.owner)
            .join(&self.repo)
            .join(file_name)
    }
}

fn cache_root() -> Result<PathBuf, HfError> {
    let home = std::env::var_os("HOME").ok_or(HfError::NoHome)?;
    Ok(PathBuf::from(home).join(".cache").join(CACHE_DIR_NAME))
}

/// Returns the cached file path, downloading it when missing or too small.
pub fn fetch(spec: &ModelSpec) -> Result<PathBuf, HfError> {
    let root = cache_root()?;
    let target = spec.cache_path(&root);
    if is_cached(&target) {
        return Ok(target);
    }
    download(spec, &target)?;
    Ok(target)
}

fn is_cached(target: &std::path::Path) -> bool {
    match fs::metadata(target) {
        Ok(meta) => meta.len() >= MIN_MODEL_BYTES,
        Err(_) => false,
    }
}

fn download(spec: &ModelSpec, target: &std::path::Path) -> Result<(), HfError> {
    let response = ureq::get(spec.resolve_url().as_str())
        .call()
        .map_err(|e| HfError::Request(e.to_string()))?;
    if response.status() != 200 {
        return Err(HfError::HttpStatus(spec.resolve_url(), response.status()));
    }
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|e| HfError::CacheCreate(dir.to_path_buf(), e))?;
    }
    let partial = partial_path(target);
    let mut file =
        fs::File::create(&partial).map_err(|e| HfError::CacheCreate(partial.clone(), e))?;
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; DOWNLOAD_CHUNK_BYTES];
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|e| HfError::Download(spec.to_string(), e))?;
        if read == 0 {
            break;
        }
        file.write_all(&buf[..read])
            .map_err(|e| HfError::Download(spec.to_string(), e))?;
    }
    file.flush()
        .map_err(|e| HfError::Download(spec.to_string(), e))?;
    if file
        .metadata()
        .map_err(|e| HfError::Download(spec.to_string(), e))?
        .len()
        < MIN_MODEL_BYTES
    {
        return Err(HfError::TooSmall(partial));
    }
    fs::rename(&partial, target).map_err(|e| HfError::CacheCreate(target.to_path_buf(), e))?;
    Ok(())
}

fn partial_path(target: &std::path::Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    target.with_file_name(name)
}

impl std::fmt::Display for ModelSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.owner, self.repo, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_RAW: &str = "rhasspy/piper-voices/en/en_US/lessac/medium/en_US-lessac-medium.onnx";

    #[test]
    fn parses_multi_segment_path() {
        let spec = ModelSpec::parse(SPEC_RAW).unwrap();
        assert_eq!(spec.owner, "rhasspy");
        assert_eq!(spec.repo, "piper-voices");
        assert_eq!(spec.path, "en/en_US/lessac/medium/en_US-lessac-medium.onnx");
    }

    #[test]
    fn rejects_short_specs() {
        assert!(ModelSpec::parse("owner/repo").is_err());
        assert!(ModelSpec::parse("owner//file").is_err());
        assert!(ModelSpec::parse("").is_err());
    }

    #[test]
    fn builds_resolve_url() {
        let spec = ModelSpec::parse(SPEC_RAW).unwrap();
        assert_eq!(
            spec.resolve_url(),
            "https://huggingface.co/rhasspy/piper-voices/resolve/main/\
             en/en_US/lessac/medium/en_US-lessac-medium.onnx"
        );
    }

    #[test]
    fn cache_path_uses_basename() {
        let spec = ModelSpec::parse(SPEC_RAW).unwrap();
        let path = spec.cache_path(std::path::Path::new("/tmp/cache"));
        assert_eq!(
            path,
            PathBuf::from("/tmp/cache/rhasspy/piper-voices/en_US-lessac-medium.onnx")
        );
    }
}
