use minijinja::value::Value;
use minijinja::{Environment, Error, ErrorKind};
use time::OffsetDateTime;
use time::format_description::modifier::{
    Day, Hour, Minute, Month, MonthRepr, OffsetHour, OffsetMinute, Period, Second, Weekday,
    WeekdayRepr, Year, YearRepr,
};
use time::format_description::well_known::Rfc3339;
use time::format_description::{Component, OwnedFormatItem};

pub fn register(env: &mut Environment<'static>) -> Result<(), Error> {
    env.add_filter("format_date", format_date);
    Ok(())
}

fn format_date(value: Value, format: String) -> Result<Value, Error> {
    let raw = match value.as_str() {
        Some(text) if !text.trim().is_empty() => text,
        Some(_) => return Ok(Value::from(String::new())),
        None => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "format_date filter expects a string input",
            ));
        }
    };

    let datetime = OffsetDateTime::parse(raw, &Rfc3339).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "format_date filter requires RFC3339 datetime strings (e.g. post.date_iso); got '{raw}': {err}"
            ),
        )
    })?;

    let format_items = translate_strftime(&format)?;
    let formatted = datetime.format(&format_items).map_err(|err| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("failed to format datetime: {err}"),
        )
    })?;

    Ok(Value::from(formatted))
}

fn translate_strftime(format: &str) -> Result<Vec<OwnedFormatItem>, Error> {
    use OwnedFormatItem as Item;

    let mut items: Vec<Item> = Vec::new();
    let mut literal = String::new();
    let mut chars = format.chars();

    let flush_literal = |items: &mut Vec<Item>, buf: &mut String| {
        if !buf.is_empty() {
            items.push(Item::Literal(buf.clone().into_bytes().into_boxed_slice()));
            buf.clear();
        }
    };

    while let Some(ch) = chars.next() {
        if ch == '%' {
            let Some(code) = chars.next() else {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "format_date filter received a dangling '%'",
                ));
            };

            flush_literal(&mut items, &mut literal);

            match code {
                '%' => items.push(Item::Literal("%".as_bytes().into())),
                'Y' => items.push(Component::Year(Year::default()).into()),
                'y' => {
                    let mut year = Year::default();
                    year.repr = YearRepr::LastTwo;
                    items.push(Component::Year(year).into());
                }
                'm' => items.push(Component::Month(Month::default()).into()),
                'b' => {
                    let mut month = Month::default();
                    month.repr = MonthRepr::Short;
                    items.push(Component::Month(month).into());
                }
                'B' => {
                    let mut month = Month::default();
                    month.repr = MonthRepr::Long;
                    items.push(Component::Month(month).into());
                }
                'd' => items.push(Component::Day(Day::default()).into()),
                'H' => items.push(Component::Hour(Hour::default()).into()),
                'I' => {
                    let mut hour = Hour::default();
                    hour.is_12_hour_clock = true;
                    items.push(Component::Hour(hour).into());
                }
                'M' => items.push(Component::Minute(Minute::default()).into()),
                'S' => items.push(Component::Second(Second::default()).into()),
                'a' => {
                    let mut weekday = Weekday::default();
                    weekday.repr = WeekdayRepr::Short;
                    items.push(Component::Weekday(weekday).into());
                }
                'A' => items.push(Component::Weekday(Weekday::default()).into()),
                'p' => {
                    let mut period = Period::default();
                    period.is_uppercase = true;
                    items.push(Component::Period(period).into());
                }
                'P' => {
                    let mut period = Period::default();
                    period.is_uppercase = false;
                    items.push(Component::Period(period).into());
                }
                'R' => {
                    items.extend(translate_strftime("%H:%M")?);
                }
                'T' => {
                    items.extend(translate_strftime("%H:%M:%S")?);
                }
                'F' => {
                    items.extend(translate_strftime("%Y-%m-%d")?);
                }
                'z' => {
                    let mut hour = OffsetHour::default();
                    hour.sign_is_mandatory = true;
                    items.push(Component::OffsetHour(hour).into());
                    items.push(Component::OffsetMinute(OffsetMinute::default()).into());
                }
                'Z' => {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        "format_date filter does not support %Z timezone names",
                    ));
                }
                other => {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        format!("format_date filter does not support %{other}"),
                    ));
                }
            }
        } else {
            literal.push(ch);
        }
    }

    flush_literal(&mut items, &mut literal);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rfc3339_datetime() {
        let value = Value::from("2025-10-01T12:08:00+02:00");
        let rendered = format_date(value, "%Y-%m-%d".to_string()).unwrap();
        assert_eq!(rendered.as_str().unwrap(), "2025-10-01");
    }

    #[test]
    fn formats_using_common_strftime_tokens() {
        let value = Value::from("2025-10-01T12:08:00+02:00");
        let rendered = format_date(value, "%a, %d %B %Y %H:%M".to_string()).unwrap();
        assert_eq!(rendered.as_str().unwrap(), "Wed, 01 October 2025 12:08");
    }

    #[test]
    fn rejects_non_string_value() {
        let value = Value::from(42);
        let err = format_date(value, "%Y".to_string()).unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::InvalidOperation));
    }

    #[test]
    fn rejects_non_rfc3339_input() {
        let value = Value::from("not-a-date");
        let err = format_date(value, "%Y".to_string()).unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::InvalidOperation));
    }
}
