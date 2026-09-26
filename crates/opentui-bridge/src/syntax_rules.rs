//! Full multi-scope syntax rules (mirrors `packages/tui/src/theme/index.ts` @ a0d9b6c).
//!
//! SOURCE EVIDENCE (TS checkout at /home/rashid/projects/opencode, commit a0d9b6c):
//! - `generateSyntax` (index.ts:556-558): `SyntaxStyle.fromTheme(getSyntaxRules(theme))`.
//! - `generateSubtleSyntax` (index.ts:560-584): same rules, fg re-alphaed to
//!   `thinkingOpacity * 255`, per-scope overrides merged (index.ts:564):
//!   `rule.scope.reduce((acc, scope) => ({ ...acc, ...overrides?.[scope] }), {})`.
//! - `SyntaxStyleOverrides` (index.ts:93): `Record<string, { italic?: boolean }>`.
//! - `getSyntaxRules` (index.ts:586-1088): 76 `{ scope[], style{fg,bg,bold,italic,underline} }`
//!   entries; color binding per rule below cites exact TS line ranges.
//! - `selectedForeground` (index.ts:95-111): explicit `selectedListItemText` when set,
//!   else black/white by luminance for transparent bg, else `background`.
//! - Token names reused via `crate::syntax_style::TokenKind` (NOT redefined here);
//!   single-scope token/attr layer lives in `crate::syntax_style`.
//! ponytail: resolved-per-theme Vec, no TextMate engine. Upgrade when scope-selector matching accepted.

#![forbid(unsafe_code)]

use crate::color::Rgba;
use crate::syntax_style::TokenKind;
use crate::theme::Theme;

/// Scope count bound per rule (TS max is 5 scopes, index.ts:748-752; 8 bounds growth).
pub const MAX_SCOPES: usize = 8;
/// Rule count in TS `getSyntaxRules` (index.ts:586-1088).
pub const RULE_COUNT: usize = 76;

/// Full rule: verbatim TS `{ scope, style }` entry with concrete colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullRule {
    pub scopes: Vec<String>,
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl FullRule {
    /// Fail-closed: `None` when `scopes` empty or over `MAX_SCOPES`.
    #[must_use]
    pub fn new(
        scopes: &[&str],
        fg: Option<Rgba>,
        bg: Option<Rgba>,
        bold: bool,
        italic: bool,
        underline: bool,
    ) -> Option<Self> {
        if scopes.is_empty() || scopes.len() > MAX_SCOPES {
            return None;
        }
        Some(Self {
            scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
            fg,
            bg,
            bold,
            italic,
            underline,
        })
    }

    /// Member scopes parsed as `TokenKind` (unparsable scopes skipped).
    #[must_use]
    pub fn kinds(&self) -> Vec<TokenKind> {
        self.scopes.iter().filter_map(|s| TokenKind::from_str(s)).collect()
    }
}

/// Mirror TS `selectedForeground` (index.ts:95-111).
#[must_use]
pub fn selected_foreground(theme: &Theme, bg: Rgba) -> Rgba {
    if theme.has_selected_list_item_text {
        return theme.selected_list_item_text;
    }
    if theme.background.a == 0 {
        let lum = 0.299 * f32::from(bg.r) + 0.587 * f32::from(bg.g) + 0.114 * f32::from(bg.b);
        return if lum > 0.5 * 255.0 { Rgba::rgb(0, 0, 0) } else { Rgba::rgb(255, 255, 255) };
    }
    theme.background
}

macro_rules! rule {
    ($out:expr, $t:expr, $fg:expr, $bg:expr, $b:expr, $i:expr, $u:expr, $($s:literal),+) => {
        if let Some(r) = FullRule::new(&[$($s),+], $fg, $bg, $b, $i, $u) {
            $out.push(r);
        }
    };
}

/// All 76 TS `getSyntaxRules` entries with theme colors bound verbatim.
#[must_use]
pub fn rules_for(theme: &Theme) -> Vec<FullRule> {
    let mut out: Vec<FullRule> = Vec::with_capacity(RULE_COUNT);
    let t = theme;
    let n: Option<Rgba> = None;
    // index.ts:588-606 (default, prompt, extmark.file, extmark.agent)
    rule!(out, t, Some(t.text), n, false, false, false, "default");
    rule!(out, t, Some(t.accent), n, false, false, false, "prompt");
    rule!(out, t, Some(t.warning), n, true, false, false, "extmark.file");
    rule!(out, t, Some(t.secondary), n, true, false, false, "extmark.agent");
    // index.ts:614-621 (extmark.paste: selected fg on warning bg, bold)
    rule!(out, t, Some(selected_foreground(t, t.warning)), Some(t.warning), true, false, false, "extmark.paste");
    // index.ts:622-653 (comment x2, string/symbol, number/boolean, character.special)
    rule!(out, t, Some(t.syntax_comment), n, false, true, false, "comment");
    rule!(out, t, Some(t.syntax_comment), n, false, true, false, "comment.documentation");
    rule!(out, t, Some(t.syntax_string), n, false, false, false, "string", "symbol");
    rule!(out, t, Some(t.syntax_number), n, false, false, false, "number", "boolean");
    rule!(out, t, Some(t.syntax_string), n, false, false, false, "character.special");
    // index.ts:654-687 (keyword family)
    rule!(out, t, Some(t.syntax_keyword), n, false, true, false, "keyword.return", "keyword.conditional", "keyword.repeat", "keyword.coroutine");
    rule!(out, t, Some(t.syntax_type), n, true, true, false, "keyword.type");
    rule!(out, t, Some(t.syntax_function), n, false, false, false, "keyword.function", "function.method");
    rule!(out, t, Some(t.syntax_keyword), n, false, true, false, "keyword");
    rule!(out, t, Some(t.syntax_keyword), n, false, false, false, "keyword.import");
    // index.ts:688-705 (operators, ternary, variables)
    rule!(out, t, Some(t.syntax_operator), n, false, false, false, "operator", "keyword.operator", "punctuation.delimiter");
    rule!(out, t, Some(t.syntax_operator), n, false, false, false, "keyword.conditional.ternary");
    rule!(out, t, Some(t.syntax_variable), n, false, false, false, "variable", "variable.parameter", "function.method.call", "function.call");
    rule!(out, t, Some(t.syntax_function), n, false, false, false, "variable.member", "function", "constructor");
    // index.ts:706-747 (type/module, constant, property, class, parameter, punctuation)
    rule!(out, t, Some(t.syntax_type), n, false, false, false, "type", "module");
    rule!(out, t, Some(t.syntax_number), n, false, false, false, "constant");
    rule!(out, t, Some(t.syntax_variable), n, false, false, false, "property");
    rule!(out, t, Some(t.syntax_type), n, false, false, false, "class");
    rule!(out, t, Some(t.syntax_variable), n, false, false, false, "parameter");
    rule!(out, t, Some(t.syntax_punctuation), n, false, false, false, "punctuation", "punctuation.bracket");
    // index.ts:748-778 (builtins x5, super, escapes, directive, punct.special)
    rule!(out, t, Some(t.error), n, false, false, false, "variable.builtin", "type.builtin", "function.builtin", "module.builtin", "constant.builtin");
    rule!(out, t, Some(t.error), n, false, false, false, "variable.super");
    rule!(out, t, Some(t.syntax_keyword), n, false, false, false, "string.escape", "string.regexp");
    rule!(out, t, Some(t.syntax_keyword), n, false, true, false, "keyword.directive");
    rule!(out, t, Some(t.syntax_operator), n, false, false, false, "punctuation.special");
    rule!(out, t, Some(t.syntax_keyword), n, false, true, false, "keyword.modifier");
    rule!(out, t, Some(t.syntax_keyword), n, false, true, false, "keyword.exception");
    // index.ts:793-849 (markdown headings, bold/strong)
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading");
    rule!(out, t, Some(t.markdown_heading), n, true, false, true, "markup.heading.1");
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading.2");
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading.3");
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading.4");
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading.5");
    rule!(out, t, Some(t.markdown_heading), n, true, false, false, "markup.heading.6");
    rule!(out, t, Some(t.markdown_strong), n, true, false, false, "markup.bold", "markup.strong");
    // index.ts:851-896 (italic, list, quote, raw, links)
    rule!(out, t, Some(t.markdown_emph), n, false, true, false, "markup.italic");
    rule!(out, t, Some(t.markdown_list_item), n, false, false, false, "markup.list");
    rule!(out, t, Some(t.markdown_block_quote), n, false, true, false, "markup.quote");
    rule!(out, t, Some(t.markdown_code), n, false, false, false, "markup.raw", "markup.raw.block");
    rule!(out, t, Some(t.markdown_code), Some(t.background), false, false, false, "markup.raw.inline");
    rule!(out, t, Some(t.markdown_link), n, false, false, true, "markup.link");
    rule!(out, t, Some(t.markdown_link_text), n, false, false, true, "markup.link.label");
    rule!(out, t, Some(t.markdown_link), n, false, false, true, "markup.link.url");
    // index.ts:905-936 (label, spell, conceal, string.special, character, float)
    rule!(out, t, Some(t.markdown_link_text), n, false, false, false, "label");
    rule!(out, t, Some(t.text), n, false, false, false, "spell", "nospell");
    rule!(out, t, Some(t.text_muted), n, false, false, false, "conceal");
    rule!(out, t, Some(t.markdown_link), n, false, false, true, "string.special", "string.special.url");
    rule!(out, t, Some(t.syntax_string), n, false, false, false, "character");
    rule!(out, t, Some(t.syntax_number), n, false, false, false, "float");
    // index.ts:943-997 (diagnostic comments, namespace, field, type.definition, export, attrs)
    rule!(out, t, Some(t.error), n, true, true, false, "comment.error");
    rule!(out, t, Some(t.warning), n, true, true, false, "comment.warning");
    rule!(out, t, Some(t.info), n, true, true, false, "comment.todo", "comment.note");
    rule!(out, t, Some(t.syntax_type), n, false, false, false, "namespace");
    rule!(out, t, Some(t.syntax_variable), n, false, false, false, "field");
    rule!(out, t, Some(t.syntax_type), n, true, false, false, "type.definition");
    rule!(out, t, Some(t.syntax_keyword), n, false, false, false, "keyword.export");
    rule!(out, t, Some(t.warning), n, false, false, false, "attribute", "annotation");
    // index.ts:998-1040 (tags, strikethrough, underline, checklists)
    rule!(out, t, Some(t.error), n, false, false, false, "tag");
    rule!(out, t, Some(t.syntax_keyword), n, false, false, false, "tag.attribute");
    rule!(out, t, Some(t.syntax_operator), n, false, false, false, "tag.delimiter");
    rule!(out, t, Some(t.text_muted), n, false, false, false, "markup.strikethrough");
    rule!(out, t, Some(t.text), n, false, false, true, "markup.underline");
    rule!(out, t, Some(t.success), n, false, false, false, "markup.list.checked");
    rule!(out, t, Some(t.text_muted), n, false, false, false, "markup.list.unchecked");
    // index.ts:1041-1087 (diff, diagnostics)
    rule!(out, t, Some(t.diff_added), Some(t.diff_added_bg), false, false, false, "diff.plus");
    rule!(out, t, Some(t.diff_removed), Some(t.diff_removed_bg), false, false, false, "diff.minus");
    rule!(out, t, Some(t.diff_context), Some(t.diff_context_bg), false, false, false, "diff.delta");
    rule!(out, t, Some(t.error), n, true, false, false, "error");
    rule!(out, t, Some(t.warning), n, true, false, false, "warning");
    rule!(out, t, Some(t.info), n, false, false, false, "info");
    rule!(out, t, Some(t.text_muted), n, false, false, false, "debug");
    out
}

/// First rule containing `scope`; `None` when unknown.
#[must_use]
pub fn lookup<'a>(rules: &'a [FullRule], scope: &str) -> Option<&'a FullRule> {
    rules.iter().find(|r| r.scopes.iter().any(|s| s == scope))
}

/// Per-scope italic override (TS `SyntaxStyleOverrides`, index.ts:93).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubtleOverride {
    pub italic: Option<bool>,
}

impl SubtleOverride {
    #[must_use]
    pub const fn none() -> Self {
        Self { italic: None }
    }
    #[must_use]
    pub const fn italic(v: bool) -> Self {
        Self { italic: Some(v) }
    }
}

/// Merge overrides per rule (TS index.ts:564: later member scopes win).
pub fn apply_overrides(rules: &mut [FullRule], overrides: &[(&str, SubtleOverride)]) {
    for r in rules.iter_mut() {
        let mut italic: Option<bool> = None;
        for scope in &r.scopes {
            for (name, o) in overrides {
                if scope == name && o.italic.is_some() {
                    italic = o.italic;
                }
            }
        }
        if let Some(v) = italic {
            r.italic = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default_opencode()
    }

    #[test]
    fn count_matches_ts() {
        assert_eq!(rules_for(&theme()).len(), RULE_COUNT);
    }

    #[test]
    fn multi_scope_hit() {
        let rules = rules_for(&theme());
        let s = lookup(&rules, "symbol").unwrap();
        assert!(s.scopes.contains(&"string".to_string()));
        assert_eq!(s.fg, Some(theme().syntax_string));
        assert_eq!(lookup(&rules, "keyword.coroutine").unwrap().italic, true);
    }

    #[test]
    fn bg_carry() {
        let rules = rules_for(&theme());
        let p = lookup(&rules, "extmark.paste").unwrap();
        assert_eq!(p.bg, Some(theme().warning));
        assert_eq!(lookup(&rules, "diff.plus").unwrap().bg, Some(theme().diff_added_bg));
        assert_eq!(
            lookup(&rules, "markup.raw.inline").unwrap().bg,
            Some(theme().background)
        );
    }

    #[test]
    fn attr_carry() {
        let rules = rules_for(&theme());
        let h1 = lookup(&rules, "markup.heading.1").unwrap();
        assert!(h1.bold && h1.underline && !h1.italic);
        let kt = lookup(&rules, "keyword.type").unwrap();
        assert!(kt.bold && kt.italic && !kt.underline);
        assert!(lookup(&rules, "markup.link").unwrap().underline);
        assert!(lookup(&rules, "error").unwrap().bold);
    }

    #[test]
    fn override_merge() {
        let mut rules = rules_for(&theme());
        apply_overrides(&mut rules, &[("comment", SubtleOverride::italic(false))]);
        assert!(!lookup(&rules, "comment").unwrap().italic);
        // None leaves rule untouched
        apply_overrides(&mut rules, &[("comment", SubtleOverride::none())]);
        assert!(!lookup(&rules, "comment").unwrap().italic);
        // later member scope wins
        let mut rules = rules_for(&theme());
        apply_overrides(
            &mut rules,
            &[
                ("comment.todo", SubtleOverride::italic(false)),
                ("comment.note", SubtleOverride::italic(true)),
            ],
        );
        assert!(lookup(&rules, "comment.note").unwrap().italic);
    }

    #[test]
    fn unknown_none() {
        let rules = rules_for(&theme());
        assert_eq!(lookup(&rules, "nope"), None);
        assert_eq!(lookup(&rules, ""), None);
    }

    #[test]
    fn scope_bound() {
        assert!(FullRule::new(&[], None, None, false, false, false).is_none());
        let nine = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
        assert!(FullRule::new(&nine, None, None, false, false, false).is_none());
        assert!(FullRule::new(&["a"], None, None, false, false, false).is_some());
    }

    #[test]
    fn kinds_reuse_tokenkind() {
        let rules = rules_for(&theme());
        assert_eq!(
            lookup(&rules, "symbol").unwrap().kinds(),
            vec![TokenKind::String, TokenKind::Symbol]
        );
    }
}
