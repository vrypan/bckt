use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn open_cache_db(root: &Path) -> Result<sled::Db> {
    let cache_dir = root.join(super::CACHE_DIR);
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create cache directory {}", cache_dir.display()))?;
    sled::open(cache_dir.join("sled")).context("failed to open cache database")
}

pub(super) fn read_cached_string(db: &sled::Db, key: &str) -> Result<Option<String>> {
    let value = db
        .get(key.as_bytes())
        .with_context(|| format!("failed to read cache key {}", key))?;
    if let Some(bytes) = value {
        let string = String::from_utf8(bytes.to_vec())
            .with_context(|| format!("cache entry for {} is not valid utf-8", key))?;
        Ok(Some(string))
    } else {
        Ok(None)
    }
}

pub(super) fn store_cached_string(db: &sled::Db, key: &str, value: &str) -> Result<()> {
    db.insert(key.as_bytes(), value.as_bytes())
        .with_context(|| format!("failed to update cache key {}", key))?;
    Ok(())
}
