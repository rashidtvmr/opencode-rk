#![forbid(unsafe_code)]

pub const MIN_ZOOM_PCT: u16 = 25;
pub const MAX_ZOOM_PCT: u16 = 400;
const ZOOM_STEP_PCT: u16 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    Idle,
    Pressed(u16, u16),
    Dragging(u16, u16, u16, u16),
}

impl DragState {
    pub fn press(&mut self, x: u16, y: u16) {
        *self = Self::Pressed(x, y);
    }

    pub fn move_to(&mut self, x: u16, y: u16) {
        match *self {
            Self::Pressed(x0, y0) if x != x0 || y != y0 => {
                *self = Self::Dragging(x0, y0, x, y);
            }
            Self::Dragging(x0, y0, _, _) => *self = Self::Dragging(x0, y0, x, y),
            _ => {}
        }
    }

    pub fn release(&mut self) {
        *self = Self::Idle;
    }

    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        matches!(self, Self::Dragging(..))
    }
}

#[must_use]
pub fn hit_node(nx: i32, ny: i32, nw: i32, nh: i32, px: i32, py: i32) -> bool {
    nw > 0
        && nh > 0
        && px >= nx
        && py >= ny
        && px < nx.saturating_add(nw)
        && py < ny.saturating_add(nh)
}

#[must_use]
pub fn zoom_step(in_out: bool, cur: u16) -> u16 {
    if in_out {
        cur.saturating_add(ZOOM_STEP_PCT)
            .clamp(MIN_ZOOM_PCT, MAX_ZOOM_PCT)
    } else {
        cur.saturating_sub(ZOOM_STEP_PCT)
            .clamp(MIN_ZOOM_PCT, MAX_ZOOM_PCT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_move_release_lifecycle() {
        let mut s = DragState::Idle;
        s.press(2, 3);
        assert_eq!(s, DragState::Pressed(2, 3));
        s.move_to(8, 9);
        assert_eq!(s, DragState::Dragging(2, 3, 8, 9));
        assert!(s.is_dragging());
        s.release();
        assert_eq!(s, DragState::Idle);
    }

    #[test]
    fn stationary_move_stays_pressed_and_idle_ignores_move() {
        let mut s = DragState::Idle;
        s.move_to(4, 5);
        assert_eq!(s, DragState::Idle);
        s.press(4, 5);
        s.move_to(4, 5);
        assert_eq!(s, DragState::Pressed(4, 5));
        assert!(!s.is_dragging());
    }

    #[test]
    fn node_hit_is_half_open_and_overflow_safe() {
        assert!(hit_node(10, 20, 5, 4, 10, 20));
        assert!(hit_node(10, 20, 5, 4, 14, 23));
        assert!(!hit_node(10, 20, 5, 4, 15, 23));
        assert!(!hit_node(i32::MAX, 0, 2, 2, i32::MAX, 0));
        assert!(!hit_node(0, 0, 0, 2, 0, 0));
    }

    #[test]
    fn zoom_steps_saturate_and_clamp() {
        assert_eq!(zoom_step(true, 100), 125);
        assert_eq!(zoom_step(false, 100), 75);
        assert_eq!(zoom_step(true, MAX_ZOOM_PCT), MAX_ZOOM_PCT);
        assert_eq!(zoom_step(false, MIN_ZOOM_PCT), MIN_ZOOM_PCT);
        assert_eq!(zoom_step(true, 0), MIN_ZOOM_PCT);
        assert_eq!(zoom_step(false, u16::MAX), MAX_ZOOM_PCT);
    }
}
