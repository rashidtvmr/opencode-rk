//! Frame height helpers shared by native painters.
//!
//! Extracts the pad/truncate idiom from `tui_entry`, `native_frame`,
//! and `paint_full` so all frames agree on exact-height output.
#![forbid(unsafe_code)]

/// Truncate or pad `lines` to exactly `height` rows (min 1).
pub fn fit_height(mut lines: Vec<String>, height: usize) -> Vec<String> {
    let height = height.max(1);
    lines.truncate(height);
    while lines.len() < height {
        lines.push(String::new());
    }
    lines
}

/// Visible transcript body rows: 3 header + 3 footer + 1 pad = 7 chrome.
pub fn body_rows(height: usize) -> usize {
    height.saturating_sub(7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_short_frame_with_empty_lines() {
        let out = fit_height(vec!["a".to_string()], 3);
        assert_eq!(out, vec!["a".to_string(), String::new(), String::new()]);
    }

    #[test]
    fn truncates_tall_frame_to_height() {
        let lines = (0..5).map(|i| i.to_string()).collect::<Vec<_>>();
        let out = fit_height(lines, 3);
        assert_eq!(out, vec!["0".to_string(), "1".to_string(), "2".to_string()]);
    }

    #[test]
    fn zero_height_clamps_to_one() {
        assert_eq!(fit_height(vec![], 0), vec![String::new()]);
        assert_eq!(
            fit_height(vec!["a".into(), "b".into()], 0),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn body_rows_reserves_seven_chrome() {
        assert_eq!(body_rows(10), 3);
        assert_eq!(body_rows(7), 0);
    }

    #[test]
    fn body_rows_saturates_on_small_height() {
        assert_eq!(body_rows(0), 0);
        assert_eq!(body_rows(3), 0);
    }
}
