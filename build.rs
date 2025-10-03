use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let theme_dir = Path::new(&manifest_dir).join("themes").join("bckt3");

    let mut files = Vec::new();
    collect_files(&theme_dir, &theme_dir, &mut files)?;
    files.sort();

    for relative in &files {
        let absolute = theme_dir.join(relative);
        println!("cargo:rerun-if-changed={}", absolute.display());
    }
    println!("cargo:rerun-if-changed={}", theme_dir.display());

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = PathBuf::from(out_dir).join("theme_bckt3.rs");
    let mut output = File::create(&dest_path)?;

    writeln!(
        output,
        "pub struct EmbeddedFile {{\n    pub path: &'static str,\n    pub contents: &'static [u8],\n}}\n"
    )?;
    writeln!(output, "pub const THEME_BCKT3_FILES: &[EmbeddedFile] = &[")?;
    for relative in &files {
        writeln!(
            output,
            "    EmbeddedFile {{ path: \"{path}\", contents: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/themes/bckt3/{path}\")), }},",
            path = relative
        )?;
    }
    writeln!(output, "];\n")?;

    Ok(())
}

fn collect_files(dir: &Path, base: &Path, files: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, base, files)?;
        } else {
            let relative = path
                .strip_prefix(base)
                .expect("file should be under base directory");
            let normalized = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push(normalized);
        }
    }
    Ok(())
}
