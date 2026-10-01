//! Cron expressions with the semantics of `node-cron` 4.x (the version the Electron app ships,
//! 4.6.0): `validate`, `TimeMatcher.match` / `getNextMatch`, and the runner's heartbeat plan.
//!
//! * 5 or 6 fields; with 5 the seconds field is `0`. Fields are separated by single spaces
//!   (runs of whitespace collapse to one space; tabs alone are illegal characters).
//! * Nicknames `@yearly` `@annually` `@monthly` `@weekly` `@daily` `@midnight` `@hourly`.
//! * `*`, lists, ranges (`a-b`, wrapping ranges such as `22-2`), steps (`*/n`, `a-b/n`), month
//!   and weekday names (full or three-letter, any case), `?` in day-of-month / day-of-week.
//! * Day of month: `L`, `L-n`, `nW`, `LW`. Day of week: `0`–`7` (7 = Sunday), `nL` (last
//!   weekday n of the month), `n#k` (k-th weekday n).
//! * Unlike POSIX cron, day-of-month and day-of-week must **both** match.
//! * Times are evaluated in a timezone (the scheduler uses the system's local timezone, as the
//!   TS resolved `Intl.DateTimeFormat().resolvedOptions().timeZone`). A local time that does not
//!   exist (DST gap) never matches; an ambiguous one (DST overlap) matches its first occurrence.

use chrono::{DateTime, Datelike, LocalResult, NaiveDate, TimeZone, Timelike};

/// One value of a converted field: a number or a special token (`L`, `15W`, `5#2`, …).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Tok {
    Int(i64),
    Str(String),
}

const NICKNAMES: &[(&str, &str)] = &[
    ("@yearly", "0 0 1 1 *"),
    ("@annually", "0 0 1 1 *"),
    ("@monthly", "0 0 1 * *"),
    ("@weekly", "0 0 * * 0"),
    ("@daily", "0 0 * * *"),
    ("@midnight", "0 0 * * *"),
    ("@hourly", "0 * * * *"),
];

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];
const SHORT_MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const WEEKDAYS: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];
const SHORT_WEEKDAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/// `{ min, max }` per field (second, minute, hour, day of month, month, day of week) used to
/// expand `*` and wrapping ranges.
const FIELD_BOUNDS: [(i64, i64); 6] = [(0, 59), (0, 59), (0, 23), (1, 31), (1, 12), (0, 6)];

/// Upper bound for values generated while expanding a range. Anything past it is out of range
/// for every field, so expansion stops there without changing validity.
const EXPANSION_CAP: i64 = 10_000;

/// `MAX_DAYS` of the matcher walk: give up after 100 years of days.
const MAX_DAYS: usize = 366 * 100;

fn resolve_nickname(expression: &str) -> String {
    let key = expression.trim().to_lowercase();
    NICKNAMES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| expression.to_string())
}

/// `str.replace(/\s{2,}/g, ' ').trim()`.
fn remove_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    for c in s.chars() {
        if c.is_whitespace() {
            run.push(c);
            continue;
        }
        flush_run(&mut out, &mut run);
        out.push(c);
    }
    flush_run(&mut out, &mut run);
    out.trim().to_string()
}

fn flush_run(out: &mut String, run: &mut String) {
    if run.chars().count() >= 2 {
        out.push(' ');
    } else {
        out.push_str(run);
    }
    run.clear();
}

/// `^[a-zA-Z0-9-*/,#? ]+$`.
fn has_only_allowed_chars(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '*' | '/' | ',' | '#' | '?' | ' ')
        })
}

/// `expression.replace(new RegExp(pattern, 'gi'), replacement)` for an ASCII pattern.
fn replace_ci(s: &str, pattern: &str, replacement: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(pos) = lower[i..].find(pattern) {
        out.push_str(&s[i..i + pos]);
        out.push_str(replacement);
        i += pos + pattern.len();
    }
    out.push_str(&s[i..]);
    out
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `parseInt` of an all-digit string, saturating instead of losing precision.
fn parse_digits(s: &str) -> i64 {
    s.parse::<i64>().unwrap_or(i64::MAX)
}

/// `^(\d+)-(\d+)(?:\/(\d+))?$`.
fn parse_range(token: &str) -> Option<(&str, &str, &str)> {
    let (range, step) = match token.split_once('/') {
        Some((r, s)) => {
            if !is_digits(s) {
                return None;
            }
            (r, s)
        }
        None => (token, "1"),
    };
    let (a, b) = range.split_once('-')?;
    (is_digits(a) && is_digits(b)).then_some((a, b, step))
}

fn expand_range(init: &str, end: &str, step_txt: &str, (min, max): (i64, i64)) -> String {
    let step = parse_digits(step_txt);
    if step < 1 {
        return format!("{init}-{end}/{step_txt}");
    }
    let first = parse_digits(init);
    let last = parse_digits(end);
    let mut numbers: Vec<String> = Vec::new();
    if first <= last {
        let mut i = first;
        while i <= last {
            numbers.push(i.to_string());
            if i > EXPANSION_CAP {
                break;
            }
            i = match i.checked_add(step) {
                Some(n) => n,
                None => break,
            };
        }
        return numbers.join(",");
    }
    let size = max - min + 1;
    let span = (last - first).rem_euclid(size);
    let mut offset = 0;
    while offset <= span {
        let mut value = first.saturating_add(offset);
        if value > max {
            value -= size;
        }
        numbers.push(value.to_string());
        offset = match offset.checked_add(step) {
            Some(n) => n,
            None => break,
        };
    }
    numbers.join(",")
}

fn is_l_offset(t: &str) -> bool {
    // /^l-\d{1,2}$/i
    let b = t.as_bytes();
    (b.len() == 3 || b.len() == 4)
        && (b[0] == b'l' || b[0] == b'L')
        && b[1] == b'-'
        && b[2..].iter().all(u8::is_ascii_digit)
}

fn is_digit_l(t: &str) -> bool {
    // /^[0-7]l$/i
    let b = t.as_bytes();
    b.len() == 2 && (b'0'..=b'7').contains(&b[0]) && (b[1] == b'l' || b[1] == b'L')
}

fn normalize_token(raw: &str) -> Tok {
    let token = raw.trim();
    if token.eq_ignore_ascii_case("l") {
        Tok::Str("L".into())
    } else if is_l_offset(token) || is_digit_l(token) || token.contains(['w', 'W']) {
        Tok::Str(token.to_uppercase())
    } else if token.contains('#') {
        Tok::Str(token.to_string())
    } else if is_digits(token) {
        Tok::Int(parse_digits(token))
    } else {
        Tok::Str(token.to_string())
    }
}

/// `convertExpression`: the six executable fields.
fn convert_expression(expression: &str) -> Option<Vec<Vec<Tok>>> {
    let mut fields: Vec<String> = remove_spaces(&resolve_nickname(expression))
        .split(' ')
        .map(str::to_string)
        .collect();
    if fields.len() == 5 {
        fields.insert(0, "0".into());
    }
    if fields.len() < 6 {
        return None;
    }
    if fields[3] == "?" {
        fields[3] = "*".into();
    }
    if fields[5] == "?" {
        fields[5] = "*".into();
    }
    for (i, name) in MONTHS.iter().enumerate() {
        fields[4] = replace_ci(&fields[4], name, &(i + 1).to_string());
    }
    for (i, name) in SHORT_MONTHS.iter().enumerate() {
        fields[4] = replace_ci(&fields[4], name, &(i + 1).to_string());
    }
    for (i, name) in WEEKDAYS.iter().enumerate() {
        fields[5] = replace_ci(&fields[5], name, &i.to_string());
    }
    for (i, name) in SHORT_WEEKDAYS.iter().enumerate() {
        fields[5] = replace_ci(&fields[5], name, &i.to_string());
    }
    let asterisk = ["0-59", "0-59", "0-23", "1-31", "1-12", "0-6"];
    for (i, rep) in asterisk.iter().enumerate() {
        fields[i] = fields[i]
            .split(',')
            .map(|t| {
                if t.contains('*') {
                    t.replacen('*', rep, 1)
                } else {
                    t.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",");
    }
    for (i, field) in fields.iter_mut().enumerate() {
        let bounds = FIELD_BOUNDS.get(i).copied().unwrap_or((0, 59));
        *field = field
            .split(',')
            .map(|t| match parse_range(t.trim()) {
                Some((a, b, s)) => expand_range(a, b, s, bounds),
                None => t.to_string(),
            })
            .collect::<Vec<_>>()
            .join(",");
    }
    let mut out: Vec<Vec<Tok>> = fields
        .iter()
        .map(|f| f.split(',').map(normalize_token).collect())
        .collect();
    let mut weekdays: Vec<Tok> = Vec::new();
    for t in out[5].drain(..) {
        let t = match t {
            Tok::Int(7) => Tok::Int(0),
            Tok::Str(s) if s.starts_with('7') => Tok::Str(format!("0{}", &s[1..])),
            other => other,
        };
        if !weekdays.contains(&t) {
            weekdays.push(t);
        }
    }
    out[5] = weekdays;
    Some(out)
}

/// `isValidExpression`: every value is an integer within `[min, max]`.
fn all_ints_within(values: &[&Tok], min: i64, max: i64) -> bool {
    values
        .iter()
        .all(|t| matches!(t, Tok::Int(n) if *n >= min && *n <= max))
}

/// `^(\d{1,2}|L)W$` (the validator uses it case-insensitively; tokens are upper-cased by then).
fn w_token_target(t: &str) -> Option<&str> {
    let upper = t.strip_suffix('W').or_else(|| t.strip_suffix('w'))?;
    if upper.eq_ignore_ascii_case("l") || (is_digits(upper) && upper.len() <= 2) {
        Some(upper)
    } else {
        None
    }
}

/// `^L-(\d{1,2})$` (case-insensitive in the validator).
fn l_offset_value(t: &str) -> Option<i64> {
    is_l_offset(t).then(|| parse_digits(&t[2..]))
}

fn is_invalid_day_of_month(values: &[Tok]) -> bool {
    let kept: Vec<&Tok> = values
        .iter()
        .filter(|t| {
            let Tok::Str(s) = t else { return true };
            if s == "L" {
                return false;
            }
            if let Some(target) = w_token_target(s) {
                if target.eq_ignore_ascii_case("l") {
                    return false;
                }
                let n = parse_digits(target);
                return !(1..=31).contains(&n);
            }
            if let Some(n) = l_offset_value(s) {
                return !(1..=30).contains(&n);
            }
            true
        })
        .collect();
    !all_ints_within(&kept, 1, 31)
}

fn has_invalid_w_modifier(raw_day_of_month: &str) -> bool {
    if !raw_day_of_month.contains(['w', 'W']) {
        return false;
    }
    raw_day_of_month.split(',').any(|token| {
        let v = token.trim();
        v.contains(['w', 'W']) && w_token_target(v).is_none()
    })
}

/// `^([0-7])#([1-5])$`.
fn nth_weekday(t: &str) -> Option<(u32, u32)> {
    let b = t.as_bytes();
    (b.len() == 3 && (b'0'..=b'7').contains(&b[0]) && b[1] == b'#' && (b'1'..=b'5').contains(&b[2]))
        .then(|| (u32::from(b[0] - b'0') % 7, u32::from(b[2] - b'0')))
}

/// `^([0-7])L$` (case-insensitive), 7 → 0.
fn last_weekday(t: &str) -> Option<u32> {
    is_digit_l(t).then(|| u32::from(t.as_bytes()[0] - b'0') % 7)
}

fn is_invalid_week_day(values: &[Tok]) -> bool {
    let kept: Vec<&Tok> = values
        .iter()
        .filter(|t| match t {
            Tok::Str(s) => nth_weekday(s).is_none() && !(is_digit_l(s) && s.ends_with('L')),
            Tok::Int(_) => true,
        })
        .collect();
    !all_ints_within(&kept, 0, 7)
}

fn max_days_in_month(month: i64) -> i64 {
    match month {
        2 => 29,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_impossible_day_of_month(days: &[Tok], months: &[Tok]) -> bool {
    let mut day_values = Vec::new();
    for d in days {
        match d {
            Tok::Int(n) => day_values.push(*n),
            Tok::Str(_) => return false,
        }
    }
    !months.iter().any(|m| match m {
        Tok::Int(m) => day_values.iter().any(|d| *d <= max_days_in_month(*m)),
        Tok::Str(_) => false,
    })
}

/// `cron.validate(expression)`.
pub fn validate(expression: &str) -> bool {
    CronExpression::parse(expression).is_ok()
}

/// A parsed, validated cron expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpression {
    pattern: String,
    fields: Vec<Vec<Tok>>,
    seconds: Vec<u32>,
    minutes: Vec<u32>,
    hours: Vec<u32>,
}

fn ints_sorted(values: &[Tok]) -> Vec<u32> {
    let mut v: Vec<u32> = values
        .iter()
        .filter_map(|t| match t {
            Tok::Int(n) => u32::try_from(*n).ok(),
            Tok::Str(_) => None,
        })
        .collect();
    v.sort_unstable();
    v
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|d| d.pred_opt())
        .map_or(31, |d| d.day())
}

fn weekday_of(year: i32, month: u32, day: u32) -> Option<u32> {
    NaiveDate::from_ymd_opt(year, month, day).map(|d| d.weekday().num_days_from_sunday())
}

fn nearest_weekday(year: i32, month: u32, target: i64) -> i64 {
    let last = i64::from(last_day_of_month(year, month));
    if target < 1 || target > last {
        return -1;
    }
    match weekday_of(year, month, target as u32) {
        Some(6) => {
            if target == 1 {
                target + 2
            } else {
                target - 1
            }
        }
        Some(0) => {
            if target == last {
                target - 2
            } else {
                target + 1
            }
        }
        _ => target,
    }
}

fn matches_day_of_month(field: &[Tok], year: i32, month: u32, day: u32) -> bool {
    let last = last_day_of_month(year, month);
    field.iter().any(|value| match value {
        Tok::Int(n) => *n == i64::from(day),
        Tok::Str(s) => {
            if s == "L" && day == last {
                return true;
            }
            // Case-sensitive in the matcher.
            if let Some(target) = s
                .strip_suffix('W')
                .filter(|t| *t == "L" || (is_digits(t) && t.len() <= 2))
            {
                let t = if target == "L" {
                    i64::from(last)
                } else {
                    parse_digits(target)
                };
                if nearest_weekday(year, month, t) == i64::from(day) {
                    return true;
                }
            }
            if s.starts_with("L-") {
                if let Some(n) = l_offset_value(s) {
                    let target = i64::from(last) - n;
                    if target >= 1 && target == i64::from(day) {
                        return true;
                    }
                }
            }
            false
        }
    })
}

fn is_last_weekday_of_month(year: i32, month: u32, day: u32) -> bool {
    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.checked_add_days(chrono::Days::new(7)))
        .is_none_or(|d| d.month() != month)
}

fn matches_day_of_week(field: &[Tok], year: i32, month: u32, day: u32, weekday: u32) -> bool {
    field.iter().any(|value| match value {
        Tok::Int(n) => *n == i64::from(weekday),
        Tok::Str(s) => {
            if let Some((wd, nth)) = nth_weekday(s) {
                return wd == weekday && (day - 1) / 7 + 1 == nth;
            }
            match last_weekday(s) {
                Some(wd) => wd == weekday && is_last_weekday_of_month(year, month, day),
                None => false,
            }
        }
    })
}

impl CronExpression {
    /// Parse and validate like `cron.validate` / `cron.parse`; the error is node-cron's message.
    pub fn parse(expression: &str) -> Result<Self, String> {
        let resolved = resolve_nickname(expression);
        if !has_only_allowed_chars(&resolved) {
            return Err("pattern includes illegal characters!".into());
        }
        let raw: Vec<String> = remove_spaces(&resolved)
            .split(' ')
            .map(str::to_string)
            .collect();
        if raw.len() != 5 && raw.len() != 6 {
            return Err(format!("expected 5 or 6 fields but got {}", raw.len()));
        }
        let patterns: Vec<String> = if raw.len() == 5 {
            std::iter::once("0".to_string()).chain(raw).collect()
        } else {
            raw
        };
        let fields = convert_expression(&resolved)
            .ok_or_else(|| format!("expected 5 or 6 fields but got {}", patterns.len()))?;

        let simple = |i: usize, min: i64, max: i64| {
            all_ints_within(&fields[i].iter().collect::<Vec<_>>(), min, max)
        };
        if !simple(0, 0, 59) {
            return Err(format!(
                "{} is a invalid expression for second",
                patterns[0]
            ));
        }
        if !simple(1, 0, 59) {
            return Err(format!(
                "{} is a invalid expression for minute",
                patterns[1]
            ));
        }
        if !simple(2, 0, 23) {
            return Err(format!("{} is a invalid expression for hour", patterns[2]));
        }
        if is_invalid_day_of_month(&fields[3]) || has_invalid_w_modifier(&patterns[3]) {
            return Err(format!(
                "{} is a invalid expression for day of month",
                patterns[3]
            ));
        }
        if !simple(4, 1, 12) {
            return Err(format!("{} is a invalid expression for month", patterns[4]));
        }
        if is_invalid_week_day(&fields[5]) {
            return Err(format!(
                "{} is a invalid expression for week day",
                patterns[5]
            ));
        }
        if is_impossible_day_of_month(&fields[3], &fields[4]) {
            return Err(format!(
                "{} {} is an impossible day of month for the given month",
                patterns[3], patterns[4]
            ));
        }
        Ok(CronExpression {
            pattern: expression.to_string(),
            seconds: ints_sorted(&fields[0]),
            minutes: ints_sorted(&fields[1]),
            hours: ints_sorted(&fields[2]),
            fields,
        })
    }

    /// The expression as given.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    fn has_int(&self, field: usize, value: u32) -> bool {
        self.fields[field].contains(&Tok::Int(i64::from(value)))
    }

    fn day_matches(&self, year: i32, month: u32, day: u32) -> bool {
        let Some(weekday) = weekday_of(year, month, day) else {
            return false;
        };
        self.has_int(4, month)
            && matches_day_of_month(&self.fields[3], year, month, day)
            && matches_day_of_week(&self.fields[5], year, month, day, weekday)
    }

    /// `TimeMatcher.match(date)`: whether `time` (seconds resolution) matches in its timezone.
    pub fn matches<Tz: TimeZone>(&self, time: &DateTime<Tz>) -> bool {
        self.has_int(0, time.second())
            && self.has_int(1, time.minute())
            && self.has_int(2, time.hour())
            && self.day_matches(time.year(), time.month(), time.day())
    }

    /// `TimeMatcher.getNextMatch(date)`: the first matching second strictly after `base`
    /// (truncated to the second), evaluated in `base`'s timezone. `None` when nothing matches
    /// within 100 years.
    pub fn next_after<Tz: TimeZone>(&self, base: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        let tz = base.timezone();
        let base_s = base.timestamp();
        let base_local = tz.timestamp_opt(base_s, 0).single()?;
        let mut date = base_local.date_naive();
        let lower = (base_local.hour(), base_local.minute(), base_local.second());
        for i in 0..MAX_DAYS {
            if self.day_matches(date.year(), date.month(), date.day()) {
                let bound = (i == 0).then_some(lower);
                if let Some(found) = self.first_time_on_day(&tz, date, bound, base_s) {
                    return Some(found);
                }
            }
            date = date.succ_opt()?;
        }
        None
    }

    fn first_time_on_day<Tz: TimeZone>(
        &self,
        tz: &Tz,
        date: NaiveDate,
        bound: Option<(u32, u32, u32)>,
        base_s: i64,
    ) -> Option<DateTime<Tz>> {
        for &hour in &self.hours {
            if let Some((bh, _, _)) = bound {
                if hour < bh {
                    continue;
                }
            }
            for &minute in &self.minutes {
                for &second in &self.seconds {
                    if let Some((bh, bm, bs)) = bound {
                        if hour * 3600 + minute * 60 + second <= bh * 3600 + bm * 60 + bs {
                            continue;
                        }
                    }
                    let local = match tz.with_ymd_and_hms(
                        date.year(),
                        date.month(),
                        date.day(),
                        hour,
                        minute,
                        second,
                    ) {
                        LocalResult::Single(t) => t,
                        LocalResult::Ambiguous(first, _) => first,
                        LocalResult::None => continue,
                    };
                    if local.timestamp() > base_s && self.matches(&local) {
                        return Some(local);
                    }
                }
            }
        }
        None
    }

    /// [`next_after`](Self::next_after) for epoch milliseconds in timezone `tz`.
    pub fn next_after_ms<Tz: TimeZone>(&self, tz: &Tz, base_ms: i64) -> Option<i64> {
        let base = tz.timestamp_millis_opt(base_ms).single()?;
        self.next_after(&base).map(|t| t.timestamp_millis())
    }
}

/// Result of one runner heartbeat (`planBeat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeatPlan {
    /// Slots that were due but are too late to run; they are skipped (logged only).
    pub missed: Vec<i64>,
    /// The slot to run now.
    pub run: Option<i64>,
    /// The next slot to wait for; `None` when the expression has no further match.
    pub next: Option<i64>,
}

/// node-cron's `missedExecutionTolerance` default: a slot runs if the heartbeat is at most one
/// second late.
pub const MISSED_EXECUTION_TOLERANCE_MS: i64 = 1000;

/// `planBeat(expected, now, toleranceMs, getNextMatch)`, all in epoch milliseconds. After a
/// system sleep (or any stall) every slot that passed by more than the tolerance is reported as
/// missed and not run; the schedule resumes with the next future slot.
pub fn plan_beat(
    expected: i64,
    now: i64,
    tolerance_ms: i64,
    next_match: impl Fn(i64) -> Option<i64>,
) -> BeatPlan {
    let mut missed = Vec::new();
    let mut slot = expected;
    loop {
        if now < slot {
            return BeatPlan {
                missed,
                run: None,
                next: Some(slot),
            };
        }
        let Some(next) = next_match(slot) else {
            return BeatPlan {
                missed,
                run: None,
                next: None,
            };
        };
        if next <= slot {
            return BeatPlan {
                missed,
                run: None,
                next: next_match(now),
            };
        }
        let gap = next - slot;
        let late_by = now - slot;
        if late_by <= tolerance_ms && late_by < gap {
            return BeatPlan {
                missed,
                run: Some(slot),
                next: Some(next),
            };
        }
        missed.push(slot);
        slot = next;
    }
}

#[cfg(test)]
#[path = "cron_tests.rs"]
mod tests;
