//! Unix time to London local time, with UK daylight saving (GMT/BST).
//!
//! `no_std` and allocation-free. Calendar maths from Howard Hinnant's
//! `days_from_civil` / `civil_from_days` algorithms.

const SECS_PER_MIN: u64 = 60;
const SECS_PER_HOUR: u64 = 3600;
const SECS_PER_DAY: u64 = 86400;

/// Converts a Unix timestamp (seconds) into London local (hour, minute, second).
pub fn unix_to_london_time(unix_sec: u64) -> (u32, u32, u32) {
    let days_since_epoch = unix_sec / SECS_PER_DAY;
    let day_sec = unix_sec % SECS_PER_DAY;

    let hour = (day_sec / SECS_PER_HOUR) as u32;
    let minute = ((day_sec % SECS_PER_HOUR) / SECS_PER_MIN) as u32;
    let second = (day_sec % SECS_PER_MIN) as u32;

    let (year, month, day, _weekday) = civil_from_days(days_since_epoch as i64);
    let offset_hours = if is_uk_dst(year, month, day, hour) {
        1
    } else {
        0
    };

    ((hour + offset_hours) % 24, minute, second)
}

/// Whether the UK is on BST at the given UTC date and hour.
///
/// BST runs from 01:00 UTC on the last Sunday of March to 01:00 UTC on the
/// last Sunday of October.
fn is_uk_dst(year: i32, month: u32, day: u32, hour_utc: u32) -> bool {
    match month {
        4..=9 => true,
        3 => {
            let last_sun = last_sunday_of_month(year, 3);
            day > last_sun || (day == last_sun && hour_utc >= 1)
        }
        10 => {
            let last_sun = last_sunday_of_month(year, 10);
            day < last_sun || (day == last_sun && hour_utc < 1)
        }
        _ => false,
    }
}

/// Day of month of the last Sunday, for a month with 31 days (March or October).
fn last_sunday_of_month(year: i32, month: u32) -> u32 {
    let (_, _, _, weekday_of_31st) = civil_from_days(days_from_civil(year, month, 31));
    31 - weekday_of_31st
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe as i64 - 719468
}

/// Returns (year, month, day, weekday) where weekday 0 = Sunday.
fn civil_from_days(z: i64) -> (i32, u32, u32, u32) {
    let weekday = (z + 4).rem_euclid(7) as u32; // 1970-01-01 was a Thursday
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d, weekday)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values generated with Python's zoneinfo (Europe/London).
    #[test]
    fn winter_is_gmt() {
        assert_eq!(unix_to_london_time(1768519800), (23, 30, 0)); // 2026-01-15T23:30Z
    }

    #[test]
    fn summer_is_bst() {
        assert_eq!(unix_to_london_time(1782907200), (13, 0, 0)); // 2026-07-01T12:00Z
    }

    #[test]
    fn bst_wraps_past_midnight() {
        assert_eq!(unix_to_london_time(1782862200), (0, 30, 0)); // 2026-06-30T23:30Z
    }

    #[test]
    fn spring_forward_2026() {
        assert_eq!(unix_to_london_time(1774745999), (0, 59, 59)); // 2026-03-29T00:59:59Z
        assert_eq!(unix_to_london_time(1774746000), (2, 0, 0)); // 2026-03-29T01:00:00Z
    }

    #[test]
    fn fall_back_2026() {
        assert_eq!(unix_to_london_time(1792889999), (1, 59, 59)); // 2026-10-25T00:59:59Z
        assert_eq!(unix_to_london_time(1792890000), (1, 0, 0)); // 2026-10-25T01:00:00Z
    }

    #[test]
    fn spring_forward_2027() {
        assert_eq!(unix_to_london_time(1806195600), (2, 0, 0)); // 2027-03-28T01:00:00Z
    }

    #[test]
    fn leap_day() {
        assert_eq!(unix_to_london_time(1709208000), (12, 0, 0)); // 2024-02-29T12:00Z
    }

    #[test]
    fn last_sundays() {
        assert_eq!(last_sunday_of_month(2026, 3), 29);
        assert_eq!(last_sunday_of_month(2026, 10), 25);
        assert_eq!(last_sunday_of_month(2027, 3), 28);
        assert_eq!(last_sunday_of_month(2027, 10), 31); // 31st is itself a Sunday
    }

    #[test]
    fn civil_round_trip() {
        for days in [-1, 0, 1, 10_000, 19_782, 20_000, 30_000] {
            let (y, m, d, _) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1, 4)); // Thursday
    }
}
