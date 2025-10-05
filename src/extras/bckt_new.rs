use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

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
    let default_date = cli
        .date
        .clone()
        .unwrap_or_else(|| now.format(&Rfc3339).expect("current time formats"));

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
    let parsed_date = parse_datetime(&date_str).unwrap_or_else(|| now);

    let tags_input = value_or_prompt(
        "Tags (comma separated)",
        cli.tags.clone().unwrap_or_else(|| "en".to_string()),
        false,
        cli.no_prompt,
    )?;
    let tags = normalize_tags(&tags_input);

    let post_type_raw = value_or_prompt(
        "Type",
        cli.post_type.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;
    let post_type = non_empty(post_type_raw);

    let abstract_raw = value_or_prompt(
        "Abstract",
        cli.abstract_text.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;
    let abstract_text = non_empty(abstract_raw);

    let language_raw = value_or_prompt(
        "Language",
        cli.language.clone().unwrap_or_default(),
        true,
        cli.no_prompt,
    )?;
    let language = non_empty(language_raw);

    let year_dir = posts_root.join(parsed_date.year().to_string());
    fs::create_dir_all(&year_dir)
        .with_context(|| format!("failed to create directory {}", year_dir.display()))?;

    let dir_name = format!("{}-{}", date_prefix(&parsed_date), &slug);
    let post_dir = year_dir.join(&dir_name);
    if post_dir.exists() {
        bail!("destination '{}' already exists", post_dir.display());
    }
    fs::create_dir_all(&post_dir)
        .with_context(|| format!("failed to create directory {}", post_dir.display()))?;

    let file_name = format!("{}.md", &dir_name);
    let file_path = post_dir.join(&file_name);

    let front_matter = build_front_matter(
        &title,
        &slug,
        &date_str,
        &tags,
        post_type.as_deref(),
        abstract_text.as_deref(),
        language.as_deref(),
    );

    let mut file_contents = String::new();
    file_contents.push_str(&front_matter);
    file_contents.push_str("\nYour content goes here.\n");

    fs::write(&file_path, file_contents)
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

fn parse_datetime(value: &str) -> Option<OffsetDateTime> {
    if let Ok(dt) = OffsetDateTime::parse(value, &Rfc3339) {
        return Some(dt);
    }

    const NAIVE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

    if let Ok(naive) = PrimitiveDateTime::parse(value, NAIVE_FORMAT) {
        return Some(naive.assume_offset(UtcOffset::UTC));
    }

    if let Some((main, offset_part)) = value.rsplit_once(' ') {
        if let Ok(naive) = PrimitiveDateTime::parse(main, NAIVE_FORMAT)
            && let Ok(offset) = parse_offset(offset_part)
        {
            return Some(naive.assume_offset(offset));
        }
    }

    None
}

fn parse_offset(value: &str) -> Result<UtcOffset> {
    if value.eq_ignore_ascii_case("UTC") || value.eq_ignore_ascii_case("Z") {
        return Ok(UtcOffset::UTC);
    }

    let trimmed = value.trim();
    if trimmed.len() < 3 {
        bail!("offset '{}' is too short", value);
    }

    let normalized = if trimmed.len() == 5 && (trimmed.starts_with('+') || trimmed.starts_with('-'))
    {
        format!("{}:{}", &trimmed[..3], &trimmed[3..])
    } else {
        trimmed.to_string()
    };

    if let Ok(offset) = UtcOffset::parse(
        &normalized,
        &format_description!("[offset_hour sign:mandatory]:[offset_minute]"),
    ) {
        return Ok(offset);
    }

    if let Ok(offset) = UtcOffset::parse(
        &normalized,
        &format_description!("[offset_hour sign:mandatory]:[offset_minute]:[offset_second]"),
    ) {
        return Ok(offset);
    }

    bail!("offset '{}' is invalid", value)
}

fn normalize_tags(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|raw| raw.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
}

fn generate_fallback_slug(now: OffsetDateTime) -> String {
    let prefix = date_prefix(&now);
    format!("{}-post", prefix)
}

fn date_prefix(dt: &OffsetDateTime) -> String {
    const PREFIX_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year repr:last_two][month][day]");
    dt.format(PREFIX_FORMAT).expect("date prefix formats")
}

fn build_front_matter(
    title: &str,
    slug: &str,
    date: &str,
    tags: &[String],
    post_type: Option<&str>,
    abstract_text: Option<&str>,
    language: Option<&str>,
) -> String {
    let mut fm = String::new();
    fm.push_str("---\n");
    fm.push_str(&format!("title: {}\n", yaml_quote(title)));
    fm.push_str(&format!("slug: {}\n", slug));
    fm.push_str(&format!("date: {}\n", yaml_quote(date)));
    if !tags.is_empty() {
        fm.push_str(&format!("tags: {}\n", tags.join(", ")));
    }
    if let Some(pt) = post_type {
        if !pt.trim().is_empty() {
            fm.push_str(&format!("type: {}\n", pt.trim()));
        }
    }
    if let Some(summary) = abstract_text {
        if !summary.trim().is_empty() {
            fm.push_str(&format!("abstract: {}\n", yaml_quote(summary.trim())));
        }
    }
    if let Some(lang) = language {
        if !lang.trim().is_empty() {
            fm.push_str(&format!("language: {}\n", lang.trim()));
        }
    }
    fm.push_str("attached:\n");
    fm.push_str("---\n\n");
    fm
}

fn yaml_quote(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn find_project_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        if current.join("bckt.yaml").exists() {
            return Ok(current);
        }
        if !current.pop() {
            bail!(
                "could not locate bckt.yaml starting from {}",
                start.display()
            );
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
