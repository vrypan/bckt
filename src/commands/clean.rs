use std::env;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

fn ensure_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to recreate {}", path.display()))?;
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove file {}", path.display()))?;
    }

    Ok(true)
}

pub fn run_clean_command() -> Result<()> {
    let root = env::current_dir().context("failed to resolve current directory")?;
    let html = root.join("html");
    let cache = root.join(".bckt");

    let removed_html = remove_path(&html)?;
    ensure_directory(&html)?;

    let removed_cache = remove_path(&cache)?;

    match (removed_html, removed_cache) {
        (true, true) => println!("Removed html output and cache state."),
        (true, false) => println!("Removed html output and created a fresh html/ directory."),
        (false, true) => println!("No html/ directory found; cleared cached state."),
        (false, false) => println!("Created empty html/ directory (no cached state found)."),
    }

    Ok(())
}
