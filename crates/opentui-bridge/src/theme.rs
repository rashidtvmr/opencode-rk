#![forbid(unsafe_code)]
//! Theme colors (mirrors `packages/tui/src/theme/index.ts` @ a0d9b6c).
//!
//! Field-per-`Theme` RGBA slot (52 color fields + `thinking_opacity` +
//! `has_selected_list_item_text`). Default values are hand-picked dark
//! approximations, NOT copies of any `assets/*.json` file.
//! ponytail: plain struct, no serde/resolver. Upgrade when theme JSON
//! loading is accepted.

use crate::color::Rgba;

/// Error for theme lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    UnknownField(String),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownField(n) => write!(f, "unknown theme field: {n}"),
        }
    }
}

impl std::error::Error for ThemeError {}

/// Standard ANSI palette size (TS `ansiToRgba` handles codes 0-15).
pub const TERMINAL_COLORS_COUNT: usize = 16;

/// Full theme (TS `Theme`, snake_case).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub primary: Rgba,
    pub secondary: Rgba,
    pub accent: Rgba,
    pub error: Rgba,
    pub warning: Rgba,
    pub success: Rgba,
    pub info: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub selected_list_item_text: Rgba,
    pub background: Rgba,
    pub background_panel: Rgba,
    pub background_element: Rgba,
    pub background_menu: Rgba,
    pub border: Rgba,
    pub border_active: Rgba,
    pub border_subtle: Rgba,
    pub diff_added: Rgba,
    pub diff_removed: Rgba,
    pub diff_context: Rgba,
    pub diff_hunk_header: Rgba,
    pub diff_highlight_added: Rgba,
    pub diff_highlight_removed: Rgba,
    pub diff_added_bg: Rgba,
    pub diff_removed_bg: Rgba,
    pub diff_context_bg: Rgba,
    pub diff_line_number: Rgba,
    pub diff_added_line_number_bg: Rgba,
    pub diff_removed_line_number_bg: Rgba,
    pub markdown_text: Rgba,
    pub markdown_heading: Rgba,
    pub markdown_link: Rgba,
    pub markdown_link_text: Rgba,
    pub markdown_code: Rgba,
    pub markdown_block_quote: Rgba,
    pub markdown_emph: Rgba,
    pub markdown_strong: Rgba,
    pub markdown_horizontal_rule: Rgba,
    pub markdown_list_item: Rgba,
    pub markdown_list_enumeration: Rgba,
    pub markdown_image: Rgba,
    pub markdown_image_text: Rgba,
    pub markdown_code_block: Rgba,
    pub syntax_comment: Rgba,
    pub syntax_keyword: Rgba,
    pub syntax_function: Rgba,
    pub syntax_variable: Rgba,
    pub syntax_string: Rgba,
    pub syntax_number: Rgba,
    pub syntax_type: Rgba,
    pub syntax_operator: Rgba,
    pub syntax_punctuation: Rgba,
    pub thinking_opacity: f32,
    pub has_selected_list_item_text: bool,
}

/// Syntax subset (every member evidenced in TS `getSyntaxRules`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntaxPalette {
    pub comment: Rgba,
    pub keyword: Rgba,
    pub function: Rgba,
    pub variable: Rgba,
    pub string: Rgba,
    pub number: Rgba,
    pub ty: Rgba,
    pub operator: Rgba,
    pub punctuation: Rgba,
}

fn lum(c: Rgba) -> f32 {
    0.299 * f32::from(c.r) + 0.587 * f32::from(c.g) + 0.114 * f32::from(c.b)
}

impl Theme {
    /// Dark defaults (approximations, opaque).
    #[must_use]
    pub const fn default_opencode() -> Self {
        let bg = Rgba::rgb(0x18, 0x18, 0x20);
        let panel = Rgba::rgb(0x22, 0x22, 0x2c);
        let elem = Rgba::rgb(0x2c, 0x2c, 0x38);
        let fg = Rgba::rgb(0xe6, 0xe6, 0xe6);
        let muted = Rgba::rgb(0xb4, 0xb4, 0xb4);
        let cyan = Rgba::rgb(0x5f, 0xd1, 0xd1);
        let magenta = Rgba::rgb(0xc0, 0x7a, 0xd1);
        let red = Rgba::rgb(0xe0, 0x5a, 0x5a);
        let yellow = Rgba::rgb(0xd1, 0xb0, 0x45);
        let green = Rgba::rgb(0x5f, 0xb0, 0x5f);
        let blue = Rgba::rgb(0x5f, 0x8f, 0xd1);
        Self {
            primary: cyan,
            secondary: magenta,
            accent: cyan,
            error: red,
            warning: yellow,
            success: green,
            info: cyan,
            text: fg,
            text_muted: muted,
            selected_list_item_text: bg,
            background: bg,
            background_panel: panel,
            background_element: elem,
            background_menu: elem,
            border: elem,
            border_active: cyan,
            border_subtle: panel,
            diff_added: green,
            diff_removed: red,
            diff_context: muted,
            diff_hunk_header: muted,
            diff_highlight_added: green,
            diff_highlight_removed: red,
            diff_added_bg: Rgba::rgb(0x1e, 0x33, 0x24),
            diff_removed_bg: Rgba::rgb(0x38, 0x22, 0x24),
            diff_context_bg: panel,
            diff_line_number: muted,
            diff_added_line_number_bg: Rgba::rgb(0x1e, 0x33, 0x24),
            diff_removed_line_number_bg: Rgba::rgb(0x38, 0x22, 0x24),
            markdown_text: fg,
            markdown_heading: fg,
            markdown_link: blue,
            markdown_link_text: cyan,
            markdown_code: green,
            markdown_block_quote: yellow,
            markdown_emph: yellow,
            markdown_strong: fg,
            markdown_horizontal_rule: muted,
            markdown_list_item: blue,
            markdown_list_enumeration: cyan,
            markdown_image: blue,
            markdown_image_text: cyan,
            markdown_code_block: fg,
            syntax_comment: muted,
            syntax_keyword: magenta,
            syntax_function: blue,
            syntax_variable: fg,
            syntax_string: green,
            syntax_number: yellow,
            syntax_type: cyan,
            syntax_operator: cyan,
            syntax_punctuation: fg,
            thinking_opacity: 0.6,
            has_selected_list_item_text: true,
        }
    }

    /// All 52 color field names (snake_case).
    #[must_use]
    pub fn names() -> &'static [&'static str] {
        &[
            "primary",
            "secondary",
            "accent",
            "error",
            "warning",
            "success",
            "info",
            "text",
            "text_muted",
            "selected_list_item_text",
            "background",
            "background_panel",
            "background_element",
            "background_menu",
            "border",
            "border_active",
            "border_subtle",
            "diff_added",
            "diff_removed",
            "diff_context",
            "diff_hunk_header",
            "diff_highlight_added",
            "diff_highlight_removed",
            "diff_added_bg",
            "diff_removed_bg",
            "diff_context_bg",
            "diff_line_number",
            "diff_added_line_number_bg",
            "diff_removed_line_number_bg",
            "markdown_text",
            "markdown_heading",
            "markdown_link",
            "markdown_link_text",
            "markdown_code",
            "markdown_block_quote",
            "markdown_emph",
            "markdown_strong",
            "markdown_horizontal_rule",
            "markdown_list_item",
            "markdown_list_enumeration",
            "markdown_image",
            "markdown_image_text",
            "markdown_code_block",
            "syntax_comment",
            "syntax_keyword",
            "syntax_function",
            "syntax_variable",
            "syntax_string",
            "syntax_number",
            "syntax_type",
            "syntax_operator",
            "syntax_punctuation",
        ]
    }

    /// Look up a color field by snake_case name.
    pub fn get(&self, name: &str) -> Result<Rgba, ThemeError> {
        match name {
            "primary" => Ok(self.primary),
            "secondary" => Ok(self.secondary),
            "accent" => Ok(self.accent),
            "error" => Ok(self.error),
            "warning" => Ok(self.warning),
            "success" => Ok(self.success),
            "info" => Ok(self.info),
            "text" => Ok(self.text),
            "text_muted" => Ok(self.text_muted),
            "selected_list_item_text" => Ok(self.selected_list_item_text),
            "background" => Ok(self.background),
            "background_panel" => Ok(self.background_panel),
            "background_element" => Ok(self.background_element),
            "background_menu" => Ok(self.background_menu),
            "border" => Ok(self.border),
            "border_active" => Ok(self.border_active),
            "border_subtle" => Ok(self.border_subtle),
            "diff_added" => Ok(self.diff_added),
            "diff_removed" => Ok(self.diff_removed),
            "diff_context" => Ok(self.diff_context),
            "diff_hunk_header" => Ok(self.diff_hunk_header),
            "diff_highlight_added" => Ok(self.diff_highlight_added),
            "diff_highlight_removed" => Ok(self.diff_highlight_removed),
            "diff_added_bg" => Ok(self.diff_added_bg),
            "diff_removed_bg" => Ok(self.diff_removed_bg),
            "diff_context_bg" => Ok(self.diff_context_bg),
            "diff_line_number" => Ok(self.diff_line_number),
            "diff_added_line_number_bg" => Ok(self.diff_added_line_number_bg),
            "diff_removed_line_number_bg" => Ok(self.diff_removed_line_number_bg),
            "markdown_text" => Ok(self.markdown_text),
            "markdown_heading" => Ok(self.markdown_heading),
            "markdown_link" => Ok(self.markdown_link),
            "markdown_link_text" => Ok(self.markdown_link_text),
            "markdown_code" => Ok(self.markdown_code),
            "markdown_block_quote" => Ok(self.markdown_block_quote),
            "markdown_emph" => Ok(self.markdown_emph),
            "markdown_strong" => Ok(self.markdown_strong),
            "markdown_horizontal_rule" => Ok(self.markdown_horizontal_rule),
            "markdown_list_item" => Ok(self.markdown_list_item),
            "markdown_list_enumeration" => Ok(self.markdown_list_enumeration),
            "markdown_image" => Ok(self.markdown_image),
            "markdown_image_text" => Ok(self.markdown_image_text),
            "markdown_code_block" => Ok(self.markdown_code_block),
            "syntax_comment" => Ok(self.syntax_comment),
            "syntax_keyword" => Ok(self.syntax_keyword),
            "syntax_function" => Ok(self.syntax_function),
            "syntax_variable" => Ok(self.syntax_variable),
            "syntax_string" => Ok(self.syntax_string),
            "syntax_number" => Ok(self.syntax_number),
            "syntax_type" => Ok(self.syntax_type),
            "syntax_operator" => Ok(self.syntax_operator),
            "syntax_punctuation" => Ok(self.syntax_punctuation),
            _ => Err(ThemeError::UnknownField(name.to_string())),
        }
    }

    /// Syntax subset of this theme.
    #[must_use]
    pub const fn syntax(&self) -> SyntaxPalette {
        SyntaxPalette {
            comment: self.syntax_comment,
            keyword: self.syntax_keyword,
            function: self.syntax_function,
            variable: self.syntax_variable,
            string: self.syntax_string,
            number: self.syntax_number,
            ty: self.syntax_type,
            operator: self.syntax_operator,
            punctuation: self.syntax_punctuation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bg_darker_than_fg() {
        let t = Theme::default_opencode();
        assert!(lum(t.background) < lum(t.text));
    }

    #[test]
    fn default_all_alpha_opaque() {
        let t = Theme::default_opencode();
        for name in Theme::names() {
            assert_eq!(t.get(name).unwrap().a, 255, "{name}");
        }
    }

    #[test]
    fn names_nonempty_contains_core() {
        assert!(!Theme::names().is_empty());
        assert_eq!(Theme::names().len(), 52);
        assert!(Theme::names().contains(&"primary"));
        assert!(Theme::names().contains(&"background"));
    }

    #[test]
    fn clone_eq() {
        let t = Theme::default_opencode();
        assert_eq!(t.clone(), t);
    }

    #[test]
    fn two_defaults_equal() {
        assert_eq!(Theme::default_opencode(), Theme::default_opencode());
    }

    #[test]
    fn syntax_palette_finite() {
        let s = Theme::default_opencode().syntax();
        for c in [s.comment, s.keyword, s.function, s.variable, s.string, s.number, s.ty, s.operator, s.punctuation] {
            assert!(lum(c).is_finite());
        }
    }

    #[test]
    fn unknown_field_errors() {
        assert!(Theme::default_opencode().get("nope").is_err());
    }
}
