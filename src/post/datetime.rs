use anyhow::{Result, bail};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

const NAIVE_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

const PREFIX_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year repr:last_two][month][day]");

/// Parses a front matter date: RFC3339, `YYYY-MM-DD HH:MM:SS`, or the latter
/// followed by a UTC offset (`+03:00`, `+0300`, `UTC`, `Z`).
pub fn parse_datetime(value: &str) -> Option<OffsetDateTime> {
    if let Ok(dt) = OffsetDateTime::parse(value, &Rfc3339) {
        return Some(dt);
    }

    if let Ok(naive) = PrimitiveDateTime::parse(value, NAIVE_FORMAT) {
        return Some(naive.assume_offset(UtcOffset::UTC));
    }

    if let Some((main, offset_part)) = value.rsplit_once(' ')
        && let Ok(naive) = PrimitiveDateTime::parse(main, NAIVE_FORMAT)
        && let Ok(offset) = parse_offset(offset_part)
    {
        return Some(naive.assume_offset(offset));
    }

    None
}

/// Parses a UTC offset in `+HH:MM`, `+HHMM`, `+HH:MM:SS`, `UTC`, or `Z` form.
pub fn parse_offset(value: &str) -> Result<UtcOffset> {
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

/// Formats the `YYMMDD` prefix used in post directory names.
pub fn date_prefix(dt: &OffsetDateTime) -> String {
    dt.format(PREFIX_FORMAT).expect("date prefix formats")
}

/// Formats a timestamp as RFC3339, the canonical front matter date form.
pub fn format_rfc3339(dt: &OffsetDateTime) -> String {
    dt.format(&Rfc3339).expect("rfc3339 formats")
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn parse_datetime_accepts_rfc3339() {
        let parsed = parse_datetime("2024-01-15T12:00:00Z").expect("parses");
        assert_eq!(parsed, datetime!(2024-01-15 12:00:00 UTC));
    }

    #[test]
    fn parse_datetime_accepts_naive_as_utc() {
        let parsed = parse_datetime("2024-01-15 12:00:00").expect("parses");
        assert_eq!(parsed, datetime!(2024-01-15 12:00:00 UTC));
    }

    #[test]
    fn parse_datetime_accepts_trailing_offset() {
        let parsed = parse_datetime("2024-01-15 12:00:00 +03:00").expect("parses");
        assert_eq!(parsed, datetime!(2024-01-15 12:00:00 +3));
        let compact = parse_datetime("2024-01-15 12:00:00 +0300").expect("parses");
        assert_eq!(compact, datetime!(2024-01-15 12:00:00 +3));
    }

    #[test]
    fn parse_datetime_rejects_garbage() {
        assert!(parse_datetime("not a date").is_none());
    }

    #[test]
    fn parse_offset_rejects_short_values() {
        assert!(parse_offset("+3").is_err());
    }

    #[test]
    fn date_prefix_uses_two_digit_year() {
        assert_eq!(date_prefix(&datetime!(2024-01-05 00:00:00 UTC)), "240105");
    }

    #[test]
    fn format_rfc3339_round_trips() {
        let dt = datetime!(2024-01-15 12:00:00 UTC);
        assert_eq!(parse_datetime(&format_rfc3339(&dt)), Some(dt));
    }
}
