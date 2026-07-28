mod filters;

use anyhow::Result;
use minijinja::value::Value;
use minijinja::{Environment, ErrorKind};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::config::Config;
use crate::utils::extract_base_path;

pub fn environment(config: &Config) -> Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            minijinja::AutoEscape::Html
        } else {
            minijinja::AutoEscape::None
        }
    });
    env.add_global("config", Value::from_serialize(config));
    env.add_global(
        "base_url",
        Value::from_safe_string(normalize_base_url(&config.base_url)),
    );
    env.add_global(
        "base_path",
        Value::from_safe_string(extract_base_path(&config.base_url)),
    );

    let default_format = config.date_format.clone();
    env.add_function(
        "now",
        move |format: Option<&str>| -> Result<String, minijinja::Error> {
            let format = format.unwrap_or(&default_format);

            if format.eq_ignore_ascii_case("RFC3339") {
                return OffsetDateTime::now_utc().format(&Rfc3339).map_err(|err| {
                    minijinja::Error::new(
                        ErrorKind::InvalidOperation,
                        format!("failed to format now(): {err}"),
                    )
                });
            }

            let description = time::format_description::parse(format).map_err(|err| {
                minijinja::Error::new(
                    ErrorKind::InvalidOperation,
                    format!("invalid date format '{format}' passed to now(): {err}"),
                )
            })?;

            OffsetDateTime::now_utc()
                .format(&description)
                .map_err(|err| {
                    minijinja::Error::new(
                        ErrorKind::InvalidOperation,
                        format!("failed to format now(): {err}"),
                    )
                })
        },
    );

    env.add_function("atproto_tid", atproto_tid);

    filters::register(&mut env)?;

    Ok(env)
}

const TID_ALPHA: &[u8; 32] = b"234567abcdefghijklmnopqrstuvwxyz";

/// Deterministic atproto TID (record key) from a post's publication date and
/// slug. FROZEN: the mapping from (date, slug) to output is a compatibility
/// promise — the output is a live PDS record key and an embedded page URL.
/// Do NOT change the hash or bit layout; doing so re-keys every published post.
/// Intentionally NOT byte-compatible with `goat` (which hashes with sha256);
/// bckt uses blake3 and the RSS feed is the only consumer of the value.
fn atproto_tid(date_rfc3339: &str, slug: &str) -> Result<String, minijinja::Error> {
    let dt = OffsetDateTime::parse(date_rfc3339, &Rfc3339).map_err(|err| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "atproto_tid(): first argument must be an RFC3339 date (e.g. post.date_iso); got '{date_rfc3339}': {err}"
            ),
        )
    })?;

    let micros = (dt.unix_timestamp_nanos() / 1_000) as u64 & 0x1F_FFFF_FFFF_FFFF;
    let digest = blake3::hash(slug.as_bytes());
    let clock = u16::from_be_bytes([digest.as_bytes()[0], digest.as_bytes()[1]]) as u64 & 0x3FF;
    let mut v = ((micros << 10) | clock) & 0x7FFF_FFFF_FFFF_FFFF;

    let mut out = [0u8; 13];
    for slot in out.iter_mut().rev() {
        *slot = TID_ALPHA[(v & 0x1F) as usize];
        v >>= 5;
    }
    // Safe: every byte came from TID_ALPHA, which is ASCII.
    Ok(String::from_utf8(out.to_vec()).expect("TID alphabet is ASCII"))
}

fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;

    #[test]
    fn tid_is_thirteen_sortable_base32_chars() {
        let out = atproto_tid("2026-07-20T09:30:00Z", "some-post").unwrap();
        assert_eq!(out.len(), 13);
        assert!(
            out.bytes()
                .all(|b| b"234567abcdefghijklmnopqrstuvwxyz".contains(&b))
        );
        // top bit clear => first char is in the low half of the alphabet
        assert!(b"234567abcdefghijklmno".contains(&out.as_bytes()[0]));
    }

    #[test]
    fn tid_is_frozen() {
        // FROZEN VECTOR — regenerate ONCE from the implementation, then never edit.
        // If this assertion ever fails, the algorithm changed and every published
        // post would be re-keyed. That is a breaking change, not a test to update.
        assert_eq!(
            atproto_tid("2026-07-20T09:30:00Z", "some-post").unwrap(),
            "3mr2yahvpk2ih"
        );
    }

    #[test]
    fn tid_is_stable_across_calls() {
        let a = atproto_tid("2026-07-20T09:30:00Z", "hello-world").unwrap();
        let b = atproto_tid("2026-07-20T09:30:00Z", "hello-world").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn tid_varies_with_slug_and_date() {
        let base = atproto_tid("2026-07-20T09:30:00Z", "a").unwrap();
        assert_ne!(base, atproto_tid("2026-07-20T09:30:00Z", "b").unwrap());
        assert_ne!(base, atproto_tid("2026-07-20T09:30:01Z", "a").unwrap());
    }

    #[test]
    fn tid_rejects_non_rfc3339_date() {
        assert!(atproto_tid("20 July 2026", "x").is_err());
    }

    #[test]
    fn tid_callable_from_template() {
        let config = Config::default();
        let mut env = environment(&config).unwrap();
        env.add_template("t", "{{ atproto_tid(d, s) }}").unwrap();
        let ctx = minijinja::context! { d => "2026-07-20T09:30:00Z", s => "some-post" };
        let rendered = env.get_template("t").unwrap().render(ctx).unwrap();
        assert_eq!(rendered.len(), 13);
    }

    #[test]
    fn config_available_in_templates() {
        let config = Config {
            title: Some("Bucket".to_string()),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("greet", "{{ config.title }}").unwrap();

        let rendered = env.get_template("greet").unwrap().render(()).unwrap();
        assert_eq!(rendered, "Bucket");
    }

    #[test]
    fn now_helper_uses_config_format() {
        let config = Config {
            date_format: "[year]".to_string(),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("when", "{{ now() }}").unwrap();

        let rendered = env.get_template("when").unwrap().render(()).unwrap();
        assert_eq!(rendered.len(), 4);
    }

    #[test]
    fn now_helper_accepts_rfc3339_keyword() {
        let config = Config::default();
        let mut env = environment(&config).unwrap();
        env.add_template("when", "{{ now('RFC3339') }}").unwrap();

        let rendered = env.get_template("when").unwrap().render(()).unwrap();
        assert!(rendered.contains('T'));
        assert!(rendered.ends_with('Z'));
    }

    #[test]
    fn base_url_has_no_trailing_slash() {
        let config = Config {
            base_url: "https://example.com/blog".to_string(),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("base", "{{ base_url }}").unwrap();

        let rendered = env.get_template("base").unwrap().render(()).unwrap();
        assert_eq!(rendered, "https://example.com/blog");
    }

    #[test]
    fn extra_config_fields_are_exposed() {
        let mut config = Config::default();
        config.extra.insert(
            "theme".to_string(),
            JsonValue::String("solarized".to_string()),
        );

        let mut env = environment(&config).unwrap();
        env.add_template("theme", "{{ config.theme }}").unwrap();

        let rendered = env.get_template("theme").unwrap().render(()).unwrap();
        assert_eq!(rendered, "solarized");
    }

    #[test]
    fn base_path_extracts_path_from_base_url() {
        let config = Config {
            base_url: "https://vrypan.net/blog/".to_string(),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("path", "{{ base_path }}").unwrap();

        let rendered = env.get_template("path").unwrap().render(()).unwrap();
        assert_eq!(rendered, "/blog");
    }

    #[test]
    fn base_path_empty_for_root_url() {
        let config = Config {
            base_url: "https://vrypan.net/".to_string(),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("path", "{{ base_path }}").unwrap();

        let rendered = env.get_template("path").unwrap().render(()).unwrap();
        assert_eq!(rendered, "");
    }

    #[test]
    fn base_path_handles_nested_paths() {
        let config = Config {
            base_url: "https://example.com/foo/bar/".to_string(),
            ..Default::default()
        };
        let mut env = environment(&config).unwrap();
        env.add_template("path", "{{ base_path }}").unwrap();

        let rendered = env.get_template("path").unwrap().render(()).unwrap();
        assert_eq!(rendered, "/foo/bar");
    }

    #[test]
    fn html_templates_escape_quotes_in_attributes() {
        let config = Config::default();
        let mut env = environment(&config).unwrap();
        env.add_template("t.html", r#"<meta content="{{ title }}">"#)
            .unwrap();
        let ctx = minijinja::context! { title => r#"Say "hello""# };
        let rendered = env.get_template("t.html").unwrap().render(ctx).unwrap();
        assert_eq!(rendered, r#"<meta content="Say &quot;hello&quot;">"#);
    }

    #[test]
    fn non_html_templates_do_not_escape() {
        let config = Config::default();
        let mut env = environment(&config).unwrap();
        env.add_template("t.xml", r#"<title>{{ title }}</title>"#)
            .unwrap();
        let ctx = minijinja::context! { title => r#"Say "hello""# };
        let rendered = env.get_template("t.xml").unwrap().render(ctx).unwrap();
        assert_eq!(rendered, r#"<title>Say "hello"</title>"#);
    }
}
