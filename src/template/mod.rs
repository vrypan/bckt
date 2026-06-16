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

    filters::register(&mut env)?;

    Ok(env)
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
