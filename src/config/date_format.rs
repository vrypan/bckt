use anyhow::{Result, bail};
use time::format_description::{self, FormatItem};

pub fn parse_format(value: &str) -> Result<()> {
    if value == "RFC3339" {
        return Ok(());
    }

    let items = format_description::parse(value)?;
    if !items
        .iter()
        .any(|item| matches!(item, FormatItem::Component(_)))
    {
        bail!("date_format must contain at least one date or time component");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_rfc3339() {
        assert!(parse_format("RFC3339").is_ok());
    }

    #[test]
    fn accept_valid_format() {
        assert!(parse_format("[year]-[month]-[day]").is_ok());
    }

    #[test]
    fn reject_empty_format() {
        assert!(parse_format("???").is_err());
    }
}
