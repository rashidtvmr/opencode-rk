#![forbid(unsafe_code)]
//! Home route layout model: logo block, prompt placement, footer band and the
//! global-form overlay.
//!
//! Pinned upstream: OpenCode `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
//!
//! - `packages/tui/src/routes/home.tsx:83-88` — root box is `flexGrow 1`,
//!   `alignItems="center"`, horizontal padding `width < 44 ? 1 : 2`.
//! - `packages/tui/src/routes/home.tsx:89-105` — the two `flexGrow` spacers,
//!   the fixed 3-row block, `<Logo />` (`flexShrink 0`), the fixed 1-row gap,
//!   `UpdateNotification`, and the prompt box (`width="100%"`,
//!   `maxWidth={75}`, `zIndex={1000}`, `paddingTop={1}`).
//! - `packages/tui/src/routes/home.tsx:106-108` — the `home.footer` band is a
//!   nonshrinking sibling (`width="100%"`, `flexShrink={0}`).
//! - `packages/tui/src/routes/home.tsx:102` — `Prompt disabled={forms().length > 0}`.
//! - `packages/tui/src/routes/home.tsx:109-120` — global form overlay:
//!   `position="absolute"`, `zIndex={2000}`, `left/right 0`, `bottom={1}`,
//!   horizontal padding 2, full-width inner box.
//! - `packages/tui/src/routes/home.tsx:20-23` — placeholder sets per mode.
//! - `packages/tui/src/routes/home.tsx:125-175` — update notification gate
//!   (`!remote || type === "available"`) and exact label text.
//! - `packages/tui/src/feature-plugins/home/footer.tsx:7-13` — visibility
//!   thresholds: `/mcps` hint at width >= 64, `/plugins` at width >= 80,
//!   version at width >= 64.
//! - `packages/tui/src/feature-plugins/home/footer.tsx:82-101` — the footer is
//!   rendered only when `height >= 14 && width >= 44`; vertical padding is 1
//!   only at `height >= 16` (0 at 14/15); horizontal padding 2; the version is
//!   the trailing, right-aligned item; blocks are separated by `gap={2}`.
//! - `packages/tui/src/feature-plugins/home/footer.tsx:15-75` — the MCP block
//!   shows `⊙ <n> MCP failed` (failed branch first) or `⊙ <n> MCP`, the
//!   plugin block shows `⊙ <n> plugin(s) failed`; both are hidden when empty.
//! - `packages/tui/src/component/logo.tsx:54-76` — logo breakpoints/hiding are
//!   owned by [`crate::native_home_logo`] (`height < 12` hides the logo).
//! - `packages/tui/src/context/route.tsx:34` — `home` is the default route;
//!   `packages/tui/src/app.tsx:1402-1443` — the home route fills the root
//!   shell's main column, so this model is parameterized by that column's
//!   `cols`/`rows` rather than by a fixed viewport.
//!
//! Rounding assumption: the two `flexGrow` spacers share the free space
//! equally; Yoga rounds a half cell up, so an odd remainder goes to the upper
//! spacer and the prompt sits one row below the exact center. The same rule is
//! used for the centered prompt's left offset (`div_ceil`). The existing
//! painter in `native_view.rs` (`home_prompt_width`/`home_positions`) applies
//! the same rule. This is an explicit assumption, not a captured golden.
//!
//! The module is a pure geometry/text model: no IO, no clock, no random
//! source. Random placeholder selection stays with the caller, which supplies
//! the index.

use crate::native_home_logo;

/// Prompt wrapper `maxWidth` on the home route (`routes/home.tsx:101`).
pub const PROMPT_MAX_WIDTH: u16 = 75;
/// Fixed block above the logo (`routes/home.tsx:90`).
pub const LOGO_BLOCK_ROWS: u16 = 3;
/// Fixed gap between the logo and the notification/prompt (`routes/home.tsx:99`).
pub const LOGO_GAP_ROWS: u16 = 1;
/// Prompt wrapper `paddingTop` (`routes/home.tsx:101`).
pub const PROMPT_PADDING_TOP: u16 = 1;
/// Prompt wrapper `zIndex` (`routes/home.tsx:101`).
pub const PROMPT_Z_INDEX: u16 = 1000;
/// Global form overlay `zIndex` (`routes/home.tsx:113`).
pub const FORM_Z_INDEX: u16 = 2000;
/// Global form overlay `bottom` inset (`routes/home.tsx:113`).
pub const FORM_BOTTOM_INSET: u16 = 1;
/// Global form overlay horizontal padding (`routes/home.tsx:113`).
pub const FORM_PAD_X: u16 = 2;
/// Footer horizontal padding (`home/footer.tsx:87-88`).
pub const FOOTER_PAD_X: u16 = 2;
/// Footer requires this width (`home/footer.tsx:82`).
pub const FOOTER_MIN_WIDTH: u16 = 44;
/// Footer requires this height (`home/footer.tsx:82`).
pub const FOOTER_MIN_HEIGHT: u16 = 14;
/// Footer gets 1 row of vertical padding from this height (`home/footer.tsx:85-86`).
pub const FOOTER_TALL_HEIGHT: u16 = 16;
/// Footer `gap` between status blocks (`home/footer.tsx:91`).
pub const FOOTER_GAP: u16 = 2;
/// `/mcps` hint threshold (`home/footer.tsx:9`).
pub const MCP_COMMAND_MIN_WIDTH: u16 = 64;
/// `/plugins` hint threshold (`home/footer.tsx:10`).
pub const PLUGIN_COMMAND_MIN_WIDTH: u16 = 80;
/// Version threshold (`home/footer.tsx:11`).
pub const VERSION_MIN_WIDTH: u16 = 64;

/// Root horizontal padding: 1 cell below 44 columns, else 2
/// (`routes/home.tsx:86-87`).
#[must_use]
pub const fn padding_x(cols: u16) -> u16 {
    if cols < 44 {
        1
    } else {
        2
    }
}

/// Footer hint visibility at a given width (`home/footer.tsx:7-13`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FooterVisibility {
    pub mcp_command: bool,
    pub plugin_command: bool,
    pub version: bool,
}

/// Exact `homeFooterVisibility` thresholds (`home/footer.tsx:7-13`).
#[must_use]
pub const fn footer_visibility(cols: u16) -> FooterVisibility {
    FooterVisibility {
        mcp_command: cols >= MCP_COMMAND_MIN_WIDTH,
        plugin_command: cols >= PLUGIN_COMMAND_MIN_WIDTH,
        version: cols >= VERSION_MIN_WIDTH,
    }
}

/// The footer `View` gate: `height >= 14 && width >= 44`
/// (`home/footer.tsx:82`).
#[must_use]
pub const fn footer_visible(cols: u16, rows: u16) -> bool {
    cols >= FOOTER_MIN_WIDTH && rows >= FOOTER_MIN_HEIGHT
}

/// Footer vertical padding: 1 only at `height >= 16`
/// (`home/footer.tsx:85-86`).
#[must_use]
pub const fn footer_pad_y(rows: u16) -> u16 {
    if rows < FOOTER_TALL_HEIGHT {
        0
    } else {
        1
    }
}

/// Rows the footer band occupies: 0 when gated off, else 1 content row plus
/// top/bottom padding (`home/footer.tsx:82-101`).
#[must_use]
pub const fn footer_rows(cols: u16, rows: u16) -> u16 {
    if !footer_visible(cols, rows) {
        0
    } else {
        1 + 2 * footer_pad_y(rows)
    }
}

/// Inputs the home route cannot derive from the terminal size alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeInputs {
    /// Rows the prompt content occupies (draft-dependent; the prompt owns its
    /// own wrapping, see `component/prompt/index.tsx`).
    pub prompt_rows: u16,
    /// Rows of the update notification (0 when there is no notice,
    /// `routes/home.tsx:100`).
    pub notification_rows: u16,
}

impl Default for HomeInputs {
    /// One prompt input row, no update notification.
    fn default() -> Self {
        Self { prompt_rows: 1, notification_rows: 0 }
    }
}

/// Resolved home geometry for one main-column size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HomeLayout {
    pub cols: u16,
    pub rows: u16,
    /// Rows reserved by the footer band (0 when gated off).
    pub footer_rows: u16,
    /// Footer horizontal padding (0 when the footer is gated off).
    pub footer_pad_x: u16,
    /// Rows left for the centered home column (`rows - footer_rows`).
    pub body_rows: u16,
    /// Root horizontal padding.
    pub pad_x: u16,
    /// Main-column width inside the root padding.
    pub inner_width: u16,
    /// Prompt wrapper width (`inner_width` capped at [`PROMPT_MAX_WIDTH`]).
    pub prompt_width: u16,
    /// Prompt wrapper left column after `alignItems="center"`.
    pub prompt_left: u16,
    /// Logo height from [`native_home_logo::dimensions`].
    pub logo_rows: u16,
    /// First logo row.
    pub logo_top: u16,
    /// First row of the prompt wrapper box (before its `paddingTop`).
    pub prompt_top: u16,
    /// First row of prompt content (`prompt_top + PROMPT_PADDING_TOP`).
    pub prompt_content_top: u16,
    pub prompt_rows: u16,
    /// Upper `flexGrow` spacer height.
    pub spacer_above: u16,
    /// Lower `flexGrow` spacer height.
    pub spacer_below: u16,
    /// Rows that cannot fit (the fixed 3-row block shrinks first, then the
    /// root box clips; upstream keeps logo/prompt/footer at `flexShrink 0`).
    pub overflow: u16,
}

/// Compute the home column geometry (`routes/home.tsx:83-105`).
#[must_use]
pub fn layout(cols: u16, rows: u16, inputs: HomeInputs) -> HomeLayout {
    let pad_x = padding_x(cols);
    let inner_width = cols.saturating_sub(pad_x.saturating_mul(2));
    let prompt_width = inner_width.min(PROMPT_MAX_WIDTH);
    let prompt_left = pad_x.saturating_add(inner_width.saturating_sub(prompt_width).div_ceil(2));
    let footer_rows = footer_rows(cols, rows);
    let body_rows = rows.saturating_sub(footer_rows);
    let logo_rows = native_home_logo::dimensions(cols, rows).1;
    let fixed = LOGO_BLOCK_ROWS
        .saturating_add(logo_rows)
        .saturating_add(LOGO_GAP_ROWS)
        .saturating_add(inputs.notification_rows)
        .saturating_add(PROMPT_PADDING_TOP)
        .saturating_add(inputs.prompt_rows);
    let free = body_rows.saturating_sub(fixed);
    let spacer_above = free.div_ceil(2);
    let spacer_below = free - spacer_above;
    let overflow = fixed.saturating_sub(body_rows);
    // `height={3}` is the only fixed block with `flexShrink: 1`.
    let block = LOGO_BLOCK_ROWS.saturating_sub(overflow.min(LOGO_BLOCK_ROWS));
    let logo_top = spacer_above.saturating_add(block);
    let prompt_top = logo_top
        .saturating_add(logo_rows)
        .saturating_add(LOGO_GAP_ROWS)
        .saturating_add(inputs.notification_rows);
    HomeLayout {
        cols,
        rows,
        footer_rows,
        footer_pad_x: if footer_rows == 0 { 0 } else { FOOTER_PAD_X },
        body_rows,
        pad_x,
        inner_width,
        prompt_width,
        prompt_left,
        logo_rows,
        logo_top,
        prompt_top,
        prompt_content_top: prompt_top.saturating_add(PROMPT_PADDING_TOP),
        prompt_rows: inputs.prompt_rows,
        spacer_above,
        spacer_below,
        overflow,
    }
}

/// Pinned logo lockup as painted cells, `(x, y, cell)` relative to the logo
/// block origin. Applies `component/logo.tsx:12-76` marker rendering to the
/// segments reported by [`native_home_logo::segments`], so painter and text
/// geometry agree on glyphs, inset (left 19 + `gap={1}`) and shading.
#[must_use]
pub(crate) fn logo_cells(cols: u16, rows: u16) -> Vec<(u16, u16, native_home_logo::Cell)> {
    let mut cells = Vec::new();
    for segment in native_home_logo::segments(cols, rows) {
        let painted = native_home_logo::render_line(segment.raw, native_home_logo::ink(segment.bold));
        for (index, cell) in painted.into_iter().enumerate() {
            cells.push((segment.x.saturating_add(index as u16), segment.y, cell));
        }
    }
    cells
}

/// Global-form overlay state: visible, z-order and prompt-disabled gate
/// (`routes/home.tsx:102,109-120`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormOverlay {
    pub visible: bool,
    pub z_index: u16,
    pub prompt_z_index: u16,
    pub bottom_inset: u16,
    pub pad_x: u16,
    /// `Prompt disabled={forms().length > 0}` (`routes/home.tsx:102`).
    pub prompt_disabled: bool,
}

/// Overlay state for the global form list (`routes/home.tsx:109-120`).
#[must_use]
pub const fn form_overlay(global_forms: usize) -> FormOverlay {
    FormOverlay {
        visible: global_forms > 0,
        z_index: FORM_Z_INDEX,
        prompt_z_index: PROMPT_Z_INDEX,
        bottom_inset: FORM_BOTTOM_INSET,
        pad_x: FORM_PAD_X,
        prompt_disabled: global_forms > 0,
    }
}

/// Absolute rows `(top, bottom)` the overlay band occupies: anchored one row
/// above the terminal bottom, growing upward by the form's own height
/// (`routes/home.tsx:113`). `None` when the form is not rendered or the band
/// has no room.
#[must_use]
pub fn form_band(rows: u16, form_rows: u16) -> Option<(u16, u16)> {
    if form_rows == 0 || rows <= FORM_BOTTOM_INSET {
        return None;
    }
    let bottom = rows - 1 - FORM_BOTTOM_INSET;
    let top = bottom.saturating_sub(form_rows - 1);
    Some((top, bottom))
}

/// Home prompt placeholders (`routes/home.tsx:20-23`).
pub const PLACEHOLDERS_NORMAL: [&str; 3] = [
    "Fix a TODO in the codebase",
    "What is the tech stack of this project?",
    "Fix broken tests",
];
/// Shell-mode placeholders (`routes/home.tsx:22`).
pub const PLACEHOLDERS_SHELL: [&str; 3] = ["ls -la", "git status", "pwd"];

/// Prompt mode selecting the placeholder set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    Normal,
    Shell,
}

/// Deterministic placeholder pick. Upstream picks a random index
/// (`routes/home.tsx:20-23`); the caller owns the random source and passes the
/// already-reduced index so this module stays pure.
#[must_use]
pub fn placeholder(mode: PromptMode, index: usize) -> &'static str {
    let set: &[&'static str; 3] = match mode {
        PromptMode::Normal => &PLACEHOLDERS_NORMAL,
        PromptMode::Shell => &PLACEHOLDERS_SHELL,
    };
    set[index % set.len()]
}

/// One update notification label: optional highlighted prefix plus text
/// (`routes/home.tsx:158-168`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticeLine {
    /// Link-styled prefix (`/update` or `/exit`), absent for remote notices.
    pub prefix: Option<&'static str>,
    /// Remaining text, already carrying its leading separator space.
    pub text: String,
}

impl NoticeLine {
    /// Flattened label, exactly as concatenated upstream.
    #[must_use]
    pub fn line(&self) -> String {
        match self.prefix {
            Some(prefix) => format!("{prefix}{}", self.text),
            None => self.text.clone(),
        }
    }
}

/// Build the notification label, or `None` when the gate hides it.
///
/// `remote` mirrors `state.source === "server" && state.remote`; the notice is
/// rendered only when `!remote || type === "available"`
/// (`routes/home.tsx:139-141`). A remote notice renders no prefix and the
/// fixed `remote server update available` text (`:164-165`).
#[must_use]
pub fn update_notice(remote: bool, installed: bool, version: &str) -> Option<NoticeLine> {
    if remote && installed {
        return None;
    }
    if remote {
        return Some(NoticeLine { prefix: None, text: "remote server update available".to_owned() });
    }
    if installed {
        return Some(NoticeLine {
            prefix: Some("/exit"),
            text: format!(" restart to use v{version}"),
        });
    }
    Some(NoticeLine {
        prefix: Some("/update"),
        text: format!(" to install v{version}"),
    })
}

/// Visible footer status blocks (`home/footer.tsx:15-75`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FooterStatus {
    /// MCP block including its `/mcps` hint when the width allows it.
    pub mcp: Option<String>,
    /// Plugin block including its `/plugins` hint when the width allows it.
    pub plugins: Option<String>,
}

/// Build the MCP/plugin blocks. `total == 0` hides the MCP block; the failed
/// branch wins and is shown first (`home/footer.tsx:23-47`). The plugin block
/// is hidden when nothing failed (`home/footer.tsx:63-74`).
#[must_use]
pub fn footer_status(
    cols: u16,
    mcp_total: usize,
    mcp_failed: usize,
    mcp_connected: usize,
    plugins_failed: usize,
) -> FooterStatus {
    let visibility = footer_visibility(cols);
    let mcp = if mcp_total == 0 {
        None
    } else {
        let mut text = if mcp_failed > 0 {
            format!("⊙ {mcp_failed} MCP failed")
        } else {
            format!("⊙ {mcp_connected} MCP")
        };
        if visibility.mcp_command {
            text.push_str(" /mcps");
        }
        Some(text)
    };
    let plugins = if plugins_failed == 0 {
        None
    } else {
        let suffix = if plugins_failed == 1 { "" } else { "s" };
        let mut text = format!("⊙ {plugins_failed} plugin{suffix} failed");
        if visibility.plugin_command {
            text.push_str(" /plugins");
        }
        Some(text)
    };
    FooterStatus { mcp, plugins }
}

/// Footer content line (without the outer padding): visible blocks joined by
/// [`FOOTER_GAP`], then the trailing version when the width allows it
/// (`home/footer.tsx:82-101`). `None` when the footer is gated off.
#[must_use]
pub fn footer_line(cols: u16, rows: u16, status: &FooterStatus, version: &str) -> Option<String> {
    if !footer_visible(cols, rows) {
        return None;
    }
    let visibility = footer_visibility(cols);
    let mut parts: Vec<&str> = Vec::new();
    if let Some(mcp) = status.mcp.as_deref() {
        parts.push(mcp);
    }
    if let Some(plugins) = status.plugins.as_deref() {
        parts.push(plugins);
    }
    let mut line = parts.join(&" ".repeat(usize::from(FOOTER_GAP)));
    if visibility.version {
        if !line.is_empty() {
            line.push_str(&" ".repeat(usize::from(FOOTER_GAP)));
        }
        line.push_str(version);
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME_80X24: HomeInputs = HomeInputs { prompt_rows: 3, notification_rows: 0 };

    #[test]
    fn lay18_home_prompt_width_logo_spacing_and_balanced_spacers() {
        // 80x24: pad 2, inner 76, prompt capped at 75, centered with the
        // half-cell remainder on the left (div_ceil, like the existing
        // painter).
        let home = layout(80, 24, HOME_80X24);
        assert_eq!(home.pad_x, 2);
        assert_eq!(home.inner_width, 76);
        assert_eq!(home.prompt_width, PROMPT_MAX_WIDTH);
        assert_eq!(home.prompt_left, 3);
        assert!(home.prompt_left + home.prompt_width + home.pad_x <= 80);

        // 80x24 footer is 3 rows (height >= 16), body = 21.
        assert_eq!(home.footer_rows, 3);
        assert_eq!(home.body_rows, 21);
        // Logo breakpoint at width 80 / height 24 is the 4-row lockup.
        assert_eq!(home.logo_rows, 4);
        // Fixed stack: 3 + logo 4 + gap 1 + notification 0 + prompt padding 1
        // + prompt 3 = 12; free 9 splits 5 above / 4 below.
        assert_eq!(home.spacer_above, 5);
        assert_eq!(home.spacer_below, 4);
        assert!(home.spacer_above + home.spacer_below == home.body_rows - 12);
        assert!(home.spacer_above <= home.spacer_below + 1);
        assert!(home.spacer_below <= home.spacer_above);
        assert_eq!(home.logo_top, home.spacer_above + LOGO_BLOCK_ROWS);
        assert_eq!(home.prompt_top, home.logo_top + home.logo_rows + LOGO_GAP_ROWS);
        assert_eq!(home.prompt_content_top, home.prompt_top + PROMPT_PADDING_TOP);
        // Two blank rows separate the logo block from prompt content when no
        // notification is present (`height={1}` gap + `paddingTop={1}`).
        assert_eq!(home.prompt_content_top - (home.logo_top + home.logo_rows), 2);
        // The column is symmetric: prompt bottom + lower spacer = body.
        assert_eq!(home.prompt_content_top + home.prompt_rows + home.spacer_below, home.body_rows);
        assert_eq!(home.overflow, 0);

        // 120x40: still capped at 75 cells, still centered.
        let wide = layout(120, 40, HOME_80X24);
        assert_eq!(wide.prompt_width, PROMPT_MAX_WIDTH);
        assert_eq!(wide.inner_width, 116);
        assert_eq!(wide.prompt_left, 2 + (116u16 - 75).div_ceil(2));

        // Width 43/44 boundary: padding 1 -> 2, so the prompt actually loses a
        // cell when the terminal grows past 43.
        let w43 = layout(43, 24, HOME_80X24);
        let w44 = layout(44, 24, HOME_80X24);
        assert_eq!(w43.pad_x, 1);
        assert_eq!(w44.pad_x, 2);
        assert_eq!(w43.prompt_width, 41);
        assert_eq!(w44.prompt_width, 40);
        assert_eq!(w43.prompt_left, 1);
        assert_eq!(w44.prompt_left, 2);
        // 44 is exactly where the footer gate opens (3 rows at height 24).
        assert_eq!(w43.footer_rows, 0);
        assert_eq!(w44.footer_rows, 3);
        assert_eq!(w43.footer_pad_x, 0);
        assert_eq!(w44.footer_pad_x, FOOTER_PAD_X);

        // A notification row keeps the prompt in place: it takes one spacer
        // row, so the spacers stay balanced and shift up instead.
        let notice = layout(80, 24, HomeInputs { prompt_rows: 3, notification_rows: 1 });
        assert_eq!(notice.spacer_above, 4);
        assert_eq!(notice.spacer_below, 4);
        assert_eq!(notice.prompt_content_top, home.prompt_content_top);
        assert_eq!(notice.prompt_content_top - (notice.logo_top + notice.logo_rows), 3);

        // Short terminal: logo hidden below 12 rows; a 6-row body exactly fits
        // the fixed stack, so both spacers collapse to 0 with no overflow.
        let short = layout(80, 6, HomeInputs::default());
        assert_eq!(short.footer_rows, 0);
        assert_eq!(short.logo_rows, 0);
        assert_eq!(short.spacer_above, 0);
        assert_eq!(short.spacer_below, 0);
        assert_eq!(short.overflow, 0);
        assert_eq!(short.logo_top, LOGO_BLOCK_ROWS);
        let short11 = layout(80, 11, HomeInputs::default());
        assert_eq!(short11.logo_rows, 0);
        assert_eq!(short11.spacer_above, 3);
        assert_eq!(short11.spacer_below, 2);

        // Overfull: the 3-row block shrinks first, then the root clips.
        let tiny = layout(80, 12, HomeInputs { prompt_rows: 10, notification_rows: 0 });
        assert_eq!(tiny.spacer_above, 0);
        assert_eq!(tiny.spacer_below, 0);
        assert!(tiny.overflow > 0);
        assert_eq!(tiny.logo_rows, 4);
        assert_eq!(tiny.logo_top, 0, "shrunken 3-row block starts at row 0");

        // Logo spacing at cell level: two 19-cell halves with a one-cell gap
        // (`component/logo.tsx:70-73`), painted without marker leakage.
        let cells = logo_cells(80, 24);
        assert_eq!(cells.len(), 4 * 38);
        assert!(cells.iter().all(|(x, y, _)| (*x < 19 || *x >= 20) && *y < 4));
        assert!(cells.iter().all(|(_, _, cell)| !matches!(cell.glyph, '_' | '^' | '~' | ',')));
        assert!(cells.iter().any(|(_, _, cell)| cell.ink == native_home_logo::Ink::Shadow));
        // The compact breakpoints paint 4x3 and 19x7 without any marker glyphs.
        assert_eq!(logo_cells(21, 24).len(), 12);
        assert_eq!(logo_cells(43, 24).len(), 7 * 19);
        assert!(logo_cells(80, 11).is_empty());
    }

    #[test]
    fn lay19_global_form_disables_prompt_and_sits_above_bottom_inset() {
        // No forms: prompt enabled, nothing overlaid.
        let idle = form_overlay(0);
        assert!(!idle.visible);
        assert!(!idle.prompt_disabled);
        assert_eq!(form_band(24, 5), Some((18, 22)));

        // One global form: prompt disabled, overlay above it in z-order.
        let open = form_overlay(1);
        assert!(open.visible);
        assert!(open.prompt_disabled, "forms().length > 0 disables the prompt");
        assert_eq!(open.z_index, FORM_Z_INDEX);
        assert_eq!(open.prompt_z_index, PROMPT_Z_INDEX);
        assert!(open.z_index > open.prompt_z_index);
        assert_eq!(open.bottom_inset, FORM_BOTTOM_INSET);
        assert_eq!(open.pad_x, FORM_PAD_X);

        // Overlay band: absolute, left/right 0, bottom = rows - 1 - 1.
        let (top, bottom) = form_band(24, 5).expect("band");
        assert_eq!(bottom, 24 - 1 - FORM_BOTTOM_INSET);
        assert_eq!(top, bottom + 1 - 5);
        // It floats over the footer band's last rows rather than sharing the
        // home column, so the z-order must beat both footer paint order and
        // the prompt wrapper.
        let home = layout(80, 24, HOME_80X24);
        let footer_top = home.body_rows;
        assert!(bottom >= footer_top, "overlay paints over the footer band");
        assert!(open.z_index > PROMPT_Z_INDEX);
        assert!(open.z_index > 1, "footer has no z-index; any positive value wins");

        // A taller form than the viewport clips at row 0, never goes negative.
        assert_eq!(form_band(24, 40), Some((0, 22)));
        // No room / no form: no band.
        assert_eq!(form_band(1, 1), None);
        assert_eq!(form_band(24, 0), None);
    }

    #[test]
    fn lay20_footer_width_and_height_gates_actions_version_and_padding() {
        // Height gate: 13/14.
        assert!(!footer_visible(80, 13));
        assert!(footer_visible(80, 14));
        assert_eq!(footer_rows(80, 13), 0);
        assert_eq!(footer_rows(80, 14), 1);
        assert_eq!(footer_rows(80, 15), 1);
        assert_eq!(footer_rows(80, 16), 3);
        assert_eq!(footer_pad_y(13), 0);
        assert_eq!(footer_pad_y(14), 0);
        assert_eq!(footer_pad_y(15), 0);
        assert_eq!(footer_pad_y(16), 1);

        // Width gate: 43/44.
        assert!(!footer_visible(43, 24));
        assert!(footer_visible(44, 24));
        assert_eq!(footer_rows(44, 24), 3);

        // Action hints and version: 63/64 and 79/80.
        let at63 = footer_visibility(63);
        let at64 = footer_visibility(64);
        let at79 = footer_visibility(79);
        let at80 = footer_visibility(80);
        assert!(!at63.mcp_command && !at63.version);
        assert!(at64.mcp_command && at64.version && !at64.plugin_command);
        assert!(at79.mcp_command && at79.version && !at79.plugin_command);
        assert!(at80.plugin_command);

        // Blocks themselves: MCP hidden with no servers, failed branch first.
        assert_eq!(footer_status(80, 0, 0, 0, 0), FooterStatus { mcp: None, plugins: None });
        let failed = footer_status(80, 2, 1, 1, 1);
        assert_eq!(failed.mcp.as_deref(), Some("⊙ 1 MCP failed /mcps"));
        assert_eq!(failed.plugins.as_deref(), Some("⊙ 1 plugin failed /plugins"));
        let ok = footer_status(64, 3, 0, 3, 2);
        assert_eq!(ok.mcp.as_deref(), Some("⊙ 3 MCP /mcps"));
        assert_eq!(ok.plugins.as_deref(), Some("⊙ 2 plugins failed"), "no /plugins below 80");
        let narrow = footer_status(63, 3, 0, 3, 0);
        assert_eq!(narrow.mcp.as_deref(), Some("⊙ 3 MCP"));

        // Full line: gap 2 between blocks, version last, gated by width and by
        // the footer being visible at all.
        assert_eq!(footer_line(80, 13, &ok, "1.2.3"), None);
        assert_eq!(footer_line(43, 24, &ok, "1.2.3"), None);
        let ok80 = footer_status(80, 3, 0, 3, 2);
        assert_eq!(ok80.plugins.as_deref(), Some("⊙ 2 plugins failed /plugins"));
        let line = footer_line(80, 24, &ok80, "1.2.3").expect("footer line");
        assert_eq!(line, "⊙ 3 MCP /mcps  ⊙ 2 plugins failed /plugins  1.2.3");
        let line64 = footer_line(64, 24, &ok, "1.2.3").expect("footer line");
        assert_eq!(line64, "⊙ 3 MCP /mcps  ⊙ 2 plugins failed  1.2.3");
        let ok63 = footer_status(63, 3, 0, 3, 2);
        let line63 = footer_line(63, 24, &ok63, "1.2.3").expect("footer line");
        assert_eq!(line63, "⊙ 3 MCP  ⊙ 2 plugins failed", "version and hints hidden below 64");
        // Empty status still yields the version line at width >= 64.
        assert_eq!(footer_line(80, 24, &FooterStatus::default(), "1.2.3").as_deref(), Some("1.2.3"));
    }

    #[test]
    fn home_placeholders_match_the_pinned_route() {
        assert_eq!(placeholder(PromptMode::Normal, 0), "Fix a TODO in the codebase");
        assert_eq!(placeholder(PromptMode::Normal, 1), "What is the tech stack of this project?");
        assert_eq!(placeholder(PromptMode::Normal, 2), "Fix broken tests");
        assert_eq!(placeholder(PromptMode::Shell, 0), "ls -la");
        assert_eq!(placeholder(PromptMode::Shell, 1), "git status");
        assert_eq!(placeholder(PromptMode::Shell, 2), "pwd");
        // Caller-supplied random index is reduced, never panics.
        assert_eq!(placeholder(PromptMode::Normal, 7), PLACEHOLDERS_NORMAL[1]);
    }

    #[test]
    fn update_notice_text_and_remote_gate_match_the_pinned_route() {
        let available = update_notice(false, false, "1.2.3").expect("client notice");
        assert_eq!(available.prefix, Some("/update"));
        assert_eq!(available.line(), "/update to install v1.2.3");
        let installed = update_notice(false, true, "1.2.3").expect("installed notice");
        assert_eq!(installed.prefix, Some("/exit"));
        assert_eq!(installed.line(), "/exit restart to use v1.2.3");
        let remote_available = update_notice(true, false, "1.2.3").expect("remote notice");
        assert_eq!(remote_available.prefix, None);
        assert_eq!(remote_available.line(), "remote server update available");
        // A remote *installed* notice is gated off entirely.
        assert_eq!(update_notice(true, true, "1.2.3"), None);
    }
}
