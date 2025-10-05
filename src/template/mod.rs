mod filters;

use anyhow::Result;
use minijinja::value::Value;
use minijinja::{Environment, ErrorKind};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::config::Config;

pub fn environment(config: &Config) -> Result<Environment<'static>> {
    let mut env = Environment::new();
    env.add_global("config", Value::from_serialize(config));
    env.add_global(
        "base_url",
        Value::from_safe_string(normalize_base_url(&config.base_url)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;

    #[test]
    fn config_available_in_templates() {
        let mut config = Config::default();
        config.title = Some("Bucket".to_string());
        let mut env = environment(&config).unwrap();
        env.add_template("greet", "{{ config.title }}").unwrap();

        let rendered = env.get_template("greet").unwrap().render(()).unwrap();
        assert_eq!(rendered, "Bucket");
    }

    #[test]
    fn now_helper_uses_config_format() {
        let mut config = Config::default();
        config.date_format = "[year]".to_string();
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
        let mut config = Config::default();
        config.base_url = "https://example.com/blog".to_string();
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
}

fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed.trim_end_matches('/').to_string()
}
