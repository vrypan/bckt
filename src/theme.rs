use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use tempfile::NamedTempFile;
use ureq::Response;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub enum GithubReference {
    Tag(String),
    Branch(String),
}

#[derive(Debug, Clone)]
pub enum ThemeSource {
    Github {
        owner: String,
        repo: String,
        reference: GithubReference,
        subdir: Option<String>,
        strip_components: Option<usize>,
    },
    Url {
        url: String,
        subdir: Option<String>,
        strip_components: Option<usize>,
    },
}

pub fn download_theme(destination: &Path, source: ThemeSource) -> Result<()> {
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

    let mut temp =
        NamedTempFile::new().context("failed to create temporary file for theme download")?;

    let (archive_url, subdir, strip_components) = match &source {
        ThemeSource::Github {
            owner,
            repo,
            reference,
            subdir,
            strip_components,
        } => {
            let (url, default_strip) = match reference {
                GithubReference::Tag(tag) => (
                    format!(
                        "https://codeload.github.com/{owner}/{repo}/zip/refs/tags/{tag}",
                        owner = owner,
                        repo = repo,
                        tag = tag
                    ),
                    1usize,
                ),
                GithubReference::Branch(branch) => (
                    format!(
                        "https://codeload.github.com/{owner}/{repo}/zip/refs/heads/{branch}",
                        owner = owner,
                        repo = repo,
                        branch = branch
                    ),
                    1usize,
                ),
            };
            (
                url,
                subdir.clone(),
                strip_components.unwrap_or(default_strip),
            )
        }
        ThemeSource::Url {
            url,
            subdir,
            strip_components,
        } => (
            url.clone(),
            subdir.clone(),
            strip_components.unwrap_or(0usize),
        ),
    };

    download_to_file(&archive_url, temp.as_file_mut())?;

    let file = File::open(temp.path()).context("failed to reopen downloaded theme archive")?;
    let mut archive = ZipArchive::new(file).context("failed to read theme archive")?;

    extract_theme_archive(
        &mut archive,
        destination,
        strip_components,
        subdir.as_deref(),
    )
}

fn download_to_file(url: &str, mut target: &mut File) -> Result<()> {
    let response = ureq::get(url)
        .set(
            "User-Agent",
            concat!(
                "bckt/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/vrypan/bckt)"
            ),
        )
        .set("Accept", "application/octet-stream")
        .call();
    let response: Response = match response {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            let status_text = resp.status_text().to_string();
            return Err(anyhow!(
                "download request failed with status {code} ({status_text}) for {url}"
            ));
        }
        Err(err) => return Err(anyhow!("failed to download {url}: {err}")),
    };

    let mut reader = response.into_reader();
    io::copy(&mut reader, &mut target)
        .with_context(|| format!("failed to write downloaded archive from {url}"))?;
    target
        .flush()
        .context("failed to flush downloaded archive to temporary file")?;
    Ok(())
}

fn extract_theme_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    destination: &Path,
    strip_components: usize,
    subdir: Option<&str>,
) -> Result<()> {
    let mut extracted_any = false;
    let subdir_path = subdir.map(PathBuf::from);

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("failed to read archive entry #{i}"))?;
        let name = entry.name().to_string();
        if entry.is_dir() {
            continue;
        }

        let relative = match compute_relative_path(&name, strip_components, subdir_path.as_deref())
        {
            Some(rel) if !rel.as_os_str().is_empty() => rel,
            _ => continue,
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

fn compute_relative_path(
    entry_name: &str,
    strip_components: usize,
    subdir: Option<&Path>,
) -> Option<PathBuf> {
    let entry_path = Path::new(entry_name);
    let total_components = entry_path.iter().count();

    if total_components <= strip_components {
        return None;
    }

    let mut stripped = PathBuf::new();
    for component in entry_path.iter().skip(strip_components) {
        stripped.push(component);
    }

    if let Some(prefix) = subdir {
        if !stripped.starts_with(prefix) {
            return None;
        }
        return stripped.strip_prefix(prefix).ok().map(|p| p.to_path_buf());
    }

    Some(stripped)
}
