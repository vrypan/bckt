use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

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
    #[arg(long, default_value = "https://hub.farcaster.xyz")]
    hub: String,
    /// Destination directory for the generated post
    #[arg(long)]
    destination: Option<PathBuf>,
}

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

    let text = extract_string(
        &cast,
        &[
            &["data", "castAddBody", "text"],
            &["cast", "text"],
            &["cast", "body", "data", "text"],
            &["result", "cast", "text"],
            &["message", "data", "text"],
        ],
    )
    .ok_or_else(|| anyhow!("cast text not found in response"))?
    .to_string();

    let mut mention_cache = HashMap::new();
    let body_with_mentions = apply_mentions(&hub, &cast, &text, &mut mention_cache)?;
    let mut body = body_with_mentions.trim_end().to_string();

    let date_part = parsed_timestamp
        .format(&date_format_custom())
        .context("failed to format post date")?;
    let short_hash_len = std::cmp::min(10, hash.len());
    let short_hash = &hash[..short_hash_len];
    let slug = format!("fc-{}-{}", date_part, short_hash);

    let dest_root = match cli.destination {
        Some(path) => path,
        None => std::env::current_dir().context("failed to determine current directory")?,
    };
    let post_dir = dest_root.join(&slug);

    if post_dir.exists() {
        return Err(anyhow!(
            "destination '{}' already exists",
            post_dir.display()
        ));
    }

    fs::create_dir_all(&post_dir)
        .with_context(|| format!("failed to create directory {}", post_dir.display()))?;

    let embed_assets = process_embeds(&hub, &cast, &post_dir, &mut body, &mut mention_cache)?;

    let front_matter_date = parsed_timestamp
        .format(&front_matter_date_format())
        .context("failed to format front matter date")?;

    let filename = format!("{}.md", slug);
    let file_path = post_dir.join(filename);

    let mut contents = String::new();
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
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?;
        segments.pop_if_empty();
        segments.extend(["v1", "userNameProofByName"]);
    }
    url.query_pairs_mut().append_pair("name", username);

    let response = ureq::get(url.as_str())
        .call()
        .map_err(|err| anyhow!("failed to resolve username '{username}': {err}"))?;

    let json: Value = response
        .into_json()
        .map_err(|err| anyhow!("failed to decode username lookup response: {err}"))?;

    extract_integer(
        &json,
        &[
            &["fid"],
            &["data", "fid"],
            &["result", "user", "fid"],
            &["user", "fid"],
        ],
    )
    .ok_or_else(|| anyhow!("fid not found for username '{username}'"))
}

fn fetch_cast(hub: &Url, fid: u64, hash: &str) -> Result<Value> {
    let mut url = hub.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?;
        segments.pop_if_empty();
        segments.extend(["v1", "castById"]);
    }
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
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        if let Some(text) = current.as_str().filter(|text| !text.is_empty()) {
            return Some(text);
        }
    }
    None
}

fn extract_integer(value: &Value, paths: &[&[&str]]) -> Option<u64> {
    for path in paths {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        match current {
            Value::Number(num) if num.is_u64() => return num.as_u64(),
            Value::Number(num) if num.is_i64() => return num.as_i64().map(|n| n as u64),
            _ => continue,
        }
    }
    None
}

fn extract_timestamp(value: &Value) -> Option<OffsetDateTime> {
    if let Some(text) = extract_string(
        value,
        &[
            &["data", "publishedAt"],
            &["data", "timestamp"],
            &["cast", "timestamp"],
            &["result", "cast", "timestamp"],
        ],
    ) {
        if let Ok(dt) = OffsetDateTime::parse(text, &Rfc3339) {
            return Some(dt);
        }
        if let Some(dt) = text.parse::<i64>().ok().and_then(convert_epoch) {
            return Some(dt);
        }
    }

    extract_integer(
        value,
        &[
            &["data", "timestamp"],
            &["cast", "timestamp"],
            &["result", "cast", "timestamp"],
            &["message", "data", "timestamp"],
        ],
    )
    .and_then(|num| convert_epoch(num as i64))
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
}

fn process_embeds(
    hub: &Url,
    value: &Value,
    post_dir: &Path,
    body: &mut String,
    cache: &mut HashMap<u64, String>,
) -> Result<EmbedAssets> {
    let mut attachments = Vec::new();
    let mut images = Vec::new();
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let mut image_index = 1usize;

    for embed in collect_embeds(value) {
        if let Some(url) = embed.get("url").and_then(Value::as_str) {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                if !body.contains(url) {
                    links.push(url.to_string());
                }
                continue;
            }

            if !seen.insert(format!("url:{url}")) {
                continue;
            }

            let content_type = fetch_content_type(url);
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
            let fid = cast_obj.get("fid").and_then(Value::as_u64).or_else(|| {
                cast_obj
                    .get("fid")
                    .and_then(Value::as_i64)
                    .map(|n| n as u64)
            });
            let hash = cast_obj.get("hash").and_then(Value::as_str);

            let (Some(fid), Some(hash)) = (fid, hash) else {
                continue;
            };

            let key = format!("cast:{fid}:{hash}");
            if !seen.insert(key) {
                continue;
            }

            match fetch_cast(hub, fid, hash) {
                Ok(embed_cast) => {
                    let embed_text_raw = extract_string(
                        &embed_cast,
                        &[
                            &["data", "castAddBody", "text"],
                            &["data", "text"],
                            &["cast", "text"],
                            &["result", "cast", "text"],
                        ],
                    )
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
    })
}

fn collect_embeds(value: &Value) -> Vec<&Value> {
    let mut results = Vec::new();
    let paths: [&[&str]; 4] = [
        &["data", "castAddBody", "embeds"],
        &["cast", "embeds"],
        &["result", "cast", "embeds"],
        &["message", "data", "castAddBody", "embeds"],
    ];

    for path in paths {
        if let Some(Value::Array(array)) = get_nested(value, path) {
            results.extend(array.iter());
        }
    }

    results
}

fn collect_mentions(value: &Value) -> Option<(Vec<u64>, Vec<usize>)> {
    const PATHS: [(&[&str], &[&str]); 4] = [
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

    for (mention_path, position_path) in PATHS {
        let Some(Value::Array(mention_values)) = get_nested(value, mention_path) else {
            continue;
        };
        let Some(Value::Array(position_values)) = get_nested(value, position_path) else {
            continue;
        };

        if mention_values.is_empty() || mention_values.len() != position_values.len() {
            continue;
        }

        let mut mentions = Vec::with_capacity(mention_values.len());
        let mut positions = Vec::with_capacity(position_values.len());
        let mut valid = true;

        for value in mention_values {
            match value_to_u64(value) {
                Some(fid) => mentions.push(fid),
                None => {
                    valid = false;
                    break;
                }
            }
        }

        if !valid {
            continue;
        }

        for value in position_values {
            match value_to_u64(value) {
                Some(pos) => positions.push(pos as usize),
                None => {
                    valid = false;
                    break;
                }
            }
        }

        if valid && !mentions.is_empty() {
            return Some((mentions, positions));
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
    let Some((mention_fids, mention_positions)) = collect_mentions(cast) else {
        return Ok(text.to_string());
    };

    if mention_fids.is_empty() {
        return Ok(text.to_string());
    }

    let mut entries: Vec<(usize, String)> = Vec::new();
    let mut seen_positions: HashSet<usize> = HashSet::new();

    for (fid, pos) in mention_fids.into_iter().zip(mention_positions.into_iter()) {
        if !seen_positions.insert(pos) {
            continue;
        }
        let handle = resolve_handle(hub, fid, cache);
        entries.push((pos, handle));
    }

    if entries.is_empty() {
        return Ok(text.to_string());
    }

    entries.sort_by_key(|(pos, _)| *pos);

    let text_len = text.len();
    let mut result = String::with_capacity(text_len + entries.len() * 8);
    let mut last_byte = 0usize;

    for (pos, handle) in entries {
        let mut byte_pos = pos.min(text_len);

        while byte_pos > 0 && !text.is_char_boundary(byte_pos) {
            byte_pos -= 1;
        }

        result.push_str(&text[last_byte..byte_pos]);
        result.push_str(&handle);

        let mut next_byte = byte_pos;
        if next_byte < text_len {
            if let Some(next_char) = text[next_byte..].chars().next() {
                let should_skip = next_char == '@'
                    || matches!(next_char as u32, 0x01 | 0x1f);
                if should_skip {
                    next_byte += next_char.len_utf8();
                }
            }
        }

        last_byte = next_byte.min(text_len);
    }

    result.push_str(&text[last_byte..]);
    Ok(result)
}

fn resolve_handle(hub: &Url, fid: u64, cache: &mut HashMap<u64, String>) -> String {
    if let Some(handle) = cache.get(&fid) {
        return handle.clone();
    }

    let handle = fetch_fname_handle(hub, fid)
        .map(|name| ensure_handle(&name))
        .unwrap_or_else(|_| format!("@fid{fid}"));
    cache.insert(fid, handle.clone());
    handle
}

fn fetch_fname_handle(hub: &Url, fid: u64) -> Result<String> {
    let mut url = hub.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("hub URL cannot be a base for segments"))?;
        segments.pop_if_empty();
        segments.extend(["v1", "userNameProofsByFid"]);
    }
    url.query_pairs_mut().append_pair("fid", &fid.to_string());

    let response = ureq::get(url.as_str())
        .call()
        .map_err(|err| anyhow!("failed to fetch username proofs for fid {}: {}", fid, err))?;

    let json: Value = response.into_json().map_err(|err| {
        anyhow!(
            "failed to decode username proofs response for fid {}: {}",
            fid, err
        )
    })?;

    const PROOF_PATHS: [&[&str]; 3] = [
        &["proofs"],
        &["data", "proofs"],
        &["result", "proofs"],
    ];

    let mut proofs: Vec<&Value> = Vec::new();
    for path in PROOF_PATHS {
        if let Some(Value::Array(items)) = get_nested(&json, path) {
            proofs.extend(items.iter());
        }
    }

    if proofs.is_empty() && is_fname_proof(&json) {
        if let Some(name) = extract_proof_name(&json) {
            return Ok(name);
        }
    }

    for proof in proofs {
        if !is_fname_proof(proof) {
            continue;
        }
        if let Some(name) = extract_proof_name(proof) {
            return Ok(name);
        }
    }

    Err(anyhow!(
        "FNAME proof not found for fid {}",
        fid
    ))
}

fn is_fname_proof(value: &Value) -> bool {
    match value.get("type") {
        Some(Value::String(kind)) => {
            kind.eq_ignore_ascii_case("USERNAME_TYPE_FNAME")
                || kind.eq_ignore_ascii_case("FNAME")
        }
        Some(Value::Number(num)) => num.as_u64() == Some(6),
        _ => false,
    }
}

fn extract_proof_name(value: &Value) -> Option<String> {
    const FIELDS: [&str; 3] = ["name", "username", "value"];
    for field in FIELDS {
        if let Some(name) = value.get(field).and_then(Value::as_str) {
            if !name.trim().is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn get_nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn value_to_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|number| number as u64))
}

fn append_quote(body: &mut String, username: &str, text: &str) {
    let mut any = false;
    for line in text.lines() {
        any = true;
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            body.push('>');
        } else {
            body.push_str("> ");
            body.push_str(trimmed);
        }
        body.push('\n');
    }

    if any {
        body.push_str(">\n");
        body.push_str("> --");
        body.push_str(username);
        body.push('\n');
    }
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

fn image_extension_from_mime(mime: &str) -> Option<String> {
    let raw = mime.split(';').next()?.trim();
    if !raw.starts_with("image/") {
        return None;
    }

    let mut subtype = &raw[6..];
    if let Some(pos) = subtype.find('+') {
        subtype = &subtype[..pos];
    }

    let ext = match subtype {
        "jpeg" | "jpg" => "jpg",
        other => other,
    };

    Some(ext.to_string())
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

fn date_format_custom() -> &'static [FormatItem<'static>] {
    time::macros::format_description!("[year]-[month]-[day]")
}

fn front_matter_date_format() -> &'static [FormatItem<'static>] {
    time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"
    )
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
