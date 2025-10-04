use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::cli::{ThemesArgs, ThemesSubcommand};
use crate::config::Config;

pub fn run_themes_command(args: ThemesArgs) -> Result<()> {
    let root = env::current_dir().context("failed to resolve current directory")?;

    match args.command {
        ThemesSubcommand::List => list_themes(&root),
        ThemesSubcommand::Use { name, force } => use_theme(&root, &name, force),
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
