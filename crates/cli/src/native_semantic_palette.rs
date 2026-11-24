//! Typed semantic theme-palette view over the V2 resolver.
//!
//! Resolution itself is source-pinned in [`crate::native_theme_runtime`]
//! (upstream `450e7b92412c780a7b5a75226bb8d49aaca24e22`,
//! `packages/theme/src/tui/{resolve,expand,select,schema}.ts`). This module is
//! a thin typed facade so a later painter task can consume resolved semantic
//! tokens by name without re-deriving hue provenance, state precedence or
//! dialog context. It does not reimplement resolution and does not weaken any
//! gate: every value here comes from the production loader.
//!
//! This is the resolver prerequisite, not whole UI acceptance.

// Prerequisite facade consumed by a later painter task; some accessors are not
// yet called from production. Keep them available without warning noise.
#![allow(dead_code)]

use std::collections::HashMap;

use serde_json::Value;

use crate::native_theme_runtime::{
    v2_theme_from_asset, HueColor, HueSource, Palette, PaletteError, SemanticPalette, Slot,
    ThemeMode, V2StatefulColor, V2Theme,
};

/// A fully resolved V2 semantic surface: the ten core slots, the contextual
/// `@dialog` palette, and the typed semantic projection with hue provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticSurface {
    /// Theme name supplied by the caller.
    pub name: String,
    /// Mode actually rendered (the document's first mode when the requested
    /// one is absent).
    pub mode: ThemeMode,
    /// Mode the caller asked for; preserved for a one-mode document.
    pub requested_mode: ThemeMode,
    /// Ten core slots resolved from the document.
    pub core: Palette,
    /// Palette resolved for the contextual `surface("dialog")` view.
    pub dialog: Palette,
    /// Typed semantic projection (provenance, states, categorical order).
    pub semantic: SemanticPalette,
}

impl SemanticSurface {
    /// Resolve a V2 document through the existing production loader.
    ///
    /// # Errors
    ///
    /// [`PaletteError`] when the document shape, a bounded reference/hue step,
    /// or any semantic role is unusable for `mode`. Fails closed; never snaps
    /// an invalid token onto a nearby hue step or infers brightness.
    pub fn from_document(
        name: &str,
        document: &Value,
        mode: ThemeMode,
    ) -> Result<Self, PaletteError> {
        let V2Theme {
            selected_mode,
            requested_mode,
            palette,
            dialog,
            semantic,
        } = v2_theme_from_asset(document, mode)?;
        Ok(Self {
            name: name.to_owned(),
            mode: selected_mode,
            requested_mode,
            core: palette,
            dialog,
            semantic,
        })
    }

    /// Core RGBA for one slot (alpha preserved verbatim, including alpha 0).
    #[must_use]
    pub fn core_slot(&self, slot: Slot) -> [u8; 4] {
        self.core.get(slot)
    }

    /// Contextual dialog RGBA for one slot.
    #[must_use]
    pub fn dialog_slot(&self, slot: Slot) -> [u8; 4] {
        self.dialog.get(slot)
    }

    /// Hue provenance of a resolved token, when the token came from a published
    /// hue scale (aliases carry the alias name, not the target's). Literals and
    /// `transparent` are untracked.
    #[must_use]
    pub fn provenance<'a>(&self, color: &'a HueColor) -> Option<&'a HueSource> {
        color.provenance.as_ref()
    }

    /// The stateful primary-action color, with all five state variants and the
    /// base already resolved and precedence rebuilt.
    #[must_use]
    pub fn primary_action(&self) -> &V2StatefulColor {
        &self.semantic.background_action_primary
    }

    /// First true state in upstream `ActionState` order, or `"base"`.
    #[must_use]
    pub fn active_state<'a>(&self, action: &'a V2StatefulColor, states: &HashMap<String, bool>) -> &'static str {
        action.active_state(states)
    }

    /// Color selected by the disabled→pressed→focused→selected→hovered state
    /// priority, falling back to the variant base.
    #[must_use]
    pub fn action_color<'a>(&self, action: &'a V2StatefulColor, states: &HashMap<String, bool>) -> &'a HueColor {
        action.state(states)
    }

    /// Shift a tracked color along its own hue scale (truncating finite
    /// amounts, non-finite as zero, index clamped to 100..900). Untracked
    /// literals are returned unchanged.
    #[must_use]
    pub fn increase(&self, color: &HueColor, amount: f64) -> HueColor {
        self.semantic.increase(color, amount)
    }

    /// Reverse-direction [`Self::increase`].
    #[must_use]
    pub fn decrease(&self, color: &HueColor, amount: f64) -> HueColor {
        self.semantic.decrease(color, amount)
    }

    /// A published hue scale (nine steps, ascending), or `None` when the theme
    /// does not publish that scale.
    #[must_use]
    pub fn scale(&self, hue: &str) -> Option<&[HueColor]> {
        self.semantic.scale(hue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Value {
        serde_json::from_str(include_str!("native_theme_assets/opencode-v2.json"))
            .expect("bundled V2 asset parses")
    }

    fn hex(token: &str) -> [u8; 4] {
        crate::native_theme_runtime::parse_color(token).expect("valid hex")
    }

    fn states_on(keys: &[&str]) -> HashMap<String, bool> {
        keys.iter().map(|key| ((*key).to_owned(), true)).collect()
    }

    #[test]
    fn native_semantic_palette_resolves_both_modes_through_the_loader() {
        let document = shipped();
        let dark = SemanticSurface::from_document("opencode-v2", &document, ThemeMode::Dark)
            .expect("dark resolves through the typed facade");
        assert_eq!(dark.mode, ThemeMode::Dark);
        assert_eq!(dark.core_slot(Slot::Background), hex("#0a0a0a"));
        assert_eq!(dark.core_slot(Slot::BackgroundPanel), hex("#141414"));
        assert_eq!(dark.core_slot(Slot::Primary), hex("#fab283"));
        assert_eq!(dark.core_slot(Slot::Error), hex("#e06c75"));
        assert_eq!(dark.dialog_slot(Slot::Background), hex("#141414"));
        assert_ne!(dark.dialog_slot(Slot::Background), dark.core_slot(Slot::Background));

        let light = SemanticSurface::from_document("opencode-v2", &document, ThemeMode::Light)
            .expect("light resolves through the typed facade");
        assert_eq!(light.mode, ThemeMode::Light);
        assert_eq!(light.core_slot(Slot::Background), hex("#ffffff"));
        assert_eq!(light.core_slot(Slot::Primary), hex("#3b7dd8"));
        assert_eq!(light.dialog_slot(Slot::Background), hex("#fafafa"));
    }

    #[test]
    fn native_semantic_palette_preserves_alias_provenance_at_equal_rgb() {
        let surface = SemanticSurface::from_document("opencode-v2", &shipped(), ThemeMode::Dark)
            .expect("dark resolves");
        let neutral = surface.scale("neutral").expect("neutral scale")[6].clone();
        let gray = surface.scale("gray").expect("gray scale")[6].clone();
        assert_eq!(neutral.rgba, gray.rgba, "alias copies are value-equal");
        assert_eq!(
            surface.provenance(&neutral),
            Some(&HueSource { hue: "neutral".into(), step: 700 })
        );
        assert_eq!(
            surface.provenance(&gray),
            Some(&HueSource { hue: "gray".into(), step: 700 })
        );
    }

    #[test]
    fn native_semantic_palette_equal_rgb_at_different_steps_is_distinct() {
        // Light neutral 800 and 900 are both #ffffff. The two published
        // swatches are value-equal but carry distinct {hue,step} provenance:
        // the resolver indexes swatch identity, it never reverse-searches RGB
        // (audit T03/T05). A painter shifting one must not snap onto the other.
        let surface = SemanticSurface::from_document("opencode-v2", &shipped(), ThemeMode::Light)
            .expect("light resolves");
        let scale = surface.scale("neutral").expect("neutral scale");
        let at_800 = &scale[7];
        let at_900 = &scale[8];
        assert_eq!(at_800.rgba, hex("#ffffff"));
        assert_eq!(at_900.rgba, hex("#ffffff"));
        assert_ne!(at_800.provenance, at_900.provenance);
        assert_eq!(surface.provenance(at_800).map(|p| p.step), Some(800));
        assert_eq!(surface.provenance(at_900).map(|p| p.step), Some(900));
        // Shifting is relative to each swatch's own step: from 800 one step up
        // lands on 900, while from 900 one step down lands back on 800. A
        // reverse-RGB lookup could not distinguish these two swatches at all.
        let raised_800 = surface.increase(at_800, 1.0);
        let lowered_900 = surface.decrease(at_900, 1.0);
        assert_ne!(raised_800.provenance, lowered_900.provenance);
        assert_eq!(raised_800.provenance.map(|p| p.step), Some(900));
        assert_eq!(lowered_900.provenance.map(|p| p.step), Some(800));
    }

    #[test]
    fn native_semantic_palette_state_priority_and_transparent_alpha() {
        let surface = SemanticSurface::from_document("opencode-v2", &shipped(), ThemeMode::Dark)
            .expect("dark resolves");
        let action = surface.primary_action();
        // Higher-priority state wins when several are true simultaneously.
        assert_eq!(
            surface.active_state(action, &states_on(&["disabled", "focused"])),
            "disabled"
        );
        // Background primary publishes no explicit $disabled, so expansion
        // fills it from its base (transparent): the higher-priority disabled
        // state must preserve alpha 0 rather than borrow text.action.primary's
        // neutral-400 disabled color.
        assert_eq!(
            surface.action_color(action, &states_on(&["disabled", "focused"])).rgba,
            [0, 0, 0, 0]
        );
        assert_eq!(
            surface.action_color(action, &states_on(&["focused", "selected"])).rgba,
            hex("#fab283")
        );
        assert_eq!(
            surface.action_color(action, &states_on(&["hovered"])).rgba,
            hex("#141414")
        );
        // Base and selected are transparent: alpha 0 is preserved, not painted opaque.
        assert_eq!(surface.action_color(action, &states_on(&[])).rgba, [0, 0, 0, 0]);
        assert_eq!(surface.action_color(action, &states_on(&["selected"])).rgba, [0, 0, 0, 0]);
    }

    #[test]
    fn native_semantic_palette_shift_clamps_and_leaves_literals_untracked() {
        let surface = SemanticSurface::from_document("opencode-v2", &shipped(), ThemeMode::Dark)
            .expect("dark resolves");
        let neutral_700 = surface.scale("neutral").expect("neutral scale")[6].clone();
        // Finite amounts truncate toward zero. increase(2.9) moves two steps
        // from neutral 700 to neutral 900 (#030303), not one step to 800; a
        // non-truncated or heuristic reading would land on #0a0a0a.
        let raised = surface.increase(&neutral_700, 2.9);
        assert_eq!(raised.rgba, hex("#030303"));
        assert_eq!(surface.provenance(&raised).map(|p| p.step), Some(900));
        // Negative amounts reverse direction: decrease(1.0) moves one step
        // back from neutral 700 to neutral 600 (#1e1e1e), not to 300.
        let lowered = surface.decrease(&neutral_700, 1.0);
        assert_eq!(lowered.rgba, hex("#1e1e1e"));
        assert_eq!(surface.provenance(&lowered).map(|p| p.step), Some(600));
        // clamping at both ends keeps the scale's own provenance.
        let low = surface.increase(&neutral_700, -999.0);
        assert_eq!(low.rgba, hex("#ffffff"));
        assert_eq!(surface.provenance(&low).map(|p| p.step), Some(100));
        let high = surface.increase(&neutral_700, f64::INFINITY);
        assert_eq!(surface.provenance(&high).map(|p| p.step), Some(700));
        // A plain literal keeps its RGBA and stays untracked under any shift.
        let literal = HueColor { rgba: hex("#123456"), provenance: None };
        assert_eq!(surface.increase(&literal, 3.0), literal);
        assert_eq!(surface.decrease(&literal, 3.0), literal);
    }

    #[test]
    fn native_semantic_palette_rejects_cyclic_references_fail_closed() {
        let mut document = shipped();
        document["base"]["syntax"]["keyword"] = Value::String("$syntax.function".into());
        document["base"]["syntax"]["function"] = Value::String("$syntax.keyword".into());
        assert!(
            SemanticSurface::from_document("cyclic", &document, ThemeMode::Dark).is_err(),
            "a published semantic projection with a reference cycle must fail closed"
        );
    }
}
