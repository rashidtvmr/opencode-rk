#![forbid(unsafe_code)]
//! Locale formatting (mirrors `packages/tui/src/util/locale.ts` @ a0d9b6c).
//!
//! ponytail: std only, no chrono/locale DB. `time`/`datetime` render UTC
//! (`HH:MM`, `YYYY-MM-DD`) not `toLocaleTimeString`/`toLocaleDateString`;
//! `todayTimeOrDateTime` compares UTC calendar days, not local tz; string
//! truncation counts `char`s, not UTF-16 units. Upgrade when a tz/locale
//! crate is accepted.

/// TS `titlecase`: uppercase every `\b\w` (ASCII `[A-Za-z0-9_]` after a
/// non-word char or at start). Unicode uppercase applied at those spots.
#[must_use]
pub fn titlecase(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_word = false;
    for (i, c) in s.char_indices() {
        let is_word = c.is_ascii_alphanumeric() || c == '_';
        if is_word && (i == 0 || !prev_word) {
            for u in c.to_uppercase() {
                out.push(u);
            }
        } else {
            out.push(c);
        }
        prev_word = is_word;
    }
    out
}

fn split_millis(millis: i64) -> (i64, u8, u8, u8, u8) {
    let secs = millis.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    // days -> civil (Howard Hinnant), Gregorian proleptic.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    if m <= 2 {
        y += 1;
    }
    (y, m, d, (tod / 3600) as u8, ((tod % 3600) / 60) as u8)
}

/// TS `time`: short local time. Simplified: UTC `HH:MM`.
#[must_use]
pub fn time(millis: i64) -> String {
    let (_, _, _, h, min) = split_millis(millis);
    format!("{h:02}:{min:02}")
}

/// TS `datetime`: `"{time} · {date}"`. Simplified: UTC date `YYYY-MM-DD`.
#[must_use]
pub fn datetime(millis: i64) -> String {
    let (y, m, d, h, min) = split_millis(millis);
    format!("{h:02}:{min:02} · {y:04}-{m:02}-{d:02}")
}

/// TS `todayTimeOrDateTime`: `time` if same calendar day as now (UTC here),
/// else `datetime`.
#[must_use]
pub fn today_time_or_datetime(millis: i64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let (yn, mn, dn, _, _) = split_millis(now_ms);
    let (y, m, d, _, _) = split_millis(millis);
    if y == yn && m == mn && d == dn {
        time(millis)
    } else {
        datetime(millis)
    }
}

/// TS `number`: `1.5K` / `2.0M` abbreviations, else plain.
#[must_use]
pub fn number(num: f64) -> String {
    if num >= 1_000_000.0 {
        format!("{:.1}M", num / 1_000_000.0)
    } else if num >= 1000.0 {
        format!("{:.1}K", num / 1000.0)
    } else if num.is_finite() && num.fract() == 0.0 && num.abs() < 1e15 {
        format!("{}", num as i64)
    } else {
        format!("{num}")
    }
}

/// TS `duration`: ms bucket formatter (`ms`/`s`/`m s`/`h m`/`d h`).
#[must_use]
pub fn duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000)
    } else if ms < 86_400_000 {
        format!("{}h {}m", ms / 3_600_000, (ms % 3_600_000) / 60_000)
    } else {
        format!("{}d {}h", ms / 86_400_000, (ms % 86_400_000) / 3_600_000)
    }
}

/// TS `truncate`: cut to `len` chars + `…` (chars, not UTF-16 units).
#[must_use]
pub fn truncate(s: &str, len: usize) -> String {
    if s.chars().count() <= len {
        return s.to_string();
    }
    s.chars().take(len.saturating_sub(1)).collect::<String>() + "…"
}

/// TS `truncateLeft`: keep the tail + leading `…`.
#[must_use]
pub fn truncate_left(s: &str, len: usize) -> String {
    if s.chars().count() <= len {
        return s.to_string();
    }
    let skip = s.chars().count() - len.saturating_sub(1);
    "…".to_string() + &s.chars().skip(skip).collect::<String>()
}

/// TS `truncateMiddle`: split keep around `…` (explicit max; TS default 35).
#[must_use]
pub fn truncate_middle(s: &str, max_length: usize) -> String {
    if s.chars().count() <= max_length {
        return s.to_string();
    }
    let keep = max_length.saturating_sub(1);
    let keep_start = keep.div_ceil(2);
    let keep_end = keep / 2;
    let start: String = s.chars().take(keep_start).collect();
    let end: String = s
        .chars()
        .skip(s.chars().count() - keep_end)
        .collect();
    format!("{start}…{end}")
}

/// TS `pluralize`: pick singular/plural by count, fill first `{}`.
#[must_use]
pub fn pluralize(count: i64, singular: &str, plural: &str) -> String {
    let template = if count == 1 { singular } else { plural };
    template.replacen("{}", &count.to_string(), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titlecase_basic() {
        assert_eq!(titlecase("hello world"), "Hello World");
        assert_eq!(titlecase("shell"), "Shell");
        assert_eq!(titlecase("foo-bar_baz"), "Foo-Bar_baz");
    }

    #[test]
    fn titlecase_empty() {
        assert_eq!(titlecase(""), "");
    }

    #[test]
    fn time_epoch_shape() {
        assert_eq!(time(0), "00:00");
        let t = time(3_661_000);
        assert_eq!(t.len(), 5);
        assert_eq!(t.chars().nth(2), Some(':'));
        assert_eq!(time(3_661_000), "01:01");
    }

    #[test]
    fn datetime_separator() {
        assert_eq!(datetime(0), "00:00 · 1970-01-01");
        assert!(datetime(1_000).contains('·'));
    }

    #[test]
    fn today_branch_matches_now() {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        assert_eq!(today_time_or_datetime(now_ms), time(now_ms));
        assert_eq!(today_time_or_datetime(0), datetime(0));
    }

    #[test]
    fn number_abbrev() {
        assert_eq!(number(999.0), "999");
        assert_eq!(number(1500.0), "1.5K");
        assert_eq!(number(2_500_000.0), "2.5M");
    }

    #[test]
    fn duration_buckets() {
        assert_eq!(duration(500), "500ms");
        assert_eq!(duration(1500), "1.5s");
        assert_eq!(duration(90_000), "1m 30s");
        assert_eq!(duration(3_660_000), "1h 1m");
        assert_eq!(duration(90_000_000), "1d 1h");
    }

    #[test]
    fn truncate_variants() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 5), "abcd…");
        assert_eq!(truncate_left("abcdef", 5), "…cdef");
        assert_eq!(truncate_middle("abcdefghij", 5), "ab…ij");
        assert_eq!(truncate_middle("abc", 35), "abc");
    }

    #[test]
    fn pluralize_picks_template() {
        assert_eq!(pluralize(1, "{} file", "{} files"), "1 file");
        assert_eq!(pluralize(3, "{} file", "{} files"), "3 files");
    }
}
