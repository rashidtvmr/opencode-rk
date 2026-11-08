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
//! - `context/local.tsx:398-417` — `model.parsed()`: the honest empty state
//!   while no selection exists (`provider: "Connect a provider"`,
//!   `model: "No provider selected"`), and the resolved display names
//!   (`provider?.name ?? providerID`, `info?.name ?? "{id} (unavailable)"`)
//!   once one does.
//! - `component/prompt/index.tsx:1539-1572` — `promptDisplay`: keep the last
//!   resolved labels while destination catalogs load, replace them on a
//!   location change.
//! - `routes/session/index.tsx:1590-1659` — exactly one of auxiliary Composer,
//!   permission, form, location-missing, prompt or nothing occupies the slot.
//! - `util/signal.ts:20-49` + `ui/animation.ts:46-52,177-189` — metadata
//!   fade-in: 0.16 s smoothstep tween; a mounted-once WeakSet key means a
//!   remount jumps to the final alpha instead of replaying the reveal.
//!
//! # The composer pane is this module's authority
//!
//! `component/prompt/index.tsx:1678-1898` composes exactly one block:
//!
//! ```text
//! outer box (:1680)
//! ├ main box, left border SplitBorder + bottomLeft "╹" (:1681-1689)
//! │ └ raised surface, paddingTop 1, responsive inset (:1690-1698)
//! │   ├ optional image strip (:1699-1766) — native_image_preview owns it
//! │   ├ textarea minHeight 1, maxHeight max(6, floor(h/3)) (:1767-1854)
//! │   └ metadata block: paddingTop 1 + row (:1855-1870)
//! ├ height-1 lip "╹"/"▀" (:1873-1898)
//! └ footer slot (:1899-1983) — location, or spinner + interrupt hint
//! ```
//!
//! [`PromptPane`] returns that block as rows, the metadata row, the
//! placeholder row and the insertion cursor cell, so no other module
//! re-derives prompt geometry. Rows reserved by the block are
//! `prompt_rows(input_rows)` (`native_prompt_layout`): the lip is inside the
//! block, the footer is its sibling.
//!
//! Everything here is pure layout/style state; the painter and the terminal
//! live in the existing view modules.
#![allow(dead_code)]

use crate::native_box_style::EffectiveBox;
use crate::native_prompt_layout::{
    prompt_inset, prompt_text_width, prompt_rows, surface_rows, wrap_prompt,
};
use crate::native_theme_runtime::{HueColor, SemanticPalette};
use crate::native_view_layout::{fit, slice_cells, visit_unpadded_cells};

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
    /// Approved oc2 extension: the observed reasoning effort, rendered after
    /// the variant with the same `· <value>` shape. Upstream has no effort
    /// token at this pin, so it is absent from parity comparisons.
    pub effort: Option<&'a str>,
}

impl<'a> Default for PromptMetadataInput<'a> {
    fn default() -> Self {
        Self {
            mode: PromptMode::Normal,
            agent: None,
            auto: false,
            model: "",
            provider: "",
            variant: None,
            limit: None,
            effort: None,
        }
    }
}

/// Upstream `local.model.parsed().provider` while no provider/model selection
/// exists (`context/local.tsx:402-408`): the honest empty state, never a
/// backend status summary such as `unset`.
pub(crate) const PROMPT_EMPTY_PROVIDER_LABEL: &str = "Connect a provider";
/// Upstream `local.model.parsed().model` while no selection exists
/// (`context/local.tsx:403-407`).
pub(crate) const PROMPT_EMPTY_MODEL_LABEL: &str = "No provider selected";

impl<'a> PromptMetadataInput<'a> {
    /// The unconnected empty state (`context/local.tsx:398-417`): while no
    /// model selection exists the row paints these exact labels — the model
    /// cell reads `No provider selected` and the provider cell invites the
    /// connect flow — instead of leaking a status summary (`unset`).
    /// `agent` stays the caller's resolved agent label
    /// (`promptDisplay().agentLabel`, `index.tsx:1559`); `None` keeps the
    /// agent cell out exactly like upstream's `agent: undefined`.
    pub(crate) fn unconnected(agent: Option<&'a str>) -> Self {
        Self {
            mode: PromptMode::Normal,
            agent,
            auto: false,
            model: PROMPT_EMPTY_MODEL_LABEL,
            provider: PROMPT_EMPTY_PROVIDER_LABEL,
            variant: None,
            limit: None,
            effort: None,
        }
    }
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
    pub effort: Option<&'a str>,
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
    /// Approved extension: `· <effort>` after the variant.
    Effort,
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
        MetadataRole::Variant | MetadataRole::Effort => MetadataStyle { fg: MetadataFg::Warning, bold: true },
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
        effort: input.effort,
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
            effort: None,
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
        "{}{}{}",
        input.variant.map_or_else(String::new, |variant| format!(" · {variant}")),
        input.effort.map_or_else(String::new, |effort| format!(" · {effort}")),
        input.limit.map_or_else(String::new, |limit| format!("  {limit}")),
    );
    let budget = 9usize.max(
        width
            .saturating_sub(cells(&prefix))
            .saturating_sub(cells(&suffix)),
    );
    let model = strip_space_before_ellipsis(&truncate_width(input.model, budget));
    PromptMetadataLayout {
        agent,
        auto: false,
        model,
        provider: None,
        variant: input.variant,
        limit: input.limit,
        effort: input.effort,
    }
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
    if let Some(effort) = layout.effort {
        parts.push("·".to_owned());
        parts.push(effort.to_owned());
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
        if let Some(effort) = layout.effort {
            runs.push((MetadataRole::VariantSeparator, "·".to_owned()));
            runs.push((MetadataRole::Effort, effort.to_owned()));
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

// ── composer pane: rows, placeholder, cursor, shortcut ────────────────
//
// Upstream geometry this block pins:
// - textarea `minHeight={1}`/`maxHeight={max(6, floor(height/3))}`
//   (`component/prompt/index.tsx:1674,1773-1774`): the composer grows with
//   the draft, then scrolls inside the cap.
// - surface `paddingTop={1}` (`:1690-1698`), metadata block
//   `paddingTop={1}` (`:1855-1870`), separate lip (`:1873-1898`).
// - placeholder text and its terminal-width budget (`:1615-1627`).
// - interrupt shortcut `esc interrupt` / `esc again to interrupt`
//   (`:138-145`, shown while `status() === "running"` at `:1912-1928`).

/// Which box presentation the composer resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptPaneStyle {
    /// Default session composer: left rail, raised surface, half-block lip
    /// (`index.tsx:1681-1698,1873-1898`). `> ` is not painted because the
    /// rail is the marker (`native_view_chat` legacy path is separate).
    Rail,
    /// Explicit full outline (`BoxId::ChatComposer` opts in through
    /// `BoxAppearance`): real border rows around the editor.
    Framed,
    /// No border at all: `> ` prompt rows, the legacy fallback.
    Plain,
}

/// Geometry of the composer block. All columns/rows are relative to the
/// block origin; `text_left`/`text_top` locate the first editor cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PromptPaneGeometry {
    pub style: PromptPaneStyle,
    /// Visible editor rows: at least one, capped by the textarea ceiling
    /// (`index.tsx:1674,1773-1774`) and then scrolled to the cursor.
    pub input_rows: usize,
    /// Rows the block reserves, including padding and the lip.
    pub height: usize,
    pub text_left: usize,
    pub text_top: usize,
    pub text_width: usize,
}

/// Resolve the composer shape exactly once.
///
/// `allow_rail` is `!BoxAppearance::home_composer_outline()` for the session
/// composer; the framed-only helper passes `false`. The rail needs the
/// default sharp, zero-padding, bordered composer and a body tall enough for
/// `input + 4` rows (`native_prompt_layout::prompt_rows`).
pub(crate) fn prompt_geometry(
    width: usize,
    height: usize,
    draft: &str,
    frame: EffectiveBox,
    allow_rail: bool,
    terminal_width: u16,
    terminal_height: u16,
) -> PromptPaneGeometry {
    let height = height.min(crate::native_view_chat::MAX_VIEW_ROWS);
    let width = width.min(u16::MAX as usize);
    let pad = usize::from(frame.padding);
    if height == 0 {
        return PromptPaneGeometry {
            style: PromptPaneStyle::Plain,
            input_rows: 0,
            height: 0,
            text_left: 0,
            text_top: 0,
            text_width: 0,
        };
    }
    if allow_rail && frame.has_border() && frame.padding == 0 && width > 5 && height >= 6 {
        // `index.tsx:1691-1692`: the horizontal inset keys on the TERMINAL
        // width (`dimensions().width < 44 ? 1 : 2`), never the pane width. The
        // pane-derived width only coincides with it when no rail/sidebar is
        // present. `prompt_inset` owns the strict `< 44` rule.
        // `index.tsx:1674`: the textarea row cap is
        // `max(6, floor(dimensions().height / 3))` — `editor_max_rows` on the
        // TERMINAL height, never the body budget; the pane only clamps it
        // structurally (`input + 4` must fit the body rows).
        let max_input = crate::native_prompt_layout::editor_max_rows(terminal_height).min(height - 4);
        let inset = usize::from(crate::native_prompt_layout::prompt_inset(terminal_width));
        let text_width = width.saturating_sub(1 + 2 * inset);
        let input = wrap_prompt(draft, text_width.max(1), draft.len())
            .rows
            .len()
            .min(max_input)
            .max(1);
        return PromptPaneGeometry {
            style: PromptPaneStyle::Rail,
            input_rows: input,
            height: input + 4,
            text_left: 1 + inset,
            text_top: 1,
            text_width,
        };
    }
    let framed = frame.has_border() && width > 2 + 2 * pad && height > 3 + 2 * pad;
    let overhead = if framed { 2 + 2 * pad } else { 0 };
    let text_width = width.saturating_sub(overhead + 2);
    let input = wrap_prompt(draft, text_width.max(1), draft.len())
        .rows
        .len()
        .clamp(1, ((height.saturating_sub(overhead)) / 2).max(1));
    PromptPaneGeometry {
        style: if framed { PromptPaneStyle::Framed } else { PromptPaneStyle::Plain },
        input_rows: input,
        height: input + overhead,
        text_left: if framed { 3 + pad } else { 2 },
        text_top: if framed { 1 + pad } else { 0 },
        text_width,
    }
}

/// Role of one composer row. The chat painter maps these onto its own row
/// classes; nothing is recovered by parsing glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptPaneRowKind {
    /// One-row top padding inside the raised surface (`index.tsx:1693`).
    SurfacePadding,
    /// An editor row, scrolled to the insertion row.
    Editor,
    /// Editor row showing the textarea placeholder because the draft is
    /// empty (`index.tsx:1769-1770`).
    Placeholder,
    /// Metadata block top padding (`index.tsx:1855`).
    MetadataPadding,
    /// The metadata row itself (`index.tsx:1856-1869`).
    Metadata,
    /// The separate half-block lip (`index.tsx:1873-1898`).
    Lip,
    /// A real outline edge; the painter supplies the glyphs.
    FrameEdge,
    /// Outline inner padding; the painter supplies the blanks.
    FramePadding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PromptPaneRow {
    pub text: String,
    pub kind: PromptPaneRowKind,
}

/// Everything the chat renderer and painter need from the composer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PromptPaneInput<'a> {
    pub width: usize,
    /// Body rows available to the chat frame (header + transcript + block).
    pub height: usize,
    pub draft: &'a str,
    /// Byte insertion offset into `draft`.
    pub cursor: usize,
    pub mode: PromptMode,
    /// Terminal columns: the metadata text budget uses the terminal width,
    /// not the prompt's own width (`metadata.tsx:28`).
    pub terminal_width: u16,
    /// Terminal rows: the textarea row cap keys on the terminal height
    /// (`editor_max_rows`, `index.tsx:1674`), never the body budget.
    pub terminal_height: u16,
    /// `status() === "running"` (`index.tsx:1912`).
    pub busy: bool,
    /// `store.interrupt > 0` (`index.tsx:1920`).
    pub interrupt_armed: bool,
    /// Already selected placeholder candidate (`placeholders.normal` /
    /// `placeholders.shell`); `None` keeps the row blank.
    pub placeholder: Option<&'a str>,
    pub metadata: PromptMetadataInput<'a>,
    pub frame: EffectiveBox,
    pub allow_rail: bool,
    /// `promptBg = decrease(background.raised.base)` (`index.tsx:1676`). The
    /// live frame threads the resolved surface here; alpha `0` makes the lip
    /// substitute spaces (`index.tsx:1879,1890-1895`). The `composer`
    /// constructor keeps the opaque raised default for direct unit APIs.
    pub background: [u8; 4],
}

impl<'a> PromptPaneInput<'a> {
    /// Session-composer input: resolves the `BoxId::ChatComposer` frame and
    /// the rail/outline decision from the caller's appearance.
    pub(crate) fn composer(
        appearance: &crate::native_box_style::BoxAppearance,
        width: usize,
        height: usize,
        draft: &'a str,
        terminal_width: u16,
        mode: PromptMode,
        busy: bool,
        metadata: PromptMetadataInput<'a>,
    ) -> Self {
        Self {
            width,
            height,
            draft,
            cursor: draft.len(),
            mode,
            terminal_width,
            terminal_height: u16::try_from(height).unwrap_or(u16::MAX),
            busy,
            interrupt_armed: false,
            placeholder: None,
            metadata,
            frame: appearance.effective(crate::native_box_style::BoxId::ChatComposer),
            allow_rail: !appearance.home_composer_outline(),
            background: [0x14, 0x14, 0x14, 0xff],
        }
    }
}

/// The composed composer block: rows, metadata row, placeholder row and the
/// insertion cursor cell. `rows` is the only place prompt glyphs are built.
#[derive(Clone, Debug)]
pub(crate) struct PromptPane<'a> {
    geometry: PromptPaneGeometry,
    rows: Vec<PromptPaneRow>,
    metadata: PromptMetadataLayout<'a>,
    metadata_row: Option<usize>,
    placeholder_row: Option<usize>,
    cursor: Option<(usize, usize)>,
    mode: PromptMode,
    busy: bool,
    interrupt_armed: bool,
}

impl<'a> PromptPane<'a> {
    /// Compose the block. Pure: the draft, cursor and facts are read only.
    pub(crate) fn compose(input: PromptPaneInput<'a>) -> Self {
        let draft = crate::native_prompt_layout::bounded_source(input.draft);
        // An editor offset beyond the visible source, or one that would split
        // UTF-8, is not a cursor: the caller's view reports no caret for it.
        let cursor_valid = input.cursor <= draft.len() && draft.is_char_boundary(input.cursor);
        let geometry = prompt_geometry(input.width, input.height, draft, input.frame, input.allow_rail, input.terminal_width, input.terminal_height);
        let text_width = geometry.text_width.max(1);
        let wrapped = wrap_prompt(draft, text_width, input.cursor);
        let window_start = if geometry.input_rows == 0 { 0 } else { wrapped.window_start(geometry.input_rows) };
        let metadata = metadata_layout(&input.metadata, input.terminal_width);
        let metadata_text = metadata_text(&metadata);
        let empty = draft.is_empty();
        let mut rows: Vec<PromptPaneRow> = Vec::with_capacity(geometry.height);
        let mut metadata_row = None;
        let mut placeholder_row = None;
        match geometry.style {
            PromptPaneStyle::Rail => {
                let blank = rail_blank(input.width);
                let inset = " ".repeat(geometry.text_left.saturating_sub(1));
                rows.push(PromptPaneRow { text: blank.clone(), kind: PromptPaneRowKind::SurfacePadding });
                for (index, line) in wrapped
                    .rows
                    .iter()
                    .skip(window_start)
                    .take(geometry.input_rows)
                    .enumerate()
                {
                    let show_placeholder = empty && index == 0 && input.placeholder.is_some();
                    let source = if show_placeholder { input.placeholder.unwrap_or_default() } else { line };
                    let text = slice_cells(source, 0, u16::try_from(text_width).unwrap_or(u16::MAX));
                    if show_placeholder {
                        placeholder_row = Some(rows.len());
                    }
                    rows.push(PromptPaneRow {
                        text: format!("{SPLIT_BORDER_VERTICAL}{inset}{text}"),
                        kind: if show_placeholder { PromptPaneRowKind::Placeholder } else { PromptPaneRowKind::Editor },
                    });
                }
                rows.push(PromptPaneRow { text: blank.clone(), kind: PromptPaneRowKind::MetadataPadding });
                metadata_row = Some(rows.len());
                rows.push(PromptPaneRow {
                    text: if metadata_text.is_empty() {
                        blank
                    } else {
                        format!(
                            "{SPLIT_BORDER_VERTICAL}{inset}{}",
                            fit(&metadata_text, u16::try_from(text_width).unwrap_or(u16::MAX))
                        )
                    },
                    kind: PromptPaneRowKind::Metadata,
                });
                rows.push(PromptPaneRow {
                    // Opaque raised surface: the painter re-fills the cell
                    // background, and upstream only substitutes spaces when
                    // `promptBg().a === 0` (`index.tsx:1879,1890-1895`).
                    text: lip_row(
                        u16::try_from(input.width).unwrap_or(u16::MAX),
                        input.background,
                    ),
                    kind: PromptPaneRowKind::Lip,
                });
            }
            PromptPaneStyle::Framed => {
                rows.push(PromptPaneRow { text: String::new(), kind: PromptPaneRowKind::FrameEdge });
                for _ in 0..usize::from(input.frame.padding) {
                    rows.push(PromptPaneRow { text: String::new(), kind: PromptPaneRowKind::FramePadding });
                }
                for (index, line) in wrapped
                    .rows
                    .iter()
                    .skip(window_start)
                    .take(geometry.input_rows)
                    .enumerate()
                {
                    let show_placeholder = empty && index == 0 && input.placeholder.is_some();
                    let source = if show_placeholder { input.placeholder.unwrap_or_default() } else { line };
                    if show_placeholder {
                        placeholder_row = Some(rows.len());
                    }
                    rows.push(PromptPaneRow {
                        text: format!("{}{source}", prompt_prefix(window_start + index)),
                        kind: if show_placeholder { PromptPaneRowKind::Placeholder } else { PromptPaneRowKind::Editor },
                    });
                }
                for _ in 0..usize::from(input.frame.padding) {
                    rows.push(PromptPaneRow { text: String::new(), kind: PromptPaneRowKind::FramePadding });
                }
                rows.push(PromptPaneRow { text: String::new(), kind: PromptPaneRowKind::FrameEdge });
            }
            PromptPaneStyle::Plain => {
                for (index, line) in wrapped
                    .rows
                    .iter()
                    .skip(window_start)
                    .take(geometry.input_rows)
                    .enumerate()
                {
                    let show_placeholder = empty && index == 0 && input.placeholder.is_some();
                    let source = if show_placeholder { input.placeholder.unwrap_or_default() } else { line };
                    if show_placeholder {
                        placeholder_row = Some(rows.len());
                    }
                    rows.push(PromptPaneRow {
                        text: truncate_cells(
                            &format!("{}{source}", prompt_prefix(window_start + index)),
                            input.width,
                        ),
                        kind: if show_placeholder { PromptPaneRowKind::Placeholder } else { PromptPaneRowKind::Editor },
                    });
                }
            }
        }
        // Cursor cell: upstream draws the insertion caret inside the
        // textarea, i.e. at the text column of the scrolled editor row.
        let cursor = match wrapped.cursor {
            Some((column, row)) if cursor_valid => (geometry.text_width > 0 && geometry.input_rows > 0).then(|| {
                (
                    geometry.text_left + column,
                    geometry.text_top + row.saturating_sub(window_start),
                )
            }),
            _ => None,
        };
        Self {
            geometry,
            rows,
            metadata,
            metadata_row,
            placeholder_row,
            cursor,
            mode: input.mode,
            busy: input.busy,
            interrupt_armed: input.interrupt_armed,
        }
    }

    pub(crate) fn geometry(&self) -> PromptPaneGeometry {
        self.geometry
    }

    pub(crate) fn style(&self) -> PromptPaneStyle {
        self.geometry.style
    }

    /// Rows reserved by the block (never more than the body budget).
    pub(crate) fn height(&self) -> usize {
        self.geometry.height
    }

    pub(crate) fn rows(&self) -> &[PromptPaneRow] {
        &self.rows
    }

    pub(crate) fn row(&self, index: usize) -> Option<&PromptPaneRow> {
        self.rows.get(index)
    }

    pub(crate) fn mode(&self) -> PromptMode {
        self.mode
    }

    /// Chosen metadata facts after the terminal-width fitting pass.
    pub(crate) fn metadata(&self) -> &PromptMetadataLayout<'a> {
        &self.metadata
    }

    pub(crate) fn metadata_text(&self) -> String {
        metadata_text(&self.metadata)
    }

    pub(crate) fn metadata_runs(&self) -> Vec<(MetadataRole, String)> {
        metadata_runs(&self.metadata)
    }

    /// Row index (inside [`PromptPane::rows`]) of the metadata row; `None`
    /// for presentations that keep metadata in the sibling footer.
    pub(crate) fn metadata_row(&self) -> Option<usize> {
        self.metadata_row
    }

    /// Row index of the placeholder row, when the draft is empty and a
    /// placeholder candidate was supplied.
    pub(crate) fn placeholder_row(&self) -> Option<usize> {
        self.placeholder_row
    }

    /// Insertion cursor cell `(column, row)` relative to the block origin.
    /// `None` for an invalid/absent cursor or a pane with no text column.
    pub(crate) fn cursor_cell(&self) -> Option<(usize, usize)> {
        self.cursor
    }

    /// Working-state shortcut (`index.tsx:138-145`, status `running` at
    /// `:1912-1928`). `None` when idle, where upstream paints the location
    /// footer instead (`:1946-1964`).
    pub(crate) fn shortcut_row(&self) -> Option<&'static str> {
        if !self.busy {
            return None;
        }
        Some(if self.interrupt_armed { "esc again to interrupt" } else { "esc interrupt" })
    }
}

/// `┃` followed by pane-width padding (`index.tsx:1683-1688` draws the rail
/// on every surface row).
fn rail_blank(width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    format!("{SPLIT_BORDER_VERTICAL}{}", " ".repeat(width.saturating_sub(1)))
}

/// `"> "` on the first rendered line, `"  "` on continuations (legacy
/// fallback and the framed extension).
fn prompt_prefix(index: usize) -> &'static str {
    if index == 0 { "> " } else { "  " }
}

fn truncate_cells(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_owned();
    }
    let mut row = String::new();
    visit_unpadded_cells(text, width, |glyph, _| row.push_str(glyph));
    row
}

/// Textarea placeholder (`index.tsx:1615-1627`).
///
/// Shell mode suggests a shell command, normal mode a prompt; the budget is
/// `width - 5` below 44 terminal columns and `min(75, width - 4) - 5` above,
/// and the result is a whole-cluster prefix trimmed at the end.
pub(crate) fn placeholder_text(
    mode: PromptMode,
    candidates: &[&str],
    index: usize,
    terminal_width: u16,
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let candidate = candidates[index % candidates.len()];
    let value = match mode {
        PromptMode::Shell => format!("Run a command… \"{candidate}\""),
        PromptMode::Normal => format!("Ask anything… \"{candidate}\""),
    };
    let columns = usize::from(terminal_width);
    let width = if columns < 44 {
        columns.saturating_sub(5)
    } else {
        columns.saturating_sub(4).min(75).saturating_sub(5)
    };
    Some(take_width(&value, width.max(1)).trim_end().to_owned())
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
            effort: None,
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

    // CP-03 — transparent mode keeps alpha through the decrease identity.
    #[test]
    fn cp03_transparent_literal_background_keeps_alpha_zero() {
        // Transparent mode feeds a literal transparent background through the
        // decrease identity (resolve.ts:181 `if (!match) return color`); the
        // surface must keep alpha 0 so the terminal shows through on blank
        // padding cells.
        let palette = dark();
        let literal = palette.decrease_default(&hue([0x48, 0x48, 0x48, 0x00]));
        assert_eq!(literal.rgba, [0x48, 0x48, 0x48, 0x00]);
        // The opaque literal path stays byte-identical as well.
        let opaque = palette.decrease_default(&hue([0x48, 0x48, 0x48, 0xff]));
        assert_eq!(opaque.rgba, [0x48, 0x48, 0x48, 0xff]);
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
            effort: None,
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
            effort: None,
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
            effort: None,
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
            effort: None,
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
            effort: None,
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

    #[test]
    fn cp06_budget_is_terminal_derived_when_the_parent_is_narrower() {
        // `metadata.tsx:26-28` measures candidates against
        // `dimensions().width` only; a prompt parent narrower than the
        // terminal (pane inset, open sidebar) never shrinks the budget.
        let long_model = "m".repeat(60);
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: None,
            auto: false,
            model: long_model.as_str(),
            provider: "",
            variant: None,
            limit: None,
            effort: None,
        };
        // Terminal 80 -> budget 67; the 60-cell model fits untouched even
        // though every real prompt parent is narrower than the terminal.
        let layout = metadata_layout(&input, 80);
        assert_eq!(layout.model, long_model);
        assert_eq!(cells(&layout.model), 60);
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
            effort: None,
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

    // Pane composition — the single authority for the composer block.

    fn frame(padding: u8) -> EffectiveBox {
        EffectiveBox {
            preset: crate::native_box_style::RadiusPreset::Sharp,
            radius: 0,
            border_style: crate::native_box_style::BorderStyle::Single,
            thickness: 1,
            padding,
        }
    }

    fn borderless() -> EffectiveBox {
        EffectiveBox {
            border_style: crate::native_box_style::BorderStyle::None,
            ..frame(0)
        }
    }

    fn composer_input<'a>(
        width: usize,
        height: usize,
        draft: &'a str,
        frame: EffectiveBox,
    ) -> PromptPaneInput<'a> {
        PromptPaneInput {
            width,
            height,
            draft,
            cursor: draft.len(),
            mode: PromptMode::Normal,
            terminal_width: u16::try_from(width).unwrap_or(u16::MAX),
            terminal_height: u16::try_from(height).unwrap_or(u16::MAX),
            busy: false,
            interrupt_armed: false,
            placeholder: None,
            metadata: PromptMetadataInput::default(),
            frame,
            allow_rail: true,
            background: [0x14, 0x14, 0x14, 0xff],
        }
    }

    #[test]
    fn pane_rail_block_is_padding_editor_metadata_and_lip() {
        let pane = PromptPane::compose(composer_input(80, 20, "hello", frame(0)));
        let geometry = pane.geometry();
        assert_eq!(geometry.style, PromptPaneStyle::Rail);
        assert_eq!((geometry.input_rows, geometry.height), (1, 5));
        assert_eq!((geometry.text_left, geometry.text_top), (3, 1));
        assert_eq!(geometry.text_width, 75);
        let kinds: Vec<_> = pane.rows().iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                PromptPaneRowKind::SurfacePadding,
                PromptPaneRowKind::Editor,
                PromptPaneRowKind::MetadataPadding,
                PromptPaneRowKind::Metadata,
                PromptPaneRowKind::Lip,
            ]
        );
        // The raised surface and the lip span the whole pane width; the
        // editor row carries the rail, the inset and the clipped text padded
        // to the text column (the right inset is applied by the painter).
        assert_eq!(pane.row(0).unwrap().text, format!("┃{}", " ".repeat(79)));
        assert_eq!(pane.row(1).unwrap().text, format!("┃  {:<75}", "hello"));
        assert_eq!(pane.row(3).unwrap().text, format!("┃{}", " ".repeat(79)));
        let lip = &pane.row(4).unwrap().text;
        assert_eq!(lip.chars().next(), Some('╹'));
        assert_eq!(cells(lip), 80);
        // Empty metadata leaves row 3 blank but still addressable.
        assert_eq!(pane.metadata_text(), "");
        assert_eq!(pane.metadata_row(), Some(3));
        assert_eq!(pane.placeholder_row(), None);
        assert_eq!(pane.cursor_cell(), Some((8, 1)));
    }

    #[test]
    fn pane_rail_caps_editor_rows_and_keeps_the_cursor_visible() {
        // `editor_max_rows` (`index.tsx:1674`): `max(6, floor(terminalHeight / 3))`,
        // keyed on the terminal height; 20 terminal rows allow 6 editor rows.
        let draft = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight";
        let pane = PromptPane::compose(composer_input(60, 20, draft, frame(0)));
        assert_eq!(pane.geometry().input_rows, 6);
        assert_eq!(pane.height(), 10);
        let editors: Vec<_> = pane
            .rows()
            .iter()
            .filter(|row| row.kind == PromptPaneRowKind::Editor)
            .collect();
        assert_eq!(editors.len(), 6);
        // The window follows the tail cursor: the last row is "eight".
        assert!(editors.last().unwrap().text.trim_end().ends_with("eight"));
        assert_eq!(editors.first().unwrap().text.trim_end(), "┃  three");
        assert!(editors.iter().all(|row| row.text.starts_with("┃  ")));
        assert_eq!(pane.cursor_cell(), Some((3 + 5, 6)));
    }

    #[test]
    fn pane_rail_cap_matches_editor_max_rows() {
        // CP-01/CP-02/CP-04: the rail editor cap is
        // `editor_max_rows(terminal_height)` = `max(6, floor(terminalHeight / 3))`
        // (`index.tsx:1674`), keyed on the TERMINAL height; the body budget
        // only clamps it structurally. The retired `((height + 2) / 3)` proxy
        // allowed 14 rows at terminal 40; `editor_max_rows` pins 13
        // (`native_prompt_layout` tests pin (12,6),(24,8),(40,13)).
        let draft = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14";
        for (terminal, cap) in [(12usize, 6usize), (24, 8), (40, 13)] {
            let pane = PromptPane::compose(composer_input(60, terminal, draft, frame(0)));
            assert_eq!(pane.geometry().input_rows, cap, "terminal={terminal}");
        }
        // The key is the terminal height, not the body budget: 20 body rows in
        // a 24-row terminal allow 8 editor rows (`floor(24 / 3)`), where keying
        // the body would clamp to `floor(20 / 3) = 6`.
        let pane = PromptPane::compose(PromptPaneInput {
            terminal_height: 24,
            ..composer_input(60, 20, draft, frame(0))
        });
        assert_eq!(pane.geometry().input_rows, 8, "cap keys on the terminal height");
        // The body budget still clamps structurally (`input + 4 <= height`).
        let pane = PromptPane::compose(PromptPaneInput {
            terminal_height: 40,
            ..composer_input(60, 10, draft, frame(0))
        });
        assert_eq!(pane.geometry().input_rows, 6, "structural clamp keeps input + 4 in the body");
    }

    #[test]
    fn pane_plain_rows_prefix_the_editor_and_show_the_placeholder() {
        let input = PromptPaneInput {
            placeholder: Some("hello world"),
            ..composer_input(20, 10, "", borderless())
        };
        let pane = PromptPane::compose(input);
        assert_eq!(pane.style(), PromptPaneStyle::Plain);
        assert_eq!((pane.geometry().text_left, pane.geometry().text_top), (2, 0));
        assert_eq!(pane.geometry().text_width, 18);
        assert_eq!(pane.height(), 1);
        assert_eq!(pane.placeholder_row(), Some(0));
        assert_eq!(pane.row(0).unwrap().kind, PromptPaneRowKind::Placeholder);
        assert_eq!(pane.row(0).unwrap().text, "> hello world");
        assert_eq!(pane.cursor_cell(), Some((2, 0)));
        // A draft replaces the placeholder row in place.
        let typed = PromptPane::compose(PromptPaneInput {
            placeholder: Some("hello world"),
            ..composer_input(20, 10, "ab", borderless())
        });
        assert_eq!(typed.placeholder_row(), None);
        assert_eq!(typed.row(0).unwrap().text, "> ab");
    }

    #[test]
    fn pane_framed_rows_are_edges_padding_and_prefixed_editor_rows() {
        let pane = PromptPane::compose(composer_input(30, 15, "x", frame(1)));
        assert_eq!(pane.style(), PromptPaneStyle::Framed);
        assert_eq!((pane.geometry().text_left, pane.geometry().text_top), (4, 2));
        assert_eq!(pane.geometry().text_width, 24);
        assert_eq!(pane.height(), 5);
        let kinds: Vec<_> = pane.rows().iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [
                PromptPaneRowKind::FrameEdge,
                PromptPaneRowKind::FramePadding,
                PromptPaneRowKind::Editor,
                PromptPaneRowKind::FramePadding,
                PromptPaneRowKind::FrameEdge,
            ]
        );
        assert_eq!(pane.row(2).unwrap().text, "> x");
        assert_eq!(pane.cursor_cell(), Some((5, 2)));
        // The framed presentation keeps metadata in the sibling footer.
        assert_eq!(pane.metadata_row(), None);
    }

    #[test]
    fn pane_shell_metadata_and_interrupt_shortcut() {
        let shell = PromptMetadataInput {
            mode: PromptMode::Shell,
            ..PromptMetadataInput::default()
        };
        let mut input = composer_input(80, 20, "", frame(0));
        input.metadata = shell;
        let idle = PromptPane::compose(input);
        assert_eq!(idle.metadata_text(), "Shell");
        let row = idle.metadata_row().expect("rail exposes its metadata row");
        assert!(idle.row(row).unwrap().text.starts_with("┃  Shell"));
        assert_eq!(idle.shortcut_row(), None, "idle paints the location footer");

        input.busy = true;
        assert_eq!(PromptPane::compose(input).shortcut_row(), Some("esc interrupt"));
        input.interrupt_armed = true;
        assert_eq!(
            PromptPane::compose(input).shortcut_row(),
            Some("esc again to interrupt")
        );
    }

    #[test]
    fn pane_placeholder_text_is_selected_and_width_budgeted() {
        assert_eq!(placeholder_text(PromptMode::Normal, &[], 0, 80), None);
        assert_eq!(
            placeholder_text(PromptMode::Normal, &["a"], 0, 80).unwrap(),
            "Ask anything… \"a\""
        );
        assert_eq!(
            placeholder_text(PromptMode::Shell, &["ls"], 0, 80).unwrap(),
            "Run a command… \"ls\""
        );
        // Below 44 columns the budget is `width - 5` cells.
        assert_eq!(placeholder_text(PromptMode::Normal, &["a"], 0, 10).unwrap(), "Ask a");
        // At 44 columns it is `min(75, width - 4) - 5`, which fits here.
        assert_eq!(
            placeholder_text(PromptMode::Normal, &["a"], 0, 44).unwrap(),
            "Ask anything… \"a\""
        );
        // Candidates cycle by index.
        let first = placeholder_text(PromptMode::Normal, &["a", "b"], 0, 80).unwrap();
        let second = placeholder_text(PromptMode::Normal, &["a", "b"], 1, 80).unwrap();
        assert!(first.contains("\"a\""));
        assert!(second.contains("\"b\""));
    }

    #[test]
    fn pane_cursor_is_cell_based_for_wide_glyphs() {
        let mut input = composer_input(80, 20, "界a", frame(0));
        input.cursor = "界".len();
        let pane = PromptPane::compose(input);
        assert_eq!(pane.cursor_cell(), Some((3 + 2, 1)));
    }

    #[test]
    fn pane_reports_no_caret_for_offsets_outside_the_visible_source() {
        let mut input = composer_input(80, 20, "abc", frame(0));
        input.cursor = 99;
        assert_eq!(PromptPane::compose(input).cursor_cell(), None);
        input.draft = "é";
        input.cursor = 1; // splits the two-byte scalar
        assert_eq!(PromptPane::compose(input).cursor_cell(), None);
    }

    #[test]
    fn pane_zero_height_composes_nothing() {
        let pane = PromptPane::compose(composer_input(80, 0, "draft", frame(0)));
        assert_eq!(pane.height(), 0);
        assert!(pane.rows().is_empty());
        assert_eq!(pane.cursor_cell(), None);
    }

    #[test]
    fn effort_is_appended_after_variant_and_hidden_in_shell_mode() {
        let input = PromptMetadataInput {
            mode: PromptMode::Normal,
            agent: Some("Build"),
            auto: false,
            model: "GPT-5",
            provider: "OpenAI",
            variant: Some("high"),
            limit: Some("$1 left"),
            effort: Some("medium"),
        };
        let layout = metadata_layout(&input, 120);
        assert_eq!(layout.effort, Some("medium"));
        assert_eq!(
            metadata_text(&layout),
            "Build · GPT-5 OpenAI · high · medium  $1 left"
        );
        let runs = metadata_runs(&layout);
        assert_eq!(
            runs[runs.len() - 2],
            (MetadataRole::Effort, "medium".to_owned())
        );
        assert_eq!(runs.last().unwrap().0, MetadataRole::Limit);
        assert_eq!(
            metadata_style(MetadataRole::Effort, false, false),
            metadata_style(MetadataRole::Variant, false, false),
            "effort reuses the variant token styling"
        );
        // Shell mode has no model row, so effort is not painted.
        let shell = PromptMetadataInput {
            mode: PromptMode::Shell,
            ..input
        };
        assert_eq!(metadata_layout(&shell, 120).effort, None);
    }

    // ── COMPOSER-PARITY (task-1): acceptance sizes 80x24 and 140x40 ──────
    //
    // Runtime evidence `p1-ui3-initial.txt` / `p1-ui3-session.txt` (140x40)
    // and `p1-ui-initial.raw` (80 cols): the composer block must sit at
    // upstream's textarea minimum and grow per wrapped line, and the in-box
    // metadata row must carry the upstream cells (agent · model provider ·
    // variant) with honest empty states instead of a status summary.

    /// Upstream textarea `minHeight={1}` (`index.tsx:1773`): an empty draft
    /// renders exactly one editor row, so the rail block is the five rows
    /// `padding / editor / metadata padding / metadata / lip`
    /// (`prompt_rows(1)`, `index.tsx:1690-1698,1855-1898`).
    #[test]
    fn composer_parity_empty_draft_renders_the_upstream_minimum_block() {
        for (width, height) in [(80usize, 24u16), (140usize, 40u16)] {
            let mut input = composer_input(width, height as usize, "", frame(0));
            input.terminal_width = u16::try_from(width).unwrap();
            input.terminal_height = height;
            input.placeholder = Some("Ask anything… \"Fix a TODO in the codebase\"");
            let pane = PromptPane::compose(input);
            let geometry = pane.geometry();
            assert_eq!(geometry.style, PromptPaneStyle::Rail, "{width}x{height}");
            assert_eq!(geometry.input_rows, 1, "{width}x{height}: minHeight=1");
            assert_eq!(geometry.height, 5, "{width}x{height}: prompt_rows(1)");
            let kinds: Vec<_> = pane.rows().iter().map(|row| row.kind).collect();
            assert_eq!(
                kinds,
                [
                    PromptPaneRowKind::SurfacePadding,
                    PromptPaneRowKind::Placeholder,
                    PromptPaneRowKind::MetadataPadding,
                    PromptPaneRowKind::Metadata,
                    PromptPaneRowKind::Lip,
                ],
                "{width}x{height}"
            );
            assert!(
                pane.row(1).unwrap().text.contains("Ask anything"),
                "{width}x{height}: placeholder painted in the single editor row"
            );
        }
    }

    /// Upstream textarea growth (`index.tsx:1773-1774`): one row per wrapped
    /// line until `maxHeight = max(6, floor(terminalHeight / 3))`
    /// (`index.tsx:1674`) — 8 editor rows at 24 terminal rows, 13 at 40.
    #[test]
    fn composer_parity_editor_grows_per_wrapped_line_to_the_terminal_cap() {
        let three_lines = "one\ntwo\nthree";
        let mut input = composer_input(140, 40, three_lines, frame(0));
        input.terminal_width = 140;
        input.terminal_height = 40;
        let pane = PromptPane::compose(input);
        assert_eq!(pane.geometry().input_rows, 3, "grows with content");
        assert_eq!(pane.height(), 7, "input + 4 block rows");
        assert_eq!(
            pane.rows().iter().filter(|row| row.kind == PromptPaneRowKind::Editor).count(),
            3
        );

        let twenty_lines = std::iter::repeat_n("line", 20)
            .collect::<Vec<_>>()
            .join("\n");
        let mut tall = composer_input(140, 40, twenty_lines.as_str(), frame(0));
        tall.terminal_width = 140;
        tall.terminal_height = 40;
        let pane = PromptPane::compose(tall);
        assert_eq!(
            pane.geometry().input_rows,
            crate::native_prompt_layout::editor_max_rows(40),
            "140x40 cap is max(6, floor(40/3)) = 13"
        );
        assert_eq!(pane.geometry().input_rows, 13);

        let twelve_lines = std::iter::repeat_n("line", 12)
            .collect::<Vec<_>>()
            .join("\n");
        let mut small = composer_input(80, 24, twelve_lines.as_str(), frame(0));
        small.terminal_width = 80;
        small.terminal_height = 24;
        let pane = PromptPane::compose(small);
        assert_eq!(
            pane.geometry().input_rows,
            crate::native_prompt_layout::editor_max_rows(24),
            "80x24 cap is max(6, floor(24/3)) = 8"
        );
        assert_eq!(pane.geometry().input_rows, 8);
    }

    /// The structural body clamp stays: the block never exceeds the chat body
    /// budget even when the terminal cap is larger
    /// (`prompt_geometry`: `editor_max_rows(terminal).min(height - 4)`).
    #[test]
    fn composer_parity_cap_yields_to_the_body_budget() {
        let twenty_lines = std::iter::repeat_n("line", 20)
            .collect::<Vec<_>>()
            .join("\n");
        let mut input = composer_input(140, 8, twenty_lines.as_str(), frame(0));
        input.terminal_width = 140;
        input.terminal_height = 40;
        let pane = PromptPane::compose(input);
        assert_eq!(pane.geometry().input_rows, 4, "body budget 8 - 4 block rows");
        assert_eq!(pane.height(), 8, "never taller than the body");
    }

    /// The in-box metadata row paints the upstream cells in order — agent
    /// (highlight), `·` (muted), model (base), provider (muted, no dot
    /// between model and provider), `· variant` (warning, bold)
    /// (`metadata.tsx:39-101`, `index.tsx:1856-1869`).
    #[test]
    fn composer_parity_footer_cells_render_agent_model_provider_variant() {
        let mut input = composer_input(140, 40, "", frame(0));
        input.terminal_width = 140;
        input.terminal_height = 40;
        input.metadata = PromptMetadataInput {
            agent: Some("Build"),
            auto: false,
            model: "Non-default fixture",
            provider: "openai",
            variant: Some("balanced"),
            ..PromptMetadataInput::default()
        };
        let pane = PromptPane::compose(input);
        assert_eq!(
            pane.metadata_text(),
            "Build · Non-default fixture openai · balanced",
            "agent · model provider · variant, no dot between model and provider"
        );
        assert_eq!(pane.metadata_row(), Some(3));
        assert!(
            pane.row(3).unwrap().text.contains("Build · Non-default fixture openai · balanced"),
            "the composed metadata row paints the cells: {:?}",
            pane.row(3).unwrap().text
        );
        let runs = pane.metadata_runs();
        assert_eq!(
            runs,
            vec![
                (MetadataRole::Agent, "Build".to_owned()),
                (MetadataRole::ModelSeparator, "·".to_owned()),
                (MetadataRole::Model, "Non-default fixture".to_owned()),
                (MetadataRole::Provider, "openai".to_owned()),
                (MetadataRole::VariantSeparator, "·".to_owned()),
                (MetadataRole::Variant, "balanced".to_owned()),
            ]
        );
        let style = |role| metadata_style(role, false, false);
        assert_eq!(style(MetadataRole::Agent).fg, MetadataFg::Highlight);
        assert_eq!(style(MetadataRole::ModelSeparator).fg, MetadataFg::Muted);
        assert_eq!(style(MetadataRole::Model).fg, MetadataFg::Base);
        assert_eq!(style(MetadataRole::Provider).fg, MetadataFg::Muted);
        let variant = style(MetadataRole::Variant);
        assert_eq!(variant.fg, MetadataFg::Warning);
        assert!(variant.bold, "variant is bold (`metadata.tsx:79`)");
    }

    /// The `auto` cell sits between the agent and the model while the
    /// permission mode is autoaccept (`metadata.tsx:48-50`,
    /// `index.tsx:1859`), and the same cells fit the 80-column budget
    /// (`metadata.tsx:28`: `terminalWidth - 13`).
    #[test]
    fn composer_parity_footer_cells_fit_80_columns_with_auto_cell() {
        let mut input = composer_input(80, 24, "", frame(0));
        input.terminal_width = 80;
        input.terminal_height = 24;
        input.metadata = PromptMetadataInput {
            agent: Some("Build"),
            auto: true,
            model: "Non-default fixture",
            provider: "openai",
            variant: Some("balanced"),
            ..PromptMetadataInput::default()
        };
        let pane = PromptPane::compose(input);
        assert_eq!(
            pane.metadata_text(),
            "Build auto · Non-default fixture openai · balanced"
        );
        let roles: Vec<_> = pane.metadata_runs().iter().map(|(role, _)| *role).collect();
        assert_eq!(
            roles,
            [
                MetadataRole::Agent,
                MetadataRole::Auto,
                MetadataRole::ModelSeparator,
                MetadataRole::Model,
                MetadataRole::Provider,
                MetadataRole::VariantSeparator,
                MetadataRole::Variant,
            ]
        );
    }

    /// Honest empty state (`context/local.tsx:398-417`): with no model
    /// selection the row paints `agent · No provider selected Connect a
    /// provider` at both acceptance widths — never a backend status summary
    /// such as `unset` (`p1-ui-initial.raw` row 16).
    #[test]
    fn composer_parity_unconnected_empty_state_matches_upstream_labels() {
        for width in [80u16, 140u16] {
            let mut input = composer_input(usize::from(width), 24, "", frame(0));
            input.terminal_width = width;
            input.metadata = PromptMetadataInput::unconnected(Some("Build"));
            let pane = PromptPane::compose(input);
            assert_eq!(
                pane.metadata_text(),
                "Build · No provider selected Connect a provider",
                "{width} columns"
            );
            let roles: Vec<_> = pane.metadata_runs().iter().map(|(role, _)| *role).collect();
            assert_eq!(
                roles,
                [
                    MetadataRole::Agent,
                    MetadataRole::ModelSeparator,
                    MetadataRole::Model,
                    MetadataRole::Provider,
                ],
                "{width} columns"
            );
            assert!(
                !pane.metadata_text().contains("unset"),
                "the empty state never leaks the status summary"
            );
        }
        // Without an agent the row is exactly the two empty-state labels
        // (`metadata.tsx:51-71` gates the separator on the agent).
        let bare = PromptMetadataInput::unconnected(None);
        let layout = metadata_layout(&bare, 140);
        assert_eq!(metadata_text(&layout), "No provider selected Connect a provider");
        // Shell mode keeps the shell/agent label and an empty model row
        // (`metadata.tsx:26`), so the empty-state labels never leak there.
        let shell = PromptMetadataInput {
            mode: PromptMode::Shell,
            ..PromptMetadataInput::unconnected(Some("Build"))
        };
        assert_eq!(metadata_text(&metadata_layout(&shell, 140)), "Build");
    }

    /// CP-04: the rail composer inset keys on the TERMINAL width, not the pane
    /// width (`packages/tui/src/component/prompt/index.tsx:1691-1692`,
    /// `dimensions().width < 44 ? 1 : 2`). A narrow pane inside a wide
    /// terminal therefore keeps inset 2; a wide pane inside a narrow terminal
    /// gets inset 1.
    #[test]
    fn cp04_rail_inset_follows_terminal_not_pane() {
        let frame = crate::native_box_style::EffectiveBox {
            border_style: crate::native_box_style::BorderStyle::Single,
            preset: crate::native_box_style::RadiusPreset::Sharp,
            radius: 0,
            thickness: 1,
            padding: 0,
        };
        // Narrow pane (24 cols), wide terminal (52): upstream inset is 2.
        let wide_term = prompt_geometry(24, 20, "x", frame, true, 52, 20);
        assert_eq!(wide_term.style, PromptPaneStyle::Rail);
        assert_eq!(wide_term.text_left, 3, "wide terminal keeps inset 2: {wide_term:?}");
        // Wide pane (80 cols), narrow terminal (43): upstream inset is 1.
        let narrow_term = prompt_geometry(80, 20, "x", frame, true, 43, 20);
        assert_eq!(narrow_term.style, PromptPaneStyle::Rail);
        assert_eq!(narrow_term.text_left, 2, "narrow terminal flips to inset 1: {narrow_term:?}");
        // The flip is strict at 44 (`< 44`).
        assert_eq!(prompt_geometry(80, 20, "x", frame, true, 44, 20).text_left, 3);
    }
}
