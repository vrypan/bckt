#![cfg(feature = "render")]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn build_theme(root: &Path) -> std::path::PathBuf {
    let theme = root.join("sources").join("fixture-theme");
    write(&theme.join("templates/base.html"), b"theme base");
    write(&theme.join("skel/style.css"), b"theme css");
    write(&theme.join("pages/about/index.html"), b"theme about");
    write(&theme.join("pages/contact/index.html"), b"theme contact");
    theme
}

fn build_demo(root: &Path) -> std::path::PathBuf {
    let demo = root.join("sources").join("fixture-demo");
    write(&demo.join("bckt.yaml"), b"title: demo config\n");
    write(&demo.join("posts/first/post.md"), b"demo first post");
    write(&demo.join("pages/about/index.html"), b"demo about");
    demo
}

fn run_init(workspace: &Path, site: &Path, theme: &Path, demo: &Path) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_bckt"))
        .current_dir(workspace)
        .env("BCKT_SHARE_PATH", workspace.join("empty-share"))
        .arg("init")
        .arg("--root")
        .arg(site)
        .arg("--theme")
        .arg(theme)
        .arg("--demo")
        .arg(demo)
        .output()
        .expect("failed to run bckt init");
    assert!(
        output.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

#[test]
fn init_demo_fresh_project_uses_demo_defaults() {
    let workspace = TempDir::new().unwrap();
    let theme = build_theme(workspace.path());
    let demo = build_demo(workspace.path());
    let site = workspace.path().join("site");
    fs::create_dir_all(&site).unwrap();

    run_init(workspace.path(), &site, &theme, &demo);

    assert_eq!(read(&site.join("bckt.yaml")), b"title: demo config\n");
    assert_eq!(read(&site.join("posts/first/post.md")), b"demo first post");
    assert_eq!(read(&site.join("pages/about/index.html")), b"demo about");
    assert_eq!(
        read(&site.join("pages/contact/index.html")),
        b"theme contact"
    );
    assert_eq!(read(&site.join("templates/base.html")), b"theme base");
    assert_eq!(read(&site.join("skel/style.css")), b"theme css");
    assert!(!site.join("posts/hello-from-bckt").exists());
}

#[test]
fn init_demo_rerun_preserves_user_edits() {
    let workspace = TempDir::new().unwrap();
    let theme = build_theme(workspace.path());
    let demo = build_demo(workspace.path());
    let site = workspace.path().join("site");
    fs::create_dir_all(&site).unwrap();
    run_init(workspace.path(), &site, &theme, &demo);

    fs::write(site.join("bckt.yaml"), b"title: edited\n").unwrap();
    fs::write(site.join("posts/first/post.md"), b"edited post").unwrap();
    fs::write(site.join("pages/about/index.html"), b"edited about").unwrap();
    write(&demo.join("posts/second/post.md"), b"demo second post");

    run_init(workspace.path(), &site, &theme, &demo);

    assert_eq!(read(&site.join("bckt.yaml")), b"title: edited\n");
    assert_eq!(read(&site.join("posts/first/post.md")), b"edited post");
    assert_eq!(read(&site.join("pages/about/index.html")), b"edited about");
    assert_eq!(
        read(&site.join("posts/second/post.md")),
        b"demo second post"
    );
}

#[test]
fn init_demo_missing_demo_preserves_existing_project() {
    let workspace = TempDir::new().unwrap();
    let theme = build_theme(workspace.path());
    let demo = build_demo(workspace.path());
    let site = workspace.path().join("site");
    fs::create_dir_all(&site).unwrap();
    run_init(workspace.path(), &site, &theme, &demo);
    fs::write(site.join("bckt.yaml"), b"title: edited\n").unwrap();
    fs::write(site.join("pages/about/index.html"), b"edited about").unwrap();

    let missing = workspace.path().join("sources").join("missing-demo");
    let output = run_init(workspace.path(), &site, &theme, &missing);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Warning: demo"), "stderr: {stderr}");
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    assert_eq!(read(&site.join("bckt.yaml")), b"title: edited\n");
    assert_eq!(read(&site.join("pages/about/index.html")), b"edited about");
    assert!(!site.join("posts/hello-from-bckt").exists());
}
