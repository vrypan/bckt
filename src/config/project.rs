use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

pub fn find_project_root(start: impl AsRef<Path>) -> Result<PathBuf> {
    let mut current = start.as_ref().to_path_buf();

    loop {
        let candidate = current.join("bckt.yaml");
        if candidate.exists() {
            return Ok(current);
        }

        if !current.pop() {
            bail!(
                "could not locate bckt.yaml starting from {}",
                start.as_ref().display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_project_root_walks_upwards() {
        let dir = TempDir::new().unwrap();
        let project = dir.path();
        let nested = project.join("posts/example");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join("bckt.yaml"), "title: test\n").unwrap();

        let discovered = find_project_root(&nested).unwrap();
        assert_eq!(discovered, project);
    }

    #[test]
    fn find_project_root_errors_when_missing() {
        let dir = TempDir::new().unwrap();
        let error = find_project_root(dir.path()).unwrap_err();
        assert!(error.to_string().contains("could not locate bckt.yaml"));
    }
}
