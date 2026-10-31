#![forbid(unsafe_code)]
//! Progress-state parity for the pinned OpenCode TUI.
//!
//! Sources (pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! - `packages/tui/src/component/spinner-frames.ts` — ten Braille frames.
//! - `packages/tui/src/component/spinner.tsx` — 80 ms interval, animations-off
//!   `⋯` fallback, optional label.
//! - `packages/tui/src/component/spinner-renderable.ts` — elapsed-time frame
//!   stepping only across consecutive render frames, timer stop on destroy.
//! - `packages/tui/src/routes/session/index.tsx` — `AssistantRetry`,
//!   `FreeLimitNotice`, `freeLimitReset`, and the optional turn-token
//!   diagnostics in `AssistantFooter`.
//!
//! Pure state: callers supply frame ids and millisecond timestamps.

/// Frame list from `spinner-frames.ts:1`.
pub const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// `interval={80}` in both spinner call sites.
pub const SPINNER_INTERVAL_MS: u64 = 80;

/// `fallback={<text>⋯</text>}` when `config.animations` is false.
pub const ANIMATIONS_OFF_GLYPH: &str = "⋯";

/// `RETRY_AFTER_MAX = 7 * 24 * 60 * 60 * 1000` from the retry policy.
pub const RETRY_AFTER_MAX_MS: u64 = 7 * 86_400_000;

const DAY_MS: u64 = 86_400_000;

/// One straight-color cell value (RGBA bytes, no float dependency).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

/// Spinner background handling: the renderable defaults to `transparent`, so
/// the parent's background shows through unless an opaque color is configured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Transparent,
    Opaque(Rgba),
}

impl Background {
    /// Compose this spinner background over the parent row background.
    #[must_use]
    pub const fn compose(self, parent: Rgba) -> Rgba {
        match self {
            Self::Transparent => parent,
            Self::Opaque(color) => color,
        }
    }
}

/// Result of `SpinnerRenderable.frames = value`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameListChange {
    /// False when the list was byte-identical and the setter returned early.
    pub accepted: bool,
    /// True when the measured intrinsic frame width changed.
    pub width_changed: bool,
}

/// Spinner clock state mirroring `SpinnerRenderable`.
#[derive(Clone, Debug)]
pub struct Spinner {
    frames: Vec<String>,
    interval_ms: u64,
    autoplay: bool,
    destroyed: bool,
    index: usize,
    elapsed_ms: u64,
    last_frame_id: Option<u64>,
    frame_width: usize,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    #[must_use]
    pub fn new() -> Self {
        let frames = SPINNER_FRAMES.iter().map(|f| (*f).to_owned()).collect();
        Self {
            frames,
            interval_ms: SPINNER_INTERVAL_MS,
            autoplay: true,
            destroyed: false,
            index: 0,
            elapsed_ms: 0,
            last_frame_id: None,
            frame_width: 1,
        }
    }

    pub fn with_frames(frames: Vec<String>, interval_ms: u64, autoplay: bool) -> Self {
        let frame_width = frames.iter().map(|f| text_width(f)).max().unwrap_or(0);
        Self {
            frames,
            interval_ms: interval_ms.max(1),
            autoplay,
            destroyed: false,
            index: 0,
            elapsed_ms: 0,
            last_frame_id: None,
            frame_width,
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.frames.get(self.index).map(String::as_str)
    }

    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn frame_width(&self) -> usize {
        self.frame_width
    }

    #[must_use]
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Replace the frame list. An identical list is ignored; otherwise the
    /// index resets to zero and the clock restarts (upstream `frames` setter).
    pub fn set_frames(&mut self, frames: Vec<String>) -> FrameListChange {
        if frames.len() == self.frames.len()
            && frames.iter().zip(&self.frames).all(|(new, old)| new == old)
        {
            return FrameListChange {
                accepted: false,
                width_changed: false,
            };
        }
        self.frames = frames;
        self.index = 0;
        let width = self.frames.iter().map(|f| text_width(f)).max().unwrap_or(0);
        let width_changed = width != self.frame_width;
        self.frame_width = width;
        self.restart();
        FrameListChange {
            accepted: true,
            width_changed,
        }
    }

    /// Interval changes restart the elapsed clock but keep the current index.
    pub fn set_interval(&mut self, interval_ms: u64) -> bool {
        let interval_ms = interval_ms.max(1);
        if interval_ms == self.interval_ms {
            return false;
        }
        self.interval_ms = interval_ms;
        self.restart();
        true
    }

    /// Autoplay changes restart the timer; the current frame is preserved.
    pub fn set_autoplay(&mut self, autoplay: bool) -> bool {
        if autoplay == self.autoplay {
            return false;
        }
        self.autoplay = autoplay;
        self.restart();
        true
    }

    /// Advance for one render frame.
    ///
    /// `frame_id` must be the renderer frame counter. Delta time is counted
    /// only across consecutive frames, so an idle gap before the spinner
    /// appeared or resumed is ignored. Returns true when the painted frame
    /// changed.
    pub fn advance(&mut self, frame_id: u64, delta_ms: u64) -> bool {
        if self.destroyed || !self.autoplay || self.frames.len() < 2 {
            return false;
        }
        if frame_id.checked_sub(1) == self.last_frame_id {
            self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        }
        self.last_frame_id = Some(frame_id);
        let interval = self.interval_ms.max(1);
        let steps = self.elapsed_ms / interval;
        if steps > 0 {
            let len = self.frames.len() as u64;
            let advance = usize::try_from(steps % len).unwrap_or(0);
            self.index = (self.index + advance) % self.frames.len();
            self.elapsed_ms -= steps * interval;
        }
        steps > 0
    }

    /// Renderer-core skips `onUpdate` for hidden or culled nodes, so a hidden
    /// frame neither accrues time nor counts as a consecutive predecessor.
    pub fn advance_visible(&mut self, visible: bool, frame_id: u64, delta_ms: u64) -> bool {
        if !visible {
            return false;
        }
        self.advance(frame_id, delta_ms)
    }

    /// Delay until the next repaint request. Mirrors
    /// `setTimeout(interval - elapsed)`, and is `None` when the timer is
    /// stopped (destroyed, autoplay off, or fewer than two frames).
    #[must_use]
    pub fn repaint_delay_ms(&self) -> Option<u64> {
        if self.destroyed || !self.autoplay || self.frames.len() < 2 {
            return None;
        }
        Some(self.interval_ms.max(1).saturating_sub(self.elapsed_ms))
    }

    /// Upstream `destroySelf()` stops the timer permanently.
    pub fn destroy(&mut self) {
        self.destroyed = true;
    }

    /// Text paint shared by `Spinner` and the compaction/running rows.
    /// With animations off the upstream fallback is `⋯`/`⋯ {children}`.
    #[must_use]
    pub fn render(&self, animations: bool, label: Option<&str>) -> String {
        let label = label.filter(|label| !label.is_empty());
        if !animations {
            return match label {
                Some(label) => format!("{ANIMATIONS_OFF_GLYPH} {label}"),
                None => ANIMATIONS_OFF_GLYPH.to_owned(),
            };
        }
        let glyph = self.current().unwrap_or(ANIMATIONS_OFF_GLYPH);
        match label {
            Some(label) => format!("{glyph} {label}"),
            None => glyph.to_owned(),
        }
    }

    fn restart(&mut self) {
        self.elapsed_ms = 0;
        self.last_frame_id = None;
    }
}

/// Width of a frame in terminal cells. Braille and the configured labels are
/// single-width; zero-width joining/combining scalars are not counted.
fn text_width(text: &str) -> usize {
    text.chars()
        .filter(|c| !c.is_control() && !is_zero_width(*c))
        .count()
}

fn is_zero_width(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}')
}

/// `SessionMessageAssistant["retry"]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryState {
    /// Absolute deadline in Unix milliseconds.
    pub at_ms: u64,
    pub attempt: u64,
    pub message: String,
}

/// `Math.max(0, Math.ceil((at - Date.now()) / 1_000))`.
#[must_use]
pub fn retry_seconds(at_ms: u64, now_ms: u64) -> u64 {
    at_ms.saturating_sub(now_ms).div_ceil(1_000)
}

/// Exact `AssistantRetry` line, including the zero/past deadline form.
#[must_use]
pub fn retry_label(retry: &RetryState, now_ms: u64) -> String {
    let seconds = retry_seconds(retry.at_ms, now_ms);
    if seconds > 0 {
        format!(
            "⚠ Retrying in {seconds}s · attempt {} · {}",
            retry.attempt, retry.message
        )
    } else {
        format!(
            "⚠ Retry due · attempt {} · {}",
            retry.attempt, retry.message
        )
    }
}

/// Footer error rendering chosen by `AssistantFooter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorLine {
    /// Interruption or an active retry suppresses the error line.
    None,
    /// `FreeLimitNotice` owns the two-line notice instead of `Error:`.
    FreeLimit,
    Ordinary(String),
}

#[must_use]
pub fn error_line(
    message: &str,
    interrupted: bool,
    has_retry: bool,
    free_limit: bool,
) -> ErrorLine {
    if interrupted || has_retry {
        return ErrorLine::None;
    }
    if free_limit {
        return ErrorLine::FreeLimit;
    }
    ErrorLine::Ordinary(format!("Error: {message}"))
}

/// Footer suffix when `error.message === "Step interrupted"`.
#[must_use]
pub const fn interruption_suffix(interrupted: bool) -> &'static str {
    if interrupted {
        " · interrupted"
    } else {
        ""
    }
}

/// `FreeLimitNotice` replaces the ordinary error line for
/// `provider.free-limit`.
#[must_use]
pub fn free_limit_notice(reset: Option<&str>) -> [String; 2] {
    let first = match reset {
        Some(reset) => format!("You've used today's free messages. They reset at {reset}."),
        None => "You've used today's free messages.".to_owned(),
    };
    [
        first,
        "Upgrade to OpenCode Go → /connect · Switch model → /models".to_owned(),
    ]
}

/// `Locale.time` vs `Locale.datetime` selection in `freeLimitReset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetFormat {
    Time,
    DateTime,
}

/// Validate `retryAfterMs` + base timestamp and choose the reset format.
///
/// Upstream compares local calendar days; this pure model uses UTC days
/// because the crate carries no timezone dependency. Invalid/absent delays
/// (negative, greater than 7 days, or overflowing) yield `None`.
#[must_use]
pub fn free_limit_reset(delay_ms: Option<u64>, base_ms: u64, now_ms: u64) -> Option<ResetFormat> {
    let delay = delay_ms?;
    if delay > RETRY_AFTER_MAX_MS {
        return None;
    }
    let at = base_ms.checked_add(delay)?;
    if utc_day(at) == utc_day(now_ms) && at.saturating_sub(now_ms) < DAY_MS {
        Some(ResetFormat::Time)
    } else {
        Some(ResetFormat::DateTime)
    }
}

/// Format a reset instant as `HH:MM` (UTC).
#[must_use]
pub fn format_reset_time(ms: u64) -> String {
    let (_, _, _, hour, minute, _) = utc_parts(ms);
    format!("{hour:02}:{minute:02}")
}

/// Format a reset instant as `HH:MM · YYYY-MM-DD` (UTC), matching
/// `Locale.datetime`'s `time · date` shape.
#[must_use]
pub fn format_reset_datetime(ms: u64) -> String {
    let (year, month, day, hour, minute, _) = utc_parts(ms);
    format!("{hour:02}:{minute:02} · {year:04}-{month:02}-{day:02}")
}

/// Format using the selected `ResetFormat`.
#[must_use]
pub fn format_reset(format: ResetFormat, at_ms: u64) -> String {
    match format {
        ResetFormat::Time => format_reset_time(at_ms),
        ResetFormat::DateTime => format_reset_datetime(at_ms),
    }
}

fn utc_day(ms: u64) -> u64 {
    ms / DAY_MS
}

fn utc_parts(ms: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = i64::try_from(ms / DAY_MS).unwrap_or(i64::MAX);
    let rem = ms % DAY_MS;
    let (year, month, day) = civil_from_days(days);
    let hour = u32::try_from(rem / 3_600_000).unwrap_or(0);
    let minute = u32::try_from((rem / 60_000) % 60).unwrap_or(0);
    let second = u32::try_from((rem / 1_000) % 60).unwrap_or(0);
    (year, month, day, hour, minute, second)
}

/// Howard Hinnant's `civil_from_days` for a Unix day number.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// `Locale.number`: `1.0K`/`1.0M` at 1,000/999,950.
#[must_use]
pub fn locale_number(value: u64) -> String {
    if value >= 999_950 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

/// Usage counters carried by one assistant step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenUsage {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.reasoning)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    #[must_use]
    pub fn new_tokens(&self) -> u64 {
        self.total().saturating_sub(self.cache_read)
    }
}

/// One assistant step for the optional `debug.turn_tokens` diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenStep {
    pub usage: TokenUsage,
    pub provider: String,
    pub model: String,
}

/// Collapsed `Tokens:` summary from `AssistantFooter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenSummary {
    pub steps: usize,
    pub latest_new: u64,
    pub latest_cached: u64,
    pub latest_total: u64,
    pub reuse_drops: usize,
}

/// Build the optional turn-token summary. `enabled` is
/// `Boolean(config.data.debug?.turn_tokens)`; steps with zero usage are
/// skipped, as upstream `flatMap` does before updating `previousCache`.
#[must_use]
pub fn token_diagnostics(steps: &[TokenStep], enabled: bool) -> Option<TokenSummary> {
    if !enabled {
        return None;
    }
    let mut count = 0usize;
    let mut latest_new = 0u64;
    let mut latest_cached = 0u64;
    let mut latest_total = 0u64;
    let mut reuse_drops = 0usize;
    let mut previous: Option<(String, String, u64)> = None;
    for step in steps {
        let total = step.usage.total();
        if total == 0 {
            continue;
        }
        count += 1;
        latest_new = step.usage.new_tokens();
        latest_cached = step.usage.cache_read;
        latest_total = total;
        if let Some((provider, model, read)) = &previous {
            if provider == &step.provider && model == &step.model {
                let drop = read.saturating_sub(step.usage.cache_read);
                let exempt = step.provider == "openai" && (1_024..=2_048).contains(&drop);
                if drop > 0 && !exempt {
                    reuse_drops += 1;
                }
            }
        }
        previous = Some((
            step.provider.clone(),
            step.model.clone(),
            step.usage.cache_read,
        ));
    }
    if count == 0 {
        return None;
    }
    Some(TokenSummary {
        steps: count,
        latest_new,
        latest_cached,
        latest_total,
        reuse_drops,
    })
}

/// Collapsed diagnostics line. `expanded` selects `- ` (open) vs `+ ` (closed).
#[must_use]
pub fn token_diagnostics_label(summary: &TokenSummary, expanded: bool) -> String {
    let mut label = format!(
        "{} Tokens: {} {} · latest: {} new · {} cached · {} total",
        if expanded { "-" } else { "+" },
        summary.steps,
        if summary.steps == 1 { "step" } else { "steps" },
        group_thousands(summary.latest_new),
        group_thousands(summary.latest_cached),
        group_thousands(summary.latest_total),
    );
    if summary.reuse_drops > 0 {
        label.push_str(&format!(
            " · ! {} likely cache {}",
            summary.reuse_drops,
            if summary.reuse_drops == 1 {
                "bust"
            } else {
                "busts"
            }
        ));
    }
    label
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retry() -> RetryState {
        RetryState {
            at_ms: 10_000,
            attempt: 3,
            message: "boom".into(),
        }
    }

    #[test]
    fn t01_spinner_clock_0_79_80_160ms() {
        let mut spinner = Spinner::new();
        assert_eq!(spinner.current(), Some("⠋"));
        assert!(!spinner.advance(0, 0));
        assert_eq!(spinner.index(), 0);
        assert_eq!(spinner.repaint_delay_ms(), Some(80));
        assert!(!spinner.advance(1, 79));
        assert_eq!(spinner.index(), 0);
        assert_eq!(spinner.repaint_delay_ms(), Some(1));
        assert!(spinner.advance(2, 1));
        assert_eq!(spinner.current(), Some("⠙"));
        assert_eq!(spinner.repaint_delay_ms(), Some(80));
        assert!(spinner.advance(3, 160));
        assert_eq!(spinner.current(), Some("⠸"));
    }

    #[test]
    fn t02_spinner_ignores_non_consecutive_render_gaps() {
        let mut spinner = Spinner::new();
        // First appearance after an idle gap: not consecutive, delta ignored.
        assert!(!spinner.advance(5, 5_000));
        assert_eq!(spinner.repaint_delay_ms(), Some(80));
        assert!(!spinner.advance(6, 40));
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
        // A non-consecutive render id ignores the whole delta.
        assert!(!spinner.advance(9, 5_000));
        assert_eq!(spinner.index(), 0);
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
        assert!(spinner.advance(10, 40));
        assert_eq!(spinner.index(), 1);
    }

    #[test]
    fn t02b_first_frame_zero_is_consecutive_with_updated_frame_negative_one() {
        // Upstream starts `updatedFrame = -1`; frame id 0 satisfies
        // `frameId - 1 === -1`, so that first delta is counted.
        let mut spinner = Spinner::new();
        spinner.advance(0, 40);
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
        assert!(spinner.advance(1, 40));
        assert_eq!(spinner.index(), 1);
    }

    #[test]
    fn t03_visibility_culling_pauses_the_clock() {
        let mut spinner = Spinner::new();
        spinner.advance(0, 0);
        assert!(!spinner.advance_visible(false, 1, 10_000));
        assert_eq!(spinner.repaint_delay_ms(), Some(80));
        // The first visible frame after culling is not consecutive, so the
        // hidden interval is ignored; the next frame accrues normally.
        assert!(!spinner.advance_visible(true, 2, 40));
        assert_eq!(spinner.repaint_delay_ms(), Some(80));
        assert!(!spinner.advance_visible(true, 3, 40));
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
        assert!(spinner.advance_visible(true, 4, 40));
        assert_eq!(spinner.index(), 1);
    }

    #[test]
    fn t04_animations_off_renders_static_glyph() {
        let spinner = Spinner::new();
        assert_eq!(spinner.render(false, None), "⋯");
        assert_eq!(spinner.render(false, Some("Compacting")), "⋯ Compacting");
        assert_eq!(spinner.render(false, Some("")), "⋯");
        assert_eq!(spinner.render(true, Some("Compacting")), "⠋ Compacting");
        assert_eq!(spinner.render(true, None), "⠋");
    }

    #[test]
    fn t05_frame_list_change_resets_index_and_reports_width() {
        let mut spinner = Spinner::new();
        spinner.advance(0, 80);
        spinner.advance(1, 80);
        assert_eq!(spinner.index(), 2);
        let change = spinner.set_frames(vec!["a".into(), "bb".into()]);
        assert!(change.accepted);
        assert!(change.width_changed);
        assert_eq!(spinner.index(), 0);
        assert_eq!(spinner.current(), Some("a"));
        assert_eq!(spinner.frame_width(), 2);
        let again = spinner.set_frames(vec!["a".into(), "bb".into()]);
        assert!(!again.accepted);
        assert!(!again.width_changed);
    }

    #[test]
    fn t06_interval_and_autoplay_restart_but_keep_index() {
        let mut spinner = Spinner::new();
        spinner.advance(0, 80);
        assert_eq!(spinner.index(), 1);
        assert!(spinner.set_interval(40));
        assert_eq!(spinner.index(), 1);
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
        assert!(!spinner.set_interval(40));
        assert!(spinner.set_autoplay(false));
        assert_eq!(spinner.repaint_delay_ms(), None);
        assert!(!spinner.advance(1, 80));
        assert!(spinner.set_autoplay(true));
        assert_eq!(spinner.repaint_delay_ms(), Some(40));
    }

    #[test]
    fn t07_destroy_stops_the_timer() {
        let mut spinner = Spinner::new();
        spinner.advance(0, 80);
        spinner.destroy();
        assert!(spinner.is_destroyed());
        assert_eq!(spinner.repaint_delay_ms(), None);
        assert!(!spinner.advance(1, 80));
    }

    #[test]
    fn t08_background_composition_follows_parent_until_opaque() {
        let parent = Rgba([10, 20, 30, 255]);
        assert_eq!(Background::Transparent.compose(parent), parent);
        let own = Rgba([1, 2, 3, 4]);
        assert_eq!(Background::Opaque(own).compose(parent), own);
    }

    #[test]
    fn t09_retry_zero_past_future_deadlines() {
        let retry = retry();
        assert_eq!(retry_seconds(10_000, 10_000), 0);
        assert_eq!(retry_seconds(10_000, 10_001), 0);
        assert_eq!(retry_seconds(10_000, 9_000), 1);
        assert_eq!(retry_seconds(10_000, 8_001), 2);
        assert_eq!(
            retry_label(&retry, 10_000),
            "⚠ Retry due · attempt 3 · boom"
        );
        assert_eq!(
            retry_label(&retry, 15_000),
            "⚠ Retry due · attempt 3 · boom"
        );
        assert_eq!(
            retry_label(&retry, 8_001),
            "⚠ Retrying in 2s · attempt 3 · boom"
        );
    }

    #[test]
    fn t10_error_line_suppression_interruption_and_free_limit() {
        assert_eq!(
            error_line("boom", false, false, false),
            ErrorLine::Ordinary("Error: boom".into())
        );
        assert_eq!(
            error_line("Step interrupted", true, false, false),
            ErrorLine::None
        );
        assert_eq!(error_line("boom", false, true, false), ErrorLine::None);
        assert_eq!(error_line("free", false, false, true), ErrorLine::FreeLimit);
        assert_eq!(interruption_suffix(true), " · interrupted");
        assert_eq!(interruption_suffix(false), "");
    }

    #[test]
    fn t11_free_limit_reset_formatting_and_bounds() {
        assert_eq!(
            free_limit_reset(Some(3_600_000), 0, 0),
            Some(ResetFormat::Time)
        );
        assert_eq!(format_reset(ResetFormat::Time, 3_600_000), "01:00");
        assert_eq!(
            free_limit_reset(Some(2 * DAY_MS), 0, 0),
            Some(ResetFormat::DateTime)
        );
        assert_eq!(
            format_reset(ResetFormat::DateTime, 2 * DAY_MS),
            "00:00 · 1970-01-03"
        );
        assert_eq!(
            free_limit_reset(Some(7 * DAY_MS), 0, 0),
            Some(ResetFormat::DateTime)
        );
        assert_eq!(free_limit_reset(Some(7 * DAY_MS + 1), 0, 0), None);
        assert_eq!(free_limit_reset(None, 0, 0), None);
        assert_eq!(
            free_limit_reset(Some(3_600_000), u64::MAX, 0),
            None,
            "overflowing base+delay is ignored"
        );
    }

    #[test]
    fn t12_free_limit_notice_lines() {
        assert_eq!(
            free_limit_notice(None)[0],
            "You've used today's free messages."
        );
        assert_eq!(
            free_limit_notice(Some("01:00"))[0],
            "You've used today's free messages. They reset at 01:00."
        );
        assert_eq!(
            free_limit_notice(None)[1],
            "Upgrade to OpenCode Go → /connect · Switch model → /models"
        );
    }

    #[test]
    fn t13_locale_number_matches_upstream() {
        assert_eq!(locale_number(999), "999");
        assert_eq!(locale_number(1_000), "1.0K");
        assert_eq!(locale_number(1_234), "1.2K");
        assert_eq!(locale_number(999_949), "999.9K");
        assert_eq!(locale_number(999_950), "1.0M");
        assert_eq!(locale_number(1_234_567), "1.2M");
    }

    #[test]
    fn t14_optional_turn_token_diagnostics() {
        let steps = vec![
            TokenStep {
                usage: TokenUsage {
                    input: 1_000,
                    output: 500,
                    reasoning: 0,
                    cache_read: 500,
                    cache_write: 0,
                },
                provider: "anthropic".into(),
                model: "m".into(),
            },
            TokenStep {
                usage: TokenUsage {
                    input: 100,
                    output: 50,
                    reasoning: 0,
                    cache_read: 100,
                    cache_write: 0,
                },
                provider: "anthropic".into(),
                model: "m".into(),
            },
            TokenStep {
                usage: TokenUsage::default(),
                provider: "anthropic".into(),
                model: "m".into(),
            },
        ];
        assert_eq!(token_diagnostics(&steps, false), None);
        let summary = token_diagnostics(&steps, true).unwrap();
        assert_eq!(summary.steps, 2);
        assert_eq!(summary.latest_new, 150);
        assert_eq!(summary.latest_cached, 100);
        assert_eq!(summary.latest_total, 250);
        assert_eq!(summary.reuse_drops, 1);
        assert_eq!(
            token_diagnostics_label(&summary, false),
            "+ Tokens: 2 steps · latest: 150 new · 100 cached · 250 total · ! 1 likely cache bust"
        );
        assert!(token_diagnostics_label(&summary, true).starts_with("- Tokens:"));
        assert_eq!(token_diagnostics(&[], true), None);
    }
}
