#![forbid(unsafe_code)]
//! Image renderable and preview gallery model (MEDIA-001 support).
//!
//! EVIDENCE / BOUNDARY
//! - The vendored library exports an image surface
//!   (`nm -gU …/libopentui.dylib`: `imageCreateFromRgba`, `imageCreateFromPixels`,
//!   `imageDecode`, `imageResize`, `imageGetPixelsPtr`, `imageGetInfo`,
//!   `bufferDrawImage`, …), but **no audited header at the selected OpenTUI
//!   commit records its signatures**; `docs/ui-parity/…/audit-opentui-boundary.md`
//!   does not bind it either. Declaring `extern "C"` here would therefore be an
//!   invented ABI, so nothing is bound: decoding/scaling/compositing below is a
//!   Rust model and fails closed on bad data.
//! - Gallery behaviour follows the OpenCode component that consumes it
//!   (`packages/tui/src/component/dialog-image-preview.tsx`, pin 450e7b9):
//!   initial index clamped to `0..=len-1` (:16), `move()` is a no-op with fewer
//!   than two images (:25) and wraps modulo the list (:27), a move resets the
//!   failed flag (:26), the header is `Image n of m` (:41), the preview height
//!   is `max(3, terminal_height - 8)` (:19), a broken image renders the text
//!   `No preview` and the footer only offers previous/next for >1 image
//!   (:45, :55, :66), and the renderable is `<image fit="fit" width="100%">`
//!   (:74-80).
//! - `MEDIA-001` additionally requires that a gallery click does not bubble, so
//!   [`Gallery::click`] reports whether the event was consumed.

use crate::buffer::{Cell, CellGrid};
use crate::color::{pack_meta, pack_rgba8, RgbaU16, INTENT_RGB};
use crate::layout::Rect;

/// Maximum image dimension in pixels (fail closed).
pub const MAX_DIM: u32 = 4096;
/// Maximum pixel count (16 MiB of RGBA).
pub const MAX_PIXELS: u32 = 4 * 1024 * 1024;
/// Rows kept for the preview chrome (`dialog-image-preview.tsx:19`).
pub const PREVIEW_CHROME_ROWS: u32 = 8;
/// Minimum preview height (`dialog-image-preview.tsx:19` `Math.max(3, …)`).
pub const MIN_PREVIEW_ROWS: u32 = 3;
/// Text shown when an image cannot be previewed (`dialog-image-preview.tsx:57`).
pub const NO_PREVIEW_LABEL: &str = "No preview";
/// Half-block glyph: upper half is the foreground pixel, lower half the
/// background pixel, so one cell carries two vertical pixels.
pub const HALF_BLOCK: char = '\u{2580}';

/// Fail-closed image errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    /// Width/height is zero, above [`MAX_DIM`], or above [`MAX_PIXELS`].
    BadSize,
    /// Pixel buffer length is not `width * height * 4`.
    BadBuffer,
    /// Source bytes are not RGBA8 data of a supported size.
    Undecodable,
}

impl core::fmt::Display for ImageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::BadSize => "image size out of range",
            Self::BadBuffer => "image pixel buffer does not match its size",
            Self::Undecodable => "image data cannot be decoded",
        })
    }
}

impl std::error::Error for ImageError {}

/// RGBA8 pixel image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl ImageData {
    /// Validated RGBA8 image. Every byte is kept; invalid sizes fail closed.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, ImageError> {
        if width == 0
            || height == 0
            || width > MAX_DIM
            || height > MAX_DIM
            || width.saturating_mul(height) > MAX_PIXELS
        {
            return Err(ImageError::BadSize);
        }
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(ImageError::BadBuffer);
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// Solid image, used by fixtures and tests.
    pub fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Result<Self, ImageError> {
        let mut buffer = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width * height {
            buffer.extend_from_slice(&rgba);
        }
        Self::new(width, height, buffer)
    }

    /// Interpret already-decoded RGBA8 bytes (an `imageDecode` result shape).
    pub fn from_pixels(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, ImageError> {
        Self::new(width, height, rgba)
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.rgba
    }

    /// Pixel at `(x, y)`, `None` outside the image.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize * self.width as usize + x as usize) * 4;
        Some([
            self.rgba[index],
            self.rgba[index + 1],
            self.rgba[index + 2],
            self.rgba[index + 3],
        ])
    }

    /// Nearest-neighbour sample into a `cols x rows` cell raster, two pixels per
    /// cell (`2 * rows` pixel rows). A missing lower pixel reuses the upper one.
    #[must_use]
    pub fn sample(&self, cols: u32, rows: u32) -> Vec<Vec<[u8; 4]>> {
        let mut out = Vec::with_capacity(rows as usize);
        if cols == 0 || rows == 0 {
            return out;
        }
        let pixel_rows = rows.saturating_mul(2);
        for row in 0..pixel_rows {
            let sy = (row as u64 * self.height as u64 / pixel_rows as u64) as u32;
            let mut line = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                let sx = (col as u64 * self.width as u64 / cols as u64) as u32;
                line.push(self.pixel(sx.min(self.width - 1), sy.min(self.height - 1)).unwrap_or([0; 4]));
            }
            out.push(line);
        }
        out
    }
}

/// Load state of one preview slot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImageState {
    /// No image selected.
    #[default]
    Empty,
    /// Loading (nothing painted yet).
    Loading,
    Ready(ImageData),
    /// Failed decode: the dialog shows [`NO_PREVIEW_LABEL`].
    Failed,
}

impl ImageState {
    #[must_use]
    pub fn image(&self) -> Option<&ImageData> {
        match self {
            Self::Ready(image) => Some(image),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

/// Scaling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fit {
    /// Shrink to fit the box, never enlarging.
    #[default]
    Contain,
    /// Use the natural size clipped to the box.
    Original,
}

/// Size of an image in cells after fitting `available` cells.
#[must_use]
pub fn fit_cells(image_w: u32, image_h: u32, available: (u32, u32), fit: Fit) -> (u32, u32) {
    if image_w == 0 || image_h == 0 || available.0 == 0 || available.1 == 0 {
        return (0, 0);
    }
    if fit == Fit::Original {
        return (image_w.min(available.0), image_h.min(available.1));
    }
    // A cell holds two vertical pixels, so the pixel box is
    // `available.0 x (available.1 * 2)`.
    let max_px_w = available.0 as u64;
    let max_px_h = available.1 as u64 * 2;
    let (cols, rows) = if image_w as u64 * max_px_h <= image_h as u64 * max_px_w {
        // Height-limited: the whole image height fits the box height.
        let cols = (image_w as u64 * available.1 as u64 + image_h as u64 - 1) / image_h as u64;
        (cols as u32, available.1)
    } else {
        // Width-limited.
        let rows = (image_h as u64 * max_px_w + 2 * image_w as u64 - 1) / (2 * image_w as u64);
        (available.0, rows as u32)
    };
    (
        cols.clamp(1, available.0),
        rows.clamp(1, available.1),
    )
}

fn color_of(pixel: [u8; 4]) -> RgbaU16 {
    pack_rgba8(pixel[0], pixel[1], pixel[2], pixel[3], pack_meta(INTENT_RGB, 0))
}

/// Paint-ready image renderable: state, fit and cell raster.
#[derive(Debug, Clone)]
pub struct ImageRenderable {
    pub state: ImageState,
    pub fit: Fit,
    max: (u32, u32),
}

impl ImageRenderable {
    #[must_use]
    pub fn new(max: (u32, u32)) -> Self {
        Self {
            state: ImageState::Empty,
            fit: Fit::Contain,
            max,
        }
    }

    /// Preview box for a terminal of `height` rows
    /// (`dialog-image-preview.tsx:19`).
    #[must_use]
    pub fn preview_box(terminal_height: u32, width: u32) -> (u32, u32) {
        (width, terminal_height.saturating_sub(PREVIEW_CHROME_ROWS).max(MIN_PREVIEW_ROWS))
    }

    pub fn set_state(&mut self, state: ImageState) {
        self.state = state;
    }

    /// Cell size the image occupies inside the renderable box.
    #[must_use]
    pub fn cell_size(&self) -> (u32, u32) {
        match self.state.image() {
            Some(image) => fit_cells(image.width(), image.height(), self.max, self.fit),
            None => (0, 0),
        }
    }

    /// Styled cells `(x, y, cell)` for the current image, relative to `(0, 0)`.
    /// Empty when nothing is ready (loading/failed paint no raster; the caller
    /// draws the label).
    #[must_use]
    pub fn cells(&self) -> Vec<(u32, u32, Cell)> {
        let (cols, rows) = self.cell_size();
        let Some(image) = self.state.image() else {
            return Vec::new();
        };
        let sampled = image.sample(cols, rows);
        let mut out = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            let top = &sampled[(row * 2) as usize];
            let bottom = &sampled[(row * 2 + 1) as usize];
            for col in 0..cols {
                let fg = color_of(top[col as usize]);
                let bg = color_of(bottom[col as usize]);
                out.push((col, row, Cell::new(HALF_BLOCK, fg, bg)));
            }
        }
        out
    }

    /// Paint into `grid` at `origin`, clipped by the grid scissor stack.
    pub fn paint(&self, grid: &mut CellGrid, origin: Rect) -> u32 {
        let mut written = 0;
        for (x, y, cell) in self.cells() {
            if grid.set(origin.x.saturating_add(x), origin.y.saturating_add(y), cell) {
                written += 1;
            }
        }
        written
    }

    /// Label to draw when there is no raster (`No preview` on failure).
    #[must_use]
    pub fn label(&self) -> Option<&'static str> {
        match self.state {
            ImageState::Failed => Some(NO_PREVIEW_LABEL),
            ImageState::Empty | ImageState::Loading | ImageState::Ready(_) => None,
        }
    }
}

/// Outcome of a gallery click. `Consumed` means the event must not bubble
/// (MEDIA-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOutcome {
    Consumed(usize),
    Bubbled,
}

/// Image gallery: index clamping, wrap-around moves and failed-state reset,
/// mirroring `dialog-image-preview.tsx:16-27`.
#[derive(Debug, Clone, Default)]
pub struct Gallery {
    items: Vec<ImageState>,
    index: usize,
    failed: bool,
}

impl Gallery {
    /// `initial` is clamped like `Math.max(0, Math.min(len - 1, initial))`.
    #[must_use]
    pub fn new(items: Vec<ImageState>, initial: usize) -> Self {
        let index = clamp_index(items.len(), initial);
        Self {
            items,
            index,
            failed: false,
        }
    }

    #[must_use]
    pub fn items(&self) -> &[ImageState] {
        &self.items
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn current(&self) -> Option<&ImageState> {
        self.items.get(self.index)
    }

    #[must_use]
    pub const fn failed(&self) -> bool {
        self.failed
    }

    /// Mark the current image as failed (decode error).
    pub fn set_failed(&mut self, failed: bool) {
        self.failed = failed;
        if failed {
            if let Some(state) = self.items.get_mut(self.index) {
                *state = ImageState::Failed;
            }
        }
    }

    /// Move by `direction`; a no-op with fewer than two images (line 25), wraps
    /// modulo the list (line 27), and resets the failed flag (line 26).
    pub fn move_by(&mut self, direction: i32) -> bool {
        if self.items.len() < 2 {
            return false;
        }
        let len = self.items.len() as i32;
        let next = ((self.index as i32 + direction) % len + len) % len;
        self.index = next as usize;
        self.failed = false;
        true
    }

    /// Preview height for this gallery's current image
    /// (`Math.max(3, height - 8)`).
    #[must_use]
    pub fn preview_height(terminal_height: u32) -> u32 {
        terminal_height.saturating_sub(PREVIEW_CHROME_ROWS).max(MIN_PREVIEW_ROWS)
    }

    /// Footer previous/next actions are only shown for more than one image
    /// (`dialog-image-preview.tsx:45`).
    #[must_use]
    pub fn has_navigation(&self) -> bool {
        self.items.len() > 1
    }

    /// Click inside the preview raster is consumed (no bubbling); a click
    /// outside bubbles to the dialog.
    pub fn click(&mut self, inside_preview: bool) -> ClickOutcome {
        if inside_preview {
            ClickOutcome::Consumed(self.index)
        } else {
            ClickOutcome::Bubbled
        }
    }

    /// Header title `Image n of m`.
    #[must_use]
    pub fn title(&self) -> String {
        format!("Image {} of {}", self.index + 1, self.items.len())
    }
}

fn clamp_index(len: usize, requested: usize) -> usize {
    if len == 0 {
        0
    } else {
        requested.min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media001_fit_preserves_aspect_with_half_blocks() {
        // 2:1 pixel image in a 10x5 box (10x10 pixels): width limited, scaled to
        // 10x5 pixels, which occupies ceil(5 / 2) = 3 cell rows.
        assert_eq!(fit_cells(20, 10, (10, 5), Fit::Contain), (10, 3));
        // Tall image is height limited.
        let (w, h) = fit_cells(10, 100, (10, 5), Fit::Contain);
        assert_eq!(h, 5);
        assert!(w <= 10 && w >= 1);
        // Wide image is width limited and collapses to a single pixel row.
        let (w, h) = fit_cells(200, 10, (10, 5), Fit::Contain);
        assert_eq!(w, 10);
        assert_eq!(h, 1);
        // Zero inputs fail closed.
        assert_eq!(fit_cells(0, 10, (10, 5), Fit::Contain), (0, 0));
        assert_eq!(fit_cells(10, 10, (0, 5), Fit::Contain), (0, 0));
        assert_eq!(fit_cells(100, 100, (10, 5), Fit::Original), (10, 5));
    }

    #[test]
    fn media001_image_validation_fails_closed() {
        assert_eq!(ImageData::new(0, 4, vec![]), Err(ImageError::BadSize));
        assert_eq!(
            ImageData::new(MAX_DIM + 1, 1, vec![0; 4]),
            Err(ImageError::BadSize)
        );
        assert_eq!(ImageData::new(2, 2, vec![0; 8]), Err(ImageError::BadBuffer));
        let img = ImageData::solid(1, 1, [1, 2, 3, 255]).unwrap();
        assert_eq!(img.pixel(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(img.pixel(1, 0), None);
    }

    #[test]
    fn media001_cells_carry_two_pixels_each() {
        // 1x2 image: top red, bottom blue.
        let img = ImageData::new(1, 2, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        let mut renderable = ImageRenderable::new((1, 1));
        renderable.set_state(ImageState::Ready(img));
        assert_eq!(renderable.cell_size(), (1, 1));
        let cells = renderable.cells();
        assert_eq!(cells.len(), 1);
        let (x, y, cell) = cells[0];
        assert_eq!((x, y), (0, 0));
        assert_eq!(cell.ch, HALF_BLOCK);
        assert_eq!(cell.fg[0], 255, "top pixel is the foreground");
        assert_eq!(cell.fg[2], 0);
        assert_eq!(cell.bg[2], 255, "bottom pixel is the background");
    }

    #[test]
    fn media001_paint_is_clipped_and_labels_failures() {
        let img = ImageData::solid(4, 4, [10, 20, 30, 255]).unwrap();
        let mut renderable = ImageRenderable::new((4, 4));
        renderable.set_state(ImageState::Ready(img));
        assert_eq!(renderable.cell_size(), (4, 2), "square pixels halve in rows");
        let mut grid = CellGrid::new(8, 4);
        grid.push_clip(Rect::new(0, 0, 3, 2));
        let written = renderable.paint(&mut grid, Rect::new(1, 1, 4, 2));
        assert_eq!(written, 2, "clip keeps only 2 of the 4x2 cells");
        assert_eq!(grid.get(1, 1).unwrap().ch, HALF_BLOCK);
        assert_eq!(grid.get(3, 1).unwrap(), Cell::BLANK);

        let mut failed = ImageRenderable::new((4, 2));
        failed.set_state(ImageState::Failed);
        assert_eq!(failed.label(), Some(NO_PREVIEW_LABEL));
        assert!(failed.cells().is_empty());
        assert_eq!(failed.cell_size(), (0, 0));
        let mut loading = ImageRenderable::new((4, 2));
        loading.set_state(ImageState::Loading);
        assert_eq!(loading.label(), None);
    }

    #[test]
    fn media001_gallery_index_clamps_and_wraps() {
        let items = vec![
            ImageState::Ready(ImageData::solid(1, 1, [1, 1, 1, 255]).unwrap()),
            ImageState::Failed,
            ImageState::Loading,
        ];
        let mut gallery = Gallery::new(items.clone(), 99);
        assert_eq!(gallery.index(), 2, "initial index is clamped");
        assert_eq!(gallery.title(), "Image 3 of 3");
        assert!(gallery.has_navigation());
        // Wraps forward past the end and backward past the start.
        assert!(gallery.move_by(1));
        assert_eq!(gallery.index(), 0);
        assert!(gallery.move_by(-1));
        assert_eq!(gallery.index(), 2);
        // A move clears the failed flag.
        gallery.set_failed(true);
        assert!(gallery.failed());
        gallery.move_by(-1);
        assert!(!gallery.failed());

        // Zero and one image behave like the component: no move, no nav.
        assert_eq!(Gallery::new(vec![], 5).index(), 0);
        let mut single = Gallery::new(vec![ImageState::Loading], 0);
        assert!(!single.has_navigation());
        assert!(!single.move_by(1));
        assert_eq!(single.index(), 0);
    }

    #[test]
    fn media001_click_inside_preview_does_not_bubble() {
        let mut gallery = Gallery::new(
            vec![
                ImageState::Loading,
                ImageState::Loading,
                ImageState::Loading,
            ],
            1,
        );
        assert_eq!(gallery.click(true), ClickOutcome::Consumed(1));
        assert_eq!(gallery.click(false), ClickOutcome::Bubbled);
    }

    #[test]
    fn media001_preview_box_matches_component() {
        assert_eq!(ImageRenderable::preview_box(40, 100), (100, 32));
        assert_eq!(ImageRenderable::preview_box(10, 100), (100, 3), "min 3");
        assert_eq!(ImageRenderable::preview_box(0, 100), (100, 3));
        assert_eq!(Gallery::preview_height(40), 32);
        assert_eq!(Gallery::preview_height(5), 3);
    }
}
