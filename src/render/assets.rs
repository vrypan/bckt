use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use blake3::Hasher;
use walkdir::WalkDir;

use super::outputs::{Inventory, relative_output};
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
        // Metadata-only digest (path + len + mtime), matching post-attachment
        // hashing in compute_post_digest. Avoids reading megabytes of skel/
        // bytes on every render; a same-length, mtime-preserved edit is the
        // accepted blind spot (use `render --force`).
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

/// Non-directory entries under `skel/`, as (source, path relative to skel/).
fn static_files(root: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let skel_dir = root.join("skel");
    let mut files = Vec::new();
    if !skel_dir.exists() {
        return Ok(files);
    }
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
        files.push((entry.path().to_path_buf(), relative.to_path_buf()));
    }
    Ok(files)
}

/// Every file `copy_static_assets` copies.
pub(super) fn static_outputs(root: &Path, html_root: &Path) -> Result<Inventory> {
    Ok(static_files(root)?
        .iter()
        .filter_map(|(_, relative)| relative_output(html_root, &html_root.join(relative)))
        .collect())
}

pub(super) fn copy_static_assets(root: &Path, html_root: &Path) -> Result<usize> {
    let files = static_files(root)?;
    for (source, relative) in &files {
        let destination = html_root.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(source, &destination).with_context(|| {
            format!(
                "failed to copy static asset from {} to {}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(files.len())
}
