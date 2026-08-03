use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use bckt::post::{
    FrontMatter, date_prefix, find_project_root, format_rfc3339, non_empty, normalize_tags,
    parse_datetime, post_dir, post_file, slugify,
};
use clap::Parser;
use time::OffsetDateTime;

#[derive(Parser, Debug)]
#[command(
    name = "bckt-new",
    version,
    about = "Scaffold a new post for a bckt project"
)]
struct Cli {
    /// Title for the new post
    #[arg(long)]
    title: Option<String>,
    /// Slug to store in front matter (defaults to slugified title)
    #[arg(long)]
    slug: Option<String>,
    /// Publication timestamp (RFC3339 or `YYYY-MM-DD HH:MM:SS`)
    #[arg(long)]
    date: Option<String>,
    /// Comma-separated list of tags
    #[arg(long)]
    tags: Option<String>,
    /// Post type (stored as `type` in front matter)
    #[arg(long = "type", value_name = "TYPE")]
    post_type: Option<String>,
    /// Abstract / summary text
    #[arg(long = "abstract", value_name = "TEXT")]
    abstract_text: Option<String>,
    /// Language code to store in front matter
    #[arg(long)]
    language: Option<String>,
    /// Destination posts directory (defaults to `<project>/posts`)
    #[arg(long)]
    posts_dir: Option<PathBuf>,
    /// Run without interactive prompts (use provided flags and defaults)
    #[arg(long)]
    no_prompt: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let current_dir = std::env::current_dir().context("failed to determine current directory")?;
    let project_root = find_project_root(&current_dir)
        .context("run this command inside a bckt project (with bckt.yaml)")?;

    let posts_root = cli
        .posts_dir
        .clone()
        .unwrap_or_else(|| project_root.join("posts"));
    fs::create_dir_all(&posts_root)
        .with_context(|| format!("failed to create posts directory {}", posts_root.display()))?;

    let now = OffsetDateTime::now_utc();
    let default_date = cli.date.clone().unwrap_or_else(|| format_rfc3339(&now));

    let title = value_or_prompt(
        "Title",
        cli.title.clone().unwrap_or_default(),
        false,
        cli.no_prompt,
    )?;

    let mut slug_candidate = cli.slug.clone().unwrap_or_else(|| slugify(&title));
    if slug_candidate.is_empty() {
        slug_candidate = generate_fallback_slug(now); // ensures non-empty even in non-interactive mode
    }

    let slug = loop {
        let entered = value_or_prompt("Slug", slug_candidate.clone(), false, cli.no_prompt)?;
        let sanitized = slugify(&entered);
        if sanitized.is_empty() {
            if cli.no_prompt {
                bail!("slug is required; provide a non-empty value with --slug");
            } else {
                println!("Slug cannot be empty. Please enter a valid value.");
                slug_candidate = generate_fallback_slug(now);
                continue;
            }
        }
        break sanitized;
    };

    let date_str = value_or_prompt("Date", default_date, false, cli.no_prompt)?;
    let parsed_date = parse_datetime(&date_str).unwrap_or(now);

    let tags_input = value_or_prompt(
        "Tags (comma separated)",
        cli.tags.clone().unwrap_or_default(),
        false,
        cli.no_prompt,
    )?;

    let post_type_raw = value_or_prompt(
        "Type",
        cli.post_type.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;

    let abstract_raw = value_or_prompt(
        "Abstract",
        cli.abstract_text.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;

    let language_raw = value_or_prompt(
        "Language",
        cli.language.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;

    let destination = post_dir(&posts_root, &parsed_date, &slug);
    if destination.exists() {
        bail!("destination '{}' already exists", destination.display());
    }
    fs::create_dir_all(&destination)
        .with_context(|| format!("failed to create directory {}", destination.display()))?;

    let front_matter = FrontMatter {
        title: Some(title),
        slug: slug.clone(),
        date: date_str,
        tags: normalize_tags(&tags_input),
        post_type: non_empty(&post_type_raw),
        abstract_text: non_empty(&abstract_raw),
        language: non_empty(&language_raw),
        attached: Vec::new(),
    };

    let file_path = post_file(&destination, &slug);
    fs::write(
        &file_path,
        front_matter.into_document("Your content goes here.\n"),
    )
    .with_context(|| format!("failed to write {}", file_path.display()))?;

    println!("Created new post at {}", file_path.display());
    Ok(())
}

fn value_or_prompt(
    label: &str,
    default: String,
    allow_empty: bool,
    no_prompt: bool,
) -> Result<String> {
    if no_prompt {
        return Ok(default);
    }

    let prompt = if default.is_empty() {
        format!("{}: ", label)
    } else {
        format!("{} [{}]: ", label, default)
    };

    print!("{}", prompt);
    io::stdout().flush().context("failed to flush prompt")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read input")?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        if allow_empty {
            Ok(String::new())
        } else {
            Ok(default)
        }
    } else {
        Ok(trimmed.to_string())
    }
}

fn generate_fallback_slug(now: OffsetDateTime) -> String {
    format!("{}-post", date_prefix(&now))
}
