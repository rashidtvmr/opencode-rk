#![forbid(unsafe_code)]
//! Byte / count / plural display helpers.
//!
//! Mirrors TS truth: `packages/opencode/src/session/tools.ts`
//! (`formatBytes`), `packages/web/src/components/share/common.tsx`
//! (`formatCount`), `packages/tui/src/util/locale.ts` (`pluralize` +
//! `number`). Uses binary units with one decimal; `format_count` covers
//! the sub-1000 plain / `k`-suffix shape from `Locale::number`.

/// `0..1024` -> `"N B"`, else one-decimal `KiB` / `MiB` / `GiB`.
#[must_use]
pub fn format_bytes(n: u64) -> String {
    const UNIT: f64 = 1024.0;
    let v = n as f64;
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB", v / UNIT)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", v / (UNIT * UNIT))
    } else {
        format!("{:.1} GiB", v / (UNIT * UNIT * UNIT))
    }
}

/// `<1000` plain, else one-decimal `"N.Nk"`.
#[must_use]
pub fn format_count(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        format!("{:.1}k", n as f64 / 1000.0)
    }
}

/// `"N <one|many>"`; singular only when `n == 1`.
#[must_use]
pub fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GiB");
    }

    #[test]
    fn count_plain_and_k() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1.0k");
        assert_eq!(format_count(1500), "1.5k");
    }

    #[test]
    fn plural_picks_form() {
        assert_eq!(plural(1, "file", "files"), "1 file");
        assert_eq!(plural(2, "file", "files"), "2 files");
        assert_eq!(plural(0, "file", "files"), "0 files");
    }

    #[test]
    fn bytes_large_gib() {
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
    }
}
