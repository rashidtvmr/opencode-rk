#![forbid(unsafe_code)]
//! SGR mouse: parsed here, then killed.
//! Policy: SGR `ESC[<b;x;yM|m` is decoded to [`Sgr`], then dropped:
//! `InputEvent::Mouse | InputEvent::Focus(_)` fold to `Noop` in
//! `native_input::map_event`, and `enable_mouse(false)` keeps SGR off.

/// Parsed SGR mouse report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sgr {
    pub b: u8,
    pub x: u16,
    pub y: u16,
}

/// Bounded SGR body parse: `[<]b;x;y[M|m]`, `len <= 24`, digits only.
#[must_use]
pub fn parse_sgr(s: &str) -> Option<Sgr> {
    if s.is_empty() || s.len() > 24 {
        return None;
    }
    let t = s
        .strip_prefix("\u{1b}[<")
        .or_else(|| s.strip_prefix('<'))
        .unwrap_or(s);
    let t = t
        .strip_suffix('M')
        .or_else(|| t.strip_suffix('m'))
        .unwrap_or(t);
    let mut it = t.split(';');
    let (bs, xs, ys) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() {
        return None;
    }
    for p in [bs, xs, ys] {
        if p.is_empty() || p.len() > 5 || !p.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    let b: u8 = bs.parse().ok()?;
    let x: u16 = xs.parse().ok()?;
    let y: u16 = ys.parse().ok()?;
    if x == 0 || y == 0 {
        return None;
    }
    Some(Sgr { b, x, y })
}

/// SGR button code 3 means release.
#[must_use]
pub const fn is_release(b: u8) -> bool {
    b == 3
}

/// Mouse stays killed: reports fold to `Noop`, `enable_mouse(false)`.
///
/// Documents that SGR is parsed (see [`parse_sgr`]) then discarded:
/// `Mouse | Focus => Noop`.
#[must_use]
pub const fn mouse_killed() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_seq() {
        let s = parse_sgr("\u{1b}[<0;10;20M").unwrap();
        assert_eq!(s, Sgr { b: 0, x: 10, y: 20 });
    }

    #[test]
    fn parses_bare_body() {
        let s = parse_sgr("3;5;7m").unwrap();
        assert!(is_release(s.b));
    }

    #[test]
    fn rejects_unbounded() {
        assert!(parse_sgr("0;1;2;3;4;5;6;7;8;9;10;11;12M").is_none());
        assert!(parse_sgr("").is_none());
        assert!(parse_sgr("<a;b;cM").is_none());
    }

    #[test]
    fn release_only_three() {
        assert!(is_release(3));
        assert!(!is_release(0) && !is_release(1) && !is_release(2));
    }

    #[test]
    fn mouse_stays_killed() {
        assert!(mouse_killed());
    }
}
