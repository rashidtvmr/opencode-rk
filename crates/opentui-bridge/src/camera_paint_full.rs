#![forbid(unsafe_code)]
//! Paint order table for the shell scene (`world.rs:43-264`).
//!
//! Maps stable `u8` paint ids to region names in `PAINT_ORDER`
//! (`world.rs:150-230`) order: `Transcript`, `Composer`, `Sidebar`,
//! `Status`. `Camera`, `pan`, `zoom`, `visible` rects, and
//! `paint_calls` live in `world.rs`; this file duplicates no types,
//! only the id order plus an `i32` range guard.

/// Paint ids in paint order: Transcript, Composer, Sidebar, Status.
pub const PAINT_IDS: [u8; 4] = [0, 1, 2, 3];

/// Name for a paint id; fail-closed `"Unknown"` when out of range.
#[must_use]
pub const fn paint_id_name(id: u8) -> &'static str {
    match id {
        0 => "Transcript",
        1 => "Composer",
        2 => "Sidebar",
        3 => "Status",
        _ => "Unknown",
    }
}

/// `i32` range guard for a visible point via `i64` widening compare.
#[must_use]
pub const fn visible_ok(x: i32, y: i32) -> bool {
    (x as i64) >= (i32::MIN as i64)
        && (x as i64) <= (i32::MAX as i64)
        && (y as i64) >= (i32::MIN as i64)
        && (y as i64) <= (i32::MAX as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_order_len() {
        assert_eq!(PAINT_IDS, [0, 1, 2, 3]);
    }

    #[test]
    fn names_cover_all_ids() {
        assert_eq!(paint_id_name(0), "Transcript");
        assert_eq!(paint_id_name(1), "Composer");
        assert_eq!(paint_id_name(2), "Sidebar");
        assert_eq!(paint_id_name(3), "Status");
        assert_eq!(paint_id_name(9), "Unknown");
    }

    #[test]
    fn guard_accepts_i32_range() {
        assert!(visible_ok(0, 0));
        assert!(visible_ok(i32::MAX, i32::MIN));
    }
}
