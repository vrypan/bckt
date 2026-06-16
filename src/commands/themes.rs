use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::cli::{ThemeInstallArgs, ThemesArgs, ThemesSubcommand};
use crate::config::Config;
use crate::theme::install_theme_archive;
use crate::utils::resolve_root;

pub fn run_themes_command(args: ThemesArgs) -> Result<()> {
    let root = resolve_root(args.root.as_deref())?;

    match args.command {
        ThemesSubcommand::List => list_themes(&root),
        ThemesSubcommand::Use { name, force } => use_theme(&root, &name, force),
        ThemesSubcommand::Install(install_args) => install_theme(&root, install_args),
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

fn install_theme(root: &Path, args: ThemeInstallArgs) -> Result<()> {
    let archive_path = Path::new(&args.path);
    if !archive_path.is_file() {
        bail!("theme archive '{}' not found", args.path);
    }

    let name = match args.name {
        Some(name) => name,
        None => archive_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string())
            .with_context(|| format!("could not derive a theme name from '{}'", args.path))?,
    };

    let themes_dir = root.join("themes");
    fs::create_dir_all(&themes_dir).context("failed to create themes directory")?;

    let destination = themes_dir.join(&name);
    if destination.exists() && !args.force {
        bail!("theme '{name}' already exists. Use --force to overwrite");
    }

    install_theme_archive(archive_path, &destination)?;
    println!("Installed theme '{name}'");
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
    // Only templates/ and skel/ define the theme's look and are replaced.
    // pages/ holds user content (e.g. an About page) and is left untouched;
    // run `bckt init` once to seed a theme's starter pages.
    for name in ["templates", "skel"] {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;

    fn write_theme_archive(path: &Path) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("templates/post.html", options).unwrap();
        zip.write_all(b"<html></html>").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn install_theme_extracts_into_named_directory() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("bckt3.zip");
        write_theme_archive(&archive);

        install_theme(
            dir.path(),
            ThemeInstallArgs {
                path: archive.to_string_lossy().into_owned(),
                name: None,
                force: false,
            },
        )
        .unwrap();

        assert!(
            dir.path()
                .join("themes/bckt3/templates/post.html")
                .is_file()
        );
    }

    #[test]
    fn install_theme_rejects_existing_without_force() {
        let dir = TempDir::new().unwrap();
        let archive = dir.path().join("bckt3.zip");
        write_theme_archive(&archive);
        fs::create_dir_all(dir.path().join("themes/bckt3")).unwrap();

        let result = install_theme(
            dir.path(),
            ThemeInstallArgs {
                path: archive.to_string_lossy().into_owned(),
                name: None,
                force: false,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn apply_theme_leaves_pages_untouched() {
        let dir = TempDir::new().unwrap();
        let theme_root = dir.path().join("themes/bckt3");
        fs::create_dir_all(theme_root.join("templates")).unwrap();
        fs::create_dir_all(theme_root.join("pages/about")).unwrap();
        fs::write(theme_root.join("templates/post.html"), "theme").unwrap();
        fs::write(theme_root.join("pages/about/index.html"), "theme about").unwrap();

        let project_root = dir.path();
        fs::create_dir_all(project_root.join("pages/about")).unwrap();
        fs::write(project_root.join("pages/about/index.html"), "my about").unwrap();

        apply_theme(&theme_root, project_root).unwrap();

        assert_eq!(
            fs::read_to_string(project_root.join("templates/post.html")).unwrap(),
            "theme"
        );
        // The user's own page survives applying a theme.
        assert_eq!(
            fs::read_to_string(project_root.join("pages/about/index.html")).unwrap(),
            "my about"
        );
    }
}
