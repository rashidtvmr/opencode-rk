#![forbid(unsafe_code)]
//! Full locale number/plural/lang helpers (extends `locale.rs`).
//!
//! Mirrors TS truth `packages/tui/src/util/locale.ts` (`pluralize`,
//! `number`); `format_number` adds thousands commas, `lang_of` takes the
//! primary subtag before `-`/`_`.
//!
//! ponytail: no locale DB/ICU; commas only, `en` fallback untouched.
//! Upgrade when a locale crate is accepted.

/// `1234567` -> `"1,234,567"`; negatives keep `-`; `i64::MIN` safe.
#[must_use]
pub fn format_number(n: i64) -> String {
    let neg = n < 0;
    let mut digits = n.unsigned_abs().to_string().into_bytes();
    digits.reverse();
    let mut out: Vec<u8> = Vec::with_capacity(digits.len() + digits.len() / 3);
    for (idx, b) in digits.iter().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(b',');
        }
        out.push(*b);
    }
    out.reverse();
    let mut s = String::from_utf8(out).expect("digits/commas are ascii");
    if neg {
        s.insert(0, '-');
    }
    s
}

/// `"N <one|many>"`; singular only when `n == 1`.
#[must_use]
pub fn pluralize(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Primary language subtag: up to first `-` or `_` (`"en-US"` -> `"en"`).
#[must_use]
pub fn lang_of(locale: &str) -> &str {
    let cut = locale.find(['-', '_']).unwrap_or(locale.len());
    &locale[..cut]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commas_grouped() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn commas_negative_and_min() {
        assert_eq!(format_number(-42), "-42");
        assert_eq!(format_number(-1234567), "-1,234,567");
        assert_eq!(format_number(i64::MIN), "-9,223,372,036,854,775,808");
    }

    #[test]
    fn pluralize_forms() {
        assert_eq!(pluralize(1, "file", "files"), "1 file");
        assert_eq!(pluralize(0, "file", "files"), "0 files");
        assert_eq!(pluralize(2, "file", "files"), "2 files");
    }

    #[test]
    fn lang_of_splits() {
        assert_eq!(lang_of("en"), "en");
        assert_eq!(lang_of("en-US"), "en");
        assert_eq!(lang_of("pt_BR"), "pt");
        assert_eq!(lang_of("zh-Hans-CN"), "zh");
        assert_eq!(lang_of(""), "");
    }
}
