use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;
use walkdir::WalkDir;

use super::templates::describe_template_error;
use super::utils::normalize_path;

pub(super) fn render_pages(
    root: &Path,
    html_root: &Path,
    env: &Environment<'static>,
    verbose: bool,
) -> Result<usize> {
    let pages_dir = root.join("pages");
    if !pages_dir.exists() {
        return Ok(0);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&pages_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.into_path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
            {
                files.push(path);
            }
        }
    }

    files.sort();

    let mut rendered_pages = 0usize;
    for path in files {
        let relative = path.strip_prefix(&pages_dir).unwrap();
        let output_path = html_root.join(relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
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
