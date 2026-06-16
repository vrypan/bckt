use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use blake3::Hasher;
use walkdir::WalkDir;

use super::utils::normalize_path;

pub(super) fn compute_static_digest(root: &Path) -> Result<String> {
    let skel_dir = root.join("skel");
    if !skel_dir.exists() {
        return Ok(Hasher::new().finalize().to_hex().to_string());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&skel_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();

    let mut hasher = Hasher::new();
    for path in files {
        let relative = path.strip_prefix(&skel_dir).with_context(|| {
            format!(
                "path {} is not under {}",
                path.display(),
                skel_dir.display()
            )
        })?;
        let normalized = normalize_path(relative);
        hasher.update(normalized.as_bytes());
        let data = fs::read(&path)
            .with_context(|| format!("failed to read static asset {}", path.display()))?;
        hasher.update(&data);
        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect static asset {}", path.display()))?;
        hasher.update(&metadata.len().to_le_bytes());
        let modified = metadata.modified().with_context(|| {
            format!(
                "failed to read modification time for static asset {}",
                path.display()
            )
        })?;
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0));
        hasher.update(&duration.as_secs().to_le_bytes());
        hasher.update(&duration.subsec_nanos().to_le_bytes());
    }

    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn copy_static_assets(root: &Path, html_root: &Path) -> Result<usize> {
    let skel_dir = root.join("skel");
    if !skel_dir.exists() {
        return Ok(0);
    }

    let mut copied = 0usize;
    for entry in WalkDir::new(&skel_dir) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(&skel_dir).with_context(|| {
            format!(
                "path {} is not under {}",
                entry.path().display(),
                skel_dir.display()
            )
        })?;
        let destination = html_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(entry.path(), &destination).with_context(|| {
            format!(
                "failed to copy static asset from {} to {}",
                entry.path().display(),
                destination.display()
            )
        })?;
        copied += 1;
    }

    Ok(copied)
}
