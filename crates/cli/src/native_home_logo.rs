//! Pinned normal-TUI logo shapes shared by text layout and native painting.
pub(crate) struct Segment {
    pub x: u16,
    pub y: u16,
    pub raw: &'static str,
    pub bold: bool,
}

const LEFT: [&str; 4] = ["                   ", "█▀▀█ █▀▀█ █▀▀█ █▀▀▄", "█__█ █__█ █^^^ █__█", "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀"];
const RIGHT: [&str; 4] = ["             ▄     ", "█▀▀▀ █▀▀█ █▀▀█ █▀▀█", "█___ █__█ █__█ █^^^", "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀"];

pub(crate) fn dimensions(cols: u16, rows: u16) -> (u16, u16) {
    if rows < 12 { (0, 0) }
    else if cols < 22 { (4, 3) }
    else if cols < 44 { (19, 7) }
    else { (39, 4) }
}

pub(crate) fn segments(cols: u16, rows: u16) -> Vec<Segment> {
    let (_, height) = dimensions(cols, rows);
    match height {
        3 => ["█▀▀█", "█__█", "▀▀▀▀"].into_iter().enumerate()
            .map(|(y, raw)| Segment { x: 0, y: y as u16, raw, bold: true }).collect(),
        7 => LEFT[1..].iter().map(|raw| (*raw, false)).chain(RIGHT.iter().map(|raw| (*raw, true)))
            .enumerate().map(|(y, (raw, bold))| Segment { x: 0, y: y as u16, raw, bold }).collect(),
        4 => (0..4).flat_map(|y| [Segment { x: 0, y, raw: LEFT[y as usize], bold: false },
            Segment { x: 20, y, raw: RIGHT[y as usize], bold: true }]).collect(),
        _ => Vec::new(),
    }
}
