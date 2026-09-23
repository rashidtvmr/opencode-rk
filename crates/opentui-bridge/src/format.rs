#![forbid(unsafe_code)]
//! Duration formatting (mirrors `packages/tui/src/util/format.ts`).
//!
//! Single export in TS source: `formatDuration`. Port uses `i64` seconds;
//! fractional TS inputs truncate toward display (integer division mirrors
//! `Math.floor` on non-negative inputs; `secs <= 0` returns `""`).

/// TS `formatDuration`: `<=0` -> `""`, `<60` Ns, `<3600` Nm Ns,
/// `<86400` Nh Nm, `<604800` `~N day(s)`, else `~N week(s)`.
#[must_use]
pub fn format_duration(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3600 {
        let mins = secs / 60;
        let remaining = secs % 60;
        return if remaining > 0 {
            format!("{mins}m {remaining}s")
        } else {
            format!("{mins}m")
        };
    }
    if secs < 86400 {
        let hours = secs / 3600;
        let remaining = (secs % 3600) / 60;
        return if remaining > 0 {
            format!("{hours}h {remaining}m")
        } else {
            format!("{hours}h")
        };
    }
    if secs < 604800 {
        let days = secs / 86400;
        return if days == 1 {
            "~1 day".to_string()
        } else {
            format!("~{days} days")
        };
    }
    let weeks = secs / 604800;
    if weeks == 1 {
        "~1 week".to_string()
    } else {
        format!("~{weeks} weeks")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_negative_empty() {
        assert_eq!(format_duration(0), "");
        assert_eq!(format_duration(-5), "");
    }

    #[test]
    fn seconds_branch() {
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn minutes_branch() {
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(61), "1m 1s");
        assert_eq!(format_duration(120), "2m");
        assert_eq!(format_duration(3599), "59m 59s");
    }

    #[test]
    fn hours_branch() {
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(3660), "1h 1m");
        assert_eq!(format_duration(86399), "23h 59m");
    }

    #[test]
    fn days_branch() {
        assert_eq!(format_duration(86400), "~1 day");
        assert_eq!(format_duration(172800), "~2 days");
        assert_eq!(format_duration(604799), "~6 days");
    }

    #[test]
    fn weeks_branch() {
        assert_eq!(format_duration(604800), "~1 week");
        assert_eq!(format_duration(1209600), "~2 weeks");
    }
}
