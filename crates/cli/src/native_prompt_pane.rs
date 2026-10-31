//! P08 — prompt surface: theme-driven border, raised background and the
//! metadata row beneath the textarea.
//!
//! Upstream reference is the selected pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22` (`packages/tui/src`):
//!
//! - `component/prompt/index.tsx:1681-1688` — main box: left border only,
//!   `SplitBorder` vertical, `bottomLeft` overridden to `╹`.
//! - `component/prompt/index.tsx:1690-1698` — raised inner surface
//!   (`promptBg`), responsive horizontal inset, one-row top padding.
//! - `component/prompt/index.tsx:1676` — `promptBg =
//!   theme.decrease(theme.background.raised.base)`.
//! - `component/prompt/index.tsx:1855-1870` — metadata block with
//!   `paddingTop={1}`, row direction, `gap={1}`.
//! - `component/prompt/index.tsx:1873-1898` — separate one-row lip: left
//!   `╹`/space and inner bottom `▀`/space depending on `promptBg().a`.
//! - `component/prompt/index.tsx:1588-1592` — border highlight selection
//!   (muted → border base, shell → primary-action selected, else agent color or
//!   border base); `:1608` — `tint(border.base, highlight, agentAlpha)`.
//! - `ui/border.ts:1-13` — `EmptyBorder`; `:15-21` — `SplitBorder`
//!   (`border: ["left","right"]`, `vertical: "┃"`).
//! - `component/prompt/metadata.tsx:8-167` — metadata content order,
//!   terminal-width text budget, candidate fitting and the fallback
//!   truncation; `:104-106` — `fade()` alpha multiplication.
//! - `theme/color.ts:43-48` — `tint` returns an opaque RGBA.
//! - `theme/component.ts:38-40` + `packages/theme/src/tui/resolve.ts:179-193`
//!   — `decrease` is a hue-scale step shift and is identity for literal colors.
//! - `context/local.tsx:59-135` — visible vs selectable agents and
//!   deduplicated categorical agent colors (step 400 with new designs, else
//!   200; explicit agent color wins; index modulo palette length).
//! - `component/prompt/index.tsx:1539-1572` — `promptDisplay`: keep the last
//!   resolved labels while destination catalogs load, replace them on a
//!   location change.
//! - `routes/session/index.tsx:1590-1659` — exactly one of auxiliary Composer,
//!   permission, form, location-missing, prompt or nothing occupies the slot.
//! - `util/signal.ts:20-49` + `ui/animation.ts:46-52,177-189` — metadata
//!   fade-in: 0.16 s smoothstep tween; a mounted-once WeakSet key means a
//!   remount jumps to the final alpha instead of replaying the reveal.
//!
//! Everything here is pure layout/style state; the painter and the terminal
//! live in the existing view modules.
#![allow(dead_code)]

use crate::native_prompt_layout::{prompt_inset, prompt_text_width, prompt_rows, surface_rows};
use crate::native_theme_runtime::{HueColor, SemanticPalette};
use crate::native_view_layout::{fit, visit_unpadded_cells};

// ── border and raised surface ─────────────────────────────────────────

/// `ui/border.ts:19-21`: `SplitBorder.customBorderChars.vertical`.
pub(crate) const SPLIT_BORDER_VERTICAL: char = '┃';
/// `component/prompt/index.tsx:1687`: main box `bottomLeft` override.
pub(crate) const PROMPT_BOTTOM_LEFT: char = '╹';
/// `component/prompt/index.tsx:1879`: lip vertical when the surface is
/// non-transparent.
pub(crate) const PROMPT_LIP_LEFT: char = '╹';
/// `component/prompt/index.tsx:1890`: lip bottom half block.
pub(crate) const PROMPT_LIP_HORIZONTAL: char = '▀';

/// Glyphs of one composed prompt: the main surface and its separate lip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PromptBorder {
    /// Main-surface left rail, drawn on every surface row
    /// (`ui/border.ts:19-21`, used at `index.tsx:1683-1688`).
    pub vertical: char,
    /// Configured `bottomLeft` (`index.tsx:1687`). A left-only OpenTUI box
    /// paints its vertical on the last row too — the height-1 lip proves that
    /// by rendering `vertical: "╹"` (`index.tsx:1873-1881`) — so this value is
    /// exposed for renderers that draw a real corner cell.
    pub bottom_left: char,
    /// Left cell of the half-block lip (`index.tsx:1879`).
    pub lip_left: char,
    /// Remaining lip cells (`index.tsx:1884-1896`).
    pub lip_horizontal: char,
}

/// Border glyphs for a prompt background. The main rail never depends on the
/// background; only the lip substitutes spaces for a transparent surface
/// (`index.tsx:1879,1890-1895`).
pub(crate) fn prompt_border(background: [u8; 4]) -> PromptBorder {
    let visible = background[3] != 0;
    PromptBorder {
        vertical: SPLIT_BORDER_VERTICAL,
        bottom_left: PROMPT_BOTTOM_LEFT,
        lip_left: if visible { PROMPT_LIP_LEFT } else { ' ' },
        lip_horizontal: if visible { PROMPT_LIP_HORIZONTAL } else { ' ' },
    }
}

/// `promptBg = theme.decrease(theme.background.raised.base)`
/// (`index.tsx:1676`). `decrease` shifts one hue step down and returns the
/// input unchanged when the token is a literal color
/// (`packages/theme/src/tui/resolve.ts:178-193`), which is exactly what
/// [`SemanticPalette::decrease_default`] does.
pub(crate) fn prompt_surface(palette: &SemanticPalette) -> HueColor {
    palette.decrease_default(&palette.background_raised_base)
}

/// Upstream `tint` (`theme/color.ts:43-48`): interpolate RGB and return an
/// **opaque** color (`RGBA.fromInts` defaults alpha to 255).
pub(crate) fn tint(base: [u8; 4], overlay: [u8; 4], alpha: f64) -> [u8; 4] {
    let blend = |b: u8, o: u8| -> u8 {
        (f64::from(b) + (f64::from(o) - f64::from(b)) * alpha)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [blend(base[0], overlay[0]), blend(base[1], overlay[1]), blend(base[2], overlay[2]), 255]
}

/// Border highlight before the reveal tint (`index.tsx:1588-1592`):
/// muted → `border.base`; shell → `text.action.primary.selected`; otherwise
/// the current agent color, falling back to `border.base`.
pub(crate) fn prompt_highlight(
    muted: bool,
    shell: bool,
    agent_color: Option<[u8; 4]>,
    shell_color: [u8; 4],
    border_base: [u8; 4],
) -> [u8; 4] {
    if muted {
        return border_base;
    }
    if shell {
        return shell_color;
    }
    agent_color.unwrap_or(border_base)
}

/// Final animated rail color (`index.tsx:1608`):
/// `tint(theme.border.base, highlight, agentMetaAlpha)`.
pub(crate) fn border_highlight(border_base: [u8; 4], highlight: [u8; 4], reveal_alpha: f64) -> [u8; 4] {
    tint(border_base, highlight, reveal_alpha)
}

// ── rows ──────────────────────────────────────────────────────────────

/// One surface row: left rail, inset, the text column, inset. Always exactly
/// `pane_width` cells (and never wider on panes too narrow for the inset).
/// Composition per `index.tsx:1681-1698`.
pub(crate) fn surface_row(pane_width: u16, terminal_width: u16, text: &str) -> String {
    if pane_width == 0 {
        return String::new();
    }
    let available = usize::from(pane_width - 1);
    let inset = usize::from(prompt_inset(terminal_width));
    let left = inset.min(available / 2);
    let right = inset.min(available - left);
    let width = available - left - right;
    format!(
        "{SPLIT_BORDER_VERTICAL}{}{}{}",
        " ".repeat(left),
        fit(text, u16::try_from(width).unwrap_or(u16::MAX)),
        " ".repeat(right)
    )
}

/// The separate half-block lip (`index.tsx:1873-1898`): one `╹` followed by
/// `▀` across the rest of the prompt, or spaces for a transparent surface.
pub(crate) fn lip_row(pane_width: u16, background: [u8; 4]) -> String {
    if pane_width == 0 {
        return String::new();
    }
    let border = prompt_border(background);
    let mut row = String::with_capacity(usize::from(pane_width));
    row.push(border.lip_left);
    row.extend(std::iter::repeat_n(
        border.lip_horizontal,
        usize::from(pane_width).saturating_sub(1),
    ));
    row
}

// ── metadata row ──────────────────────────────────────────────────────

/// Which prompt is occupying the composer slot. Upstream renders a single
/// `Switch` branch, never two (`routes/session/index.tsx:1591-1660`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptSlot {
    /// `composer.open`, or a child session without forms
    /// (`routes/session/index.tsx:1595-1611`): the auxiliary Composer replaces
    /// the editable prompt.
    AuxiliaryComposer,
    /// First prompted permission request (`routes/session/index.tsx:1612-1621`).
    Permission,
    /// First pending form (`routes/session/index.tsx:1622-1629`).
    Form,
    /// Session location unavailable (`routes/session/index.tsx:1630-1641`).
    LocationMissing,
    /// Disabled session (`routes/session/index.tsx:1642-1658`): the prompt is
    /// hidden.
    Hidden,
    /// The editable Prompt (`routes/session/index.tsx:1643-1657`).
    Prompt,
}

/// Slot precedence, highest first. CP-01: replacement, not duplication.
pub(crate) fn prompt_slot(
    composer_open: bool,
    child_without_forms: bool,
    permission_pending: bool,
    form_pending: bool,
    location_missing: bool,
    disabled: bool,
) -> PromptSlot {
    if composer_open || child_without_forms {
        return PromptSlot::AuxiliaryComposer;
    }
    if permission_pending {
        return PromptSlot::Permission;
    }
    if form_pending {
        return PromptSlot::Form;
    }
    if location_missing {
        return PromptSlot::LocationMissing;
    }
    if disabled {
        return PromptSlot::Hidden;
    }
    PromptSlot::Prompt
}

/// Prompt mode (`component/prompt/metadata.tsx:9`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptMode {
    Normal,
    Shell,
}

/// Presentation facts for the metadata row. `agent` is the already resolved
/// display label (`promptDisplay().agentLabel`, `index.tsx:1858`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PromptMetadataInput<'a> {
    pub mode: PromptMode,
    pub agent: Option<&'a str>,
    /// `local.permission.mode === "autoaccept"` (`index.tsx:1859`).
    pub auto: bool,
    pub model: &'a str,
    pub provider: &'a str,
    pub variant: Option<&'a str>,
    /// Workspace usage limit text, not context usage (`index.tsx:1863`).
    pub limit: Option<&'a str>,
}

/// The chosen candidate: exactly what the row renders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PromptMetadataLayout<'a> {
    pub agent: Option<&'a str>,
    pub auto: bool,
    pub model: String,
    pub provider: Option<&'a str>,
    pub variant: Option<&'a str>,
    pub limit: Option<&'a str>,
}

/// A styled text node of the row, in upstream order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetadataRole {
    Agent,
    Auto,
    /// Muted `·` between agent and model (`metadata.tsx:53-55`).
    ModelSeparator,
    Model,
    Provider,
    /// Muted `·` before the variant (`metadata.tsx:75`).
    VariantSeparator,
    Variant,
    Limit,
}

/// Foreground token for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetadataFg {
    /// Agent highlight color (`metadata.tsx:46`).
    Highlight,
    /// `theme.text.muted`.
    Muted,
    /// `theme.text.base`, or muted while the prompt is muted
    /// (`metadata.tsx:61`).
    Base,
    /// `theme.text.feedback.warning.base` (`metadata.tsx:77`).
    Warning,
    /// `theme.text.feedback.error.base` (`metadata.tsx:91`).
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MetadataStyle {
    pub fg: MetadataFg,
    pub bold: bool,
}

/// Per-node styling from `metadata.tsx:39-101`.
pub(crate) fn metadata_style(role: MetadataRole, muted: bool, limit_danger: bool) -> MetadataStyle {
    let style = |fg| MetadataStyle { fg, bold: false };
    match role {
        MetadataRole::Agent => style(MetadataFg::Highlight),
        MetadataRole::Auto | MetadataRole::ModelSeparator | MetadataRole::VariantSeparator => {
            style(MetadataFg::Muted)
        }
        MetadataRole::Model => style(if muted { MetadataFg::Muted } else { MetadataFg::Base }),
        MetadataRole::Provider => style(MetadataFg::Muted),
        MetadataRole::Variant => MetadataStyle { fg: MetadataFg::Warning, bold: true },
        MetadataRole::Limit => style(if limit_danger { MetadataFg::Error } else { MetadataFg::Muted }),
    }
}

/// One candidate of the fitting algorithm (`metadata.tsx:130-141`).
fn candidate<'a>(
    input: &PromptMetadataInput<'a>,
    agent: Option<&'a str>,
    auto: bool,
    provider: Option<&'a str>,
) -> PromptMetadataLayout<'a> {
    PromptMetadataLayout {
        agent,
        auto,
        model: input.model.to_owned(),
        provider,
        variant: input.variant,
        limit: input.limit,
    }
}

/// `promptMetadataLayout` (`metadata.tsx:117-156`).
///
/// The text budget uses the **terminal** width, not the prompt's measured
/// width: `max(0, terminalWidth - (terminalWidth < 44 ? 9 : 13))`
/// (`metadata.tsx:28`). Below 44 columns the agent and provider are omitted
/// (`:127-128`).
pub(crate) fn metadata_layout<'a>(
    input: &PromptMetadataInput<'a>,
    terminal_width: u16,
) -> PromptMetadataLayout<'a> {
    if input.mode == PromptMode::Shell {
        // `metadata.tsx:26`: Shell (or the shell agent) with an empty model.
        return PromptMetadataLayout {
            agent: Some(input.agent.unwrap_or("Shell")),
            auto: false,
            model: String::new(),
            provider: None,
            variant: None,
            limit: None,
        };
    }

    let width = usize::from(terminal_width.saturating_sub(if terminal_width < 44 { 9 } else { 13 }));
    let agent = if terminal_width >= 44 { input.agent } else { None }.filter(|agent| !agent.is_empty());
    let provider = if terminal_width >= 44 { Some(input.provider) } else { None }
        .filter(|provider| !provider.is_empty());
    let short = provider.map(|provider| provider.rsplit(" / ").next().unwrap_or(provider));

    // Candidates, in upstream order: all fields, drop `auto`, shorten the
    // provider to its last `" / "` segment, drop the provider.
    for layout in [
        candidate(input, agent, input.auto, provider),
        candidate(input, agent, false, provider),
        candidate(input, agent, false, short),
        candidate(input, agent, false, None),
    ] {
        if cells(&metadata_text(&layout)) <= width {
            return layout;
        }
    }

    // `metadata.tsx:145-155`: nothing fits; keep agent/variant/limit and
    // truncate the model to at least a nine-cell budget.
    let prefix = agent.map_or_else(String::new, |agent| format!("{agent} · "));
    let suffix = format!(
        "{}{}",
        input.variant.map_or_else(String::new, |variant| format!(" · {variant}")),
        input.limit.map_or_else(String::new, |limit| format!("  {limit}")),
    );
    let budget = 9usize.max(
        width
            .saturating_sub(cells(&prefix))
            .saturating_sub(cells(&suffix)),
    );
    let model = strip_space_before_ellipsis(&truncate_width(input.model, budget));
    PromptMetadataLayout { agent, auto: false, model, provider: None, variant: input.variant, limit: input.limit }
}

/// Flat text of a layout, exactly as `text()` measures it
/// (`metadata.tsx:158-167`). The limit node carries the extra `marginLeft={1}`
/// as a leading space, so joining with single spaces yields two spaces.
pub(crate) fn metadata_text(layout: &PromptMetadataLayout<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(agent) = layout.agent {
        parts.push(agent.to_owned());
    }
    if layout.auto {
        parts.push("auto".to_owned());
    }
    if !layout.model.is_empty() {
        if layout.agent.is_some() {
            parts.push("·".to_owned());
        }
        parts.push(layout.model.clone());
    }
    if let Some(provider) = layout.provider {
        parts.push(provider.to_owned());
    }
    if let Some(variant) = layout.variant {
        parts.push("·".to_owned());
        parts.push(variant.to_owned());
    }
    if let Some(limit) = layout.limit {
        parts.push(format!(" {limit}"));
    }
    parts.join(" ")
}

/// Rendered nodes in upstream order. There is no separator dot between the
/// model and the provider (`metadata.tsx:56-71`); the model node is gated on a
/// non-empty model, which also gates provider/variant/limit (`:51-97`).
pub(crate) fn metadata_runs(layout: &PromptMetadataLayout<'_>) -> Vec<(MetadataRole, String)> {
    let mut runs: Vec<(MetadataRole, String)> = Vec::new();
    if let Some(agent) = layout.agent {
        runs.push((MetadataRole::Agent, agent.to_owned()));
    }
    if layout.auto {
        runs.push((MetadataRole::Auto, "auto".to_owned()));
    }
    if !layout.model.is_empty() {
        if layout.agent.is_some() {
            runs.push((MetadataRole::ModelSeparator, "·".to_owned()));
        }
        runs.push((MetadataRole::Model, layout.model.clone()));
        if let Some(provider) = layout.provider {
            runs.push((MetadataRole::Provider, provider.to_owned()));
        }
        if let Some(variant) = layout.variant {
            runs.push((MetadataRole::VariantSeparator, "·".to_owned()));
            runs.push((MetadataRole::Variant, variant.to_owned()));
        }
        if let Some(limit) = layout.limit {
            runs.push((MetadataRole::Limit, format!(" {limit}")));
        }
    }
    runs
}

/// Width of a string in terminal cells (OpenTUI cluster segmentation).
pub(crate) fn cells(text: &str) -> usize {
    visit_unpadded_cells(text, text.len().saturating_mul(2).saturating_add(1), |_, _| {})
}

/// `Locale.takeWidth` (`util/locale.ts:62-75`): whole grapheme clusters while
/// they fit.
pub(crate) fn take_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    let mut full = false;
    visit_unpadded_cells(text, width, |glyph, cells| {
        if full {
            return;
        }
        if used + cells > width {
            full = true;
            return;
        }
        used += cells;
        out.push_str(glyph);
    });
    out
}

/// `Locale.truncateWidth` (`util/locale.ts:77-82`): `…` when one cell is all
/// that fits, else a fitting prefix plus `…`.
pub(crate) fn truncate_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if cells(text) <= width {
        return text.to_owned();
    }
    if width == 1 {
        return "…".to_owned();
    }
    format!("{}…", take_width(text, width - 1))
}

/// `metadata.tsx:152`: `.replace(/\s+…$/, "…")`.
fn strip_space_before_ellipsis(text: &str) -> String {
    match text.strip_suffix('…') {
        Some(prefix) => format!("{}…", prefix.trim_end_matches(char::is_whitespace)),
        None => text.to_owned(),
    }
}

// ── agent labels and colors ───────────────────────────────────────────

/// `Locale.titlecase` (`util/locale.ts:3-5`): uppercase the first word
/// character after a word boundary. JS `\w` is `[A-Za-z0-9_]`.
pub(crate) fn titlecase(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut boundary = true;
    for c in text.chars() {
        let word = c.is_ascii_alphanumeric() || c == '_';
        if boundary && c.is_ascii_alphabetic() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        boundary = !word;
    }
    out
}

/// Minimal agent facts needed by the prompt (`context/local.tsx:59-135`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AgentFact<'a> {
    pub id: &'a str,
    pub hidden: bool,
    /// `agent.mode === "subagent"`.
    pub subagent: bool,
    /// Explicit configured color (`context/local.tsx:130`).
    pub color: Option<[u8; 4]>,
}

/// Main-agent picker list: not hidden and not subagent-only
/// (`context/local.tsx:59-61`).
pub(crate) fn selectable_agents<'a>(agents: &'a [AgentFact<'a>]) -> Vec<&'a AgentFact<'a>> {
    agents.iter().filter(|agent| !agent.hidden && !agent.subagent).collect()
}

/// Color-index list: every non-hidden agent, including subagent-only ones
/// (`context/local.tsx:62-64,128-135`).
pub(crate) fn visible_agents<'a>(agents: &'a [AgentFact<'a>]) -> Vec<&'a AgentFact<'a>> {
    agents.iter().filter(|agent| !agent.hidden).collect()
}

/// Deduplicated categorical colors at step 400 with new designs, else 200
/// (`context/local.tsx:77-84`). `scales` is the theme's `categorical` hues in
/// document order, each with the nine upstream steps.
pub(crate) fn agent_palette(scales: &[[[u8; 4]; 9]], new_designs: bool) -> Vec<[u8; 4]> {
    let step = if new_designs { 3 } else { 1 };
    let mut palette: Vec<[u8; 4]> = Vec::with_capacity(scales.len());
    for scale in scales {
        let color = scale[step];
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    palette
}

/// Current agent color (`context/local.tsx:128-135`): explicit color wins;
/// otherwise the visible-agent index modulo the palette; an unknown id falls
/// back to the first palette entry.
pub(crate) fn agent_color(
    agents: &[AgentFact<'_>],
    id: &str,
    palette: &[[u8; 4]],
) -> Option<[u8; 4]> {
    let visible = visible_agents(agents);
    match visible.iter().position(|agent| agent.id == id) {
        Some(index) => visible[index]
            .color
            .or_else(|| palette.get(index % palette.len().max(1)).copied()),
        None => palette.first().copied(),
    }
}

// ── display retention during catalog reload ──────────────────────────

/// Last resolved prompt labels (`index.tsx:1537-1567`). `agent_label` is the
/// titlecased agent id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PromptDisplay {
    pub agent_label: Option<String>,
    pub model_label: String,
    pub provider_label: String,
    pub variant: Option<String>,
}

impl PromptDisplay {
    /// The memo's initial value (`index.tsx:1566-1572`): live model/provider,
    /// no agent or variant yet.
    pub(crate) fn initial(model: &str, provider: &str) -> Self {
        Self {
            agent_label: None,
            model_label: model.to_owned(),
            provider_label: provider.to_owned(),
            variant: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PromptDisplayInput<'a> {
    /// Same location key as the session (`index.tsx:1549`).
    pub location_matches: bool,
    /// Location agent list still loading or the model catalog not ready
    /// (`index.tsx:1551`).
    pub loading: bool,
    /// A load error for this location (`index.tsx:1552-1553`).
    pub failed: bool,
    pub agent_id: Option<&'a str>,
    pub model: &'a str,
    pub provider: &'a str,
    pub variant: Option<&'a str>,
}

/// Keep the previous labels while loading, replace them once the catalog is
/// available or failed, and never leak labels across locations.
pub(crate) fn next_prompt_display(previous: &PromptDisplay, input: &PromptDisplayInput<'_>) -> PromptDisplay {
    if !input.location_matches || (input.loading && !input.failed) {
        return previous.clone();
    }
    PromptDisplay {
        agent_label: input.agent_id.map(titlecase),
        model_label: input.model.to_owned(),
        provider_label: input.provider.to_owned(),
        variant: input.variant.map(str::to_owned),
    }
}

// ── initial reveal ────────────────────────────────────────────────────

/// `createFadeIn` tween duration (`util/signal.ts:25`), in milliseconds.
pub(crate) const METADATA_FADE_MS: u64 = 160;

/// Smoothstep tween progress (`ui/animation.ts:35,46-52,177-189`).
pub(crate) fn fade_alpha(elapsed_ms: u64) -> f64 {
    let progress = (elapsed_ms as f64 / METADATA_FADE_MS as f64).min(1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

/// One metadata fade-in (`util/signal.ts:20-49`). Upstream keeps a per-local
/// `revealed` flag in a WeakSet, so a prompt remount jumps to the final alpha
/// instead of replaying the reveal (`index.tsx:1594-1595,1606-1608`).
#[derive(Clone, Debug, Default)]
pub(crate) struct MetadataReveal {
    revealed: bool,
    started_ms: Option<u64>,
}

impl MetadataReveal {
    /// `visible_at_mount` is `show()` at construction; for a remount after a
    /// previous reveal pass `true`.
    pub(crate) fn new(visible_at_mount: bool) -> Self {
        Self { revealed: visible_at_mount, started_ms: None }
    }

    /// Alpha for a fixed clock. `!visible` jumps to 0; disabled animation
    /// jumps to 1; an already revealed target keeps its running tween and
    /// only jumps to 1 when the tween is over or was never started.
    pub(crate) fn alpha(&mut self, now_ms: u64, visible: bool, animations_enabled: bool) -> f64 {
        if !visible {
            self.started_ms = None;
            return 0.0;
        }
        if !animations_enabled {
            self.revealed = true;
            self.started_ms = None;
            return 1.0;
        }
        if self.revealed {
            return match self.started_ms {
                Some(start) => fade_alpha(now_ms.saturating_sub(start)),
                None => 1.0,
            };
        }
        self.revealed = true;
        self.started_ms = Some(now_ms);
        0.0
    }
}

/// Row count helper re-exported for callers that compose the pane.
pub(crate) fn pane_rows(input_rows: usize) -> usize {
    prompt_rows(input_rows)
}

/// Surface row count helper (top padding, editor, metadata block).
pub(crate) fn pane_surface_rows(input_rows: usize) -> usize {
    surface_rows(input_rows)
}

/// Text column helper mirroring [`prompt_text_width`].
pub(crate) fn pane_text_width(pane_width: u16, terminal_width: u16) -> usize {
    prompt_text_width(pane_width, terminal_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_theme_runtime::{RuntimeTheme, ThemeMode};

    fn dark() -> SemanticPalette {
        RuntimeTheme::builtin_default(ThemeMode::Dark)
            .semantic
            .expect("shipped V2 asset resolves")
    }

    fn light() -> SemanticPalette {
        RuntimeTheme::builtin_default(ThemeMode::Light)
            .semantic
            .expect("shipped V2 asset resolves")
    }

    fn hue(value: [u8; 4]) -> crate::native_theme_runtime::HueColor {
        crate::native_theme_runtime::HueColor { rgba: value, provenance: None }
    }

    fn layout<'a>(agent: Option<&'a str>, model: &'a str, provider: &'a str) -> PromptMetadataInput<'a> {
        PromptMetadataInput {
            mode: PromptMode::Normal,
            agent,
            auto: false,
            model,
            provider,
            variant: None,
            limit: None,
        }
    }

    // CP-02 — border and lip glyphs.
    #[test]
    fn cp02_border_is_left_rail_and_separate_half_block_lip() {
        let border = prompt_border([0x14, 0x14, 0x14, 0xff]);
        assert_eq!(border.vertical, '┃');
        assert_eq!(border.bottom_left, '╹');
        assert_eq!(border.lip_left, '╹');
        assert_eq!(border.lip_horizontal, '▀');
        let lip = lip_row(44, [0x14, 0x14, 0x14, 0xff]);
        assert_eq!(lip.chars().next(), Some('╹'));
        assert_eq!(cells(&lip), 44);
        assert!(lip.chars().skip(1).all(|c| c == '▀'));
        assert_eq!(lip_row(0, [0; 4]), "");
    }

    #[test]
    fn cp02_transparent_prompt_background_substitutes_spaces() {
        let border = prompt_border([0, 0, 0, 0]);
        // The main rail is independent of the surface alpha (index.tsx:1683-1688).
        assert_eq!(border.vertical, '┃');
        assert_eq!(border.bottom_left, '╹');
        assert_eq!(border.lip_left, ' ');
        assert_eq!(border.lip_horizontal, ' ');
        assert_eq!(lip_row(8, [0, 0, 0, 0]), " ".repeat(8));
    }

    // CP-03 — raised surface through theme transformation.
    #[test]
    fn cp03_prompt_surface_is_decrease_of_raised_background() {
        for palette in [dark(), light()] {
            let raised = palette.background_raised_base.clone();
            let surface = prompt_surface(&palette);
            // gray.700 in the shipped asset decreases to gray.600.
            let gray = palette.scale("gray").expect("gray hue present");
            let step = palette
                .source(&raised)
                .map(|source| source.step)
                .expect("raised base is a hue reference");
            let index = (step / 100 - 1) as usize;
            assert_eq!(surface.rgba, gray[index - 1].rgba);
            assert_ne!(surface.rgba, raised.rgba);
            assert_eq!(surface.rgba[3], raised.rgba[3], "alpha is preserved");
        }
        // A literal token has no scale position, so decrease is the identity
        // (resolve.ts:181 `if (!match) return color`).
        let palette = dark();
        assert_eq!(palette.decrease_default(&hue([0x48, 0x48, 0x48, 0xff])).rgba, [0x48, 0x48, 0x48, 0xff]);
    }

    // CP-04 — padding and inner width at 40/43/44 columns.
    #[test]
    fn cp04_surface_rows_are_exactly_the_pane_width_at_40_43_44() {
        for columns in [40u16, 43, 44, 80] {
            let row = surface_row(columns, columns, "draft");
            assert_eq!(cells(&row), usize::from(columns), "columns={columns}");
            assert_eq!(row.chars().next(), Some('┃'));
        }
        // 43 -> inset 1 (40 text cells), 44 -> inset 2 (39 text cells).
        let at_43 = surface_row(43, 43, &"x".repeat(80));
        assert_eq!(cells(&at_43), 43);
        assert_eq!(at_43.chars().filter(|c| *c == 'x').count(), 40);
        let at_44 = surface_row(44, 44, &"x".repeat(80));
        assert_eq!(cells(&at_44), 44);
        assert_eq!(at_44.chars().filter(|c| *c == 'x').count(), 39);
        // A pane narrower than the inset budget still never overflows.
        for columns in 0..8u16 {
            assert_eq!(cells(&surface_row(columns, 43, "abcdef")), usize::from(columns));
        }
        assert_eq!(pane_rows(1), 5);
        assert_eq!(pane_surface_rows(1), 4);
        assert_eq!(pane_text_width(43, 43), 40);
    }

    // CP-05 — content order and styles.
    #[test]
    fn cp05_metadata_order_has_no_dot_between_model_and_provider() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: Some("Build"),
            auto: true,
            model: "GPT-5",
            provider: "OpenAI",
            variant: Some("high"),
            limit: Some("$3.20 left"),
        };
        let layout = metadata_layout(&input, 120);
        assert_eq!(
            metadata_text(&layout),
            "Build auto · GPT-5 OpenAI · high  $3.20 left"
        );
        let runs = metadata_runs(&layout);
        let joined = runs.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join(" ");
        assert_eq!(joined, metadata_text(&layout));
        assert_eq!(
            runs.iter().map(|(role, _)| *role).collect::<Vec<_>>(),
            [
                MetadataRole::Agent,
                MetadataRole::Auto,
                MetadataRole::ModelSeparator,
                MetadataRole::Model,
                MetadataRole::Provider,
                MetadataRole::VariantSeparator,
                MetadataRole::Variant,
                MetadataRole::Limit,
            ]
        );
        // No separator between model and provider (metadata.tsx:56-71).
        let model = runs.iter().position(|(role, _)| *role == MetadataRole::Model).unwrap();
        let provider = runs.iter().position(|(role, _)| *role == MetadataRole::Provider).unwrap();
        assert_eq!(provider, model + 1);
    }

    #[test]
    fn cp05_metadata_runs_map_to_upstream_styles() {
        assert_eq!(metadata_style(MetadataRole::Agent, false, false).fg, MetadataFg::Highlight);
        assert_eq!(metadata_style(MetadataRole::Auto, false, false).fg, MetadataFg::Muted);
        assert_eq!(metadata_style(MetadataRole::Model, false, false).fg, MetadataFg::Base);
        assert_eq!(metadata_style(MetadataRole::Model, true, false).fg, MetadataFg::Muted);
        assert_eq!(metadata_style(MetadataRole::Provider, false, false).fg, MetadataFg::Muted);
        let variant = metadata_style(MetadataRole::Variant, false, false);
        assert_eq!((variant.fg, variant.bold), (MetadataFg::Warning, true));
        assert_eq!(metadata_style(MetadataRole::Limit, false, false).fg, MetadataFg::Muted);
        assert_eq!(metadata_style(MetadataRole::Limit, false, true).fg, MetadataFg::Error);
    }

    // CP-06 — candidate fitting.
    #[test]
    fn cp06_metadata_threshold_omits_agent_and_provider_below_44() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: Some("Build"),
            auto: false,
            model: "GPT-5",
            provider: "OpenAI",
            variant: None,
            limit: None,
        };
        // 43 -> budget 34; agent/provider are dropped (metadata.tsx:127-128).
        let at_43 = metadata_layout(&input, 43);
        assert_eq!(at_43.agent, None);
        assert_eq!(at_43.provider, None);
        assert_eq!(metadata_text(&at_43), "GPT-5");
        // 44 -> budget 31 but the agent and provider are eligible again.
        let at_44 = metadata_layout(&input, 44);
        assert_eq!(at_44.agent, Some("Build"));
        assert_eq!(at_44.provider, Some("OpenAI"));
        assert_eq!(metadata_text(&at_44), "Build · GPT-5 OpenAI");
    }

    #[test]
    fn cp06_metadata_fit_drops_candidates_in_upstream_order() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: Some("Build"),
            auto: true,
            model: "GPT-5",
            provider: "Vendor / Cloud / OpenAI",
            variant: None,
            limit: None,
        };
        let wide = metadata_layout(&input, 120);
        assert!(wide.auto, "everything fits at 120 columns");
        assert_eq!(wide.provider, Some("Vendor / Cloud / OpenAI"));

        // Terminal 44 -> budget 31:
        //   all fields            = 40 cells (rejected)
        //   without auto          = 35 cells (rejected)
        //   last provider segment = 20 cells (selected)
        let narrow = metadata_layout(&input, 44);
        assert!(!narrow.auto, "auto is the first field dropped");
        assert_eq!(narrow.provider, Some("OpenAI"), "provider shortens to its last / segment");

        // A long model makes the shortened-provider candidate fail too:
        // 5 + 1 + 1 + 1 + 22 + 1 + 6 = 37 > 31, while the provider-less
        // candidate is exactly 30 cells.
        let long = PromptMetadataInput { model: "a-very-long-model-name", ..input };
        let fitted = metadata_layout(&long, 44);
        assert_eq!(fitted.provider, None, "provider is the last field dropped");
        assert_eq!(fitted.model, "a-very-long-model-name");
        assert_eq!(cells(&metadata_text(&fitted)), 30);

        // Longer still: no candidate fits and the fallback truncates.
        let longer_model = "m".repeat(40);
        let longer = PromptMetadataInput { model: longer_model.as_str(), ..long };
        let truncated = metadata_layout(&longer, 44);
        assert_eq!(truncated.provider, None);
        assert!(truncated.model.ends_with('…'));
        assert_eq!(cells(&metadata_text(&truncated)), 31);
    }

    #[test]
    fn cp06_fallback_truncates_model_to_a_nine_cell_minimum() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: Some("Build"),
            auto: true,
            model: "an-extremely-long-model-display-name",
            provider: "OpenAI",
            variant: Some("high"),
            limit: Some("$1 left"),
        };
        let fitted = metadata_layout(&input, 44);
        // budget 31 - prefix 8 - suffix (7 + 8) = 8 -> clamped to 9 cells.
        assert_eq!(fitted.agent, Some("Build"));
        assert!(!fitted.auto);
        assert_eq!(fitted.provider, None);
        assert_eq!(fitted.variant, Some("high"));
        assert_eq!(fitted.limit, Some("$1 left"));
        assert_eq!(cells(&fitted.model), 9);
        assert!(fitted.model.ends_with('…'));
        assert!(!fitted.model.contains(" …"), "space before ellipsis is removed");
    }

    #[test]
    fn cp06_wide_unicode_is_measured_in_cells() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: None,
            auto: false,
            model: "界界界界界",
            provider: "",
            variant: None,
            limit: None,
        };
        // Terminal 43 -> budget 34. The 10-cell model fits.
        assert_eq!(metadata_layout(&input, 43).model, "界界界界界");
        let wide_model = "界".repeat(30);
        let fitted = metadata_layout(
            &PromptMetadataInput { model: wide_model.as_str(), ..input },
            43,
        );
        // Fallback budget is 34 cells; a 2-cell cluster cannot fill the last
        // cell, so the result is 16 wide glyphs + `…` = 33 cells.
        assert_eq!(cells(&fitted.model), 33);
        assert!(fitted.model.ends_with('…'));
        assert_eq!(cells("界"), 2);
        assert_eq!(truncate_width("界界界", 4), "界…");
        assert_eq!(truncate_width("界界界", 1), "…");
    }

    // CP-07 — data-driven agent selection and colors.
    #[test]
    fn cp07_agent_selection_and_palette_color_are_data_driven() {
        const BLUE: [u8; 4] = [0, 0, 255, 255];
        const PURPLE: [u8; 4] = [128, 0, 128, 255];
        const GREEN: [u8; 4] = [0, 128, 0, 255];
        const RED: [u8; 4] = [255, 0, 0, 255];
        let scale = |color| [color; 9];
        let scales = [scale(BLUE), scale(PURPLE), scale(BLUE), scale(GREEN)];
        // Deduplicated in document order; step 200 by default, 400 with new
        // designs.
        assert_eq!(agent_palette(&scales, false), vec![BLUE, PURPLE, GREEN]);
        let agents = [
            AgentFact { id: "build", hidden: false, subagent: false, color: None },
            AgentFact { id: "hidden", hidden: true, subagent: false, color: Some(RED) },
            AgentFact { id: "reviewer", hidden: false, subagent: true, color: None },
            AgentFact { id: "custom", hidden: false, subagent: false, color: Some(RED) },
            AgentFact { id: "third", hidden: false, subagent: false, color: None },
        ];
        // Picker excludes subagent-only and hidden agents (local.tsx:59-61).
        assert_eq!(
            selectable_agents(&agents).iter().map(|a| a.id).collect::<Vec<_>>(),
            ["build", "custom", "third"]
        );
        // Color index uses every non-hidden agent, including subagent-only.
        assert_eq!(
            visible_agents(&agents).iter().map(|a| a.id).collect::<Vec<_>>(),
            ["build", "reviewer", "custom", "third"]
        );
        let palette = agent_palette(&scales, false);
        assert_eq!(agent_color(&agents, "build", &palette), Some(BLUE));
        assert_eq!(agent_color(&agents, "reviewer", &palette), Some(PURPLE));
        assert_eq!(agent_color(&agents, "custom", &palette), Some(RED), "explicit color wins");
        assert_eq!(agent_color(&agents, "missing", &palette), Some(BLUE), "index -1 -> palette[0]");
        // Palette wrap: visible index 3 over a two-entry palette -> palette[1].
        assert_eq!(agent_color(&agents, "third", &[PURPLE, GREEN]), Some(GREEN));
        let wrapped = [AgentFact { id: "x", hidden: false, subagent: false, color: None }];
        assert_eq!(agent_color(&wrapped, "x", &[PURPLE]), Some(PURPLE));
        assert_eq!(agent_color(&wrapped, "x", &[]), None);
    }

    // CP-08 — display retention during catalog reload.
    #[test]
    fn cp08_catalog_reload_retains_previous_labels_until_resolved() {
        let previous = PromptDisplay {
            agent_label: Some("Build".to_owned()),
            model_label: "GPT-5".to_owned(),
            provider_label: "OpenAI".to_owned(),
            variant: Some("high".to_owned()),
        };
        let loading = PromptDisplayInput {
            location_matches: true,
            loading: true,
            failed: false,
            agent_id: Some("plan"),
            model: "Claude",
            provider: "Anthropic",
            variant: None,
        };
        assert_eq!(next_prompt_display(&previous, &loading), previous);

        let loaded = PromptDisplayInput { loading: false, ..loading };
        let resolved = next_prompt_display(&previous, &loaded);
        assert_eq!(resolved.agent_label.as_deref(), Some("Plan"));
        assert_eq!(resolved.model_label, "Claude");
        assert_eq!(resolved.provider_label, "Anthropic");
        assert_eq!(resolved.variant, None);

        // A load error resolves the display too (index.tsx:1552-1554).
        let failed = PromptDisplayInput { loading: true, failed: true, ..loaded };
        assert_eq!(next_prompt_display(&previous, &failed).model_label, "Claude");

        // A different location never leaks labels.
        let other_location = PromptDisplayInput { location_matches: false, ..loaded };
        assert_eq!(next_prompt_display(&previous, &other_location), previous);
        assert_eq!(titlecase("build"), "Build");
        assert_eq!(titlecase("code-reviewer"), "Code-Reviewer");
        assert_eq!(titlecase("my_agent"), "My_agent");
    }

    // CP-09 — initial reveal and muted/shell border states.
    #[test]
    fn cp09_metadata_reveal_tweens_once_and_never_replays() {
        assert!((fade_alpha(0) - 0.0).abs() < f64::EPSILON);
        assert!((fade_alpha(METADATA_FADE_MS / 2) - 0.5).abs() < 1e-9);
        assert!((fade_alpha(METADATA_FADE_MS) - 1.0).abs() < f64::EPSILON);
        assert!((fade_alpha(u64::MAX) - 1.0).abs() < f64::EPSILON);

        let mut reveal = MetadataReveal::new(false);
        assert_eq!(reveal.alpha(1_000, true, true), 0.0, "starts at zero");
        assert!((reveal.alpha(1_080, true, true) - 0.5).abs() < 1e-9);
        assert!((reveal.alpha(1_160, true, true) - 1.0).abs() < f64::EPSILON);
        // Re-show after hiding jumps to the target, no second tween.
        assert_eq!(reveal.alpha(2_000, false, true), 0.0);
        assert!((reveal.alpha(2_001, true, true) - 1.0).abs() < f64::EPSILON);

        // Remount after a previous reveal (WeakSet hit) jumps immediately.
        let mut remount = MetadataReveal::new(true);
        assert!((remount.alpha(5_000, true, true) - 1.0).abs() < f64::EPSILON);
        // Reduced animation mode jumps as well.
        let mut static_mode = MetadataReveal::new(false);
        assert!((static_mode.alpha(0, true, false) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cp09_highlight_precedence_is_muted_shell_agent_border() {
        let border = [0x48, 0x48, 0x48, 0xff];
        let shell = [0xfa, 0xb2, 0x83, 0xff];
        let agent = [0x5c, 0x9c, 0xf5, 0xff];
        assert_eq!(prompt_highlight(true, true, Some(agent), shell, border), border);
        assert_eq!(prompt_highlight(false, true, Some(agent), shell, border), shell);
        assert_eq!(prompt_highlight(false, false, Some(agent), shell, border), agent);
        assert_eq!(prompt_highlight(false, false, None, shell, border), border);

        // Reveal tint is opaque and interpolates toward the highlight.
        assert_eq!(border_highlight(border, agent, 0.0), border);
        assert_eq!(border_highlight(border, agent, 1.0), agent);
        assert_eq!(border_highlight(border, agent, 0.5), [0x52, 0x72, 0x9f, 255]);
        assert_eq!(tint([0, 0, 0, 0], [10, 20, 30, 0], 1.0)[3], 255);
    }

    // CP-01 — one owner of the composer slot.
    #[test]
    fn cp01_prompt_slot_replaces_auxiliary_composer_and_permission() {
        assert_eq!(
            prompt_slot(true, false, true, true, true, true),
            PromptSlot::AuxiliaryComposer
        );
        assert_eq!(
            prompt_slot(false, true, true, true, true, true),
            PromptSlot::AuxiliaryComposer
        );
        assert_eq!(prompt_slot(false, false, true, true, true, true), PromptSlot::Permission);
        assert_eq!(prompt_slot(false, false, false, true, true, true), PromptSlot::Form);
        assert_eq!(prompt_slot(false, false, false, false, true, true), PromptSlot::LocationMissing);
        assert_eq!(prompt_slot(false, false, false, false, false, true), PromptSlot::Hidden);
        assert_eq!(prompt_slot(false, false, false, false, false, false), PromptSlot::Prompt);
    }

    #[test]
    fn shell_mode_renders_shell_label_and_empty_model() {
        let input = PromptMetadataInput {
            mode: PromptMode::Shell,
            agent: None,
            auto: true,
            model: "GPT-5",
            provider: "OpenAI",
            variant: Some("high"),
            limit: Some("$1 left"),
        };
        let layout = metadata_layout(&input, 120);
        assert_eq!(layout.agent, Some("Shell"));
        assert_eq!(layout.model, "");
        assert_eq!(layout.provider, None);
        assert_eq!(layout.variant, None);
        assert_eq!(layout.limit, None);
        assert_eq!(metadata_text(&layout), "Shell");
        assert_eq!(metadata_runs(&layout).len(), 1);
    }
}
