use crate::MailParseError;

enum DateParseState {
    DayOfWeek,
    Date,
    Month,
    Year,
    Hour,
    Minute,
    Second,
    Timezone,
    End,
}

fn days_in_month(month: i64, year: i64) -> i64 {
    match month {
        0 | 2 | 4 | 6 | 7 | 9 | 11 => 31,
        3 | 5 | 8 | 10 => 30,
        1 => {
            if (year % 400) == 0 {
                29
            } else if (year % 100) == 0 {
                28
            } else if (year % 4) == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn seconds_to_date(year: i64, month: i64, day: i64) -> i64 {
    let mut result: i64 = 0;
    for y in 1970..2001 {
        if y == year {
            break;
        }
        result += 86400 * 365;
        if (y % 4) == 0 {
            result += 86400;
        }
    }
    let mut y = 2001;
    while y < year {
        if year - y >= 400 {
            result += (86400 * 365 * 400) + (86400 * 97);
            y += 400;
            continue;
        }
        if year - y >= 100 {
            result += (86400 * 365 * 100) + (86400 * 24);
            y += 100;
            continue;
        }
        if year - y >= 4 {
            result += (86400 * 365 * 4) + (86400);
            y += 4;
            continue;
        }
        result += 86400 * 365;
        y += 1;
    }
    for m in 0..month {
        result += 86400 * days_in_month(m, year)
    }
    result + 86400 * (day - 1)
}

/// Convert a date field from an email header into a UNIX epoch timestamp.
/// This function handles the most common formatting of date fields found in
/// email headers. It may fail to parse some of the more creative formattings.
///
/// # Examples
/// ```
///     use mailparse::dateparse;
///     assert_eq!(dateparse("Sun, 02 Oct 2016 07:06:22 -0700 (PDT)").unwrap(), 1475417182);
///     assert_eq!(dateparse("Sun Oct   2 07:06:22 PDT 2016").unwrap(), 1475417182);
/// ```
pub fn dateparse(date: &str) -> Result<i64, MailParseError> {
    let mut result = 0;
    let mut year = 0;
    let mut month = 0;
    let mut day_of_month = 0;

    let date_parts =
        if date.contains(',')
            { [DateParseState::DayOfWeek, DateParseState::Date, DateParseState::Month, DateParseState::Year, DateParseState::Hour, DateParseState::Minute, DateParseState::Second, DateParseState::Timezone, DateParseState::End] }
        else
            { [DateParseState::DayOfWeek, DateParseState::Month, DateParseState::Date, DateParseState::Hour, DateParseState::Minute, DateParseState::Second, DateParseState::Timezone, DateParseState::Year, DateParseState::End] }
        ;
    let mut state_iter = date_parts.iter();

    let mut state = state_iter.next().unwrap();
    for tok in date.split(|c| c == ' ' || c == ':') {
        if tok.is_empty() {
            continue;
        }
        match state {
            DateParseState::DayOfWeek => {
                state = state_iter.next().unwrap();
                continue;
            },
            DateParseState::Date => {
                if let Ok(v) = tok.parse::<u8>() {
                    if !(1..=31).contains(&v) {
                        return Err(MailParseError::Generic("Invalid day"));
                    }
                    day_of_month = v;
                    state = state_iter.next().unwrap();
                };
                continue;
            }
            DateParseState::Month => {
                month = match tok.to_uppercase().as_str() {
                    "JAN" | "JANUARY" => 0,
                    "FEB" | "FEBRUARY" => 1,
                    "MAR" | "MARCH" => 2,
                    "APR" | "APRIL" => 3,
                    "MAY" => 4,
                    "JUN" | "JUNE" => 5,
                    "JUL" | "JULY" => 6,
                    "AUG" | "AUGUST" => 7,
                    "SEP" | "SEPTEMBER" => 8,
                    "OCT" | "OCTOBER" => 9,
                    "NOV" | "NOVEMBER" => 10,
                    "DEC" | "DECEMBER" => 11,
                    _ => return Err(MailParseError::Generic("Unrecognized month")),
                };
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::Year => {
                year = match tok.parse::<u32>() {
                    Ok(v) if v < 70 => 2000 + v,
                    Ok(v) if v < 100 => 1900 + v,
                    Ok(v) if v < 1970 => return Err(MailParseError::Generic("Disallowed year")),
                    Ok(v) => v,
                    Err(_) => return Err(MailParseError::Generic("Invalid year")),
                };
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::Hour => {
                let hour = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err(MailParseError::Generic("Invalid hour")),
                };
                result += 3600 * i64::from(hour);
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::Minute => {
                let minute = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err(MailParseError::Generic("Invalid minute")),
                };
                result += 60 * i64::from(minute);
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::Second => {
                let second = match tok.parse::<u8>() {
                    Ok(v) => v,
                    Err(_) => return Err(MailParseError::Generic("Invalid second")),
                };
                result += i64::from(second);
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::Timezone => {
                let (tz, tz_sign) = match tok.parse::<i32>() {
                    Ok(v) if !(-2400..=2400).contains(&v) => {
                        return Err(MailParseError::Generic("Invalid timezone"))
                    }
                    Ok(v) if v < 0 => (-v, -1),
                    Ok(v) => (v, 1),
                    Err(_) => {
                        dbg!(tok);
                        match tok.to_uppercase().replace(['(', ')'], "").as_str() {
                            // This list taken from IETF RFC 822
                            // Added more common abbreviations from
                            // https://en.wikipedia.org/wiki/List_of_time_zone_abbreviations
                            "M" => (1200, -1),
                            "BEST" => (1100, -1),
                            "HST" => (1000, -1),
                            "AKST" => (900, -1),
                            "PST" => (800, -1),
                            "MST" | "PDT" => (700, -1),
                            "CST" | "MDT" => (600, -1),
                            "EST" | "CDT" => (500, -1),
                            "AST" | "EDT" => (400, -1),
                            "NST" => (330, -1),
                            "A" => (100, -1),
                            "UTC" | "WET" | "UT" | "GMT" | "Z" => (0, 1),
                            "CET" | "N" => (100, 1),
                            "CEST" | "EET" => (200, 1),
                            "MSK" => (300, 1),
                            "IST" => (530, 1),
                            "AWST" => (800, 1),
                            "ACST" => (930, 1),
                            "AEST" => (1000, 1),
                             "Y" => (1200, 1),
                            _ => return Err(MailParseError::Generic("Invalid timezone")),
                        }
                    }
                };
                let tz_hours = tz / 100;
                let tz_mins = tz % 100;
                let tz_delta = (tz_hours * 3600) + (tz_mins * 60);
                if tz_sign < 0 {
                    result += i64::from(tz_delta);
                } else {
                    result -= i64::from(tz_delta);
                }
                state = state_iter.next().unwrap();
                continue;
            }
            DateParseState::End => { break; }
        }
    }
    result += seconds_to_date(i64::from(year), i64::from(month), i64::from(day_of_month));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dates() {
        assert_eq!(
            dateparse("Sun, 25 Sep 2016 18:36:33 -0400").unwrap(),
            1474842993
        );
        assert_eq!(
            dateparse("Fri, 01 Jan 2100 11:12:13 +0000").unwrap(),
            4102485133
        );
        assert_eq!(
            dateparse("Fri, 31 Dec 2100 00:00:00 +0000").unwrap(),
            4133894400
        );
        assert_eq!(
            dateparse("Fri, 31 Dec 2399 00:00:00 +0000").unwrap(),
            13569379200
        );
        assert_eq!(
            dateparse("Fri, 31 Dec 2400 00:00:00 +0000").unwrap(),
            13601001600
        );
        assert_eq!(dateparse("17 Sep 2016 16:05:38 -1000").unwrap(), 1474164338);
        assert_eq!(
            dateparse("Fri, 30 Nov 2012 20:57:23 GMT").unwrap(),
            1354309043
        );

        // Day cannot be zero.
        assert!(dateparse("Wed, 0 Jan 1970 00:00:00 +0000").is_err());

        // Regression test for integer overflow on invalid timezone.
        assert!(dateparse("Thu, 1 Jan 1970 00:00:00 +2147483647").is_err());
    }
}
