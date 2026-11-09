//! `Retry-After` values: delay-seconds (`120`) or an HTTP date in the IMF-fixdate
//! form every server must send (`Sun, 06 Nov 1994 08:49:37 GMT`, RFC 9110 §5.6.7).
//! Hand-parsed: one fixed format does not justify a date crate.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Parses a `Retry-After` header value into a wait from `now`. A date in the past
/// waits zero; anything unparsable is `None` (caller falls back to its backoff).
pub fn parse_retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let at = UNIX_EPOCH + Duration::from_secs(parse_imf_fixdate(value)?);
    Some(at.duration_since(now).unwrap_or(Duration::ZERO))
}

/// `Sun, 06 Nov 1994 08:49:37 GMT` → seconds since the Unix epoch.
fn parse_imf_fixdate(value: &str) -> Option<u64> {
    let (_weekday, rest) = value.split_once(", ")?;
    let mut parts = rest.split(' ');
    let day: u64 = parts.next()?.parse().ok()?;
    let month_name = parts.next()?;
    let month = MONTHS.iter().position(|m| *m == month_name)? as u64 + 1;
    let year: u64 = parts.next()?.parse().ok()?;
    let mut clock = parts.next()?.split(':');
    let hour: u64 = clock.next()?.parse().ok()?;
    let minute: u64 = clock.next()?.parse().ok()?;
    let second: u64 = clock.next()?.parse().ok()?;
    if parts.next()? != "GMT" || parts.next().is_some() || clock.next().is_some() {
        return None;
    }
    if !(1..=31).contains(&day) || year < 1970 || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm),
/// for years ≥ 1970 only.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12; // March = 0
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_seconds() {
        let now = SystemTime::now();
        assert_eq!(
            parse_retry_after("120", now),
            Some(Duration::from_secs(120))
        );
        assert_eq!(parse_retry_after(" 0 ", now), Some(Duration::ZERO));
    }

    #[test]
    fn http_date_relative_to_now() {
        // 1994-11-06 08:49:37 UTC = 784111777.
        assert_eq!(
            parse_imf_fixdate("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(784_111_777)
        );
        let now = UNIX_EPOCH + Duration::from_secs(784_111_777 - 30);
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:49:37 GMT", now),
            Some(Duration::from_secs(30))
        );
        // Leap day + date already past → zero wait.
        assert_eq!(
            parse_imf_fixdate("Thu, 29 Feb 2024 00:00:00 GMT"),
            Some(1_709_164_800)
        );
        let later = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        assert_eq!(
            parse_retry_after("Thu, 29 Feb 2024 00:00:00 GMT", later),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn missing_or_garbage_is_none() {
        let now = SystemTime::now();
        for value in [
            "",
            "soon",
            "-5",
            "Sun, 06 Nov 1994 08:49:37 PST",
            "06 Nov 1994",
        ] {
            assert_eq!(parse_retry_after(value, now), None, "{value}");
        }
    }
}
