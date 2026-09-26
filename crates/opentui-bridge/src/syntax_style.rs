#![forbid(unsafe_code)]
//! Syntax-highlight contracts (mirrors `packages/tui/src/theme/index.ts` @ a0d9b6c).
//!
//! SOURCE EVIDENCE (TS checkout at /home/rashid/projects/opencode, commit a0d9b6c):
//! - `generateSyntax` (index.ts:556-557): `SyntaxStyle.fromTheme(getSyntaxRules(theme))`.
//! - `generateSubtleSyntax` (index.ts:560-584): same rules, every `foreground`
//!   re-alphaed to `thinkingOpacity * 255`, per-scope italic overrides.
//! - `getSyntaxRules` (index.ts:586+): rule shape `{ scope: string[],
//!   style: { foreground, background?, bold?, italic?, underline? } }`,
//!   token -> color/attrs. All `TokenKind` names below are verbatim scopes.
//! - `TerminalColors` (index.ts:353-362): `terminalMode` reads
//!   `defaultBackground`; `generateSystem` falls back to `palette[0]` (bg)
//!   and `palette[7]` (fg).
//! - Language ids reused via `crate::filetype` (NOT redefined here).
//! - Base syntax colors reused via `crate::theme::SyntaxPalette`.
//! - Attribute bits reused via `crate::attributes::TextAttributes` (stored raw).
//! ponytail: fixed token enum + Vec bound, no TextMate engine. Upgrade when
//! real scope-selector matching is accepted.

use crate::attributes::TextAttributes;
use crate::color::Rgba;
use crate::theme::SyntaxPalette;

pub use crate::filetype::language_of;

/// Max rules per layer (TS `getSyntaxRules` has ~76 `scope:` entries; 128 bounds growth).
pub const MAX_RULES: usize = 128;
/// Standard ANSI palette size (TS `ansiToRgba` codes 0-15).
pub const PALETTE_LEN: usize = 16;

macro_rules! tokens {
    ($($v:ident => $s:literal),*) => {
        /// Highlight token (verbatim `scope` names from TS `getSyntaxRules`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum TokenKind {
            $($v),*
        }
        impl TokenKind {
            /// Verbatim scope string.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),* }
            }
            /// Parse a scope string; fail-closed `None`.
            #[must_use]
            pub fn from_str(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$v),)* _ => None }
            }
        }
    };
}

tokens! {
    Default => "default", Prompt => "prompt", ExtmarkFile => "extmark.file",
    ExtmarkAgent => "extmark.agent", ExtmarkPaste => "extmark.paste",
    Comment => "comment", CommentDocumentation => "comment.documentation",
    String => "string", Symbol => "symbol", Number => "number", Boolean => "boolean",
    CharacterSpecial => "character.special", KeywordReturn => "keyword.return",
    KeywordConditional => "keyword.conditional", KeywordRepeat => "keyword.repeat",
    KeywordCoroutine => "keyword.coroutine", KeywordType => "keyword.type",
    KeywordFunction => "keyword.function", FunctionMethod => "function.method",
    Keyword => "keyword", KeywordImport => "keyword.import", Operator => "operator",
    KeywordOperator => "keyword.operator", PunctuationDelimiter => "punctuation.delimiter",
    KeywordConditionalTernary => "keyword.conditional.ternary", Variable => "variable",
    VariableParameter => "variable.parameter", FunctionMethodCall => "function.method.call",
    FunctionCall => "function.call", VariableMember => "variable.member",
    Function => "function", Constructor => "constructor", Type => "type",
    Module => "module", Constant => "constant", Property => "property",
    Class => "class", Parameter => "parameter", Punctuation => "punctuation",
    PunctuationBracket => "punctuation.bracket", VariableBuiltin => "variable.builtin",
    TypeBuiltin => "type.builtin", FunctionBuiltin => "function.builtin",
    ModuleBuiltin => "module.builtin", ConstantBuiltin => "constant.builtin",
    VariableSuper => "variable.super", StringEscape => "string.escape",
    StringRegexp => "string.regexp", KeywordDirective => "keyword.directive",
    PunctuationSpecial => "punctuation.special", KeywordModifier => "keyword.modifier",
    KeywordException => "keyword.exception", MarkupHeading => "markup.heading",
    MarkupHeading1 => "markup.heading.1", MarkupHeading2 => "markup.heading.2",
    MarkupHeading3 => "markup.heading.3", MarkupHeading4 => "markup.heading.4",
    MarkupHeading5 => "markup.heading.5", MarkupHeading6 => "markup.heading.6",
    MarkupBold => "markup.bold", MarkupStrong => "markup.strong",
    MarkupItalic => "markup.italic", MarkupList => "markup.list",
    MarkupQuote => "markup.quote", MarkupRaw => "markup.raw",
    MarkupRawBlock => "markup.raw.block", MarkupRawInline => "markup.raw.inline",
    MarkupLink => "markup.link", MarkupLinkLabel => "markup.link.label",
    MarkupLinkUrl => "markup.link.url", Label => "label", Spell => "spell",
    Nospell => "nospell", Conceal => "conceal", StringSpecial => "string.special",
    StringSpecialUrl => "string.special.url", Character => "character",
    Float => "float", CommentError => "comment.error",
    CommentWarning => "comment.warning", CommentTodo => "comment.todo",
    CommentNote => "comment.note", Namespace => "namespace", Field => "field",
    TypeDefinition => "type.definition", KeywordExport => "keyword.export",
    Attribute => "attribute", Annotation => "annotation", Tag => "tag",
    TagAttribute => "tag.attribute", TagDelimiter => "tag.delimiter",
    MarkupStrikethrough => "markup.strikethrough", MarkupUnderline => "markup.underline",
    MarkupListChecked => "markup.list.checked", MarkupListUnchecked => "markup.list.unchecked",
    DiffPlus => "diff.plus", DiffMinus => "diff.minus", DiffDelta => "diff.delta",
    Error => "error", Warning => "warning", Info => "info", Debug => "debug"
}

/// One token -> color/attrs rule (TS `{ scope, style }` entry, single scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleRule {
    pub token: TokenKind,
    pub fg: Option<Rgba>,
    pub attributes: u16,
}

impl StyleRule {
    pub const fn new(token: TokenKind, fg: Option<Rgba>, attributes: u16) -> Self {
        Self { token, fg, attributes }
    }
}

/// Layered theme: exact rules, subtle rules (TS `generateSubtleSyntax`),
/// default fallback (TS `default` scope -> `theme.text`).
#[derive(Debug, Clone, Default)]
pub struct SyntaxTheme {
    rules: Vec<StyleRule>,
    subtle: Vec<StyleRule>,
    default: Option<StyleRule>,
}

impl SyntaxTheme {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Push exact rule; `false` (dropped) past `MAX_RULES`.
    pub fn push_rule(&mut self, rule: StyleRule) -> bool {
        if self.rules.len() >= MAX_RULES {
            return false;
        }
        self.rules.push(rule);
        true
    }

    /// Push subtle rule; `false` (dropped) past `MAX_RULES`.
    pub fn push_subtle(&mut self, rule: StyleRule) -> bool {
        if self.subtle.len() >= MAX_RULES {
            return false;
        }
        self.subtle.push(rule);
        true
    }

    pub fn set_default(&mut self, rule: StyleRule) {
        self.default = Some(rule);
    }

    /// Fallback chain: exact -> subtle -> default; `None` when all miss.
    #[must_use]
    pub fn lookup(&self, token: &str) -> Option<&StyleRule> {
        if let Some(r) = self.rules.iter().find(|r| r.token.as_str() == token) {
            return Some(r);
        }
        if let Some(r) = self.subtle.iter().find(|r| r.token.as_str() == token) {
            return Some(r);
        }
        self.default.as_ref().filter(|d| d.token.as_str() == token || TokenKind::from_str(token).is_some())
    }

    /// Build core 9-slot theme from a `SyntaxPalette` + text color.
    /// Attrs mirror `getSyntaxRules`: comment/keyword italic.
    #[must_use]
    pub fn from_syntax_palette(p: SyntaxPalette, text: Rgba, thinking_opacity: f32) -> Self {
        let italic = TextAttributes::ITALIC.as_u16();
        let core = [
            (TokenKind::Comment, p.comment, italic),
            (TokenKind::Keyword, p.keyword, italic),
            (TokenKind::Function, p.function, 0),
            (TokenKind::Variable, p.variable, 0),
            (TokenKind::String, p.string, 0),
            (TokenKind::Number, p.number, 0),
            (TokenKind::Type, p.ty, 0),
            (TokenKind::Operator, p.operator, 0),
            (TokenKind::Punctuation, p.punctuation, 0),
        ];
        let mut t = Self::empty();
        for (token, fg, attrs) in core {
            t.push_rule(StyleRule::new(token, Some(fg), attrs));
            t.push_subtle(StyleRule::new(token, Some(subtle_fg(fg, thinking_opacity)), attrs));
        }
        t.set_default(StyleRule::new(TokenKind::Default, Some(text), 0));
        t
    }
}

/// Mirror `generateSubtleSyntax`: foreground re-alphaed to `thinkingOpacity * 255`.
#[must_use]
pub fn subtle_fg(fg: Rgba, thinking_opacity: f32) -> Rgba {
    let a = (thinking_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(fg.r, fg.g, fg.b, a)
}

/// Terminal palette (TS `TerminalColors`): bg/fg + 16 ANSI colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPalette {
    pub bg: Rgba,
    pub fg: Rgba,
    pub palette: [Rgba; PALETTE_LEN],
}

impl TerminalPalette {
    /// Dark defaults: palette = ANSI 0-15 (TS `ansiToRgba`), bg/fg = palette ends.
    #[must_use]
    pub fn default_dark() -> Self {
        let mut palette = [Rgba::rgb(0, 0, 0); PALETTE_LEN];
        for (i, slot) in palette.iter_mut().enumerate() {
            *slot = Rgba::from_ansi256(i as u8);
        }
        Self { bg: palette[0], fg: palette[7], palette }
    }

    /// Mirror `terminalMode`: luminance of bg over 0.5 -> light.
    #[must_use]
    pub fn mode(&self) -> &'static str {
        let lum = 0.299 * f32::from(self.bg.r) + 0.587 * f32::from(self.bg.g) + 0.114 * f32::from(self.bg.b);
        if lum > 0.5 * 255.0 { "light" } else { "dark" }
    }

    /// Fail-closed indexed read.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Rgba> {
        self.palette.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn sample() -> SyntaxTheme {
        SyntaxTheme::from_syntax_palette(Theme::default_opencode().syntax(), Rgba::rgb(1, 2, 3), 0.6)
    }

    #[test]
    fn exact_hit() {
        let t = sample();
        let r = t.lookup("keyword").unwrap();
        assert_eq!(r.token, TokenKind::Keyword);
        assert_eq!(r.attributes, TextAttributes::ITALIC.as_u16());
    }

    #[test]
    fn fallback_to_subtle() {
        let mut t = SyntaxTheme::empty();
        t.push_subtle(StyleRule::new(TokenKind::String, Some(Rgba::rgb(9, 9, 9)), 0));
        assert_eq!(t.lookup("string").unwrap().fg, Some(Rgba::rgb(9, 9, 9)));
    }

    #[test]
    fn fallback_to_default() {
        let mut t = SyntaxTheme::empty();
        t.set_default(StyleRule::new(TokenKind::Default, Some(Rgba::rgb(1, 2, 3)), 0));
        assert_eq!(t.lookup("class").unwrap().fg, Some(Rgba::rgb(1, 2, 3)));
    }

    #[test]
    fn unknown_none() {
        assert_eq!(sample().lookup("nope"), None);
        assert_eq!(SyntaxTheme::empty().lookup("keyword"), None);
    }

    #[test]
    fn token_roundtrip() {
        assert_eq!(TokenKind::from_str("keyword.directive"), Some(TokenKind::KeywordDirective));
        assert_eq!(TokenKind::KeywordDirective.as_str(), "keyword.directive");
        assert_eq!(TokenKind::from_str("nope"), None);
    }

    #[test]
    fn palette_len() {
        let p = TerminalPalette::default_dark();
        assert_eq!(p.palette.len(), PALETTE_LEN);
        assert_eq!(p.get(15), Some(Rgba::rgb(255, 255, 255)));
        assert_eq!(p.get(16), None);
        assert_eq!(p.mode(), "dark");
    }

    #[test]
    fn subtle_differs() {
        let t = sample();
        let exact = t.rules.iter().find(|r| r.token == TokenKind::Comment).unwrap();
        let subtle = t.subtle.iter().find(|r| r.token == TokenKind::Comment).unwrap();
        assert_eq!(exact.fg.unwrap().a, 255);
        assert_eq!(subtle.fg.unwrap().a, (0.6f32 * 255.0).round() as u8);
        assert_ne!(exact.fg, subtle.fg);
    }

    #[test]
    fn bound_rejects_overflow() {
        let mut t = SyntaxTheme::empty();
        for _ in 0..MAX_RULES {
            assert!(t.push_rule(StyleRule::new(TokenKind::Keyword, None, 0)));
        }
        assert!(!t.push_rule(StyleRule::new(TokenKind::Keyword, None, 0)));
        assert!(!SyntaxTheme::empty().push_subtle_many());
    }

    #[test]
    fn language_reuse() {
        assert_eq!(language_of("app.tsx"), Some("typescriptreact"));
        assert_eq!(language_of("foo.xyz"), None);
    }
}

#[cfg(test)]
impl SyntaxTheme {
    fn push_subtle_many(&mut self) -> bool {
        for _ in 0..MAX_RULES {
            if !self.push_subtle(StyleRule::new(TokenKind::Keyword, None, 0)) {
                return false;
            }
        }
        self.push_subtle(StyleRule::new(TokenKind::Keyword, None, 0))
    }
}
