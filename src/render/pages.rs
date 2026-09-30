use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::Environment;
use walkdir::WalkDir;

use super::outputs::{Inventory, relative_output};
use super::templates::describe_template_error;
use super::utils::normalize_path;

pub(super) fn render_pages(
    root: &Path,
    html_root: &Path,
    env: &Environment<'static>,
    verbose: bool,
) -> Result<usize> {
    let pages_dir = root.join("pages");
    let files = collect_page_files(&pages_dir)?;

    let mut rendered_pages = 0usize;
    for path in files {
        let relative = path.strip_prefix(&pages_dir).with_context(|| {
            format!(
                "path {} is not under {}",
                path.display(),
                pages_dir.display()
            )
        })?;
        let output_path = html_root.join(relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        if !is_html_file(&path) {
            fs::copy(&path, &output_path).with_context(|| {
                format!(
                    "failed to copy page asset from {} to {}",
                    path.display(),
                    output_path.display()
                )
            })?;
            super::utils::log_status(
                verbose,
                "PAGE",
                format!("Copied {}", normalize_path(relative)),
            );
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read page template {}", path.display()))?;

        let scope = format!("rendering standalone page {}", normalize_path(relative));
        let template_name = normalize_path(relative);
        let rendered = env
            .render_str(&source, minijinja::context! {})
            .map_err(|err| describe_template_error(&scope, &template_name, err))?;

        fs::write(&output_path, rendered)
            .with_context(|| format!("failed to write page {}", output_path.display()))?;

        super::utils::log_status(
            verbose,
            "PAGE",
            format!("Rendered {}", normalize_path(relative)),
        );
        rendered_pages += 1;
    }

    Ok(rendered_pages)
}

fn collect_page_files(pages_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !pages_dir.exists() {
        return Ok(files);
    }
    for entry in WalkDir::new(pages_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

/// Every file `render_pages` generates, rendered or copied.
pub(super) fn page_outputs(root: &Path, html_root: &Path) -> Result<Inventory> {
    let pages_dir = root.join("pages");
    let files = collect_page_files(&pages_dir)?;
    Ok(files
        .iter()
        .filter_map(|path| path.strip_prefix(&pages_dir).ok())
        .filter_map(|relative| relative_output(html_root, &html_root.join(relative)))
        .collect())
}

fn is_html_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
}
