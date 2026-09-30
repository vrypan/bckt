use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};
use blake3::Hasher;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::{Rfc2822, Rfc3339};

pub(super) use crate::utils::xml_escape;

pub(super) fn log_status(enabled: bool, label: &str, message: impl AsRef<str>) {
    if enabled {
        println!("[{}] {}", label, message.as_ref());
    }
}

pub(super) fn compute_cache_digest<T: Serialize>(value: &T) -> Result<String> {
    let data = serde_json::to_vec(value).context("failed to serialize cache payload")?;
    let mut hasher = Hasher::new();
    hasher.update(&data);
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

pub(super) fn remove_dir_if_empty(path: &Path) -> Result<()> {
    match fs::remove_dir(path) {
        Ok(_) => Ok(()),
        Err(err)
            if err.kind() == ErrorKind::NotFound || err.kind() == ErrorKind::DirectoryNotEmpty =>
        {
            Ok(())
        }
        Err(err) => {
            Err(err).with_context(|| format!("failed to remove directory {}", path.display()))
        }
    }
}

pub(super) struct PaginationLayout {
    pub per_page: usize,
    pub regular_page_count: usize,
}

pub(super) fn compute_pagination_layout(
    post_count: usize,
    homepage_posts: usize,
) -> PaginationLayout {
    let per_page = std::cmp::max(1, homepage_posts);
    let remainder = post_count % per_page;
    let home_page_size = if post_count < per_page {
        post_count
    } else if remainder == 0 {
        per_page
    } else if remainder < per_page {
        remainder + per_page
    } else {
        per_page
    };
    let regular_page_count = (post_count - home_page_size) / per_page;
    PaginationLayout {
        per_page,
        regular_page_count,
    }
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|comp| comp.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn format_rfc3339(date: &OffsetDateTime) -> Result<String> {
    date.format(&Rfc3339)
        .context("failed to format RFC3339 date")
}

pub(super) fn format_rfc2822(date: &OffsetDateTime) -> Result<String> {
    date.format(&Rfc2822)
        .context("failed to format RFC2822 date")
}

/// Make `value` safe inside a CDATA section by splitting every `]]>` across two
/// sections: `]]` stays in the first, `>` opens the next.
pub(super) fn sanitize_cdata(value: &str) -> String {
    if value.contains("]]>") {
        value.replace("]]>", "]]]]><![CDATA[>")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded_cdata(value: &str) -> String {
        let xml = format!("<root><![CDATA[{}]]></root>", sanitize_cdata(value));
        let doc = roxmltree::Document::parse(&xml)
            .unwrap_or_else(|err| panic!("invalid XML for {value:?}: {err}\n{xml}"));
        doc.root_element()
            .children()
            .filter_map(|node| node.text())
            .collect()
    }

    #[test]
    fn cdata_round_trip() {
        let cases = [
            "",
            "plain café — 日本 🚀",
            "a]]>b",
            "]]>start",
            "end]]>",
            "]]>]]>",
            "]]>>",
            "]]]]]>",
            "x]]]]>]]>y",
        ];
        for value in cases {
            assert_eq!(decoded_cdata(value), value, "{value:?}");
        }
    }

    #[test]
    fn cdata_without_delimiter_is_unchanged() {
        let value = "<p>a ] b ]] c > d</p>";
        assert_eq!(sanitize_cdata(value), value);
    }
}
