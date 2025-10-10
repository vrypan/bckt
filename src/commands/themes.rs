use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::cli::{ThemeDownloadArgs, ThemesArgs, ThemesSubcommand};
use crate::config::Config;
use crate::theme::{GithubReference, ThemeSource, download_theme};
use crate::utils::resolve_root;

pub fn run_themes_command(args: ThemesArgs) -> Result<()> {
    let root = resolve_root(args.root.as_deref())?;

    match args.command {
        ThemesSubcommand::List => list_themes(&root),
        ThemesSubcommand::Use { name, force } => use_theme(&root, &name, force),
        ThemesSubcommand::Download(download_args) => download_theme_into(&root, download_args),
    }
}

fn list_themes(root: &Path) -> Result<()> {
    let themes_dir = root.join("themes");
    if !themes_dir.exists() {
        println!("No themes installed.");
        return Ok(());
    }

    let entries = fs::read_dir(&themes_dir)
        .with_context(|| format!("failed to read themes directory {}", themes_dir.display()))?;

    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    if names.is_empty() {
        println!("No themes installed.");
        return Ok(());
    }

    names.sort_unstable();

    let config_path = root.join("bckt.yaml");
    let active = Config::load(&config_path)
        .ok()
        .and_then(|config| config.theme);

    for name in names {
        if Some(&name) == active.as_ref() {
            println!("* {}", name);
        } else {
            println!("  {}", name);
        }
    }

    Ok(())
}

fn use_theme(root: &Path, name: &str, force: bool) -> Result<()> {
    let theme_root = root.join("themes").join(name);
    if !theme_root.exists() {
        bail!("theme '{}' is not installed", name);
    }

    confirm_overwrite(root, force)?;
    apply_theme(&theme_root, root)?;

    let config_path = root.join("bckt.yaml");
    let mut config = Config::load(&config_path)?;
    config.theme = Some(name.to_string());
    config.save(&config_path)?;

    println!("Applied theme '{}'.", name);
    Ok(())
}

fn download_theme_into(root: &Path, args: ThemeDownloadArgs) -> Result<()> {
    let themes_dir = root.join("themes");
    fs::create_dir_all(&themes_dir).context("failed to create themes directory")?;

    let destination = themes_dir.join(&args.name);
    if destination.exists() {
        if args.force {
            fs::remove_dir_all(&destination).with_context(|| {
                format!("failed to remove existing theme {}", destination.display())
            })?;
        } else {
            bail!(
                "theme '{}' already exists. Use --force to overwrite",
                args.name
            );
        }
    }

    let repo_spec_info = args
        .github
        .as_ref()
        .map(|spec| parse_github_spec(spec))
        .transpose()?;

    let source = if let Some(url) = &args.url {
        ThemeSource::Url {
            url: url.clone(),
            subdir: args.subdir.clone(),
            strip_components: args.strip_components,
        }
    } else if let Some((owner, repo, repo_path)) = repo_spec_info {
        let reference = select_github_reference(args.tag.clone(), args.branch.clone());
        ThemeSource::Github {
            owner,
            repo,
            reference,
            subdir: derive_subdir(args.subdir.clone(), repo_path, &args.name),
            strip_components: Some(args.strip_components.unwrap_or(1)),
        }
    } else {
        bail!("either --url or --github must be provided");
    };

    download_theme(&destination, source)?;
    println!("Downloaded theme '{}'", args.name);
    Ok(())
}

fn confirm_overwrite(project_root: &Path, force: bool) -> Result<()> {
    if force {
        return Ok(());
    }

    let conflicts: Vec<&str> = ["templates", "skel"]
        .into_iter()
        .filter(|&name| {
            let path = project_root.join(name);
            directory_has_contents(&path).unwrap_or(false)
        })
        .collect();

    if conflicts.is_empty() {
        return Ok(());
    }

    println!(
        "The following directories will be overwritten: {}",
        conflicts.join(", ")
    );
    print!("Proceed? [y/N]: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation input")?;

    let answer = input.trim().to_lowercase();
    if matches!(answer.as_str(), "y" | "yes") {
        Ok(())
    } else {
        bail!("theme installation aborted by user");
    }
}

fn directory_has_contents(path: &Path) -> Result<bool> {
    if !path.exists() || !path.is_dir() {
        return Ok(false);
    }
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to read directory {}", path.display()))?;
    Ok(entries.next().is_some())
}

fn parse_github_spec(spec: &str) -> Result<(String, String, Option<String>)> {
    let mut segments = spec.split('/').collect::<Vec<_>>();
    if segments.len() < 2 {
        bail!("invalid GitHub specification '{spec}' (expected owner/repo[/path])");
    }

    let owner = segments.remove(0);
    let repo = segments.remove(0);

    if owner.is_empty() || repo.is_empty() {
        bail!("invalid GitHub specification '{spec}'");
    }

    let subdir = if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    };

    Ok((owner.to_string(), repo.to_string(), subdir))
}

fn select_github_reference(tag: Option<String>, branch: Option<String>) -> GithubReference {
    match (tag, branch) {
        (Some(tag), _) => GithubReference::Tag(tag),
        (None, Some(branch)) => GithubReference::Branch(branch),
        (None, None) => GithubReference::Branch("main".to_string()),
    }
}

fn apply_theme(theme_root: &Path, project_root: &Path) -> Result<()> {
    for name in ["templates", "skel", "pages"] {
        let source_path = theme_root.join(name);
        if source_path.exists() {
            let destination_path = project_root.join(name);
            copy_dir(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest).with_context(|| format!("failed to remove {}", dest.display()))?;
    }
    fs::create_dir_all(dest).with_context(|| format!("failed to create {}", dest.display()))?;

    for entry in WalkDir::new(src) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(src)
            .with_context(|| format!("failed to strip prefix for {}", entry.path().display()))?;
        let target = dest.join(relative);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        fs::copy(entry.path(), &target).with_context(|| {
            format!(
                "failed to copy {} to {}",
                entry.path().display(),
                target.display()
            )
        })?;
    }

    Ok(())
}

fn derive_subdir(
    explicit: Option<String>,
    repo_path: Option<String>,
    theme_name: &str,
) -> Option<String> {
    if let Some(explicit) = explicit {
        return Some(explicit);
    }

    let repo_path = repo_path?;
    let mut components: Vec<String> = repo_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect();

    match components.last() {
        Some(last) if last == theme_name => {}
        _ => components.push(theme_name.to_string()),
    }

    Some(components.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_spec_handles_owner_repo() {
        let (owner, repo, path) = parse_github_spec("vrypan/bckt").unwrap();
        assert_eq!(owner, "vrypan");
        assert_eq!(repo, "bckt");
        assert!(path.is_none());
    }

    #[test]
    fn parse_github_spec_handles_nested_path() {
        let (owner, repo, path) = parse_github_spec("vrypan/bckt/themes").unwrap();
        assert_eq!(owner, "vrypan");
        assert_eq!(repo, "bckt");
        assert_eq!(path.as_deref(), Some("themes"));
    }

    #[test]
    fn parse_github_spec_requires_owner_repo() {
        assert!(parse_github_spec("vrypan").is_err());
    }

    #[test]
    fn derive_subdir_prefers_explicit_value() {
        let result = derive_subdir(
            Some("custom/path".to_string()),
            Some("themes".to_string()),
            "bckt3",
        );
        assert_eq!(result.as_deref(), Some("custom/path"));
    }

    #[test]
    fn derive_subdir_appends_theme_name_when_missing() {
        let result = derive_subdir(None, Some("themes".to_string()), "bckt3");
        assert_eq!(result.as_deref(), Some("themes/bckt3"));
    }

    #[test]
    fn derive_subdir_respects_existing_theme_name() {
        let result = derive_subdir(None, Some("themes/bckt3".to_string()), "bckt3");
        assert_eq!(result.as_deref(), Some("themes/bckt3"));
    }

    #[test]
    fn derive_subdir_returns_none_without_repo_path() {
        assert!(derive_subdir(None, None, "bckt3").is_none());
    }
}
