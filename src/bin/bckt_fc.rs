use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "bckt-fc",
    about = "Generate a Farcaster post stub from a cast id"
)]
struct Cli {
    /// Cast identifier in the form username/hash
    #[arg(long)]
    castid: String,
    /// Farcaster hub base URL
    #[arg(long, default_value = "http://hub.merv.fun:3381")]
    hub: String,
    /// Destination directory for the generated post
    #[arg(long)]
    destination: Option<PathBuf>,
    /// Do not download video embeds locally
    #[arg(long)]
    no_local_video: bool,
}

// Pre-compiled static format descriptions for date formatting
static DATE_FORMAT: &[FormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]");
static FRONT_MATTER_FORMAT: &[FormatItem<'static>] = time::macros::format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"
);

// Static path arrays to avoid repeated allocations
static CAST_TEXT_PATHS: &[&[&str]] = &[
    &["data", "castAddBody", "text"],
    &["cast", "text"],
    &["cast", "body", "data", "text"],
    &["result", "cast", "text"],
    &["message", "data", "text"],
];

static FID_PATHS: &[&[&str]] = &[
    &["fid"],
    &["data", "fid"],
    &["result", "user", "fid"],
    &["user", "fid"],
];

static TIMESTAMP_STRING_PATHS: &[&[&str]] = &[
    &["data", "publishedAt"],
    &["data", "timestamp"],
    &["cast", "timestamp"],
    &["result", "cast", "timestamp"],
];

static TIMESTAMP_PATHS: &[&[&str]] = &[
    &["data", "timestamp"],
    &["cast", "timestamp"],
    &["result", "cast", "timestamp"],
    &["message", "data", "timestamp"],
];

static EMBED_PATHS: &[&[&str]] = &[
    &["data", "castAddBody", "embeds"],
    &["cast", "embeds"],
    &["result", "cast", "embeds"],
    &["message", "data", "castAddBody", "embeds"],
];

static EMBED_TEXT_PATHS: &[&[&str]] = &[
    &["data", "castAddBody", "text"],
    &["data", "text"],
    &["cast", "text"],
    &["result", "cast", "text"],
];

static MENTION_PATHS: &[(&[&str], &[&str])] = &[
    (
        &["data", "castAddBody", "mentions"],
        &["data", "castAddBody", "mentionsPositions"],
    ),
    (&["cast", "mentions"], &["cast", "mentionsPositions"]),
    (
        &["result", "cast", "mentions"],
        &["result", "cast", "mentionsPositions"],
    ),
    (
        &["message", "data", "castAddBody", "mentions"],
        &["message", "data", "castAddBody", "mentionsPositions"],
    ),
];

static PROOF_PATHS: &[&[&str]] = &[&["proofs"], &["data", "proofs"], &["result", "proofs"]];

static PROOF_NAME_FIELDS: &[&str] = &["name", "username", "value"];

static YT_DLP_CHECK: OnceLock<Result<(), String>> = OnceLock::new();

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let (username, hash) = parse_castid(&cli.castid)?;
    let hub = Url::parse(&cli.hub).context("failed to parse hub URL")?;
    let fid = resolve_fid(&hub, username)?;

    let cast = fetch_cast(&hub, fid, hash)?;

    let parsed_timestamp =
        extract_timestamp(&cast).ok_or_else(|| anyhow!("cast timestamp not found in response"))?;

    let text = extract_string(&cast, CAST_TEXT_PATHS)
        .ok_or_else(|| anyhow!("cast text not found in response"))?
        .to_string();

    let mut mention_cache = HashMap::new();
    let body_with_mentions = apply_mentions(&hub, &cast, &text, &mut mention_cache)?;
    let mut body = body_with_mentions.trim_end().to_string();

    let date_part = parsed_timestamp
        .format(DATE_FORMAT)
        .context("failed to format post date")?;
    let short_hash_len = hash.len().min(10);
    let short_hash = &hash[..short_hash_len];
    let slug = format!("fc-{}-{}", date_part, short_hash);

    let dest_root = cli
        .destination
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let post_dir = dest_root.join(&slug);

    if post_dir.exists() {
        return Err(anyhow!(
            "destination '{}' already exists",
            post_dir.display()
        ));
    }

    fs::create_dir_all(&post_dir)
        .with_context(|| format!("failed to create directory {}", post_dir.display()))?;

    let embed_assets = process_embeds(
        &hub,
        &cast,
        &post_dir,
        &mut body,
        &mut mention_cache,
        !cli.no_local_video,
    )?;

    let front_matter_date = parsed_timestamp
        .format(FRONT_MATTER_FORMAT)
        .context("failed to format front matter date")?;

    let filename = format!("{}.md", slug);
    let file_path = post_dir.join(filename);

    // Pre-calculate capacity for contents string
    let mut contents_capacity =
        200 + slug.len() + front_matter_date.len() + cli.castid.len() + body.len();
    if !embed_assets.attachments.is_empty() {
        contents_capacity += embed_assets
            .attachments
            .iter()
            .map(|s| s.len())
            .sum::<usize>()
            + embed_assets.attachments.len() * 4;
    }
    if !embed_assets.images.is_empty() {
        contents_capacity += embed_assets.images.iter().map(|s| s.len()).sum::<usize>()
            + embed_assets.images.len() * 4;
    }
    if !embed_assets.videos.is_empty() {
        contents_capacity += embed_assets.videos.iter().map(|s| s.len()).sum::<usize>()
            + embed_assets.videos.len() * 6;
    }

    let mut contents = String::with_capacity(contents_capacity);
    contents.push_str("---\n");
    contents.push_str("title: \"\"\n");
    contents.push_str(&format!("slug: \"{}\"\n", slug));
    contents.push_str(&format!("date: \"{}\"\n", front_matter_date));
    contents.push_str("type: farcaster\n");
    contents.push_str(&format!("castid: {}\n", cli.castid));
    if !embed_assets.attachments.is_empty() {
        contents.push_str("attached:\n");
        for name in &embed_assets.attachments {
            contents.push_str("  - ");
            contents.push_str(name);
            contents.push('\n');
        }
    }
    if !embed_assets.images.is_empty() {
        contents.push_str("images:\n");
        for name in &embed_assets.images {
            contents.push_str("  - ");
            contents.push_str(name);
            contents.push('\n');
        }
    }
    if !embed_assets.videos.is_empty() {
        contents.push_str("videos:\n");
        for name in &embed_assets.videos {
            contents.push_str("  - ");
            contents.push_str(name);
            contents.push('\n');
        }
    }
    contents.push_str("---\n\n");
    contents.push_str(&body);
    if !body.ends_with('\n') {
        contents.push('\n');
    }

    fs::write(&file_path, contents)
        .with_context(|| format!("failed to write {}", file_path.display()))?;

    println!("Created {}", file_path.display());

    Ok(())
}

fn parse_castid(input: &str) -> Result<(&str, &str)> {
    let mut parts = input.splitn(2, '/');
    let username = parts
        .next()
        .ok_or_else(|| anyhow!("missing username in castid"))?;
    let hash = parts
        .next()
        .ok_or_else(|| anyhow!("missing hash in castid"))?;

    if username.is_empty() || hash.is_empty() {
        return Err(anyhow!("castid must be in the form username/hash"));
    }

    Ok((username, hash))
}

fn resolve_fid(hub: &Url, username: &str) -> Result<u64> {
    let mut url = hub.clone();
    url.path_segments_mut()
        .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?
        .pop_if_empty()
        .extend(&["v1", "userNameProofByName"]);
    url.query_pairs_mut().append_pair("name", username);

    let response = ureq::get(url.as_str())
        .call()
        .map_err(|err| anyhow!("failed to resolve username '{username}': {err}"))?;

    let json: Value = response
        .into_json()
        .map_err(|err| anyhow!("failed to decode username lookup response: {err}"))?;

    extract_integer(&json, FID_PATHS)
        .ok_or_else(|| anyhow!("fid not found for username '{username}'"))
}

fn fetch_cast(hub: &Url, fid: u64, hash: &str) -> Result<Value> {
    let mut url = hub.clone();
    url.path_segments_mut()
        .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?
        .pop_if_empty()
        .extend(&["v1", "castById"]);

    url.query_pairs_mut()
        .append_pair("fid", &fid.to_string())
        .append_pair("hash", hash);

    let response = ureq::get(url.as_str())
        .call()
        .map_err(|err| anyhow!("failed to fetch cast: {err}"))?;

    response
        .into_json()
        .map_err(|err| anyhow!("failed to decode cast response: {err}"))
}

fn extract_string<'a>(value: &'a Value, paths: &[&[&str]]) -> Option<&'a str> {
    for path in paths {
        if let Some(result) = get_nested(value, path)
            && let Some(text) = result.as_str()
            && !text.is_empty()
        {
            return Some(text);
        }
    }
    None
}

fn extract_integer(value: &Value, paths: &[&[&str]]) -> Option<u64> {
    for path in paths {
        if let Some(current) = get_nested(value, path) {
            match current {
                Value::Number(num) if num.is_u64() => return num.as_u64(),
                Value::Number(num) if num.is_i64() => return num.as_i64().map(|n| n as u64),
                _ => continue,
            }
        }
    }
    None
}

fn extract_timestamp(value: &Value) -> Option<OffsetDateTime> {
    if let Some(text) = extract_string(value, TIMESTAMP_STRING_PATHS) {
        if let Ok(dt) = OffsetDateTime::parse(text, &Rfc3339) {
            return Some(dt);
        }
        if let Some(dt) = text.parse::<i64>().ok().and_then(convert_epoch) {
            return Some(dt);
        }
    }

    extract_integer(value, TIMESTAMP_PATHS).and_then(|num| convert_epoch(num as i64))
}

fn convert_epoch(value: i64) -> Option<OffsetDateTime> {
    const FARCASTER_EPOCH_UNIX: i64 = 1_609_459_200; // 2021-01-01T00:00:00Z

    let seconds = if value >= 10_000_000_000 {
        value / 1000
    } else if value >= 0 {
        value + FARCASTER_EPOCH_UNIX
    } else {
        return None;
    };

    OffsetDateTime::from_unix_timestamp(seconds).ok()
}

struct EmbedAssets {
    attachments: Vec<String>,
    images: Vec<String>,
    videos: Vec<String>,
}

fn process_embeds(
    hub: &Url,
    value: &Value,
    post_dir: &Path,
    body: &mut String,
    cache: &mut HashMap<u64, String>,
    download_videos: bool,
) -> Result<EmbedAssets> {
    let mut attachments = Vec::new();
    let mut images = Vec::new();
    let mut videos = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let mut image_index = 1usize;
    let mut video_index = 1usize;

    for embed in collect_embeds(value) {
        if let Some(url) = embed.get("url").and_then(Value::as_str) {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                if !body.contains(url) {
                    links.push(url.to_string());
                }
                continue;
            }

            if !seen.insert(url.to_string()) {
                continue;
            }

            let lower_url = url.to_ascii_lowercase();
            let content_type = fetch_content_type(url);
            let is_video = looks_like_video_url(&lower_url)
                || content_type.as_deref().is_some_and(is_video_mime);

            if download_videos && is_video {
                let prefix = format!("video{:02}", video_index);
                let files = download_video_with_yt_dlp(url, post_dir, &prefix)?;
                for file in files {
                    if !attachments.contains(&file) {
                        attachments.push(file.clone());
                    }
                    if !videos.contains(&file) {
                        videos.push(file);
                    }
                }
                video_index += 1;
                continue;
            }

            if let Some(ext) = content_type.as_deref().and_then(image_extension_from_mime) {
                let filename = format!("image{:02}.{}", image_index, ext);
                image_index += 1;
                let destination = post_dir.join(&filename);
                match download_image(url, &destination) {
                    Ok(()) => {
                        attachments.push(filename.clone());
                        images.push(filename);
                        continue;
                    }
                    Err(err) => {
                        eprintln!("Warning: failed to download {url}: {err}");
                    }
                }
            }

            if !body.contains(url) {
                links.push(url.to_string());
            }
            continue;
        }

        if let Some(cast_obj) = embed.get("castId") {
            let fid = value_to_u64(cast_obj.get("fid"));
            let hash = cast_obj
                .get("hash")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("cast hash not found"))?;

            let key = format!("cast:{}:{}", fid, hash);
            if !seen.insert(key) {
                continue;
            }

            match fetch_cast(hub, fid, hash) {
                Ok(embed_cast) => {
                    let embed_text_raw = extract_string(&embed_cast, EMBED_TEXT_PATHS)
                        .unwrap_or("")
                        .to_string();

                    let embed_text_processed =
                        apply_mentions(hub, &embed_cast, &embed_text_raw, cache)?;
                    let embed_text = embed_text_processed.trim();
                    if embed_text.is_empty() {
                        continue;
                    }

                    let username = resolve_handle(hub, fid, cache);

                    if !body.ends_with('\n') {
                        body.push('\n');
                    }
                    body.push('\n');
                    append_quote(body, &username, embed_text);
                }
                Err(err) => {
                    eprintln!(
                        "Warning: failed to fetch embedded cast {} / {}: {}",
                        fid, hash, err
                    );
                }
            }
        }
    }

    if !links.is_empty() {
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push('\n');
        for link in links {
            body.push_str(&link);
            body.push('\n');
        }
    }

    Ok(EmbedAssets {
        attachments,
        images,
        videos,
    })
}

fn collect_embeds(value: &Value) -> Vec<&Value> {
    let mut results = Vec::new();

    for path in EMBED_PATHS {
        if let Some(Value::Array(array)) = get_nested(value, path) {
            results.extend(array.iter());
        }
    }

    results
}

fn collect_mentions(value: &Value) -> Option<(Vec<u64>, Vec<usize>)> {
    for (mention_path, position_path) in MENTION_PATHS {
        let mention_values = get_nested(value, mention_path)?.as_array()?;
        let position_values = get_nested(value, position_path)?.as_array()?;

        if mention_values.is_empty() || mention_values.len() != position_values.len() {
            continue;
        }

        let mentions: Option<Vec<u64>> = mention_values
            .iter()
            .map(|v| Some(value_to_u64(Some(v))))
            .collect();
        let positions: Option<Vec<usize>> = position_values
            .iter()
            .map(|v| Some(value_to_u64(Some(v)) as usize))
            .collect();

        match (mentions, positions) {
            (Some(m), Some(p)) if !m.is_empty() => return Some((m, p)),
            _ => continue,
        }
    }

    None
}

fn apply_mentions(
    hub: &Url,
    cast: &Value,
    text: &str,
    cache: &mut HashMap<u64, String>,
) -> Result<String> {
    let (mention_fids, mention_positions) = match collect_mentions(cast) {
        Some(data) => data,
        None => return Ok(text.to_string()),
    };

    if mention_fids.is_empty() {
        return Ok(text.to_string());
    }

    let mut entries: Vec<(usize, String)> = mention_fids
        .into_iter()
        .zip(mention_positions)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|(fid, pos)| (pos, resolve_handle(hub, fid, cache)))
        .collect();

    if entries.is_empty() {
        return Ok(text.to_string());
    }

    entries.sort_unstable_by_key(|(pos, _)| *pos);

    let text_len = text.len();
    let mut result = String::with_capacity(text_len + entries.len() * 8);
    let mut last_byte = 0;

    for (pos, handle) in entries {
        let mut byte_pos = pos.min(text_len);

        // Find proper char boundary
        while byte_pos > 0 && !text.is_char_boundary(byte_pos) {
            byte_pos -= 1;
        }

        result.push_str(&text[last_byte..byte_pos]);
        result.push_str(&handle);

        let mut next_byte = byte_pos;
        if next_byte < text_len
            && let Some(next_char) = text[next_byte..].chars().next()
        {
            let should_skip = next_char == '@' || matches!(next_char as u32, 0x01 | 0x1f);
            if should_skip {
                next_byte += next_char.len_utf8();
            }
        }

        last_byte = next_byte.min(text_len);
    }

    result.push_str(&text[last_byte..]);
    Ok(result)
}

fn resolve_handle(hub: &Url, fid: u64, cache: &mut HashMap<u64, String>) -> String {
    cache.get(&fid).cloned().unwrap_or_else(|| {
        let handle = fetch_fname_handle(hub, fid)
            .map(|name| ensure_handle(&name))
            .unwrap_or_else(|_| format!("@fid{fid}"));
        cache.insert(fid, handle.clone());
        handle
    })
}

fn fetch_fname_handle(hub: &Url, fid: u64) -> Result<String> {
    let mut url = hub.clone();
    url.path_segments_mut()
        .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?
        .pop_if_empty()
        .extend(&["v1", "userNameProofsByFid"]);
    url.query_pairs_mut().append_pair("fid", &fid.to_string());

    let response = ureq::get(url.as_str())
        .call()
        .map_err(|err| anyhow!("failed to fetch username proofs for fid {}: {}", fid, err))?;

    let json: Value = response.into_json().map_err(|err| {
        anyhow!(
            "failed to decode username proofs response for fid {}: {}",
            fid,
            err
        )
    })?;

    let mut proofs: Vec<&Value> = Vec::new();
    for path in PROOF_PATHS {
        if let Some(Value::Array(items)) = get_nested(&json, path) {
            proofs.extend(items.iter());
        }
    }

    if proofs.is_empty()
        && is_fname_proof(&json)
        && let Some(name) = extract_proof_name(&json)
    {
        return Ok(name);
    }

    for proof in proofs {
        if is_fname_proof(proof)
            && let Some(name) = extract_proof_name(proof)
        {
            return Ok(name);
        }
    }

    Err(anyhow!("FNAME proof not found for fid {}", fid))
}

fn is_fname_proof(value: &Value) -> bool {
    match value.get("type") {
        Some(Value::String(kind)) => {
            kind.eq_ignore_ascii_case("USERNAME_TYPE_FNAME") || kind.eq_ignore_ascii_case("FNAME")
        }
        Some(Value::Number(num)) => num.as_u64() == Some(6),
        _ => false,
    }
}

fn extract_proof_name(value: &Value) -> Option<String> {
    for field in PROOF_NAME_FIELDS {
        if let Some(name) = value.get(field).and_then(Value::as_str) {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn get_nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |current, key| current.get(key))
}

fn value_to_u64(value: Option<&Value>) -> u64 {
    value
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
        .unwrap_or(0)
}

fn append_quote(body: &mut String, username: &str, text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }

    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            body.push('>');
        } else {
            body.push_str("> ");
            body.push_str(trimmed);
        }
        body.push('\n');
    }

    body.push_str(">\n> --");
    body.push_str(username);
    body.push('\n');
}

fn ensure_handle(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with('@') {
        trimmed.to_string()
    } else {
        format!("@{}", trimmed.trim_start_matches('@'))
    }
}

fn fetch_content_type(url: &str) -> Option<String> {
    match ureq::head(url).call() {
        Ok(resp) => resp.header("content-type").map(|s| s.to_string()),
        Err(ureq::Error::Status(_, resp)) => resp.header("content-type").map(|s| s.to_string()),
        Err(_) => None,
    }
}

fn image_extension_from_mime(mime: &str) -> Option<&str> {
    let raw = mime.split(';').next()?.trim();
    if !raw.starts_with("image/") {
        return None;
    }

    let mut subtype = &raw[6..];
    if let Some(pos) = subtype.find('+') {
        subtype = &subtype[..pos];
    }

    Some(match subtype {
        "jpeg" | "jpg" => "jpg",
        other => other,
    })
}

fn download_image(url: &str, destination: &Path) -> Result<()> {
    let mut reader = ureq::get(url)
        .call()
        .map_err(|err| anyhow!("failed to download {url}: {err}"))?
        .into_reader();

    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .map_err(|err| anyhow!("failed to read body from {url}: {err}"))?;

    fs::write(destination, &buffer)
        .with_context(|| format!("failed to write {}", destination.display()))?;
    Ok(())
}

fn looks_like_video_url(url: &str) -> bool {
    const VIDEO_EXTENSIONS: &[&str] = &[
        ".m3u8", ".m3u", ".mp4", ".mov", ".webm", ".mkv", ".avi", ".mpg", ".mpeg", ".ogv",
    ];
    let without_query = url.split('?').next().unwrap_or(url);
    VIDEO_EXTENSIONS
        .iter()
        .any(|ext| without_query.ends_with(ext))
}

fn is_video_mime(mime: &str) -> bool {
    let normalized = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    normalized.starts_with("video/")
        || matches!(
            normalized.as_str(),
            "application/vnd.apple.mpegurl"
                | "application/x-mpegurl"
                | "application/dash+xml"
                | "application/mp2t"
        )
}

fn download_video_with_yt_dlp(url: &str, post_dir: &Path, prefix: &str) -> Result<Vec<String>> {
    ensure_yt_dlp_available()?;

    let output_template = post_dir.join(format!("{}.%(ext)s", prefix));

    let status = Command::new("yt-dlp")
        .arg("--no-playlist")
        .arg("--no-progress")
        .arg("--quiet")
        .arg("--no-warnings")
        .arg("--no-cache-dir")
        .arg("--restrict-filenames")
        .arg("-o")
        .arg(output_template.to_string_lossy().as_ref())
        .arg(url)
        .status()
        .with_context(|| format!("failed to execute yt-dlp for {url}"))?;

    if !status.success() {
        return Err(anyhow!(
            "yt-dlp exited with status {} while processing {url}",
            status
        ));
    }

    let mut files = Vec::new();
    let prefix_with_dot = format!("{}.", prefix);

    for entry in fs::read_dir(post_dir)
        .with_context(|| format!("failed to read directory {}", post_dir.display()))?
    {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(&prefix_with_dot) {
            continue;
        }
        if name.ends_with(".part") || name.contains(".part.") || name.ends_with(".ytdl") {
            continue;
        }
        files.push(name.into_owned());
    }

    files.sort();
    files.dedup();

    if files.is_empty() {
        return Err(anyhow!("yt-dlp did not produce an output file for {url}"));
    }

    Ok(files)
}

fn ensure_yt_dlp_available() -> Result<()> {
    let result = YT_DLP_CHECK.get_or_init(|| {
        match Command::new("yt-dlp")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("yt-dlp --version exited with status {}", status)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err("yt-dlp not found in PATH".to_string())
            }
            Err(err) => Err(format!("failed to execute yt-dlp: {err}")),
        }
    });

    match result {
        Ok(()) => Ok(()),
        Err(message) => Err(anyhow!(message.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_mentions_respects_byte_offsets() {
        let hub = Url::parse("https://example.com").unwrap();
        let cast = json!({
            "data": {
                "castAddBody": {
                    "mentions": [1],
                    "mentionsPositions": [2]
                }
            }
        });

        let mut cache = HashMap::new();
        cache.insert(1, "@alice".to_string());

        let text = "éa";
        let result = apply_mentions(&hub, &cast, text, &mut cache).unwrap();

        assert_eq!(result, "é@alicea");
    }
}
