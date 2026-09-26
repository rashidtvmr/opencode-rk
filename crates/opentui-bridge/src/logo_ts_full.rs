#![forbid(unsafe_code)]
//! Full TS wordmark rows: `left[i] + " " + right[i]` from `logo.ts:1-4`.
//! Halves verbatim in `crate::logo_art` (untouched); gap mirrors
//! `presentation.ts:25` (`${pad}${left} ${right}`, pad empty here).
//! ponytail: static joined rows; upgrade to pad/gap args when needed.
pub const LOGO_FULL_ROWS: [&str; 4] = [
    "                                 ▄     ",
    "█▀▀█ █▀▀█ █▀▀█ █▀▀▄ █▀▀▀ █▀▀█ █▀▀█ █▀▀█",
    "█__█ █__█ █^^^ █__█ █___ █__█ █__█ █^^^",
    "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀",
];
#[must_use]
pub fn logo_line(i: usize) -> &'static str {
    LOGO_FULL_ROWS.get(i).copied().unwrap_or("")
}
#[must_use]
pub fn logo_rows() -> usize {
    LOGO_FULL_ROWS.len()
}
#[must_use]
pub fn logo_width() -> usize {
    LOGO_FULL_ROWS
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_and_oob() {
        assert_eq!(logo_rows(), 4);
        assert_eq!(logo_line(1), LOGO_FULL_ROWS[1]);
        assert_eq!(logo_line(9), "");
    }
    #[test]
    fn exact_join() {
        assert_eq!(logo_line(1), "█▀▀█ █▀▀█ █▀▀█ █▀▀▄ █▀▀▀ █▀▀█ █▀▀█ █▀▀█");
        assert!(logo_line(0).ends_with("▄     "));
    }
    #[test]
    fn width_is_39() {
        assert_eq!(logo_width(), 39);
        assert!(LOGO_FULL_ROWS.iter().all(|s| s.chars().count() == 39));
    }
}
