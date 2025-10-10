use anyhow::{Context, Result, bail};
use time::UtcOffset;

pub fn parse_timezone(value: &str) -> Result<UtcOffset> {
    if value.eq_ignore_ascii_case("UTC") || value.eq_ignore_ascii_case("Z") {
        return Ok(UtcOffset::UTC);
    }

    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let sign_char = chars
        .next()
        .with_context(|| format!("default_timezone '{}' is empty", value))?;
    let sign = match sign_char {
        '+' => 1,
        '-' => -1,
        _ => bail!("default_timezone must start with '+' or '-'"),
    };

    let remainder = chars.as_str();
    let mut parts = remainder.split(':');
    let hours_str = parts
        .next()
        .with_context(|| format!("default_timezone '{}' missing hour component", value))?;
    let minutes_str = parts.next().unwrap_or("0");
    let seconds_str = parts.next().unwrap_or("0");

    if parts.next().is_some() {
        bail!("default_timezone '{}' has too many components", value);
    }

    let hours: i8 = hours_str
        .parse()
        .with_context(|| format!("default_timezone '{}' hour component invalid", value))?;
    let minutes: i8 = minutes_str
        .parse()
        .with_context(|| format!("default_timezone '{}' minute component invalid", value))?;
    let seconds: i8 = seconds_str
        .parse()
        .with_context(|| format!("default_timezone '{}' second component invalid", value))?;

    UtcOffset::from_hms(sign * hours, sign * minutes, sign * seconds)
        .with_context(|| format!("default_timezone '{}' out of range", value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_utc_variants() {
        assert!(parse_timezone("UTC").is_ok());
        assert!(parse_timezone("Z").is_ok());
        assert!(parse_timezone("utc").is_ok());
    }

    #[test]
    fn parse_positive_offset() {
        let offset = parse_timezone("+05:30").unwrap();
        assert_eq!(offset.whole_hours(), 5);
    }

    #[test]
    fn parse_negative_offset() {
        let offset = parse_timezone("-08:00").unwrap();
        assert_eq!(offset.whole_hours(), -8);
    }

    #[test]
    fn reject_invalid_format() {
        assert!(parse_timezone("Mars/Station").is_err());
        assert!(parse_timezone("invalid").is_err());
    }
}
