use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use walkdir::WalkDir;
use zip::ZipArchive;

/// Environment variable pointing to a bckt data root directory — a directory
/// that contains `themes/` and `demo/` subdirectories. Uses the platform path
/// separator to allow multiple entries.
pub const SHARE_PATH_ENV: &str = "BCKT_SHARE_PATH";

/// Candidate bckt data roots, in priority order:
/// 1. entries from `BCKT_SHARE_PATH`;
/// 2. the directory containing the executable (tarball layout:
///    `bckt`, `themes/bckt3/`, `demo/microblog/`, …);
/// 3. `../share/bckt` relative to the resolved executable — the conventional
///    `<prefix>/bin` + `<prefix>/share/<pkg>` layout used by Homebrew and
///    other prefix installs.
fn share_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(value) = env::var_os(SHARE_PATH_ENV) {
        for part in env::split_paths(&value) {
            if !part.as_os_str().is_empty() {
                roots.push(part);
            }
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
        // Resolve symlinks (e.g. Homebrew's bin symlink into the Cellar) before
        // deriving the prefix's share/bckt directory.
        if let Ok(real) = exe.canonicalize()
            && let Some(prefix_root) = real.parent().and_then(|bin| bin.parent())
        {
            roots.push(prefix_root.join("share").join("bckt"));
        }
    }
    roots
}

/// Directories searched for bundled themes, derived from share roots by
/// appending `themes/`.
pub fn theme_search_paths() -> Vec<PathBuf> {
    share_roots()
        .into_iter()
        .map(|r| r.join("themes"))
        .collect()
}

/// Resolve a theme spec to a local source: either a `.zip` archive or a theme
/// directory. A spec that ends in `.zip` or contains a path separator is treated
/// as a direct filesystem path; a bare name is looked up across the theme search
/// paths, preferring `<name>.zip` and falling back to a `<name>/` directory.
pub fn resolve_theme(spec: &str) -> Result<PathBuf> {
    if spec.ends_with(".zip") || spec.contains('/') || spec.contains(std::path::MAIN_SEPARATOR) {
        let candidate = Path::new(spec);
        if candidate.is_dir() || candidate.is_file() {
            return Ok(candidate.to_path_buf());
        }
        bail!("theme '{}' not found", spec);
    }

    let file_name = format!("{spec}.zip");
    for dir in theme_search_paths() {
        let archive = dir.join(&file_name);
        if archive.is_file() {
            return Ok(archive);
        }
        let theme_dir = dir.join(spec);
        if theme_dir.is_dir() {
            return Ok(theme_dir);
        }
    }
    bail!(
        "theme '{spec}' not found in theme search path (set {SHARE_PATH_ENV}, or pass a path to a .zip archive or theme directory)"
    )
}

/// Directories searched for demo content, derived from share roots by
/// appending `demo/`.
fn demo_search_paths() -> Vec<PathBuf> {
    share_roots().into_iter().map(|r| r.join("demo")).collect()
}

/// Resolve a demo name to a local directory. A spec that contains a path
/// separator is treated as a direct filesystem path; a bare name is looked up
/// across the demo search paths.
pub fn resolve_demo(name: &str) -> Result<PathBuf> {
    if name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        let candidate = Path::new(name);
        if candidate.is_dir() {
            return Ok(candidate.to_path_buf());
        }
        bail!("demo '{}' not found", name);
    }

    for dir in demo_search_paths() {
        let demo_dir = dir.join(name);
        if demo_dir.is_dir() {
            return Ok(demo_dir);
        }
    }
    bail!("demo '{name}' not found (set {SHARE_PATH_ENV}, or pass a path to a demo directory)")
}

/// Install a theme into `destination`, replacing any existing contents. The
/// source may be a `.zip` archive (whose contents are extracted) or a theme
/// directory (whose contents are copied). Either way the theme directories
/// (`templates/`, `skel/`, `pages/`) are expected at the source root.
pub fn install_theme_source(source: &Path, destination: &Path) -> Result<()> {
    if source.is_dir() {
        return install_theme_dir(source, destination);
    }
    install_theme_archive(source, destination)
}

fn install_theme_dir(source: &Path, destination: &Path) -> Result<()> {
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve theme directory {}", source.display()))?;
    let target = canonical_target(destination)?;
    if target == source || target.starts_with(&source) || source.starts_with(&target) {
        bail!(
            "theme source and destination overlap: {} -> {}",
            source.display(),
            destination.display()
        );
    }

    prepare_destination(destination)?;

    let mut copied = false;
    for entry in WalkDir::new(&source) {
        let entry = entry
            .with_context(|| format!("failed to read theme directory {}", source.display()))?;
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(&source).with_context(|| {
            format!(
                "path {} is not under {}",
                entry.path().display(),
                source.display()
            )
        })?;
        let out_path = destination.join(relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        fs::copy(entry.path(), &out_path).with_context(|| {
            format!(
                "failed to copy {} to {}",
                entry.path().display(),
                out_path.display()
            )
        })?;
        copied = true;
    }

    if !copied {
        bail!("theme directory {} is empty", source.display());
    }
    Ok(())
}

fn install_theme_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    prepare_destination(destination)?;

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

/// Remove `destination` if it exists and recreate it as an empty directory.
fn prepare_destination(destination: &Path) -> Result<()> {
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
    Ok(())
}

/// Canonical path a destination *will* have, even when it (and some of its
/// parents) do not exist yet: walk up to the nearest existing ancestor,
/// canonicalise it, then re-append the trailing components. Used to detect
/// source/destination overlap before clobbering the destination.
fn canonical_target(destination: &Path) -> Result<PathBuf> {
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    let mut current = destination;
    loop {
        if let Ok(canon) = current.canonicalize() {
            let mut result = canon;
            for part in suffix.iter().rev() {
                result.push(part);
            }
            return Ok(result);
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                suffix.push(name.to_os_string());
                current = parent;
            }
            // No existing ancestor (e.g. a relative path); use it as-is.
            _ => return Ok(destination.to_path_buf()),
        }
    }
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
        install_theme_source(&archive, &destination).unwrap();

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
        install_theme_source(&archive, &destination).unwrap();

        assert!(!dir.path().join("escape.txt").exists());
        assert!(destination.join("templates/post.html").is_file());
    }

    #[test]
    fn installs_directory_source() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("src-theme");
        fs::create_dir_all(source.join("templates")).unwrap();
        fs::create_dir_all(source.join("skel/assets/js")).unwrap();
        fs::write(source.join("templates/post.html"), "<html></html>").unwrap();
        fs::write(source.join("skel/assets/js/search.js"), "// search").unwrap();

        let destination = dir.path().join("themes/theme");
        install_theme_source(&source, &destination).unwrap();

        assert!(destination.join("templates/post.html").is_file());
        assert!(destination.join("skel/assets/js/search.js").is_file());
    }

    #[test]
    fn rejects_overlapping_source_and_destination() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("themes/bckt3");
        fs::create_dir_all(source.join("templates")).unwrap();
        fs::write(source.join("templates/post.html"), "<html></html>").unwrap();

        // Installing a directory onto itself must not delete the source.
        let result = install_theme_source(&source, &source);
        assert!(result.is_err());
        assert!(source.join("templates/post.html").is_file());
    }

    #[test]
    fn resolve_direct_paths() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("theme.zip");
        write_archive(&archive, &[("templates/post.html", "<html></html>")]);
        assert_eq!(resolve_theme(archive.to_str().unwrap()).unwrap(), archive);

        let theme_dir = dir.path().join("a/themedir");
        fs::create_dir_all(&theme_dir).unwrap();
        assert_eq!(
            resolve_theme(theme_dir.to_str().unwrap()).unwrap(),
            theme_dir
        );

        let missing = dir.path().join("missing.zip");
        assert!(resolve_theme(missing.to_str().unwrap()).is_err());
    }

    #[test]
    fn resolve_named_theme_searches_path() {
        let dir = TempDir::new().unwrap();
        // BCKT_SHARE_PATH points at a share root; themes live under themes/.
        let themes_dir = dir.path().join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        // A .zip is preferred for one name...
        let archive = themes_dir.join("bckt3.zip");
        write_archive(&archive, &[("templates/post.html", "<html></html>")]);
        // ...and a directory is the fallback for another.
        let other_dir = themes_dir.join("plain");
        fs::create_dir_all(&other_dir).unwrap();

        // SAFETY: env-dependent assertions are consolidated into this single
        // test so the global var is not mutated by concurrent tests.
        unsafe { env::set_var(SHARE_PATH_ENV, dir.path()) };
        let zip = resolve_theme("bckt3");
        let dir_theme = resolve_theme("plain");
        let missing = resolve_theme("does-not-exist");
        unsafe { env::remove_var(SHARE_PATH_ENV) };

        assert_eq!(zip.unwrap(), archive);
        assert_eq!(dir_theme.unwrap(), other_dir);
        assert!(missing.is_err());
    }
}
