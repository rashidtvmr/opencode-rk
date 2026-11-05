#![forbid(unsafe_code)]
//! P16 image preview, gallery and local image attachments.
//!
//! Upstream evidence (pin 450e7b92412c780a7b5a75226bb8d49aaca24e22):
//! - preview dialog: `packages/tui/src/component/dialog-image-preview.tsx`
//!   — initial index clamp `:16`, wrap-only-when-multiple navigation `:24-28`,
//!   `Image N of M` header `:41`, previous/next actions only when multiple
//!   `:43-51`, `No preview` / mention / `Image N` footer `:54-57`, legacy
//!   footer arms `:60-72`, image height `max(3, height - 8)` `:19`, `xlarge`
//!   centered dialog `:21-22`.
//! - user gallery: `component/prompt/index.tsx:396-420,1699-1767` — image
//!   prefix filter, `max(4, min(8, floor(height/4)))` cells, width = height*2,
//!   first three visible, clicking an image opens the dialog at that index and
//!   stops propagation, `+N more` also opens at index 3.
//! - tool gallery: `routes/session/index.tsx:2656-2769` — `inlineToolImages`
//!   accepts only `type: "file"` image data URIs, `session.image_preview`
//!   gates the gallery, three visible cells, but the overflow cell has no
//!   click handler.
//! - image dedupe: `prompt/attachment.ts:14-50`.
//! - local attachment limits and MIME map:
//!   `component/prompt/local-attachment.ts:5,29-79,160-176`.
//!
//! MEDIA-001 has no kit fixture; the tests below are authored cases.
//! Pure model: filesystem work is behind [`LocalFiles`] and no IO happens here.
//!
//! v2 (wave-6 reachability): [`PreviewSurface`] is the rendered half of the
//! preview dialog — size negotiation, aspect-preserving [`fit_box`], the body
//! placeholder for a terminal that cannot draw images, the metadata caption
//! row and the previous/next action row. It is consumed by
//! `crate::native_view_pages::Page::ImagePreview` through
//! `PageState::open_image_preview`, so the model is reachable from the page
//! router instead of only from tests. Upstream evidence for the surface is
//! repeated next to [`PreviewSurface`]; it remains the dialog of
//! `component/dialog-image-preview.tsx`, not a route of its own.

use std::collections::HashMap;

/// `local-attachment.ts:5`
pub const MAX_LOCAL_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
/// `local-attachment.ts:4`
pub const MAX_PASTED_FILEPATHS: usize = 32;
/// `dialog-image-preview.tsx:19`
pub const PREVIEW_IMAGE_MIN_HEIGHT: u16 = 3;
/// `dialog-image-preview.tsx:19`
pub const PREVIEW_VERTICAL_CHROME: u16 = 8;
/// `prompt/index.tsx:399` / `routes/session/index.tsx:2664`
pub const GALLERY_MIN_HEIGHT: u16 = 4;
/// `prompt/index.tsx:399`
pub const GALLERY_MAX_HEIGHT: u16 = 8;
/// `prompt/index.tsx:401` / `routes/session/index.tsx:2665`
pub const GALLERY_VISIBLE_LIMIT: usize = 3;
/// `prompt/index.tsx:1730`
pub const GALLERY_OVERFLOW_WIDTH: u16 = 8;

// ---------------------------------------------------------------------------
// Local attachments (`local-attachment.ts`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    Missing,
    TooLarge,
}

/// The three filesystem reads the attachment reader needs
/// (`local-attachment.ts:7-11`).
pub trait LocalFiles {
    fn read_text(&self, path: &str, max_bytes: usize) -> Result<String, AttachmentError>;
    fn read_bytes(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, AttachmentError>;
    fn mime(&self, path: &str) -> Result<String, AttachmentError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalAttachment {
    /// SVG is carried as text; every other supported MIME is binary
    /// (`local-attachment.ts:168-175`).
    Text { mime: String, content: String },
    Binary { mime: String, content: Vec<u8> },
}

impl LocalAttachment {
    #[must_use]
    pub fn mime(&self) -> &str {
        match self {
            Self::Text { mime, .. } | Self::Binary { mime, .. } => mime,
        }
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        match self {
            Self::Text { content, .. } => content.len(),
            Self::Binary { content, .. } => content.len(),
        }
    }
}

/// `local-attachment.ts:61-70`
#[must_use]
pub fn mime_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension {
        ".avif" => "image/avif",
        ".gif" => "image/gif",
        ".jpeg" => "image/jpeg",
        ".jpg" => "image/jpeg",
        ".pdf" => "application/pdf",
        ".png" => "image/png",
        ".svg" => "image/svg+xml",
        ".webp" => "image/webp",
        _ => return None,
    })
}

/// `local-attachment.ts:160-176`: unsupported MIME, missing/oversized content
/// and read failures all collapse to `None`.
#[must_use]
pub fn read_local_attachment_with<F: LocalFiles>(
    files: &F,
    path: &str,
    max_bytes: usize,
) -> Option<LocalAttachment> {
    let mime = files.mime(path).ok()?;
    if !mime.starts_with("image/") && mime != "application/pdf" {
        return None;
    }
    if mime == "image/svg+xml" {
        let content = files.read_text(path, max_bytes).ok()?;
        if content.len() > max_bytes {
            return None;
        }
        return Some(LocalAttachment::Text { mime, content });
    }
    let content = files.read_bytes(path, max_bytes).ok()?;
    if content.len() > max_bytes {
        return None;
    }
    Some(LocalAttachment::Binary { mime, content })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedAttachment {
    Text { content: String, filename: String },
    File { uri: String, filename: String },
}

/// `local-attachment.ts:44-58`: binary attachments become data URIs.
#[must_use]
pub fn data_uri(mime: &str, content: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(content))
}

#[must_use]
pub fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

/// `local-attachment.ts:29-59`: a single pasted path wins; otherwise a
/// multi-path paste is read sequentially under one shared byte budget.
#[must_use]
pub fn resolve_pasted_attachments<F: LocalFiles>(
    files: &F,
    text: &str,
    platform: &str,
) -> Option<Vec<ResolvedAttachment>> {
    let pasted = text.trim();
    let filepath = normalize_pasted_filepath(pasted, platform);
    if filepath.starts_with("http://") || filepath.starts_with("https://") {
        return None;
    }
    let mut attachments: Vec<(String, LocalAttachment)> = Vec::new();
    match read_local_attachment_with(files, &filepath, MAX_LOCAL_ATTACHMENT_BYTES) {
        Some(attachment) => attachments.push((filepath, attachment)),
        None => {
            let filepaths = parse_pasted_filepaths(pasted, platform);
            if filepaths.len() <= 1 {
                return None;
            }
            let mut remaining = MAX_LOCAL_ATTACHMENT_BYTES;
            for candidate in filepaths {
                let next = read_local_attachment_with(files, &candidate, remaining)?;
                remaining = remaining.saturating_sub(next.byte_len());
                attachments.push((candidate, next));
            }
        }
    }
    Some(
        attachments
            .into_iter()
            .map(|(filepath, attachment)| {
                let filename = basename(&filepath, platform);
                match attachment {
                    LocalAttachment::Text { content, .. } => {
                        ResolvedAttachment::Text { content, filename }
                    }
                    LocalAttachment::Binary { mime, content } => ResolvedAttachment::File {
                        uri: data_uri(&mime, &content),
                        filename,
                    },
                }
            })
            .collect(),
    )
}

#[must_use]
pub fn basename(path: &str, platform: &str) -> String {
    let separator = if platform == "win32" { '\\' } else { '/' };
    path.rsplit(separator).next().unwrap_or(path).to_string()
}

/// `local-attachment.ts:81-87`
#[must_use]
pub fn normalize_pasted_filepath(value: &str, platform: &str) -> String {
    let raw = value.trim_matches(|character| character == '\'' || character == '"');
    if let Some(url) = decode_file_url(raw, platform) {
        return url;
    }
    if platform == "win32" {
        return raw.to_string();
    }
    unescape_backslashes(raw)
}

fn unescape_backslashes(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(next) = characters.next() {
                output.push(next);
            }
        } else {
            output.push(character);
        }
    }
    output
}

/// `local-attachment.ts:89-105`
#[must_use]
pub fn decode_file_url(value: &str, platform: &str) -> Option<String> {
    let rest = value.strip_prefix("file://")?;
    let rest = rest.split(|character| character == '?' || character == '#').next().unwrap_or(rest);
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let lowercase = path.to_ascii_lowercase();
    if lowercase.contains("%2f") || lowercase.contains("%5c") {
        return None;
    }
    let pathname = percent_decode(path)?;
    if platform != "win32" {
        if !authority.is_empty() && authority != "localhost" {
            return None;
        }
        return Some(pathname);
    }
    let mut local = pathname;
    if local.len() >= 3
        && local.as_bytes()[0] == b'/'
        && local.as_bytes()[2] == b':'
        && local.as_bytes()[1].is_ascii_alphabetic()
    {
        local.remove(0);
    }
    let local = local.replace('/', "\\");
    if !authority.is_empty() && authority != "localhost" {
        Some(format!("\\\\{authority}{local}"))
    } else {
        Some(local)
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            output.push(byte);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

/// `local-attachment.ts:107-158`: shell-style quoted whitespace splitting
/// with a hard 32-path ceiling; an unclosed quote yields no paths.
#[must_use]
pub fn parse_pasted_filepaths(value: &str, platform: &str) -> Vec<String> {
    let input = if value.contains("file://") {
        value
            .split('\n')
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        value.to_string()
    };
    let characters: Vec<char> = input.chars().collect();
    let mut result: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut index = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if let Some(active) = quote {
            if character == active {
                quote = None;
                index += 1;
                continue;
            }
            if character == '\\'
                && platform != "win32"
                && active == '"'
                && index + 1 < characters.len()
            {
                current.push(characters[index + 1]);
                index += 2;
                continue;
            }
            current.push(character);
            index += 1;
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            index += 1;
            continue;
        }
        if character == '\\' && platform != "win32" && index + 1 < characters.len() {
            current.push(characters[index + 1]);
            index += 2;
            continue;
        }
        if character.is_whitespace() {
            push_path(&mut result, &mut current, platform);
            if result.len() > MAX_PASTED_FILEPATHS {
                return Vec::new();
            }
            index += 1;
            continue;
        }
        current.push(character);
        index += 1;
    }
    if quote.is_some() {
        return Vec::new();
    }
    push_path(&mut result, &mut current, platform);
    if result.len() > MAX_PASTED_FILEPATHS {
        return Vec::new();
    }
    result
}

fn push_path(result: &mut Vec<String>, current: &mut String, platform: &str) {
    if current.is_empty() {
        return;
    }
    let value = std::mem::take(current);
    result.push(decode_file_url(&value, platform).unwrap_or(value));
}

// ---------------------------------------------------------------------------
// Preview dialog (`dialog-image-preview.tsx`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewImage {
    pub uri: String,
    pub mention: Option<String>,
}

impl PreviewImage {
    #[must_use]
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            mention: None,
        }
    }

    #[must_use]
    pub fn with_mention(uri: impl Into<String>, mention: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            mention: Some(mention.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewSize {
    XLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewDialogSpec {
    pub id: &'static str,
    pub image_id: &'static str,
    pub size: PreviewSize,
    pub centered: bool,
}

/// `dialog-image-preview.tsx:21-22,75`
pub const PREVIEW_DIALOG: PreviewDialogSpec = PreviewDialogSpec {
    id: "prompt-image-viewer",
    image_id: "prompt-image-viewer-image",
    size: PreviewSize::XLarge,
    centered: true,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewAction {
    pub title: &'static str,
    pub key: &'static str,
}

/// Resource-load phase of the preview image (`diff-viewer-image.tsx:36-42`):
/// the bytes decode asynchronously, so the surface is `Loading` until the image
/// resource resolves and then `Ready`. The pinned dialog itself carries only a
/// `failed` signal (`dialog-image-preview.tsx:17`); this mirrors the load phase
/// MEDIA-001 ("loading, ready, ... error") requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewLoad {
    Loading,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagePreviewState {
    images: Vec<PreviewImage>,
    index: usize,
    failed: bool,
    load: PreviewLoad,
}

impl ImagePreviewState {
    /// `prompt/index.tsx:403-406`: opening with no images is a no-op.
    #[must_use]
    pub fn open(images: Vec<PreviewImage>, initial: i64) -> Option<Self> {
        if images.is_empty() {
            return None;
        }
        let max = (images.len() - 1) as i64;
        let index = initial.clamp(0, max) as usize;
        Some(Self {
            images,
            index,
            failed: false,
            load: PreviewLoad::Ready,
        })
    }

    #[must_use]
    pub fn images(&self) -> &[PreviewImage] {
        &self.images
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn current(&self) -> &PreviewImage {
        &self.images[self.index]
    }

    #[must_use]
    pub fn can_navigate(&self) -> bool {
        self.images.len() > 1
    }

    /// `dialog-image-preview.tsx:24-28`: navigation is disabled for a single
    /// image and wraps; moving clears a previous failure.
    pub fn move_by(&mut self, direction: i64) {
        if !self.can_navigate() {
            return;
        }
        self.failed = false;
        let length = self.images.len() as i64;
        self.index = ((self.index as i64 + direction).rem_euclid(length)) as usize;
    }

    pub fn mark_failed(&mut self) {
        self.failed = true;
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Mark the image as still decoding; the surface renders the muted
    /// `Loading image…` slot (`diff-viewer-image.tsx:38-40`) and paints no box.
    pub fn mark_loading(&mut self) {
        self.load = PreviewLoad::Loading;
    }

    /// Mark the image as decoded (`diff-viewer-image.tsx:41-88`).
    pub fn mark_ready(&mut self) {
        self.load = PreviewLoad::Ready;
    }

    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.load == PreviewLoad::Loading
    }

    /// `dialog-image-preview.tsx:41`
    #[must_use]
    pub fn header(&self) -> String {
        format!("Image {} of {}", self.index + 1, self.images.len())
    }

    /// `dialog-image-preview.tsx:55,66`
    #[must_use]
    pub fn footer_label(&self) -> String {
        if self.failed {
            return "No preview".to_string();
        }
        self.current()
            .mention
            .clone()
            .unwrap_or_else(|| format!("Image {}", self.index + 1))
    }

    /// `dialog-image-preview.tsx:43-51`
    #[must_use]
    pub fn actions(&self) -> Vec<PreviewAction> {
        if !self.can_navigate() {
            return Vec::new();
        }
        vec![
            PreviewAction {
                title: "previous",
                key: "←",
            },
            PreviewAction {
                title: "next",
                key: "→",
            },
        ]
    }

    /// `dialog-image-preview.tsx:19`: `max(3, height - 8)`.
    #[must_use]
    pub fn image_height(terminal_height: u16) -> u16 {
        terminal_height
            .saturating_sub(PREVIEW_VERTICAL_CHROME)
            .max(PREVIEW_IMAGE_MIN_HEIGHT)
    }
}

// ---------------------------------------------------------------------------
// Preview surface: fit/protocol negotiation, body box, caption and actions
// ---------------------------------------------------------------------------
//
// This is the renderable half of the preview dialog: the rows a page emits for
// an image part, plus the cell box the painter composites the real image into.
// All geometry is computed once here; the page (crate::native_view_pages)
// renders these rows and never recomputes the box.
//
// Upstream evidence (pin 450e7b92412c780a7b5a75226bb8d49aaca24e22):
// - `component/dialog-image-preview.tsx:70-80`: the body is a single `<image>`
//   with `fit="fit"`, `protocol="auto"`, `width="100%"` and
//   `height={max(3, height - 8)}` (`:19`).
// - `component/dialog-image-preview.tsx:52` header `Image N of M`; `:54-57`
//   footer child (`No preview` in `feedback.error.base`, else the mention text
//   or `Image N` in `text.muted`); `:43-51` previous/next actions only when
//   more than one image; `:60-72` legacy footer `← previous` / `next →`.
// - `component/prompt/index.tsx:1717-1718,1732-1739`: gallery cells are
//   `height() x height()*2` and use `fit="cover"`; `:1724-1732` a decode
//   failure renders the muted `No preview` fallback instead of the image.
// - `routes/session/index.tsx:2686-2705`: the tool gallery has the same
//   `fit="cover"` cells and the same `No preview` fallback.
//
// Deviation, not parity: the pin renders through OpenTUI with
// `protocol="auto"`; the negotiated protocol lives in the native bridge, which
// this pure module cannot inspect. [`ImageProtocol::None`] therefore emits an
// explicit muted placeholder row instead of a silently blank box, so a
// terminal that cannot draw images never looks like an empty success.

/// Image drawing protocol negotiated with the terminal.
///
/// Upstream asks for `protocol="auto"` (`dialog-image-preview.tsx:77`,
/// `prompt/index.tsx:1735`, `routes/session/index.tsx:2699`) and lets OpenTUI
/// pick kitty/iTerm2/sixel. The native bridge owns that negotiation; this
/// module only needs to know whether the chosen protocol can draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    Iterm2,
    Sixel,
    /// No terminal image protocol is available.
    None,
}

impl ImageProtocol {
    #[must_use]
    pub fn draws(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// `fit` mode of an image renderable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewFit {
    /// `fit="fit"`: aspect-preserving contain inside the cell box
    /// (`dialog-image-preview.tsx:78`).
    Fit,
    /// `fit="cover"`: fill the cell box and crop the overflow
    /// (`prompt/index.tsx:1736`, `routes/session/index.tsx:2699`).
    Cover,
}

/// Decoded image size in terminal cells. Caller-fed: decoding is not modeled
/// by this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

impl ImageSize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// The cell box an image occupies inside its available area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FitBox {
    pub width: u16,
    pub height: u16,
}

/// Aspect-preserving fit of `natural` inside `avail_width` x `avail_height`.
///
/// `PreviewFit::Fit` is the OpenTUI contain behavior of
/// `dialog-image-preview.tsx:78`: the box keeps the image's aspect ratio and
/// touches the tighter of the two available edges. `PreviewFit::Cover` fills
/// the whole available box (`prompt/index.tsx:1736`). A zero available edge
/// yields a zero box; an unknown natural size (`0`) yields the whole area so
/// the painter's own fallback decides.
#[must_use]
pub fn fit_box(
    avail_width: u16,
    avail_height: u16,
    natural: ImageSize,
    fit: PreviewFit,
) -> FitBox {
    if avail_width == 0 || avail_height == 0 {
        return FitBox {
            width: 0,
            height: 0,
        };
    }
    if natural.width == 0 || natural.height == 0 {
        return FitBox {
            width: avail_width,
            height: avail_height,
        };
    }
    if fit == PreviewFit::Cover {
        return FitBox {
            width: avail_width,
            height: avail_height,
        };
    }
    let natural_width = u64::from(natural.width);
    let natural_height = u64::from(natural.height);
    // Height when the box spans the full width, rounded half up.
    let height_at_full_width =
        (u64::from(avail_width) * natural_height + natural_width / 2) / natural_width;
    if height_at_full_width <= u64::from(avail_height) {
        return FitBox {
            width: avail_width,
            height: u16::try_from(height_at_full_width).unwrap_or(avail_height).max(1),
        };
    }
    // Width when the box spans the full height, rounded half up.
    let width_at_full_height =
        (u64::from(avail_height) * natural_width + natural_height / 2) / natural_height;
    FitBox {
        width: u16::try_from(width_at_full_height)
            .unwrap_or(avail_width)
            .clamp(1, avail_width),
        height: avail_height,
    }
}

/// Role of one preview row (mapped to the page palette by the caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewRole {
    /// Dialog header (`DialogHeader title`).
    Heading,
    /// Image placeholder/body row.
    Body,
    /// Footer child: mention text or `Image N` (`text.muted`).
    Caption,
    /// `No preview` and other failure text (`feedback.error.base`).
    Error,
    /// Previous/next action row.
    Hint,
}

/// One rendered preview row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLine {
    pub text: String,
    pub role: PreviewRole,
}

/// The preview surface for one image part: dialog state plus the negotiated
/// protocol, the decoded size (when known) and the fit mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewSurface {
    state: ImagePreviewState,
    protocol: ImageProtocol,
    natural: Option<ImageSize>,
    fit: PreviewFit,
}

impl PreviewSurface {
    /// Dialog surface: `fit="fit"` (`dialog-image-preview.tsx:78`).
    #[must_use]
    pub fn dialog(
        state: ImagePreviewState,
        protocol: ImageProtocol,
        natural: Option<ImageSize>,
    ) -> Self {
        Self {
            state,
            protocol,
            natural,
            fit: PreviewFit::Fit,
        }
    }

    /// Gallery-cell surface: `fit="cover"` (`prompt/index.tsx:1736`).
    #[must_use]
    pub fn gallery(
        state: ImagePreviewState,
        protocol: ImageProtocol,
        natural: Option<ImageSize>,
    ) -> Self {
        Self {
            state,
            protocol,
            natural,
            fit: PreviewFit::Cover,
        }
    }

    #[must_use]
    pub fn state(&self) -> &ImagePreviewState {
        &self.state
    }

    /// Mutable dialog state (navigation/failure) without rebuilding geometry.
    pub fn state_mut(&mut self) -> &mut ImagePreviewState {
        &mut self.state
    }

    #[must_use]
    pub fn protocol(&self) -> ImageProtocol {
        self.protocol
    }

    pub fn set_protocol(&mut self, protocol: ImageProtocol) {
        self.protocol = protocol;
    }

    pub fn set_natural(&mut self, natural: Option<ImageSize>) {
        self.natural = natural;
    }

    #[must_use]
    pub fn fit(&self) -> PreviewFit {
        self.fit
    }

    /// Visible body rows for a surface of `width` x `height` cells: the header
    /// always, the footer child always, the action row when navigation exists
    /// and the remaining rows are the image box.
    #[must_use]
    pub fn body_rows(&self, width: u16, height: u16) -> u16 {
        if width == 0 || height == 0 {
            return 0;
        }
        let actions = u16::from(self.state.can_navigate());
        height.saturating_sub(1 + 1 + actions)
    }

    /// Cell box the painter composites the decoded image into. `None` when the
    /// surface has no body rows at this height, or when the terminal cannot
    /// draw images at all (the placeholder row is rendered instead).
    #[must_use]
    pub fn image_box(&self, width: u16, height: u16) -> Option<FitBox> {
        let body = self.body_rows(width, height);
        if body == 0 || !self.protocol.draws() || self.state.failed() || self.state.is_loading() {
            return None;
        }
        let natural = self.natural.unwrap_or(ImageSize::new(0, 0));
        Some(fit_box(width, body, natural, self.fit))
    }

    /// Render the surface as bounded rows, never splitting a character.
    #[must_use]
    pub fn rows(&self, width: u16, height: u16) -> Vec<PreviewLine> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let clip = |text: &str| clip_chars(text, usize::from(width));
        let mut out = vec![PreviewLine {
            text: clip(&self.state.header()),
            role: PreviewRole::Heading,
        }];
        if height == 1 {
            return out;
        }
        let caption = PreviewLine {
            text: clip(&self.state.footer_label()),
            role: if self.state.failed() {
                PreviewRole::Error
            } else {
                PreviewRole::Caption
            },
        };
        let actions = self.state.can_navigate().then(|| PreviewLine {
            // Legacy footer bindings (`dialog-image-preview.tsx:60-72`).
            text: clip("← previous    next →"),
            role: PreviewRole::Hint,
        });
        let body_rows = self.body_rows(width, height);
        for _ in 0..body_rows {
            out.push(PreviewLine {
                text: String::new(),
                role: PreviewRole::Body,
            });
        }
        if self.state.failed() {
            // A decode failure replaces the image with the muted fallback
            // (`prompt/index.tsx:1724-1732`); the caption stays the error.
            if let Some(row) = out.get_mut(1) {
                row.text = centered("No preview", width);
                row.role = PreviewRole::Error;
            }
        } else if self.state.is_loading() && body_rows > 0 {
            // Decoding in progress (`diff-viewer-image.tsx:38-40`): a muted
            // slot, never the error fallback. The footer caption is unaffected.
            if let Some(row) = out.get_mut(1) {
                row.text = centered("Loading image…", width);
                row.role = PreviewRole::Caption;
            }
        } else if !self.protocol.draws() && body_rows > 0 {
            if let Some(row) = out.get_mut(1) {
                row.text = centered(
                    "image preview unavailable: terminal cannot draw images",
                    width,
                );
                row.role = PreviewRole::Error;
            }
        }
        out.push(caption);
        if let Some(actions) = actions {
            if out.len() < usize::from(height) {
                out.push(actions);
            }
        }
        out.truncate(usize::from(height));
        out
    }
}

/// Column-bounded cut by characters (never splits a character).
fn clip_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Center `text` in exactly `width` columns: leading spaces, then the text,
/// then trailing spaces; a text wider than `width` is cut at the column bound.
/// Upstream centers cell fallbacks with
/// `alignItems="center" justifyContent="center"` (`prompt/index.tsx:1724-1730`).
fn centered(text: &str, width: u16) -> String {
    let width = usize::from(width);
    let clipped = clip_chars(text, width);
    let text_width = clipped.chars().count();
    let left = width.saturating_sub(text_width) / 2;
    let right = width.saturating_sub(text_width + left);
    format!("{}{}{}", " ".repeat(left), clipped, " ".repeat(right))
}

// ---------------------------------------------------------------------------
// Gallery (`prompt/index.tsx:1699-1767`, `routes/session/index.tsx:2666-2769`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GallerySource {
    /// Composer gallery: image cells and `+N more` open the dialog.
    User,
    /// Tool-result gallery: image cells open the dialog; `+N more` has no
    /// click handler upstream.
    Tool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GalleryClick {
    None,
    Open {
        initial: usize,
        stop_propagation: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GalleryCell {
    Image {
        index: usize,
        width: u16,
        height: u16,
    },
    Overflow {
        count: usize,
        width: u16,
        height: u16,
        opens_preview: bool,
    },
}

impl GalleryCell {
    /// Left clicks on image cells open the preview at that index and stop
    /// propagation (`prompt/index.tsx:1727-1734`,
    /// `routes/session/index.tsx:2688-2694`). The tool-gallery overflow cell
    /// has no handler, so its click is inert and does not bubble.
    #[must_use]
    pub fn click(self, button: MouseButton) -> GalleryClick {
        if button != MouseButton::Left {
            return GalleryClick::None;
        }
        match self {
            Self::Image { index, .. } => GalleryClick::Open {
                initial: index,
                stop_propagation: true,
            },
            Self::Overflow {
                opens_preview: true,
                ..
            } => GalleryClick::Open {
                initial: GALLERY_VISIBLE_LIMIT,
                stop_propagation: true,
            },
            Self::Overflow { .. } => GalleryClick::None,
        }
    }

    #[must_use]
    pub fn width(self) -> u16 {
        match self {
            Self::Image { width, .. } | Self::Overflow { width, .. } => width,
        }
    }

    #[must_use]
    pub fn height(self) -> u16 {
        match self {
            Self::Image { height, .. } | Self::Overflow { height, .. } => height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalleryModel {
    pub enabled: bool,
    pub height: u16,
    pub cell_width: u16,
    pub cells: Vec<GalleryCell>,
    /// Images beyond the three visible cells.
    pub hidden: usize,
}

impl GalleryModel {
    #[must_use]
    pub fn empty(enabled: bool, terminal_height: u16) -> Self {
        Self {
            enabled,
            height: gallery_height(terminal_height),
            cell_width: gallery_cell_width(gallery_height(terminal_height)),
            cells: Vec::new(),
            hidden: 0,
        }
    }
}

/// `prompt/index.tsx:399`: `max(4, min(8, floor(height / 4)))`.
#[must_use]
pub fn gallery_height(terminal_height: u16) -> u16 {
    (terminal_height / 4).clamp(GALLERY_MIN_HEIGHT, GALLERY_MAX_HEIGHT)
}

/// `prompt/index.tsx:400`
#[must_use]
pub fn gallery_cell_width(height: u16) -> u16 {
    height.saturating_mul(2)
}

/// Builds the gallery for the given source. `enabled` is the
/// `prompt.image_preview` / `session.image_preview` config gate
/// (`prompt/index.tsx:1699`, `routes/session/index.tsx:2663`).
#[must_use]
pub fn gallery_model(
    images: &[PreviewImage],
    enabled: bool,
    terminal_height: u16,
    source: GallerySource,
) -> GalleryModel {
    if !enabled || images.is_empty() {
        return GalleryModel::empty(enabled, terminal_height);
    }
    let height = gallery_height(terminal_height);
    let width = gallery_cell_width(height);
    let visible = images.len().min(GALLERY_VISIBLE_LIMIT);
    let mut cells: Vec<GalleryCell> = (0..visible)
        .map(|index| GalleryCell::Image {
            index,
            width,
            height,
        })
        .collect();
    let hidden = images.len() - visible;
    if hidden > 0 {
        cells.push(GalleryCell::Overflow {
            count: hidden,
            width: GALLERY_OVERFLOW_WIDTH,
            height,
            opens_preview: source == GallerySource::User,
        });
    }
    GalleryModel {
        enabled,
        height,
        cell_width: width,
        cells,
        hidden,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptFile {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mention: Option<String>,
}

/// `prompt/attachment.ts:40-50`: duplicate image data URIs with identical
/// name/description/mention metadata are dropped; non-image or mentionless
/// entries always pass through.
#[must_use]
pub fn deduplicate_prompt_images(files: &[PromptFile]) -> Vec<PromptFile> {
    if files.len() < 2 {
        return files.to_vec();
    }
    let mut seen: HashMap<(Option<String>, Option<String>, Option<String>), Vec<String>> =
        HashMap::new();
    files
        .iter()
        .filter(|file| {
            if !file.uri.starts_with("data:image/") || file.mention.is_none() {
                return true;
            }
            let key = (
                file.name.clone(),
                file.description.clone(),
                file.mention.clone(),
            );
            let payloads = seen.entry(key).or_default();
            if payloads.iter().any(|uri| uri == &file.uri) {
                return false;
            }
            payloads.push(file.uri.clone());
            true
        })
        .cloned()
        .collect()
}

/// `prompt/index.tsx:397`: only `data:image/` attachments reach the gallery.
#[must_use]
pub fn image_attachments(files: &[PromptFile]) -> Vec<PreviewImage> {
    deduplicate_prompt_images(files)
        .into_iter()
        .filter(|file| file.uri.starts_with("data:image/"))
        .map(|file| PreviewImage {
            uri: file.uri,
            mention: file.mention,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolContent {
    pub kind: Option<String>,
    pub mime: Option<String>,
    pub uri: Option<String>,
}

/// `routes/session/index.tsx:2721-2726`
#[must_use]
pub fn inline_tool_images(contents: &[ToolContent]) -> Vec<PreviewImage> {
    contents
        .iter()
        .filter(|content| {
            content.kind.as_deref() == Some("file")
                && content
                    .mime
                    .as_deref()
                    .is_some_and(|mime| mime.starts_with("image/"))
                && content
                    .uri
                    .as_deref()
                    .is_some_and(|uri| uri.starts_with("data:image/"))
        })
        .filter_map(|content| content.uri.clone().map(PreviewImage::new))
        .collect()
}

/// MEDIA-001 live bridge: map a `ToolState` part's bounded `metadata` JSON to
/// the tool gallery model. Upstream reads `state.content`
/// (`util/tool-display.ts:59` `toolDisplayContent`, consumed by
/// `inlineToolImages` at `routes/session/index.tsx:2721-2726`); this port
/// carries no separate content field, so a `content` array on the real tool
/// metadata object is projected to [`ToolContent`] and filtered by
/// [`inline_tool_images`]. Empty or malformed metadata yields no images; nothing
/// is fabricated.
#[must_use]
pub fn inline_tool_images_from_metadata(metadata: &str) -> Vec<PreviewImage> {
    if metadata.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(metadata) else {
        return Vec::new();
    };
    let Some(items) = value.get("content").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let contents: Vec<ToolContent> = items
        .iter()
        .map(|item| ToolContent {
            kind: item
                .get("type")
                .or_else(|| item.get("kind"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            mime: item
                .get("mime")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            uri: item
                .get("uri")
                .or_else(|| item.get("url"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    inline_tool_images(&contents)
}

/// `prompt/attachment.ts:14-18`
#[must_use]
pub fn attachment_kind(uri: &str) -> Option<&'static str> {
    if uri.starts_with("data:image/") {
        Some("Image")
    } else if uri.starts_with("data:application/pdf;") {
        Some("PDF")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemoryFiles {
        files: HashMap<String, (String, Vec<u8>)>,
    }

    impl MemoryFiles {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
            }
        }

        fn insert(&mut self, path: &str, mime: &str, bytes: Vec<u8>) {
            self.files.insert(path.to_string(), (mime.to_string(), bytes));
        }
    }

    impl LocalFiles for MemoryFiles {
        fn read_text(&self, path: &str, max_bytes: usize) -> Result<String, AttachmentError> {
            let (_, bytes) = self.files.get(path).ok_or(AttachmentError::Missing)?;
            if bytes.len() > max_bytes {
                return Err(AttachmentError::TooLarge);
            }
            String::from_utf8(bytes.clone()).map_err(|_| AttachmentError::Missing)
        }

        fn read_bytes(&self, path: &str, max_bytes: usize) -> Result<Vec<u8>, AttachmentError> {
            let (_, bytes) = self.files.get(path).ok_or(AttachmentError::Missing)?;
            if bytes.len() > max_bytes {
                return Err(AttachmentError::TooLarge);
            }
            Ok(bytes.clone())
        }

        fn mime(&self, path: &str) -> Result<String, AttachmentError> {
            self.files
                .get(path)
                .map(|(mime, _)| mime.clone())
                .ok_or(AttachmentError::Missing)
        }
    }

    fn images(count: usize) -> Vec<PreviewImage> {
        (0..count)
            .map(|index| PreviewImage::with_mention(format!("data:image/png;base64,{index}"), format!("[Image {}]", index + 1)))
            .collect()
    }

    // MEDIA-001 (authored case; the kit has no upstream fixture): preview
    // off/on, 0/1/3/4 images, failed image, short/tall terminals, user/tool
    // previews, gallery click and non-bubbling.
    #[test]
    fn media001_image_preview_and_gallery_states() {
        // Off: nothing renders even when images exist.
        assert!(gallery_model(&images(4), false, 24, GallerySource::User)
            .cells
            .is_empty());

        // Zero images: opening is a no-op.
        assert!(ImagePreviewState::open(Vec::new(), 0).is_none());
        assert!(gallery_model(&[], true, 24, GallerySource::User)
            .cells
            .is_empty());

        // One image: clamped index, no actions, no wrap.
        let mut single = ImagePreviewState::open(images(1), 5).expect("one image");
        assert_eq!(single.index(), 0);
        assert_eq!(single.header(), "Image 1 of 1");
        assert_eq!(single.footer_label(), "[Image 1]");
        assert!(single.actions().is_empty());
        single.move_by(1);
        assert_eq!(single.index(), 0);

        // Three images: all three visible, no overflow cell.
        let three = gallery_model(&images(3), true, 24, GallerySource::User);
        assert_eq!(three.cells.len(), 3);
        assert_eq!(three.hidden, 0);
        assert_eq!(three.height, 6, "24 / 4");
        assert_eq!(three.cell_width, 12);

        // Four images: +1 more cell, width 8; user overflow opens index 3.
        let four = gallery_model(&images(4), true, 24, GallerySource::User);
        assert_eq!(four.cells.len(), 4);
        assert_eq!(four.hidden, 1);
        let overflow = four.cells[3];
        assert_eq!(overflow.width(), GALLERY_OVERFLOW_WIDTH);
        assert_eq!(
            overflow.click(MouseButton::Left),
            GalleryClick::Open {
                initial: 3,
                stop_propagation: true
            }
        );
        assert_eq!(
            four.cells[1].click(MouseButton::Left),
            GalleryClick::Open {
                initial: 1,
                stop_propagation: true
            }
        );
        assert_eq!(four.cells[1].click(MouseButton::Other), GalleryClick::None);

        // Tool previews: same three visible cells but the overflow is inert.
        let tool = gallery_model(&images(4), true, 24, GallerySource::Tool);
        assert_eq!(tool.cells.len(), 4);
        assert_eq!(tool.cells[3].click(MouseButton::Left), GalleryClick::None);

        // Failed image: footer switches to `No preview`; navigating (when
        // multiple images exist) clears the failure.
        let mut multiple = ImagePreviewState::open(images(3), 2).expect("three images");
        assert_eq!(multiple.header(), "Image 3 of 3");
        multiple.mark_failed();
        assert!(multiple.failed());
        assert_eq!(multiple.footer_label(), "No preview");
        multiple.move_by(1);
        assert_eq!(multiple.index(), 0, "wraps 2 -> 0");
        assert!(!multiple.failed());

        // Short and tall terminals.
        assert_eq!(ImagePreviewState::image_height(8), 3);
        assert_eq!(ImagePreviewState::image_height(10), 3);
        assert_eq!(ImagePreviewState::image_height(24), 16);
        assert_eq!(ImagePreviewState::image_height(60), 52);
        assert_eq!(gallery_height(10), 4);
        assert_eq!(gallery_height(16), 4);
        assert_eq!(gallery_height(24), 6);
        assert_eq!(gallery_height(60), 8);

        // Dialog contract.
        assert_eq!(PREVIEW_DIALOG.id, "prompt-image-viewer");
        assert!(PREVIEW_DIALOG.centered);
        assert_eq!(PREVIEW_DIALOG.size, PreviewSize::XLarge);

        // User previews dedupe identical image data URIs by metadata+payload.
        let duplicate = PromptFile {
            uri: "data:image/png;base64,AAAA".into(),
            name: Some("a.png".into()),
            description: None,
            mention: Some("[Image 1]".into()),
        };
        let distinct_payload = PromptFile {
            uri: "data:image/png;base64,BBBB".into(),
            ..duplicate.clone()
        };
        let mentionless = PromptFile {
            uri: "data:image/png;base64,AAAA".into(),
            mention: None,
            ..duplicate.clone()
        };
        let non_image = PromptFile {
            uri: "data:application/pdf;base64,AAAA".into(),
            mention: Some("[PDF 1]".into()),
            ..duplicate.clone()
        };
        let deduped = deduplicate_prompt_images(&[
            duplicate.clone(),
            duplicate.clone(),
            distinct_payload,
            mentionless,
            non_image,
        ]);
        assert_eq!(deduped.len(), 4);
        assert_eq!(image_attachments(&deduped).len(), 3);
        assert_eq!(attachment_kind("data:application/pdf;base64,x"), Some("PDF"));

        // Tool content filtering keeps only file image data URIs.
        let tool_images = inline_tool_images(&[
            ToolContent {
                kind: Some("file".into()),
                mime: Some("image/png".into()),
                uri: Some("data:image/png;base64,AAAA".into()),
            },
            ToolContent {
                kind: Some("text".into()),
                mime: Some("image/png".into()),
                uri: Some("data:image/png;base64,BBBB".into()),
            },
            ToolContent {
                kind: Some("file".into()),
                mime: Some("application/pdf".into()),
                uri: Some("data:application/pdf;base64,CCCC".into()),
            },
        ]);
        assert_eq!(tool_images.len(), 1);
        assert_eq!(tool_images[0].uri, "data:image/png;base64,AAAA");
    }

    // MEDIA-001 attachment supply: 20 MiB budget, SVG-as-text, data URIs,
    // pasted path parsing and file-URL normalisation.
    #[test]
    fn media001_local_attachment_and_paste_model() {
        assert_eq!(MAX_LOCAL_ATTACHMENT_BYTES, 20 * 1024 * 1024);
        assert_eq!(mime_for_extension(".png"), Some("image/png"));
        assert_eq!(mime_for_extension(".PNG"), None, "upstream lowercases first");
        assert_eq!(mime_for_extension(".txt"), None);
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(&[0x00, 0xff]), "AP8=");

        let mut files = MemoryFiles::new();
        files.insert("/tmp/shot.png", "image/png", vec![1, 2, 3]);
        files.insert("/tmp/icon.svg", "image/svg+xml", b"<svg/>".to_vec());
        files.insert("/tmp/notes.txt", "text/plain", b"hi".to_vec());
        files.insert("/tmp/huge.png", "image/png", vec![0; 32]);

        let png = read_local_attachment_with(&files, "/tmp/shot.png", MAX_LOCAL_ATTACHMENT_BYTES)
            .expect("png reads");
        assert_eq!(png.mime(), "image/png");
        assert!(matches!(png, LocalAttachment::Binary { .. }));
        assert_eq!(
            resolve_pasted_attachments(&files, "/tmp/shot.png", "linux")
                .expect("single attachment")[0],
            ResolvedAttachment::File {
                uri: "data:image/png;base64,AQID".into(),
                filename: "shot.png".into()
            }
        );
        let svg = read_local_attachment_with(&files, "/tmp/icon.svg", MAX_LOCAL_ATTACHMENT_BYTES)
            .expect("svg reads");
        assert!(matches!(svg, LocalAttachment::Text { .. }));
        assert!(read_local_attachment_with(&files, "/tmp/notes.txt", MAX_LOCAL_ATTACHMENT_BYTES).is_none());
        assert!(read_local_attachment_with(&files, "/tmp/missing.png", 10).is_none());
        assert!(read_local_attachment_with(&files, "/tmp/huge.png", 8).is_none());
        assert!(resolve_pasted_attachments(&files, "https://example.com/a.png", "linux").is_none());

        // Quoted multi-path paste shares one 20 MiB budget.
        let pasted = resolve_pasted_attachments(
            &files,
            "'/tmp/shot.png' \"/tmp/icon.svg\"",
            "linux",
        )
        .expect("two paths");
        assert_eq!(pasted.len(), 2);
        assert!(matches!(pasted[1], ResolvedAttachment::Text { .. }));

        assert_eq!(
            normalize_pasted_filepath("file:///tmp/a%20b.png", "linux"),
            "/tmp/a b.png"
        );
        assert_eq!(
            normalize_pasted_filepath("file://server/share/a.png", "linux"),
            "file://server/share/a.png"
        );
        assert_eq!(
            normalize_pasted_filepath("file:///C:/Users/a.png", "win32"),
            "C:\\Users\\a.png"
        );
        assert_eq!(
            normalize_pasted_filepath("file://server/share/a.png", "win32"),
            "\\\\server\\share\\a.png"
        );
        assert_eq!(normalize_pasted_filepath("'/tmp/a b.png'", "linux"), "/tmp/a b.png");
        assert_eq!(normalize_pasted_filepath("/tmp/a\\ b.png", "linux"), "/tmp/a b.png");

        assert_eq!(
            parse_pasted_filepaths("'/tmp/a b.png' /tmp/c.png", "linux"),
            vec!["/tmp/a b.png".to_string(), "/tmp/c.png".to_string()]
        );
        assert!(parse_pasted_filepaths("'/tmp/unclosed.png", "linux").is_empty());
        let too_many = (0..33)
            .map(|index| format!("/tmp/{index}.png"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(parse_pasted_filepaths(&too_many, "linux").is_empty());
        let file_url_list = "# comment\nfile:///tmp/a.png\nfile:///tmp/b.png\n";
        assert_eq!(
            parse_pasted_filepaths(file_url_list, "linux"),
            vec!["/tmp/a.png".to_string(), "/tmp/b.png".to_string()]
        );
    }

    // MEDIA-001 surface: the preview dialog rows a page emits — header,
    // aspect-fit image box, caption, failure placeholder, unsupported
    // protocol placeholder, actions only when multiple images.
    #[test]
    fn media001_preview_surface_geometry_caption_and_fallbacks() {
        // Aspect-preserving contain (`dialog-image-preview.tsx:78`).
        let wide = fit_box(40, 20, ImageSize::new(16, 9), PreviewFit::Fit);
        assert_eq!(wide.height, 20, "taller than the 16:9 box, so height binds");
        assert_eq!(wide.width, 36, "20 * 16 / 9 rounded");
        let square = fit_box(90, 30, ImageSize::new(100, 100), PreviewFit::Fit);
        assert_eq!((square.width, square.height), (30, 30));
        let exact = fit_box(40, 10, ImageSize::new(4, 1), PreviewFit::Fit);
        assert_eq!((exact.width, exact.height), (40, 10));
        // Cover fills the requested box (`prompt/index.tsx:1736`).
        let cover = fit_box(40, 20, ImageSize::new(16, 9), PreviewFit::Cover);
        assert_eq!((cover.width, cover.height), (40, 20));
        // Unknown natural size and empty available area.
        let unknown = fit_box(10, 4, ImageSize::new(0, 0), PreviewFit::Fit);
        assert_eq!((unknown.width, unknown.height), (10, 4));
        let empty = fit_box(0, 4, ImageSize::new(16, 9), PreviewFit::Fit);
        assert_eq!((empty.width, empty.height), (0, 0));
        // Extreme ratios still occupy at least one cell on the short side.
        let sliver = fit_box(200, 20, ImageSize::new(10_000, 1), PreviewFit::Fit);
        assert_eq!(sliver.height, 1);
        assert_eq!(sliver.width, 200);

        let three = images(3);
        let state = ImagePreviewState::open(three, 1).expect("three images");
        let mut surface = PreviewSurface::dialog(state, ImageProtocol::Kitty, Some(ImageSize::new(16, 9)));
        assert_eq!(surface.fit(), PreviewFit::Fit);
        assert_eq!(surface.protocol(), ImageProtocol::Kitty);
        // Header always first; body box honours the header+caption+action rows.
        assert_eq!(surface.body_rows(80, 24), 21);
        let box_ = surface.image_box(80, 24).expect("drawable image box");
        assert_eq!(box_.width, 37, "16:9 contain inside 80 x 21");
        assert_eq!(box_.height, 21);

        let rows = surface.rows(80, 24);
        assert_eq!(rows.len(), 24);
        assert_eq!(rows[0].text, "Image 2 of 3");
        assert_eq!(rows[0].role, PreviewRole::Heading);
        // Body rows reserve the image area.
        assert!(rows[1..22].iter().all(|row| row.text.is_empty()));
        // Footer child: the mention text (`dialog-image-preview.tsx:56`).
        assert_eq!(rows[22].text, "[Image 2]");
        assert_eq!(rows[22].role, PreviewRole::Caption);
        // Multiple images get the previous/next action row (`:43-51,60-72`).
        assert_eq!(rows[23].text, "← previous    next →");
        assert_eq!(rows[23].role, PreviewRole::Hint);

        // A single image: no actions, so the body grows by one row.
        let single = PreviewSurface::dialog(
            ImagePreviewState::open(images(1), 0).expect("one image"),
            ImageProtocol::Kitty,
            None,
        );
        assert_eq!(single.body_rows(80, 24), 22);
        let rows = single.rows(80, 24);
        assert_eq!(rows.len(), 24, "rows stay exactly the surface height");
        assert_eq!(rows[0].text, "Image 1 of 1");
        assert!(!rows.iter().any(|row| row.text.contains("previous")));

        // Decode failure: `No preview` replaces the body and colours the
        // caption as an error (`:54-57`, `prompt/index.tsx:1724-1732`).
        surface.state_mut().mark_failed();
        assert!(surface.image_box(80, 24).is_none(), "no box for a failed decode");
        let rows = surface.rows(80, 24);
        assert_eq!(rows[1].role, PreviewRole::Error);
        assert!(rows[1].text.contains("No preview"));
        assert_eq!(rows[22].text, "No preview");
        assert_eq!(rows[22].role, PreviewRole::Error);
        surface.state_mut().move_by(1);
        assert!(!surface.state().failed());
        assert_eq!(surface.rows(80, 24)[22].text, "[Image 3]");

        // Terminal without an image protocol: explicit placeholder, no box.
        let mut blocked = PreviewSurface::dialog(
            ImagePreviewState::open(images(2), 0).expect("two images"),
            ImageProtocol::None,
            Some(ImageSize::new(4, 4)),
        );
        assert!(!blocked.protocol().draws());
        assert!(blocked.image_box(60, 12).is_none());
        let rows = blocked.rows(60, 12);
        assert_eq!(rows[1].role, PreviewRole::Error);
        assert!(rows[1].text.contains("terminal cannot draw images"));
        assert_eq!(rows[1].text.chars().count(), 60, "placeholder fills the row");
        // A negotiated protocol restores the box.
        blocked.set_protocol(ImageProtocol::Sixel);
        assert!(blocked.image_box(60, 12).is_some());
        // Gallery cells use cover so they always fill their cell box.
        let cell = PreviewSurface::gallery(
            ImagePreviewState::open(images(1), 0).expect("one image"),
            ImageProtocol::Iterm2,
            Some(ImageSize::new(2, 8)),
        );
        assert_eq!(cell.fit(), PreviewFit::Cover);
        // Cover fills the body area (header + caption are not image rows), so a
        // single-image cell at height 6 gets four body rows.
        assert_eq!(cell.image_box(12, 6).map(|box_| (box_.width, box_.height)), Some((12, 4)));

        // Degenerate geometry never panics and never exceeds its bound.
        for width in [0u16, 1, 2, 7] {
            for height in [0u16, 1, 2, 3, 9] {
                let rows = surface.rows(width, height);
                assert!(rows.len() <= usize::from(height));
                for row in &rows {
                    assert!(row.text.chars().count() <= usize::from(width), "{:?}", row.text);
                }
            }
        }
        assert!(surface.rows(0, 10).is_empty());
        assert!(surface.rows(10, 0).is_empty());
    }

    // MEDIA-001 loading phase: an undecoded image renders the muted
    // `Loading image…` slot, keeps the footer caption, and has no image box
    // until it becomes ready (`diff-viewer-image.tsx:36-42`).
    #[test]
    fn media001_preview_loading_slot() {
        let state = ImagePreviewState::open(images(2), 0).expect("two images");
        let mut surface =
            PreviewSurface::dialog(state, ImageProtocol::Kitty, Some(ImageSize::new(4, 4)));
        assert!(!surface.state().is_loading(), "ready by default");
        assert!(surface.image_box(60, 12).is_some());
        surface.state_mut().mark_loading();
        assert!(surface.state().is_loading());
        assert!(surface.image_box(60, 12).is_none(), "no box while decoding");
        let rows = surface.rows(60, 12);
        assert_eq!(rows[1].role, PreviewRole::Caption);
        assert!(rows[1].text.contains("Loading image"), "{:?}", rows[1].text);
        assert_eq!(rows[1].text.chars().count(), 60, "slot fills the row");
        // The footer caption is unaffected by the load phase.
        assert_eq!(rows[10].text, "[Image 1]");
        surface.state_mut().mark_ready();
        assert!(!surface.state().is_loading());
        assert!(surface.image_box(60, 12).is_some());
        assert!(surface.rows(60, 12)[1].text.is_empty(), "ready body is blank");
    }
}
