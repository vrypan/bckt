use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use zip::ZipArchive;

/// Environment variable holding additional directories to search for bundled
/// theme archives (`<name>.zip`). Uses the platform path separator.
pub const THEME_PATH_ENV: &str = "BCKT_THEME_PATH";

/// Directories searched for bundled theme archives, in priority order: entries
/// from `BCKT_THEME_PATH` first, then the directory containing the executable
/// (so a distribution bundle can ship `bckt` and `bckt3.zip` side by side).
pub fn theme_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(value) = env::var_os(THEME_PATH_ENV) {
        for part in env::split_paths(&value) {
            if !part.as_os_str().is_empty() {
                paths.push(part);
            }
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.to_path_buf());
    }
    paths
}

/// Resolve a theme spec to a local `.zip` archive. A spec ending in `.zip` is
/// treated as a direct filesystem path; otherwise it is treated as a theme name
/// and looked up as `<name>.zip` across the theme search paths.
pub fn resolve_theme_archive(spec: &str) -> Result<PathBuf> {
    if spec.ends_with(".zip") {
        let candidate = Path::new(spec);
        if candidate.is_file() {
            return Ok(candidate.to_path_buf());
        }
        bail!("theme archive '{}' not found", spec);
    }

    let file_name = format!("{spec}.zip");
    let search_paths = theme_search_paths();
    for dir in &search_paths {
        let path = dir.join(&file_name);
        if path.is_file() {
            return Ok(path);
        }
    }
    bail!(
        "theme '{spec}' not found in theme search path (set {THEME_PATH_ENV}, or pass a path to a .zip archive)"
    )
}

/// Install a theme from a local `.zip` archive into `destination`, replacing any
/// existing contents. The archive is expected to contain the theme directories
/// (`templates/`, `skel/`, `pages/`) at its root.
pub fn install_theme_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination).with_context(|| {
            format!(
                "failed to remove existing directory {}",
                destination.display()
            )
        })?;
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create directory {}", destination.display()))?;

    let file = File::open(archive_path)
        .with_context(|| format!("failed to open theme archive {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read theme archive {}", archive_path.display()))?;

    extract_archive(&mut archive, destination)
}

fn extract_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, destination: &Path) -> Result<()> {
    let mut extracted_any = false;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("failed to read archive entry #{i}"))?;
        if entry.is_dir() {
            continue;
        }

        let Some(relative) = safe_relative_path(entry.name()) else {
            continue;
        };

        let out_path = destination.join(&relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        let mut outfile = File::create(&out_path)
            .with_context(|| format!("failed to create file {}", out_path.display()))?;
        io::copy(&mut entry, &mut outfile)
            .with_context(|| format!("failed to write {}", out_path.display()))?;

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out_path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("failed to set permissions on {}", out_path.display()))?;
        }

        extracted_any = true;
    }

    if !extracted_any {
        return Err(anyhow!("no files extracted from archive"));
    }

    Ok(())
}

/// Sanitise an archive entry name into a relative path, rejecting absolute paths
/// and any `..` components to guard against zip-slip.
fn safe_relative_path(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;

    fn write_archive(path: &Path, files: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, contents) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn installs_archive_contents_at_root() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("theme.zip");
        write_archive(
            &archive,
            &[
                ("templates/post.html", "<html></html>"),
                ("skel/assets/js/search.js", "// search"),
            ],
        );

        let destination = dir.path().join("themes/theme");
        install_theme_archive(&archive, &destination).unwrap();

        assert!(destination.join("templates/post.html").is_file());
        assert!(destination.join("skel/assets/js/search.js").is_file());
    }

    #[test]
    fn rejects_zip_slip_entries() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("evil.zip");
        write_archive(
            &archive,
            &[
                ("../escape.txt", "nope"),
                ("templates/post.html", "<html></html>"),
            ],
        );

        let destination = dir.path().join("themes/evil");
        install_theme_archive(&archive, &destination).unwrap();

        assert!(!dir.path().join("escape.txt").exists());
        assert!(destination.join("templates/post.html").is_file());
    }

    #[test]
    fn resolve_named_theme_uses_search_path() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("bckt3.zip");
        write_archive(&archive, &[("templates/post.html", "<html></html>")]);

        // SAFETY: tests in this module run single-threaded for this env var.
        unsafe { env::set_var(THEME_PATH_ENV, dir.path()) };
        let resolved = resolve_theme_archive("bckt3").unwrap();
        unsafe { env::remove_var(THEME_PATH_ENV) };

        assert_eq!(resolved, archive);
    }

    #[test]
    fn resolve_missing_theme_errors() {
        unsafe { env::set_var(THEME_PATH_ENV, "/nonexistent-theme-dir") };
        let result = resolve_theme_archive("does-not-exist");
        unsafe { env::remove_var(THEME_PATH_ENV) };
        assert!(result.is_err());
    }
}
