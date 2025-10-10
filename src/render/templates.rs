use std::error::Error as StdError;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use minijinja::value::Value as TemplateValue;
use minijinja::{Environment, Error as TemplateError};
use walkdir::WalkDir;

use super::utils::normalize_path;

pub(super) fn render_template_with_scope(
    template: &minijinja::Template<'_, '_>,
    context: TemplateValue,
    scope: &str,
) -> Result<String> {
    let template_name = template.name().to_string();
    template
        .render(context)
        .map_err(|err| describe_template_error(scope, &template_name, err))
}

pub(super) fn describe_template_error(
    scope: &str,
    template_name: &str,
    err: TemplateError,
) -> anyhow::Error {
    let actual_template = err.name().unwrap_or(template_name).to_string();
    let line = err.line();
    let kind = err.kind();
    let detail = err.detail().map(str::to_string);
    let summary = err.to_string();
    let nested = StdError::source(&err).map(|source| source.to_string());

    let mut message = String::new();
    let _ = write!(&mut message, "{}: template '{}'", scope, actual_template);

    if actual_template != template_name {
        let _ = write!(&mut message, " (inherited from '{}')", template_name);
    }

    if let Some(line_no) = line {
        let _ = write!(&mut message, " at line {}", line_no);
    }

    let _ = write!(&mut message, "\nkind: {:?}", kind);

    let payload = detail.unwrap_or(summary);
    let _ = write!(&mut message, "\nmessage: {}", payload);

    if let Some(source) = nested {
        let _ = write!(&mut message, "\ncaused by: {}", source);
    }

    anyhow!(message)
}

pub(super) fn load_templates(root: &Path, env: &mut Environment<'static>) -> Result<String> {
    let templates_dir = root.join("templates");
    if !templates_dir.exists() {
        bail!("templates directory {} not found", templates_dir.display());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&templates_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
    }
    files.sort();

    let mut hasher = blake3::Hasher::new();

    for path in files {
        let template_body = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template {}", path.display()))?;
        let relative_path = path.strip_prefix(&templates_dir).unwrap();
        let relative_name = normalize_path(relative_path);
        hasher.update(relative_name.as_bytes());
        hasher.update(template_body.as_bytes());
        let name_static = Box::leak(relative_name.clone().into_boxed_str());
        let template_static = Box::leak(template_body.into_boxed_str());
        env.add_template(name_static, template_static)
            .with_context(|| format!("failed to register template {}", relative_name))?;
    }

    Ok(hasher.finalize().to_hex().to_string())
}
