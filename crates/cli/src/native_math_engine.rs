//! Real math layout producer: bounded port of `packages/latex/src/parser.ts`,
//! `symbols.ts` and `layout.ts` into a [`MathLayoutEngine`] that returns
//! painted glyph cells (MD-009).
//!
//! Upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`. The engine computes
//! `renderLatex = layoutMath(parseLatex(...))`
//! (`packages/latex/src/render.ts:1-12`) and mirrors the strict-mode call the
//! live code-fence path makes (`packages/latex/src/markdown.ts:112-118`,
//! `strict: true`, `displayMode: true`).
//!
//! Ported construct set (parser + layout): groups `{...}`, scripts `^`/`_`
//! (compact Unicode superscripts/subscripts), the full symbol table
//! (`symbols.ts`), named/large operators, the `\frac` family and `\binom`,
//! `\sqrt`, `\left...\right` / `\middle` / sizing commands, accents, variants
//! (`\mathbb`, `\mathbf`, ...), `\text`/`\operatorname`,
//! `\overbrace`/`\underbrace`, `\overset`/`\underset`, `\not`, `\pmod`
//! family, spacing commands, all matrix environments and
//! `\textcolor`/`\color`.
//!
//! Documented partial (each with upstream evidence):
//! * `ParseOptions.macros` / `expandLatexMacros` are NOT ported. Upstream's
//!   only live caller passes no macros (`markdown.ts:112-118`), so expansion
//!   is the identity there; strict unknown-command failure is preserved.
//! * Option keys (`maxSourceLength`, `maxDepth`, `maxExpandedLength`,
//!   `maxExpand`) are pinned to the upstream defaults (`limits.ts:3-4`);
//!   [`MathLayoutRequest`] carries no options.
//! * [`MathCell`] stores one `char` per cell. Multi-char grapheme clusters
//!   (combining negations from `\not x` where no precomposed glyph exists)
//!   keep only their base char; the negation slash is dropped. Upstream
//!   stores the whole grapheme (`types.ts:80-83`, painted as one chunk in
//!   `markdown.ts:58-72`).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use unicode_segmentation::UnicodeSegmentation;

use crate::native_math::{
    MathCell, MathFailure, MathLayout, MathLayoutEngine, MathLayoutRequest, MathStyle,
    MAX_MATH_NESTING_DEPTH, MAX_MATH_SOURCE_CHARS, cell_width, check_nesting_depth,
    check_source_length,
};

// ── Node model (`packages/latex/src/types.ts:8-48`) ───────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolRole {
    Ordinary,
    Binary,
    Relation,
    Operator,
    Punctuation,
    Opening,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Normal,
    Bold,
    Italic,
    Sans,
    Monospace,
    DoubleStruck,
    Script,
    Fraktur,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumeratorAlign {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MathNode {
    Row(Vec<MathNode>),
    Symbol {
        value: String,
        role: Option<SymbolRole>,
    },
    Text(String),
    Space(usize),
    Fraction {
        numerator: Box<MathNode>,
        denominator: Box<MathNode>,
        bar: bool,
        numerator_align: Option<NumeratorAlign>,
    },
    Root {
        body: Box<MathNode>,
        index: Option<Box<MathNode>>,
    },
    Scripts {
        base: Box<MathNode>,
        superscript: Option<Box<MathNode>>,
        subscript: Option<Box<MathNode>>,
    },
    Delimited {
        left: String,
        body: Box<MathNode>,
        right: String,
    },
    Matrix {
        rows: Vec<Vec<MathNode>>,
        environment: &'static str,
        columns: Option<String>,
    },
    Brace {
        body: Box<MathNode>,
        over: bool,
    },
    Accent {
        accent: &'static str,
        body: Box<MathNode>,
    },
    Variant {
        variant: Variant,
        body: Box<MathNode>,
    },
    OverUnder {
        base: Box<MathNode>,
        over: Option<Box<MathNode>>,
        under: Option<Box<MathNode>>,
    },
    Color {
        color: String,
        body: Box<MathNode>,
    },
    Operator {
        value: String,
        limits: bool,
    },
}

impl MathNode {
    fn symbol(value: &str, role: SymbolRole) -> Self {
        Self::Symbol {
            value: value.to_owned(),
            role: Some(role),
        }
    }

    /// `row(body)` (`parser.ts:568-571`): single nodes unwrap.
    fn wrap_row(body: Vec<MathNode>) -> Self {
        if body.len() == 1 {
            body.into_iter().next().unwrap_or(Self::Row(Vec::new()))
        } else {
            Self::Row(body)
        }
    }
}

/// `LatexParseError` (`packages/latex/src/types.ts:63-71`); position is a
/// char offset (OC2 scalar-index convention, see `native_math.rs` notes).
#[derive(Clone, Debug, PartialEq, Eq)]
struct LatexParseError {
    #[allow(dead_code)]
    message: String,
    position: usize,
}

// ── Tables (`packages/latex/src/symbols.ts`) ──────────────────────────

const ORDINARY_SYMBOLS: &[(&str, char)] = &[
    ("alpha", 'α'),
    ("beta", 'β'),
    ("gamma", 'γ'),
    ("delta", 'δ'),
    ("epsilon", 'ε'),
    ("varepsilon", 'ϵ'),
    ("zeta", 'ζ'),
    ("eta", 'η'),
    ("theta", 'θ'),
    ("vartheta", 'ϑ'),
    ("iota", 'ι'),
    ("kappa", 'κ'),
    ("lambda", 'λ'),
    ("mu", 'μ'),
    ("nu", 'ν'),
    ("xi", 'ξ'),
    ("omicron", 'ο'),
    ("pi", 'π'),
    ("varpi", 'ϖ'),
    ("rho", 'ρ'),
    ("varrho", 'ϱ'),
    ("sigma", 'σ'),
    ("varsigma", 'ς'),
    ("tau", 'τ'),
    ("upsilon", 'υ'),
    ("phi", 'ϕ'),
    ("varphi", 'φ'),
    ("chi", 'χ'),
    ("psi", 'ψ'),
    ("omega", 'ω'),
    ("Gamma", 'Γ'),
    ("Delta", 'Δ'),
    ("Theta", 'Θ'),
    ("Lambda", 'Λ'),
    ("Xi", 'Ξ'),
    ("Pi", 'Π'),
    ("Sigma", 'Σ'),
    ("Upsilon", 'Υ'),
    ("Phi", 'Φ'),
    ("Psi", 'Ψ'),
    ("Omega", 'Ω'),
    ("infty", '∞'),
    ("ell", 'ℓ'),
    ("hbar", 'ℏ'),
    ("imath", 'ı'),
    ("jmath", 'ȷ'),
    ("Re", 'ℜ'),
    ("Im", 'ℑ'),
    ("aleph", 'ℵ'),
    ("beth", 'ℶ'),
    ("gimel", 'ℷ'),
    ("daleth", 'ℸ'),
    ("partial", '∂'),
    ("nabla", '∇'),
    ("angle", '∠'),
    ("measuredangle", '∡'),
    ("triangle", '△'),
    ("square", '□'),
    ("lozenge", '◊'),
    ("top", '⊤'),
    ("bot", '⊥'),
    ("emptyset", '∅'),
    ("varnothing", '∅'),
    ("forall", '∀'),
    ("exists", '∃'),
    ("nexists", '∄'),
    ("neg", '¬'),
    ("lnot", '¬'),
    ("prime", '′'),
    ("backprime", '‵'),
    ("clubsuit", '♣'),
    ("diamondsuit", '♢'),
    ("heartsuit", '♡'),
    ("spadesuit", '♠'),
    ("checkmark", '✓'),
];

const BINARY_SYMBOLS: &[(&str, char)] = &[
    ("pm", '±'),
    ("mp", '∓'),
    ("times", '×'),
    ("div", '÷'),
    ("cdot", '·'),
    ("ast", '∗'),
    ("star", '⋆'),
    ("circ", '∘'),
    ("bullet", '∙'),
    ("oplus", '⊕'),
    ("ominus", '⊖'),
    ("otimes", '⊗'),
    ("oslash", '⊘'),
    ("odot", '⊙'),
    ("cap", '∩'),
    ("cup", '∪'),
    ("uplus", '⊎'),
    ("sqcap", '⊓'),
    ("sqcup", '⊔'),
    ("vee", '∨'),
    ("lor", '∨'),
    ("wedge", '∧'),
    ("land", '∧'),
    ("setminus", '∖'),
    ("wr", '≀'),
    ("diamond", '⋄'),
    ("bigtriangleup", '△'),
    ("bigtriangledown", '▽'),
    ("triangleleft", '◁'),
    ("triangleright", '▷'),
];

const RELATION_SYMBOLS: &[(&str, char)] = &[
    ("equals", '='),
    ("neq", '≠'),
    ("ne", '≠'),
    ("equiv", '≡'),
    ("approx", '≈'),
    ("sim", '∼'),
    ("simeq", '≃'),
    ("cong", '≅'),
    ("asymp", '≍'),
    ("propto", '∝'),
    ("lt", '<'),
    ("gt", '>'),
    ("le", '≤'),
    ("leq", '≤'),
    ("ge", '≥'),
    ("geq", '≥'),
    ("ll", '≪'),
    ("gg", '≫'),
    ("prec", '≺'),
    ("succ", '≻'),
    ("preceq", '⪯'),
    ("succeq", '⪰'),
    ("subset", '⊂'),
    ("supset", '⊃'),
    ("subseteq", '⊆'),
    ("supseteq", '⊇'),
    ("sqsubset", '⊏'),
    ("sqsupset", '⊐'),
    ("sqsubseteq", '⊑'),
    ("sqsupseteq", '⊒'),
    ("in", '∈'),
    ("ni", '∋'),
    ("notin", '∉'),
    ("owns", '∋'),
    ("vdash", '⊢'),
    ("dashv", '⊣'),
    ("models", '⊨'),
    ("mid", '∣'),
    ("parallel", '∥'),
    ("perp", '⊥'),
    ("smile", '⌣'),
    ("frown", '⌢'),
];

const ARROW_SYMBOLS: &[(&str, char)] = &[
    ("leftarrow", '←'),
    ("gets", '←'),
    ("rightarrow", '→'),
    ("to", '→'),
    ("leftrightarrow", '↔'),
    ("Leftarrow", '⇐'),
    ("Rightarrow", '⇒'),
    ("Leftrightarrow", '⇔'),
    ("mapsto", '↦'),
    ("hookleftarrow", '↩'),
    ("hookrightarrow", '↪'),
    ("leftharpoonup", '↼'),
    ("leftharpoondown", '↽'),
    ("rightharpoonup", '⇀'),
    ("rightharpoondown", '⇁'),
    ("rightleftharpoons", '⇌'),
    ("longleftarrow", '⟵'),
    ("longrightarrow", '⟶'),
    ("longleftrightarrow", '⟷'),
    ("Longleftarrow", '⟸'),
    ("Longrightarrow", '⟹'),
    ("Longleftrightarrow", '⟺'),
    ("longmapsto", '⟼'),
    ("uparrow", '↑'),
    ("downarrow", '↓'),
    ("updownarrow", '↕'),
    ("Uparrow", '⇑'),
    ("Downarrow", '⇓'),
    ("Updownarrow", '⇕'),
    ("nearrow", '↗'),
    ("searrow", '↘'),
    ("swarrow", '↙'),
    ("nwarrow", '↖'),
];

const PUNCTUATION_SYMBOLS: &[(&str, char)] = &[
    ("cdots", '⋯'),
    ("ldots", '…'),
    ("dots", '…'),
    ("vdots", '⋮'),
    ("ddots", '⋱'),
    ("colon", ':'),
];

const LARGE_OPERATORS: &[(&str, char)] = &[
    ("sum", '∑'),
    ("prod", '∏'),
    ("coprod", '∐'),
    ("int", '∫'),
    ("iint", '∬'),
    ("iiint", '∭'),
    ("oint", '∮'),
    ("bigcap", '⋂'),
    ("bigcup", '⋃'),
    ("bigvee", '⋁'),
    ("bigwedge", '⋀'),
    ("bigoplus", '⨁'),
    ("bigotimes", '⨂'),
    ("bigodot", '⨀'),
];

const NAMED_OPERATORS: &[&str] = &[
    "arccos", "arcsin", "arctan", "arg", "cos", "cosh", "cot", "coth", "csc", "deg", "det", "dim",
    "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "liminf", "limsup", "ln", "log", "max", "min",
    "mod", "Pr", "sec", "sin", "sinh", "sup", "tan", "tanh",
];

/// `delimiterTable` (`symbols.ts:280-300`). `\backslash` is intentionally
/// absent: upstream intercepts it as an ordinary command (`parser.ts:352`).
const DELIMITER_ENTRIES: &[(&str, &str)] = &[
    ("(", "("),
    (")", ")"),
    ("[", "["),
    ("]", "]"),
    ("\\{", "{"),
    ("\\}", "}"),
    ("{", "{"),
    ("}", "}"),
    ("|", "│"),
    ("\\|", "║"),
    ("vert", "│"),
    ("Vert", "║"),
    ("lvert", "│"),
    ("rvert", "│"),
    ("lVert", "║"),
    ("rVert", "║"),
    ("lbrace", "{"),
    ("rbrace", "}"),
    ("langle", "⟨"),
    ("rangle", "⟩"),
    ("lfloor", "⌊"),
    ("rfloor", "⌋"),
    ("lceil", "⌈"),
    ("rceil", "⌉"),
    (".", ""),
];

const SPACING_COMMANDS: &[(&str, usize)] = &[
    (",", 0),
    (":", 1),
    (";", 1),
    ("!", 0),
    ("quad", 2),
    ("qquad", 4),
    ("enspace", 1),
    ("thinspace", 0),
];

const MATRIX_ENVIRONMENTS: &[&str] = &[
    "matrix",
    "pmatrix",
    "bmatrix",
    "Bmatrix",
    "vmatrix",
    "Vmatrix",
    "cases",
    "aligned",
    "align",
    "gathered",
    "gather",
    "smallmatrix",
    "array",
];

const ACCENTS: &[(&str, &str)] = &[
    ("hat", "hat"),
    ("widehat", "hat"),
    ("bar", "bar"),
    ("overline", "overline"),
    ("underline", "underline"),
    ("vec", "vec"),
    ("tilde", "tilde"),
    ("widetilde", "tilde"),
    ("dot", "dot"),
    ("ddot", "ddot"),
];

const VARIANTS: &[(&str, Variant)] = &[
    ("mathrm", Variant::Normal),
    ("textrm", Variant::Normal),
    ("mathnormal", Variant::Normal),
    ("mathbf", Variant::Bold),
    ("boldsymbol", Variant::Bold),
    ("bm", Variant::Bold),
    ("mathit", Variant::Italic),
    ("mathsf", Variant::Sans),
    ("mathtt", Variant::Monospace),
    ("mathbb", Variant::DoubleStruck),
    ("mathcal", Variant::Script),
    ("mathscr", Variant::Script),
    ("mathfrak", Variant::Fraktur),
];

const SUPERSCRIPTS: &[(char, char)] = &[
    ('0', '⁰'),
    ('1', '¹'),
    ('2', '²'),
    ('3', '³'),
    ('4', '⁴'),
    ('5', '⁵'),
    ('6', '⁶'),
    ('7', '⁷'),
    ('8', '⁸'),
    ('9', '⁹'),
    ('+', '⁺'),
    ('-', '⁻'),
    ('=', '⁼'),
    ('(', '⁽'),
    (')', '⁾'),
    ('n', 'ⁿ'),
    ('i', 'ⁱ'),
];

const SUBSCRIPTS: &[(char, char)] = &[
    ('0', '₀'),
    ('1', '₁'),
    ('2', '₂'),
    ('3', '₃'),
    ('4', '₄'),
    ('5', '₅'),
    ('6', '₆'),
    ('7', '₇'),
    ('8', '₈'),
    ('9', '₉'),
    ('+', '₊'),
    ('-', '₋'),
    ('=', '₌'),
    ('(', '₍'),
    (')', '₎'),
    ('a', 'ₐ'),
    ('e', 'ₑ'),
    ('h', 'ₕ'),
    ('i', 'ᵢ'),
    ('j', 'ⱼ'),
    ('k', 'ₖ'),
    ('l', 'ₗ'),
    ('m', 'ₘ'),
    ('n', 'ₙ'),
    ('o', 'ₒ'),
    ('p', 'ₚ'),
    ('r', 'ᵣ'),
    ('s', 'ₛ'),
    ('t', 'ₜ'),
    ('u', 'ᵤ'),
    ('v', 'ᵥ'),
    ('x', 'ₓ'),
];

static SYMBOL_TABLE: LazyLock<HashMap<String, (char, SymbolRole)>> = LazyLock::new(|| {
    let mut table = HashMap::new();
    for (name, value) in ORDINARY_SYMBOLS.iter() {
        table.insert((*name).to_owned(), (*value, SymbolRole::Ordinary));
    }
    for (name, value) in BINARY_SYMBOLS.iter() {
        table.insert((*name).to_owned(), (*value, SymbolRole::Binary));
    }
    for (name, value) in RELATION_SYMBOLS.iter() {
        table.insert((*name).to_owned(), (*value, SymbolRole::Relation));
    }
    for (name, value) in ARROW_SYMBOLS.iter() {
        table.insert((*name).to_owned(), (*value, SymbolRole::Relation));
    }
    for (name, value) in PUNCTUATION_SYMBOLS.iter() {
        table.insert((*name).to_owned(), (*value, SymbolRole::Punctuation));
    }
    table
});

static LARGE_OPERATOR_TABLE: LazyLock<HashMap<String, char>> = LazyLock::new(|| {
    LARGE_OPERATORS
        .iter()
        .map(|(name, value)| ((*name).to_owned(), *value))
        .collect()
});

static NAMED_OPERATOR_SET: LazyLock<HashSet<String>> =
    LazyLock::new(|| NAMED_OPERATORS.iter().map(|name| (*name).to_owned()).collect());

static DELIMITER_TABLE: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    DELIMITER_ENTRIES
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
});

static SPACING_TABLE: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
    SPACING_COMMANDS
        .iter()
        .map(|(name, width)| ((*name).to_owned(), *width))
        .collect()
});

static ACCENT_TABLE: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    ACCENTS
        .iter()
        .map(|(name, accent)| ((*name).to_owned(), *accent))
        .collect()
});

static VARIANT_TABLE: LazyLock<HashMap<String, Variant>> = LazyLock::new(|| {
    VARIANTS
        .iter()
        .map(|(name, variant)| ((*name).to_owned(), *variant))
        .collect()
});

static SUPERSCRIPT_TABLE: LazyLock<HashMap<char, char>> =
    LazyLock::new(|| SUPERSCRIPTS.iter().copied().collect());
static SUBSCRIPT_TABLE: LazyLock<HashMap<char, char>> =
    LazyLock::new(|| SUBSCRIPTS.iter().copied().collect());

/// `negateSymbol` (`parser.ts:575-598` plus the `value̸` fallback).
fn negate_symbol(value: char) -> String {
    let negated = match value {
        '=' => '≠',
        '∈' => '∉',
        '∋' => '∌',
        '≡' => '≢',
        '≈' => '≉',
        '∼' => '≁',
        '<' => '≮',
        '>' => '≯',
        '≤' => '≰',
        '≥' => '≱',
        '⊂' => '⊄',
        '⊃' => '⊅',
        '⊆' => '⊈',
        '⊇' => '⊉',
        '∣' => '∤',
        '∥' => '∦',
        other => return format!("{other}\u{338}"),
    };
    negated.to_string()
}

/// `inferRole` (`parser.ts:573-584`).
fn infer_role(value: char) -> SymbolRole {
    if "+-*/×÷±∓".contains(value) {
        return SymbolRole::Binary;
    }
    if "=<>≤≥≠≈∈∉⊂⊃".contains(value) {
        return SymbolRole::Relation;
    }
    if ",;:".contains(value) {
        return SymbolRole::Punctuation;
    }
    if "([{".contains(value) {
        return SymbolRole::Opening;
    }
    if ")]}".contains(value) {
        return SymbolRole::Closing;
    }
    SymbolRole::Ordinary
}

// ── Parser (`packages/latex/src/parser.ts:78-566`) ────────────────────

/// Where `parseRow` stops (`parser.ts:113-139` optional `stop` predicate).
#[derive(Clone, Copy, Debug)]
enum Stop {
    /// Run to end of input.
    None,
    /// Stop at one char (`]` in optional arguments).
    At(char),
    /// Stop at a `\right` command (`parseLeftRight`).
    RightDelimiter,
    /// Matrix-cell stop: `&`, the `\\` boundary, or `pending_stop_marker`.
    MatrixCell,
}

fn is_command_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '@'
}

struct Parser<'s> {
    chars: &'s [char],
    position: usize,
    depth: usize,
    max_depth: usize,
    strict: bool,
    /// Active `\end{...}` marker while inside a matrix environment.
    pending_stop_marker: String,
}

impl<'s> Parser<'s> {
    fn new(source: &'s [char], strict: bool, max_depth: usize) -> Self {
        Self {
            chars: source,
            position: 0,
            depth: 0,
            max_depth,
            strict,
            pending_stop_marker: String::new(),
        }
    }

    /// `Parser.parse` (`parser.ts:104-111`).
    fn parse(&mut self) -> Result<MathNode, LatexParseError> {
        let body = self.parse_row(Stop::None)?;
        self.skip_math_whitespace();
        if !self.done() {
            return self.fail(format!("Unexpected \"{}\"", self.peek().unwrap()), self.position);
        }
        Ok(MathNode::wrap_row(body))
    }

    fn parse_row(&mut self, stop: Stop) -> Result<Vec<MathNode>, LatexParseError> {
        let mut body = Vec::new();
        while !self.done() {
            self.skip_math_whitespace();
            if self.done() || self.stop_matches(stop) {
                break;
            }
            let current = self.peek().unwrap();
            if current == '}' {
                break;
            }
            if current == '^' || current == '_' {
                self.position += 1;
                let script = Box::new(self.parse_argument()?);
                let previous = body.pop().unwrap_or(MathNode::Row(Vec::new()));
                let node = match previous {
                    MathNode::Scripts {
                        base,
                        mut superscript,
                        mut subscript,
                    } => {
                        if current == '^' {
                            superscript = Some(script);
                        } else {
                            subscript = Some(script);
                        }
                        MathNode::Scripts {
                            base,
                            superscript,
                            subscript,
                        }
                    }
                    other => {
                        let superscript = (current == '^').then(|| script.clone());
                        let subscript = (current == '_').then_some(script);
                        MathNode::Scripts {
                            base: Box::new(other),
                            superscript,
                            subscript,
                        }
                    }
                };
                body.push(node);
                continue;
            }
            if current == '\\' && self.apply_limits_modifier(&mut body) {
                continue;
            }
            body.push(self.parse_atom()?);
        }
        Ok(body)
    }

    fn stop_matches(&self, stop: Stop) -> bool {
        match stop {
            Stop::None => false,
            Stop::At(value) => self.peek() == Some(value),
            Stop::RightDelimiter => self.at_right_delimiter(),
            Stop::MatrixCell => {
                self.peek() == Some('&')
                    || self.starts_with("\\\\")
                    || self.starts_with(&self.pending_stop_marker)
            }
        }
    }

    /// `parseAtom` (`parser.ts:141-153`): per-atom depth guard.
    fn parse_atom(&mut self) -> Result<MathNode, LatexParseError> {
        self.depth += 1;
        if self.depth > self.max_depth {
            self.depth -= 1;
            return self.fail(
                format!("LaTeX nesting exceeds the {}-level limit", self.max_depth),
                self.position,
            );
        }
        let result = self.parse_atom_inner();
        self.depth -= 1;
        result
    }

    fn parse_atom_inner(&mut self) -> Result<MathNode, LatexParseError> {
        let current = self.peek().unwrap();
        if current == '{' {
            return self.parse_group();
        }
        if current == '\\' {
            return self.parse_command();
        }
        if current == '~' {
            self.position += 1;
            return Ok(MathNode::Space(1));
        }
        self.position += 1;
        Ok(MathNode::symbol(&current.to_string(), infer_role(current)))
    }

    /// `parseCommand` (`parser.ts:155-367`), branch order preserved.
    fn parse_command(&mut self) -> Result<MathNode, LatexParseError> {
        let start = self.position;
        let command = self.read_command()?;

        if command == "\\" {
            return Ok(MathNode::Row(Vec::new()));
        }
        if command == "begin" {
            return self.parse_environment();
        }
        if matches!(command.as_str(), "frac" | "dfrac" | "tfrac" | "cfrac") {
            self.skip_math_whitespace();
            let mut numerator_align = None;
            if command == "cfrac" && self.peek() == Some('[') {
                match self.parse_cfrac_alignment() {
                    Some(align) => numerator_align = align,
                    None => {
                        return self.fail(
                            "Unsupported \\cfrac alignment; expected [l], [r], or []",
                            start,
                        );
                    }
                }
            }
            let numerator = Box::new(self.parse_argument()?);
            let denominator = Box::new(self.parse_argument()?);
            return Ok(MathNode::Fraction {
                numerator,
                denominator,
                bar: true,
                numerator_align,
            });
        }
        if matches!(command.as_str(), "binom" | "dbinom" | "tbinom") {
            let numerator = Box::new(self.parse_argument()?);
            let denominator = Box::new(self.parse_argument()?);
            return Ok(MathNode::Delimited {
                left: "(".to_owned(),
                body: Box::new(MathNode::Fraction {
                    numerator,
                    denominator,
                    bar: false,
                    numerator_align: None,
                }),
                right: ")".to_owned(),
            });
        }
        if command == "sqrt" {
            let index = self.parse_optional_argument()?;
            let body = Box::new(self.parse_argument()?);
            return Ok(MathNode::Root { body, index });
        }
        if command == "left" {
            return self.parse_left_right();
        }
        if command == "middle" {
            let value = self.read_delimiter()?;
            return Ok(MathNode::Symbol { value, role: None });
        }
        if command == "right" {
            self.position = start;
            return self.fail("Unexpected \\right", start);
        }
        if let Some(accent) = ACCENT_TABLE.get(&command).copied() {
            let body = Box::new(self.parse_argument()?);
            return Ok(MathNode::Accent { accent, body });
        }
        if let Some(variant) = VARIANT_TABLE.get(&command).copied() {
            let body = if command == "textrm" {
                Box::new(MathNode::Text(self.read_text_group()?))
            } else {
                Box::new(self.parse_argument()?)
            };
            return Ok(MathNode::Variant { variant, body });
        }
        if command == "text" || command == "mbox" {
            return Ok(MathNode::Text(self.read_text_group()?));
        }
        if command == "operatorname" {
            let limits = self.peek() == Some('*');
            if limits {
                self.position += 1;
            }
            return Ok(MathNode::Operator {
                value: self.read_text_group()?,
                limits,
            });
        }
        if command == "overset" || command == "stackrel" {
            let over = Box::new(self.parse_argument()?);
            let base = Box::new(self.parse_argument()?);
            return Ok(MathNode::OverUnder {
                base,
                over: Some(over),
                under: None,
            });
        }
        if command == "underset" {
            let under = Box::new(self.parse_argument()?);
            let base = Box::new(self.parse_argument()?);
            return Ok(MathNode::OverUnder {
                base,
                over: None,
                under: Some(under),
            });
        }
        if command == "overbrace" || command == "underbrace" {
            let body = Box::new(self.parse_argument()?);
            return Ok(MathNode::Brace {
                body,
                over: command == "overbrace",
            });
        }
        if command == "textcolor" {
            let color = self.read_raw_group()?;
            let body = Box::new(self.parse_argument()?);
            return Ok(MathNode::Color { color, body });
        }
        if command == "color" {
            let color = self.read_raw_group()?;
            let body = MathNode::wrap_row(self.parse_row(Stop::None)?);
            return Ok(MathNode::Color {
                color,
                body: Box::new(body),
            });
        }
        if command == "not" {
            let target = self.parse_atom()?;
            return Ok(match target {
                MathNode::Symbol { value, role } => MathNode::Symbol {
                    value: negate_symbol(
                        value.chars().next().unwrap_or(char::REPLACEMENT_CHARACTER),
                    ),
                    role,
                },
                other => MathNode::Row(vec![
                    MathNode::symbol("¬", SymbolRole::Ordinary),
                    other,
                ]),
            });
        }
        if command == "pmod" {
            let argument = self.parse_argument()?;
            return Ok(MathNode::Row(vec![
                MathNode::Space(1),
                MathNode::Text("(mod ".to_owned()),
                argument,
                MathNode::Text(")".to_owned()),
            ]));
        }
        if command == "mod" || command == "bmod" {
            return Ok(MathNode::Operator {
                value: "mod".to_owned(),
                limits: false,
            });
        }
        if command == "displaylines" {
            self.skip_math_whitespace();
            self.expect('{')?;
            return self.parse_matrix("gathered", "}", None);
        }
        if matches!(
            command.as_str(),
            "limits"
                | "nolimits"
                | "displaystyle"
                | "textstyle"
                | "scriptstyle"
                | "scriptscriptstyle"
        ) {
            return Ok(MathNode::Row(Vec::new()));
        }
        if is_sizing_delimiter_command(&command) {
            let value = self.read_delimiter()?;
            return Ok(MathNode::Symbol { value, role: None });
        }
        if let Some(width) = self.spacing(&command) {
            return Ok(MathNode::Space(width));
        }
        if let Some((value, role)) = SYMBOL_TABLE.get(&command).copied() {
            return Ok(MathNode::Symbol {
                value: value.to_string(),
                role: Some(role),
            });
        }
        if let Some(value) = LARGE_OPERATOR_TABLE.get(&command).copied() {
            return Ok(MathNode::Operator {
                value: value.to_string(),
                limits: !command.contains("int"),
            });
        }
        if NAMED_OPERATOR_SET.contains(&command) {
            return Ok(MathNode::Operator {
                value: command.clone(),
                limits: command.starts_with("lim") || command == "min" || command == "max",
            });
        }
        if command == "backslash" {
            return Ok(MathNode::symbol("\\", SymbolRole::Ordinary));
        }
        if let Some(value) = DELIMITER_TABLE.get(&command) {
            return Ok(MathNode::Symbol {
                value: value.clone(),
                role: None,
            });
        }
        if command == "{" || command == "}" {
            return Ok(MathNode::symbol(&command, SymbolRole::Ordinary));
        }
        if matches!(command.as_str(), "%" | "#" | "$" | "&" | "_") {
            return Ok(MathNode::symbol(&command, SymbolRole::Ordinary));
        }
        if self.strict {
            return self.fail(format!("Unsupported command \\{command}"), start);
        }
        Ok(MathNode::Text(format!("\\{command}")))
    }

    fn spacing(&self, command: &str) -> Option<usize> {
        SPACING_TABLE.get(command).copied()
    }

    /// `/^\[([lr]?)\]/` (`parser.ts:177`), `position` on the opening `[`.
    fn parse_cfrac_alignment(&mut self) -> Option<Option<NumeratorAlign>> {
        match self.chars.get(self.position + 1).copied() {
            Some(']') => {
                self.position += 2;
                Some(None)
            }
            Some('l') if self.chars.get(self.position + 2) == Some(&']') => {
                self.position += 3;
                Some(Some(NumeratorAlign::Left))
            }
            Some('r') if self.chars.get(self.position + 2) == Some(&']') => {
                self.position += 3;
                Some(Some(NumeratorAlign::Right))
            }
            _ => None,
        }
    }

    /// `parseEnvironment` (`parser.ts:369-395`).
    fn parse_environment(&mut self) -> Result<MathNode, LatexParseError> {
        let raw = self.read_raw_group()?;
        let unstarred = raw.strip_suffix('*').unwrap_or(&raw);
        let Some(environment) = MATRIX_ENVIRONMENTS
            .iter()
            .copied()
            .find(|name| *name == unstarred)
        else {
            if self.strict {
                return self.fail(
                    format!("Unsupported environment {unstarred}"),
                    self.position,
                );
            }
            return Ok(MathNode::Text(self.read_until_end(&raw)?));
        };
        let columns = if environment == "array" {
            let columns: String = self
                .read_raw_group()?
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            if columns.is_empty()
                || !columns.chars().all(|ch| "lcr|".contains(ch))
                || !columns.contains(['l', 'c', 'r'])
            {
                return self.fail(
                    "Unsupported array columns; expected l, c, r, and |",
                    self.position,
                );
            }
            Some(columns)
        } else {
            None
        };
        self.parse_matrix(environment, &format!("\\end{{{raw}}}"), columns)
    }

    /// `parseMatrix` (`parser.ts:397-465`).
    fn parse_matrix(
        &mut self,
        environment: &'static str,
        end: &str,
        columns: Option<String>,
    ) -> Result<MathNode, LatexParseError> {
        let mut rows: Vec<Vec<MathNode>> = Vec::new();
        let mut cells: Vec<MathNode> = Vec::new();
        let saved_marker = std::mem::replace(&mut self.pending_stop_marker, end.to_owned());
        loop {
            self.skip_math_whitespace();
            if self.done() {
                break;
            }
            if self.starts_with(end) && cells.is_empty() {
                break;
            }
            let cell_start = self.position;
            cells.push(MathNode::wrap_row(self.parse_row(Stop::MatrixCell)?));
            self.skip_math_whitespace();

            if self.peek() == Some('&') {
                self.position += 1;
                continue;
            }
            if self.starts_with("\\\\") {
                self.position += 2;
                self.consume_optional_bracket();
                rows.push(std::mem::take(&mut cells));
                continue;
            }
            if self.starts_with(end) {
                break;
            }
            // Empty cells are valid only when a cell, row, or closing
            // delimiter advances the parser (`parser.ts:459-461`).
            if self.position == cell_start {
                self.pending_stop_marker = saved_marker;
                return self.fail(
                    format!("Unexpected \"{}\" in {environment}", self.peek().unwrap_or('?')),
                    self.position,
                );
            }
        }
        self.pending_stop_marker = saved_marker;
        if !self.starts_with(end) {
            return self.fail(format!("Missing {end}"), self.position);
        }
        self.expect_str(end)?;
        if !cells.is_empty() || rows.is_empty() {
            rows.push(cells);
        }
        Ok(MathNode::Matrix {
            rows,
            environment,
            columns,
        })
    }

    /// `parseLeftRight` (`parser.ts:467-476`).
    fn parse_left_right(&mut self) -> Result<MathNode, LatexParseError> {
        let left = self.read_delimiter()?;
        let body = MathNode::wrap_row(self.parse_row(Stop::RightDelimiter)?);
        if !self.at_right_delimiter() {
            return self.fail("Missing \\right", self.position);
        }
        self.read_command()?;
        let right = self.read_delimiter()?;
        Ok(MathNode::Delimited {
            left,
            body: Box::new(body),
            right,
        })
    }

    /// `parseArgument` (`parser.ts:478-483`).
    fn parse_argument(&mut self) -> Result<MathNode, LatexParseError> {
        self.skip_math_whitespace();
        if self.peek() == Some('{') {
            return self.parse_group();
        }
        if self.done() {
            return self.fail("Expected an argument", self.position);
        }
        self.parse_atom()
    }

    /// `parseGroup` (`parser.ts:485-489`).
    fn parse_group(&mut self) -> Result<MathNode, LatexParseError> {
        self.expect('{')?;
        let body = MathNode::wrap_row(self.parse_row(Stop::None)?);
        self.expect('}')?;
        Ok(body)
    }

    /// `parseOptionalArgument` (`parser.ts:491-500`).
    fn parse_optional_argument(&mut self) -> Result<Option<Box<MathNode>>, LatexParseError> {
        self.skip_math_whitespace();
        if self.peek() != Some('[') {
            return Ok(None);
        }
        self.position += 1;
        let body = MathNode::wrap_row(self.parse_row(Stop::At(']'))?);
        self.expect(']')?;
        Ok(Some(Box::new(body)))
    }

    /// `consumeOptionalBracket` (`parser.ts:502-517`).
    fn consume_optional_bracket(&mut self) {
        self.skip_math_whitespace();
        if self.peek() != Some('[') {
            return;
        }
        self.position += 1;
        let mut depth = 0usize;
        while !self.done() {
            let ch = self.chars[self.position];
            self.position += 1;
            if ch == '[' {
                depth += 1;
            }
            if ch == ']' {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return;
                }
            }
        }
    }

    /// `readDelimiter` (`parser.ts:519-536`).
    fn read_delimiter(&mut self) -> Result<String, LatexParseError> {
        self.skip_math_whitespace();
        if self.done() {
            return self.fail("Expected a delimiter", self.position);
        }
        let start = self.position;
        if self.peek() == Some('\\') {
            let command = self.read_command()?;
            let delimiter = DELIMITER_TABLE
                .get(&format!("\\{command}"))
                .or_else(|| DELIMITER_TABLE.get(&command));
            if let Some(value) = delimiter {
                return Ok(value.clone());
            }
            if self.strict {
                return self.fail(format!("Unsupported delimiter \\{command}"), start);
            }
            return Ok(format!("\\{command}"));
        }
        let token = self.chars[self.position];
        self.position += 1;
        let token_string = token.to_string();
        Ok(DELIMITER_TABLE
            .get(&token_string)
            .cloned()
            .unwrap_or(token_string))
    }

    /// `readCommand` (`parser.ts:538-553`).
    fn read_command(&mut self) -> Result<String, LatexParseError> {
        self.expect('\\')?;
        if self.done() {
            return Ok("\\".to_owned());
        }
        let next = self.peek().unwrap();
        if !is_command_letter(next) {
            self.position += 1;
            return Ok(next.to_string());
        }
        let start = self.position;
        while !self.done() && is_command_letter(self.peek().unwrap()) {
            self.position += 1;
        }
        let command: String = self.chars[start..self.position].iter().collect();
        if self.peek() == Some(' ') {
            self.position += 1;
        }
        Ok(command)
    }

    /// `readRawGroup` (`parser.ts:555-571`).
    fn read_raw_group(&mut self) -> Result<String, LatexParseError> {
        self.skip_math_whitespace();
        self.expect('{')?;
        let start = self.position;
        let mut depth = 1usize;
        while !self.done() {
            let index = self.position;
            let ch = self.chars[index];
            self.position += 1;
            let escaped = (ch == '{' || ch == '}') && self.is_escaped(index);
            if ch == '{' && !escaped {
                depth += 1;
            }
            if ch == '}' && !escaped {
                depth -= 1;
                if depth == 0 {
                    return Ok(self.chars[start..index].iter().collect());
                }
            }
        }
        self.fail("Unterminated group", start)
    }

    /// `applyLimitsModifier` (`parser.ts:573-589`).
    fn apply_limits_modifier(&mut self, body: &mut Vec<MathNode>) -> bool {
        let tail: String = self.chars[self.position..].iter().collect();
        let matched = ["limits", "nolimits"].into_iter().find(|name| {
            let token = format!("\\{name}");
            tail.starts_with(&token)
                && tail
                    .get(token.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|next| !is_command_letter(next))
        });
        let Some(name) = matched else {
            return false;
        };
        self.position += 1 + name.len();

        let limits = name == "limits";
        match body.last_mut() {
            Some(MathNode::Operator { limits: flag, .. }) => *flag = limits,
            Some(MathNode::Scripts { base, .. }) => {
                if let MathNode::Operator { limits: flag, .. } = base.as_mut() {
                    *flag = limits;
                }
            }
            _ => {}
        }
        true
    }

    /// `isEscaped` (`parser.ts:591-598`).
    fn is_escaped(&self, index: usize) -> bool {
        let mut slash_count = 0usize;
        let mut cursor = index;
        while cursor > 0 && self.chars[cursor - 1] == '\\' {
            slash_count += 1;
            cursor -= 1;
        }
        slash_count % 2 == 1
    }

    /// `readTextGroup` (`parser.ts:600-613`).
    fn read_text_group(&mut self) -> Result<String, LatexParseError> {
        let raw = self.read_raw_group()?;
        let chars: Vec<char> = raw.chars().collect();
        let mut output = String::new();
        let mut index = 0usize;
        while index < chars.len() {
            if chars[index] == '\\' && index + 1 < chars.len() {
                let mut end = index + 1;
                if is_command_letter(chars[end]) {
                    while end < chars.len() && is_command_letter(chars[end]) {
                        end += 1;
                    }
                } else {
                    end += 1;
                }
                let command: String = chars[index + 1..end].iter().collect();
                let original: String = chars[index..end].iter().collect();
                output.push_str(&text_command_replacement(&command, &original));
                index = end;
                continue;
            }
            output.push(chars[index]);
            index += 1;
        }
        Ok(output.replace('~', " "))
    }

    /// `readUntilEnd` (`parser.ts:615-623`).
    fn read_until_end(&mut self, environment: &str) -> Result<String, LatexParseError> {
        let marker: Vec<char> = format!("\\end{{{environment}}}").chars().collect();
        let mut index = self.position;
        while index + marker.len() <= self.chars.len() {
            if self.chars[index..index + marker.len()] == marker[..] {
                let content: String = self.chars[self.position..index].iter().collect();
                self.position = index + marker.len();
                return Ok(content);
            }
            index += 1;
        }
        self.fail(
            format!("Missing \\end{{{environment}}}"),
            self.position,
        )
    }

    /// `skipMathWhitespace` (`parser.ts:625-640`).
    fn skip_math_whitespace(&mut self) {
        while !self.done() {
            let current = self.peek().unwrap();
            if current.is_whitespace() {
                self.position += 1;
                continue;
            }
            if current == '%' {
                while !self.done() && self.peek() != Some('\n') {
                    self.position += 1;
                }
                continue;
            }
            break;
        }
    }

    fn expect(&mut self, value: char) -> Result<(), LatexParseError> {
        if self.peek() == Some(value) {
            self.position += 1;
            Ok(())
        } else {
            self.fail(format!("Expected \"{value}\""), self.position)
        }
    }

    fn expect_str(&mut self, value: &str) -> Result<(), LatexParseError> {
        if self.starts_with(value) {
            self.position += value.chars().count();
            Ok(())
        } else {
            self.fail(format!("Expected \"{value}\""), self.position)
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn done(&self) -> bool {
        self.position >= self.chars.len()
    }

    fn starts_with(&self, value: &str) -> bool {
        let mut offset = self.position;
        for expected in value.chars() {
            if self.chars.get(offset).copied() != Some(expected) {
                return false;
            }
            offset += 1;
        }
        true
    }

    fn at_right_delimiter(&self) -> bool {
        self.starts_with("\\right")
            && self
                .chars
                .get(self.position + 6)
                .is_none_or(|next| !is_command_letter(*next))
    }

    fn fail<T>(&self, message: impl Into<String>, position: usize) -> Result<T, LatexParseError> {
        Err(LatexParseError {
            message: message.into(),
            position,
        })
    }
}

/// `readTextGroup` replacement table (`parser.ts:602-610`).
fn text_command_replacement(command: &str, original: &str) -> String {
    if command.len() == 1 && "{}%#$&_ ".contains(command) {
        return command.to_owned();
    }
    match command {
        "textbackslash" => "\\".to_owned(),
        "!" => String::new(),
        other => match SPACING_TABLE.get(other).copied() {
            Some(width) => " ".repeat(width.max(1)),
            None => original.to_owned(),
        },
    }
}

/// `/^(?:big|Big|bigg|Bigg)[lrm]?$/` (`parser.ts:331-333`).
fn is_sizing_delimiter_command(command: &str) -> bool {
    for prefix in ["big", "Big", "bigg", "Bigg"] {
        if let Some(rest) = command.strip_prefix(prefix) {
            if rest.is_empty() || matches!(rest, "l" | "r" | "m") {
                return true;
            }
        }
    }
    false
}

// ── Layout (`packages/latex/src/layout.ts`) ───────────────────────────

/// Internal box (`layout.ts:7-12`).
struct Boxed {
    width: usize,
    height: usize,
    baseline: usize,
    cells: Vec<Vec<Option<MathCell>>>,
}

#[derive(Clone, Debug)]
struct LayoutContext {
    display_mode: bool,
    compact_scripts: bool,
    style: Option<MathStyle>,
    variant: Option<Variant>,
}

impl LayoutContext {
    fn new(display_mode: bool) -> Self {
        Self {
            display_mode,
            compact_scripts: true,
            style: None,
            variant: None,
        }
    }
}

/// `layoutMath` (`layout.ts:64-77`).
fn layout_math(node: MathNode, display_mode: bool) -> Boxed {
    let context = LayoutContext::new(display_mode);
    layout_node(&node, &context)
}

fn layout_node(node: &MathNode, context: &LayoutContext) -> Boxed {
    match node {
        MathNode::Row(body) => layout_row(body, context),
        // Upstream cases "symbol" | "text" | "operator" (`layout.ts:82-84`).
        MathNode::Symbol { value, .. }
        | MathNode::Operator { value, .. }
        | MathNode::Text(value) => {
            text_box(&apply_variant(value, context.variant), context.style.clone())
        }
        MathNode::Space(width) => blank(*width, 1, 0),
        MathNode::Fraction {
            numerator,
            denominator,
            bar,
            numerator_align,
        } => layout_fraction(numerator, denominator, *bar, *numerator_align, context),
        MathNode::Root { body, index } => layout_root(body, index.as_deref(), context),
        MathNode::Scripts {
            base,
            superscript,
            subscript,
        } => layout_scripts(base, superscript.as_deref(), subscript.as_deref(), context),
        MathNode::Delimited { left, body, right } => {
            layout_delimited(left, body, right, context)
        }
        MathNode::Matrix {
            rows,
            environment,
            columns,
        } => layout_matrix(rows, environment, columns.as_deref(), context),
        MathNode::Brace { body, over } => layout_brace(body, *over, context),
        MathNode::Accent { accent, body } => layout_accent(accent, body, context),
        MathNode::Variant { variant, body } => {
            let modified = with_variant(context, *variant);
            layout_node(body, &modified)
        }
        MathNode::OverUnder { base, over, under } => {
            layout_over_under(base, over.as_deref(), under.as_deref(), context)
        }
        MathNode::Color { color, body } => {
            let mut style = context.style.clone().unwrap_or_default();
            style.color = Some(color.clone());
            let modified = LayoutContext {
                display_mode: context.display_mode,
                compact_scripts: context.compact_scripts,
                style: Some(style),
                variant: context.variant,
            };
            layout_node(body, &modified)
        }
    }
}

fn layout_row(nodes: &[MathNode], context: &LayoutContext) -> Boxed {
    if nodes.is_empty() {
        return blank(0, 1, 0);
    }

    let mut laid_out: Vec<Boxed> = Vec::new();
    let mut previous_role: Option<SymbolRole> = None;

    for (index, node) in nodes.iter().enumerate() {
        let raw_role = node_role(node);
        let role = normalize_binary_role(
            raw_role,
            previous_role,
            next_significant_role(nodes, index + 1),
        );
        if needs_math_space(previous_role, role, laid_out.len()) {
            laid_out.push(blank(1, 1, 0));
        }
        laid_out.push(layout_node(node, context));
        if !matches!(node, MathNode::Space(_)) {
            previous_role = Some(role.unwrap_or(SymbolRole::Ordinary));
        }
    }

    hpack(laid_out)
}

fn layout_fraction(
    numerator_node: &MathNode,
    denominator_node: &MathNode,
    bar: bool,
    numerator_align: Option<NumeratorAlign>,
    context: &LayoutContext,
) -> Boxed {
    let numerator = layout_node(numerator_node, context);
    let denominator = layout_node(denominator_node, context);
    let width = numerator.width.max(denominator.width) + 2;
    // Barless fractions (binomials) still reserve an axis row so surrounding
    // atoms align between the two entries (`layout.ts:91-95`).
    let gap = 1usize;
    let height = numerator.height + denominator.height + gap;
    // TeX places a fraction's math axis on its rule (`layout.ts:96-98`).
    let baseline = numerator.height;
    let mut result = blank(width, height, baseline);

    let numerator_x = match numerator_align {
        Some(NumeratorAlign::Left) => 1,
        Some(NumeratorAlign::Right) => width - numerator.width - 1,
        None => (width - numerator.width) / 2,
    };
    overlay(&mut result, &numerator, numerator_x, 0);
    if bar {
        draw_horizontal(&mut result, numerator.height, 0, width, "─", context.style.clone());
    }
    overlay(
        &mut result,
        &denominator,
        (width - denominator.width) / 2,
        numerator.height + gap,
    );
    result
}

fn layout_root(
    body_node: &MathNode,
    index_node: Option<&MathNode>,
    context: &LayoutContext,
) -> Boxed {
    let body = layout_node(body_node, context);
    let index = index_node.map(|node| layout_node(node, context));
    // The index ends beside the overbar, never inside the hook or radicand
    // (`layout.ts:113-118`).
    let index_width = index.as_ref().map_or(0, |b| b.width.saturating_sub(1));
    let top = index.as_ref().map_or(0, |b| b.height.saturating_sub(1));
    let body_x = index_width + 2;
    let width = body_x + body.width;
    let height = top + body.height + 1;
    let baseline = top + body.baseline + 1;
    let mut result = blank(width, height, baseline);

    set_cell(&mut result, body_x - 1, top, "╭", context.style.clone());
    draw_horizontal(&mut result, top, body_x, body.width, "─", context.style.clone());
    for y in top + 1..height - 1 {
        set_cell(&mut result, body_x - 1, y, "│", context.style.clone());
    }
    set_cell(&mut result, body_x - 2, height - 1, "╰", context.style.clone());
    set_cell(&mut result, body_x - 1, height - 1, "╯", context.style.clone());
    overlay(&mut result, &body, body_x, top + 1);
    if let Some(index) = index {
        overlay(&mut result, &index, 0, 0);
    }
    result
}

fn layout_scripts(
    base_node: &MathNode,
    superscript_node: Option<&MathNode>,
    subscript_node: Option<&MathNode>,
    context: &LayoutContext,
) -> Boxed {
    // Empty scripts (`x^{}` / `x_{}`) do not layout (`layout.ts:159-161`).
    let superscript_node = superscript_node.filter(|node| simple_node_text(node) != Some(String::new()));
    let subscript_node = subscript_node.filter(|node| simple_node_text(node) != Some(String::new()));
    match base_node {
        MathNode::Brace { .. } => {
            return layout_over_under(base_node, superscript_node, subscript_node, context);
        }
        MathNode::Operator { limits: true, .. } if context.display_mode => {
            return layout_over_under(base_node, superscript_node, subscript_node, context);
        }
        _ => {}
    }

    let base = layout_node(base_node, context);
    if context.compact_scripts && base.height == 1 {
        let superscript = map_script_text(superscript_node, &SUPERSCRIPT_TABLE);
        let subscript = map_script_text(subscript_node, &SUBSCRIPT_TABLE);
        if let (Some(superscript), Some(subscript)) = (superscript, subscript) {
            let combined = format!("{superscript}{subscript}");
            return hpack(vec![base, text_box(&combined, context.style.clone())]);
        }
    }

    let superscript = superscript_node.map(|node| layout_node(node, context));
    let subscript = subscript_node.map(|node| layout_node(node, context));
    let script_width = superscript
        .as_ref()
        .map_or(subscript.as_ref().map_or(0, |b| b.width), |b| b.width)
        .max(subscript.as_ref().map_or(0, |b| b.width));
    let top_height = superscript.as_ref().map_or(0, |b| b.height);
    let bottom_height = subscript.as_ref().map_or(0, |b| b.height);
    let width = base.width + script_width;
    let height = top_height + base.height + bottom_height;
    let baseline = top_height + base.baseline;
    let mut result = blank(width, height, baseline);

    overlay(&mut result, &base, 0, top_height);
    if let Some(superscript) = superscript {
        overlay(&mut result, &superscript, base.width, 0);
    }
    if let Some(subscript) = subscript {
        overlay(&mut result, &subscript, base.width, top_height + base.height);
    }
    result
}

fn layout_over_under(
    base_node: &MathNode,
    over_node: Option<&MathNode>,
    under_node: Option<&MathNode>,
    context: &LayoutContext,
) -> Boxed {
    let base = layout_node(base_node, context);
    let over = over_node.map(|node| layout_node(node, context));
    let under = under_node.map(|node| layout_node(node, context));
    let width = base
        .width
        .max(over.as_ref().map_or(0, |b| b.width))
        .max(under.as_ref().map_or(0, |b| b.width));
    let over_height = over.as_ref().map_or(0, |b| b.height);
    let height = over_height + base.height + under.as_ref().map_or(0, |b| b.height);
    let baseline = over_height + base.baseline;
    let mut result = blank(width, height, baseline);

    if let Some(over) = over {
        overlay(&mut result, &over, (width - over.width) / 2, 0);
    }
    overlay(&mut result, &base, (width - base.width) / 2, over_height);
    if let Some(under) = under {
        overlay(
            &mut result,
            &under,
            (width - under.width) / 2,
            over_height + base.height,
        );
    }
    result
}

fn layout_delimited(
    left: &str,
    body_node: &MathNode,
    right: &str,
    context: &LayoutContext,
) -> Boxed {
    let body = layout_node(body_node, context);
    let left_box = delimiter_box(left, body.height, body.baseline, true, context.style.clone());
    let right_box = delimiter_box(right, body.height, body.baseline, false, context.style.clone());
    hpack(vec![left_box, body, right_box])
}

fn layout_matrix(
    rows: &[Vec<MathNode>],
    environment: &str,
    columns: Option<&str>,
    context: &LayoutContext,
) -> Boxed {
    let cell_rows: Vec<Vec<Boxed>> = rows
        .iter()
        .map(|row| row.iter().map(|cell| layout_node(cell, context)).collect())
        .collect();
    let column_chars: Vec<char> = columns
        .map(|columns| {
            columns
                .chars()
                .filter(|ch| matches!(ch, 'l' | 'c' | 'r'))
                .collect()
        })
        .unwrap_or_default();
    // `columns.split(/[lcr]/)` run lengths (`layout.ts:279-280`).
    let rules: Vec<usize> = columns
        .map(|columns| {
            columns
                .split(['l', 'c', 'r'])
                .map(|run| run.chars().count())
                .collect()
        })
        .unwrap_or_default();
    let column_count = cell_rows
        .iter()
        .map(|row| row.len())
        .chain(std::iter::once(column_chars.len()))
        .max()
        .unwrap_or(0);
    let column_widths: Vec<usize> = (0..column_count)
        .map(|column| {
            cell_rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.width)
                .max()
                .unwrap_or(0)
        })
        .collect();
    let row_ascents: Vec<usize> = cell_rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.baseline).max().unwrap_or(0))
        .collect();
    let row_descents: Vec<usize> = cell_rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.height.saturating_sub(cell.baseline + 1))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let row_heights: Vec<usize> = row_ascents
        .iter()
        .zip(row_descents.iter())
        .map(|(ascent, descent)| ascent + 1 + descent)
        .collect();
    let aligned = environment == "aligned" || environment == "align";
    let column_gap = if environment == "cases" || aligned { 2 } else { 1 };
    let gaps: Vec<usize> = (0..=column_count)
        .map(|boundary| {
            let edge = boundary == 0 || boundary == column_count;
            let rule = rules.get(boundary).copied().unwrap_or(0);
            if rule > 0 {
                rule + (if edge { 1 } else { 2 })
            } else if edge {
                0
            } else {
                column_gap
            }
        })
        .collect();
    let width = column_widths.iter().sum::<usize>() + gaps.iter().sum::<usize>();
    let height = (row_heights.iter().sum::<usize>() + rows.len().saturating_sub(1)).max(1);
    let mut result = blank(width, height, height / 2);
    let mut y = 0usize;

    for (row_index, cells) in cell_rows.iter().enumerate() {
        let mut x = gaps.first().copied().unwrap_or(0);
        for column in 0..column_count {
            let column_width = column_widths[column];
            if let Some(cell) = cells.get(column) {
                let alignment = column_chars
                    .get(column)
                    .copied()
                    .unwrap_or(
                        if environment == "cases" {
                            'l'
                        } else if aligned {
                            if column % 2 == 0 {
                                'r'
                            } else {
                                'l'
                            }
                        } else {
                            'c'
                        },
                    );
                let cell_x = x
                    + match alignment {
                        'l' => 0,
                        'r' => column_width - cell.width,
                        _ => (column_width - cell.width) / 2,
                    };
                let cell_y = y + row_ascents[row_index] - cell.baseline;
                overlay(&mut result, cell, cell_x, cell_y);
            }
            x += column_width + gaps.get(column + 1).copied().unwrap_or(0);
        }
        y += row_heights[row_index] + 1;
    }

    let mut boundary_x = 0usize;
    for boundary in 0..=column_count {
        for rule in 0..rules.get(boundary).copied().unwrap_or(0) {
            for row in 0..height {
                let column = boundary_x + usize::from(boundary != 0) + rule;
                set_cell(&mut result, column, row, "│", context.style.clone());
            }
        }
        boundary_x += gaps.get(boundary).copied().unwrap_or(0)
            + column_widths.get(boundary).copied().unwrap_or(0);
    }

    let baseline = result.baseline;
    match matrix_delimiters(environment) {
        Some((left, right)) => hpack(vec![
            delimiter_box(left, height, baseline, true, context.style.clone()),
            result,
            delimiter_box(right, height, baseline, false, context.style.clone()),
        ]),
        None => result,
    }
}

fn layout_brace(body_node: &MathNode, over: bool, context: &LayoutContext) -> Boxed {
    let body = layout_node(body_node, context);
    let width = body.width.max(3);
    let baseline = if over { body.baseline + 1 } else { body.baseline };
    let mut result = blank(width, body.height + 1, baseline);
    let y = if over { 0 } else { body.height };
    overlay(
        &mut result,
        &body,
        (width - body.width) / 2,
        usize::from(over),
    );
    draw_horizontal(&mut result, y, 0, width, "─", context.style.clone());
    set_cell(&mut result, 0, y, if over { "╭" } else { "╰" }, context.style.clone());
    set_cell(
        &mut result,
        width - 1,
        y,
        if over { "╮" } else { "╯" },
        context.style.clone(),
    );
    set_cell(
        &mut result,
        (width - 1) / 2,
        y,
        if over { "┴" } else { "┬" },
        context.style.clone(),
    );
    result
}

fn layout_accent(accent: &str, body_node: &MathNode, context: &LayoutContext) -> Boxed {
    let body = layout_node(body_node, context);
    if accent == "underline" {
        let mut result = blank(body.width, body.height + 1, body.baseline);
        overlay(&mut result, &body, 0, 0);
        draw_horizontal(&mut result, body.height, 0, body.width, "─", context.style.clone());
        return result;
    }

    let baseline = body.baseline + 1;
    let mut result = blank(body.width, body.height + 1, baseline);
    overlay(&mut result, &body, 0, 1);
    let mark: &str = match accent {
        "hat" if body.width == 1 => "^",
        "hat" => "⌢",
        "bar" | "overline" => "─",
        "vec" => "→",
        "tilde" => "~",
        "dot" => "·",
        _ => "¨",
    };

    if accent == "bar" || accent == "overline" {
        draw_horizontal(&mut result, 0, 0, body.width, mark, context.style.clone());
    } else {
        let mark_width = mark.chars().map(cell_width).sum::<usize>();
        let x = body.width.saturating_sub(mark_width) / 2;
        set_cell(&mut result, x, 0, mark, context.style.clone());
    }
    result
}

fn delimiter_box(
    delimiter: &str,
    height: usize,
    baseline: usize,
    left: bool,
    style: Option<MathStyle>,
) -> Boxed {
    if delimiter.is_empty() {
        return blank(0, height, baseline);
    }
    if height <= 1 {
        return text_box(delimiter, style);
    }
    let glyphs = delimiter_glyphs(delimiter);
    let width = glyphs
        .iter()
        .map(|glyph| glyph.chars().map(cell_width).sum::<usize>())
        .max()
        .unwrap_or(1);
    let mut result = blank(width, height, baseline);
    for y in 0..height {
        let glyph = if y == 0 {
            glyphs[0]
        } else if y == height - 1 {
            glyphs[2]
        } else {
            glyphs[1]
        };
        set_cell(&mut result, 0, y, glyph, style.clone());
    }
    if matches!(delimiter, "{" | "}") && height >= 3 {
        let glyph = if left { "⎨" } else { "⎬" };
        set_cell(&mut result, 0, height / 2, glyph, style);
    }
    result
}

/// `delimiterGlyphs` (`layout.ts:452-481`).
fn delimiter_glyphs(delimiter: &str) -> [&'static str; 3] {
    match delimiter {
        "(" => ["⎛", "⎜", "⎝"],
        ")" => ["⎞", "⎟", "⎠"],
        "[" => ["⎡", "⎢", "⎣"],
        "]" => ["⎤", "⎥", "⎦"],
        "{" => ["⎧", "⎪", "⎩"],
        "}" => ["⎫", "⎪", "⎭"],
        "⌊" => ["│", "│", "⌊"],
        "⌋" => ["│", "│", "⌋"],
        "⌈" => ["⌈", "│", "│"],
        "⌉" => ["⌉", "│", "│"],
        "⟨" => ["/", "│", "\\"],
        "⟩" => ["\\", "│", "/"],
        other => {
            let leaked: &'static str = Box::leak(other.to_owned().into_boxed_str());
            [leaked, leaked, leaked]
        }
    }
}

/// `matrixDelimiters` (`layout.ts:483-503`).
fn matrix_delimiters(environment: &str) -> Option<(&'static str, &'static str)> {
    match environment {
        "pmatrix" => Some(("(", ")")),
        "bmatrix" => Some(("[", "]")),
        "Bmatrix" => Some(("{", "}")),
        "vmatrix" => Some(("│", "│")),
        "Vmatrix" => Some(("║", "║")),
        "cases" => Some(("{", "")),
        _ => None,
    }
}

fn hpack(boxes: Vec<Boxed>) -> Boxed {
    if boxes.is_empty() {
        return blank(0, 1, 0);
    }
    let ascent = boxes.iter().map(|b| b.baseline).max().unwrap_or(0);
    let descent = boxes
        .iter()
        .map(|b| b.height.saturating_sub(b.baseline + 1))
        .max()
        .unwrap_or(0);
    let width = boxes.iter().map(|b| b.width).sum();
    let mut result = blank(width, ascent + descent + 1, ascent);
    let mut x = 0usize;
    for item in &boxes {
        overlay(&mut result, item, x, ascent - item.baseline);
        x += item.width;
    }
    result
}

/// `textBox` (`layout.ts:660-673`): grapheme clusters, one cell each.
fn text_box(text: &str, style: Option<MathStyle>) -> Boxed {
    let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(text, true).collect();
    let width = graphemes
        .iter()
        .map(|grapheme| grapheme.chars().map(cell_width).sum::<usize>())
        .sum();
    let mut result = blank(width, 1, 0);
    let mut x = 0usize;
    for grapheme in graphemes {
        set_cell(&mut result, x, 0, grapheme, style.clone());
        let first = grapheme.chars().next().unwrap_or(' ');
        x += cell_width(first);
    }
    result
}

fn blank(width: usize, height: usize, baseline: usize) -> Boxed {
    Boxed {
        width,
        height: height.max(1),
        baseline,
        cells: vec![vec![None; width]; height.max(1)],
    }
}

fn overlay(target: &mut Boxed, source: &Boxed, x: usize, y: usize) {
    for source_y in 0..source.height {
        for source_x in 0..source.width {
            if let Some(Some(cell)) = source
                .cells
                .get(source_y)
                .and_then(|row| row.get(source_x))
            {
                set_cell(target, x + source_x, y + source_y, &cell.ch.to_string(), cell.style.clone());
            }
        }
    }
}

fn draw_horizontal(
    target: &mut Boxed,
    y: usize,
    x: usize,
    width: usize,
    ch: &str,
    style: Option<MathStyle>,
) {
    for offset in 0..width {
        set_cell(target, x + offset, y, ch, style.clone());
    }
}

fn set_cell(target: &mut Boxed, x: usize, y: usize, glyph: &str, style: Option<MathStyle>) {
    if x >= target.width || y >= target.height {
        return;
    }
    // Multi-char grapheme clusters keep their base char (see module note);
    // cells are one column wide, matching `cellWidth` bookkeeping.
    let ch = glyph.chars().next().unwrap_or(' ');
    // Upstream wraps every set with a truthy (possibly empty) style object;
    // an all-default style renders identically to none, so normalize.
    let style = style.filter(|style| *style != MathStyle::default());
    target.cells[y][x] = Some(MathCell { ch, style });
}

/// `nodeRole` (`layout.ts:531-544`).
fn node_role(node: &MathNode) -> Option<SymbolRole> {
    match node {
        MathNode::Symbol { role, .. } => *role,
        MathNode::Operator { .. } => Some(SymbolRole::Operator),
        // Tall constructs get an operator's terminal-cell side bearing
        // (`layout.ts:537-541`).
        MathNode::Fraction { .. } | MathNode::Root { .. } | MathNode::Matrix { .. } => {
            Some(SymbolRole::Operator)
        }
        MathNode::Scripts { base, .. } => node_role(base),
        _ => None,
    }
}

/// `needsMathSpace` (`layout.ts:546-560`).
fn needs_math_space(
    previous: Option<SymbolRole>,
    current: Option<SymbolRole>,
    count: usize,
) -> bool {
    if count == 0 {
        return false;
    }
    if matches!(
        previous,
        Some(SymbolRole::Punctuation) | Some(SymbolRole::Opening)
    ) || matches!(
        current,
        Some(SymbolRole::Punctuation) | Some(SymbolRole::Closing)
    ) {
        return false;
    }
    matches!(
        previous,
        Some(SymbolRole::Binary) | Some(SymbolRole::Relation) | Some(SymbolRole::Operator)
    ) || matches!(
        current,
        Some(SymbolRole::Binary) | Some(SymbolRole::Relation) | Some(SymbolRole::Operator)
    )
}

/// `normalizeBinaryRole` (`layout.ts:562-584`).
fn normalize_binary_role(
    role: Option<SymbolRole>,
    previous: Option<SymbolRole>,
    next: Option<SymbolRole>,
) -> Option<SymbolRole> {
    if role != Some(SymbolRole::Binary) {
        return role;
    }
    if matches!(
        previous,
        None
            | Some(SymbolRole::Binary)
            | Some(SymbolRole::Relation)
            | Some(SymbolRole::Operator)
            | Some(SymbolRole::Punctuation)
            | Some(SymbolRole::Opening)
    ) || matches!(
        next,
        None
            | Some(SymbolRole::Binary)
            | Some(SymbolRole::Relation)
            | Some(SymbolRole::Punctuation)
            | Some(SymbolRole::Closing)
    ) {
        return Some(SymbolRole::Ordinary);
    }
    role
}

/// `nextSignificantRole` (`layout.ts:586-596`).
fn next_significant_role(nodes: &[MathNode], start: usize) -> Option<SymbolRole> {
    for node in &nodes[start..] {
        if matches!(node, MathNode::Space(_)) {
            continue;
        }
        return Some(node_role(node).unwrap_or(SymbolRole::Ordinary));
    }
    None
}

/// `simpleNodeText` (`layout.ts:598-609`).
fn simple_node_text(node: &MathNode) -> Option<String> {
    match node {
        MathNode::Symbol { value, .. }
        | MathNode::Text(value)
        | MathNode::Operator { value, .. } => Some(value.clone()),
        MathNode::Row(body) => {
            let mut joined = String::new();
            for item in body {
                joined.push_str(&simple_node_text(item)?);
            }
            Some(joined)
        }
        _ => None,
    }
}

/// `mapScript` (`layout.ts:611-621`): an absent script maps to the empty
/// string; non-simple or unmappable scripts give [`None`].
fn map_script_text(
    node: Option<&MathNode>,
    table: &LazyLock<HashMap<char, char>>,
) -> Option<String> {
    let text = match node {
        None => String::new(),
        Some(node) => simple_node_text(node)?,
    };
    let mut result = String::new();
    for ch in text.chars() {
        let mapped = table.get(&ch).copied()?;
        result.push(mapped);
    }
    Some(result)
}

/// `withVariant` (`layout.ts:623-630`): bold/italic merge over the current
/// style; other variants keep the existing style entries.
fn with_variant(context: &LayoutContext, variant: Variant) -> LayoutContext {
    let mut style = context.style.clone().unwrap_or_default();
    if variant == Variant::Bold {
        style.bold = true;
    }
    if variant == Variant::Italic {
        style.italic = true;
    }
    LayoutContext {
        display_mode: context.display_mode,
        compact_scripts: context.compact_scripts,
        style: Some(style),
        variant: Some(variant),
    }
}

/// Per-char variant exceptions (`layout.ts:610-636`).
fn variant_exception(variant: &Variant, ch: char) -> Option<&'static str> {
    match variant {
        Variant::DoubleStruck => match ch {
            'C' => Some("ℂ"),
            'H' => Some("ℍ"),
            'N' => Some("ℕ"),
            'P' => Some("ℙ"),
            'Q' => Some("ℚ"),
            'R' => Some("ℝ"),
            'Z' => Some("ℤ"),
            _ => None,
        },
        Variant::Script => match ch {
            'B' => Some("ℬ"),
            'E' => Some("ℰ"),
            'F' => Some("ℱ"),
            'H' => Some("ℋ"),
            'I' => Some("ℐ"),
            'L' => Some("ℒ"),
            'M' => Some("ℳ"),
            'R' => Some("ℛ"),
            'e' => Some("ℯ"),
            'g' => Some("ℊ"),
            'o' => Some("ℴ"),
            _ => None,
        },
        Variant::Fraktur => match ch {
            'C' => Some("ℭ"),
            'H' => Some("ℌ"),
            'I' => Some("ℑ"),
            'R' => Some("ℜ"),
            'Z' => Some("ℨ"),
            _ => None,
        },
        _ => None,
    }
}

/// `applyVariant` (`layout.ts:602-707`).
fn apply_variant(value: &str, variant: Option<Variant>) -> String {
    let Some(variant) = variant else {
        return value.to_owned();
    };
    if matches!(variant, Variant::Normal | Variant::Bold | Variant::Italic) {
        return value.to_owned();
    }

    let (upper, lower, digits) = match variant {
        Variant::DoubleStruck => (Some(0x1d538), 0x1d552, Some(0x1d7d8)),
        Variant::Script => (Some(0x1d49c), 0x1d4b6, None),
        Variant::Fraktur => (Some(0x1d504), 0x1d51e, None),
        Variant::Sans => (Some(0x1d5a0), 0x1d5ba, Some(0x1d7e2)),
        Variant::Monospace => (Some(0x1d670), 0x1d68a, Some(0x1d7f6)),
        _ => (None, 0, None),
    };
    let Some(upper) = upper else {
        return value.to_owned();
    };
    value
        .chars()
        .map(|ch| {
            if let Some(exception) = variant_exception(&variant, ch) {
                return exception.chars().next().unwrap_or(ch);
            }
            match ch {
                'A'..='Z' => char::from_u32(upper + (ch as u32 - 'A' as u32)).unwrap_or(ch),
                'a'..='z' => char::from_u32(lower + (ch as u32 - 'a' as u32)).unwrap_or(ch),
                '0'..='9' => digits
                    .and_then(|base| char::from_u32(base + (ch as u32 - '0' as u32)))
                    .unwrap_or(ch),
                _ => ch,
            }
        })
        .collect()
}

/// Flatten a laid-out [`Boxed`] into the public [`MathLayout`].
fn box_to_layout(boxed: Boxed) -> MathLayout {
    MathLayout::with_cells(boxed.width, boxed.height, boxed.baseline, boxed.cells)
}

// ── Engine ────────────────────────────────────────────────────────────

/// Upstream-faithful engine: `renderLatex(source, { strict: true,
/// displayMode: true })` (`packages/latex/src/markdown.ts:112-118`).
#[derive(Clone, Debug)]
pub struct UpstreamMathEngine {
    /// Live callers pass `strict: true`; non-strict mode renders unknown
    /// commands as literal text like upstream's default parser options.
    pub strict: bool,
}

impl Default for UpstreamMathEngine {
    /// `markdown.ts:113` – `strict: true` and `displayMode: true`.
    fn default() -> Self {
        Self { strict: true }
    }
}

impl MathLayoutEngine for UpstreamMathEngine {
    fn layout(&self, request: &MathLayoutRequest<'_>) -> Result<MathLayout, MathFailure> {
        // Upstream input checks (`parser.ts:66-77`, `limits.ts:14-40`). The
        // renderer pre-applies the same limits; the engine stays standalone.
        check_source_length(request.source, MAX_MATH_SOURCE_CHARS)
            .map_err(|chars| MathFailure::TooLarge { chars })?;
        check_nesting_depth(request.source, MAX_MATH_NESTING_DEPTH)
            .map_err(|offset| MathFailure::TooDeep { offset })?;

        let chars: Vec<char> = request.source.chars().collect();
        let node = Parser::new(&chars, self.strict, MAX_MATH_NESTING_DEPTH)
            .parse()
            .map_err(|error| MathFailure::Parse {
                offset: error.position,
            })?;
        Ok(box_to_layout(layout_math(node, request.display_mode)))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_math::{MathFence, MathOutcome, MathRenderer, MathTheme};

    fn engine_layout(source: &str) -> Result<MathLayout, MathFailure> {
        let theme = MathTheme::default();
        let request = MathLayoutRequest {
            source,
            display_mode: true,
            theme: &theme,
        };
        UpstreamMathEngine::default().layout(&request)
    }

    fn rendered_rows(source: &str) -> Vec<String> {
        let layout = engine_layout(source).expect("layout");
        (0..layout.height)
            .map(|y| layout.row_text(y))
            .collect()
    }

    /// MD-009. Fraction layout mirrors `layout.ts:88-111`: numerator on top,
    /// a full-width `─` rule on the baseline row, denominator below, side
    /// bearing columns for operators.
    #[test]
    fn md009_engine_fraction_paints_real_cells() {
        let layout = engine_layout("\\frac{1}{2}").expect("fraction layout");
        assert_eq!((layout.width, layout.height, layout.baseline), (3, 3, 1));
        assert_eq!(
            rendered_rows("\\frac{1}{2}"),
            vec![" 1".to_owned(), "───".to_owned(), " 2".to_owned()]
        );
        // Cells are real painted glyphs, not the sentinel blank grid.
        assert_eq!(layout.cells[1][0].as_ref().expect("bar").ch, '─');
        assert_eq!(layout.cells[0][1].as_ref().expect("num").ch, '1');
    }

    /// MD-009. Scripts use compact Unicode superscripts/subscripts when the
    /// base is one row tall (`layout.ts:169-181`); unmappable or multi-row
    /// scripts fall back to stacked layout (`layout.ts:183-207`).
    #[test]
    fn md009_engine_scripts_compact_and_stacked() {
        // Compact: `x^2` -> "x²" on one row.
        let layout = engine_layout("x^2").expect("scripts layout");
        assert_eq!((layout.width, layout.height, layout.baseline), (2, 1, 0));
        assert_eq!(layout.row_text(0), "x²");

        // Compact subscript: `x_i` -> "xᵢ".
        let layout = engine_layout("x_i").expect("subscript layout");
        assert_eq!(layout.row_text(0), "xᵢ");

        let layout = engine_layout("x^a_b").expect("stacked scripts layout");
        assert_eq!((layout.width, layout.height), (2, 3));
        assert_eq!(rendered_rows("x^a_b"), vec![" a", "x", " b"]);
    }

    /// MD-009. Symbol roles drive math spacing (`layout.ts:126-143`,
    /// `needsMathSpace`) and the symbols table resolves greek letters
    /// (`symbols.ts` `ordinary`).
    #[test]
    fn md009_engine_symbols_infer_roles_and_math_space() {
        let layout = engine_layout("\\alpha+\\beta").expect("symbols layout");
        let row = layout.row_text(0);
        assert_eq!(row, "α + β");
        let cells: Vec<char> = layout.cells[0]
            .iter()
            .map(|cell| cell.as_ref().map(|cell| cell.ch).unwrap_or(' '))
            .collect();
        assert_eq!(cells, vec!['α', ' ', '+', ' ', 'β']);

        // Punctuation and opening delimiters suppress the math space.
        let layout = engine_layout("\\alpha,\\beta").expect("punct layout");
        assert_eq!(layout.row_text(0), "α,β");
    }

    /// MD-009. Groups and `\sqrt` use the upstream radical glyphs
    /// (`layout.ts:113-135`).
    #[test]
    fn md009_engine_groups_and_sqrt_paint_real_glyphs() {
        // Group binds before the script: `{a+b}^2` -> "a + b²".
        let layout = engine_layout("{a+b}^2").expect("group layout");
        assert_eq!(layout.row_text(0), "a + b²");

        // Radical: hook and overbar around the radicand.
        let layout = engine_layout("\\sqrt{x}").expect("sqrt layout");
        assert_eq!((layout.width, layout.height, layout.baseline), (3, 2, 1));
        assert_eq!(rendered_rows("\\sqrt{x}"), vec![" ╭─", "╰╯x"]);
    }

    /// MD-009. Strict mode (live path, `markdown.ts:113`) fails closed on
    /// unsupported commands with the upstream parse position.
    #[test]
    fn md009_engine_strict_unknown_command_fails_with_offset() {
        let failure = engine_layout("\\unsupported{x}").expect_err("parse error");
        assert_eq!(failure, MathFailure::Parse { offset: 0 });

        let failure = engine_layout("ab\\unknown").expect_err("parse error 2");
        assert_eq!(failure, MathFailure::Parse { offset: 2 });
    }

    /// MD-009. The same public API the live `native_view_chat.rs` call site
    /// uses: `MathFence` -> `MathRenderer::render` with the real engine
    /// returns a Rendered frame carrying painted cells.
    #[test]
    fn md009_live_renderer_produces_painted_cells() {
        let fence = MathFence::from_code_block(
            Some("math"),
            "\\frac{m}{n}",
            "```math\n\\frac{m}{n}\n```\n",
            7,
            false,
        )
        .expect("math fence");
        let mut renderer = MathRenderer::new(
            UpstreamMathEngine::default(),
            MathTheme::new("text.base", "text.muted"),
        );
        let outcome = renderer.render(&fence, 80);
        let frame = outcome.frame().expect("rendered frame");
        assert_eq!(frame.layout.width, 3);
        let rows: Vec<String> = (0..frame.layout.height)
            .map(|y| frame.layout.row_text(y))
            .collect();
        assert_eq!(rows, vec![" m".to_owned(), "───".to_owned(), " n".to_owned()]);
        // Fenced language `math` and `latex` both route to the engine
        // (`packages/latex/src/plugin.ts:11-12`); cache reuse keeps cells.
        let again = renderer.render(&fence, 80);
        let cached = again.frame().expect("cached");
        assert!(cached.from_cache);
        assert_eq!(cached.layout, frame.layout);
    }

    /// MD-009. Matrix environments paint aligned columns with delimiters
    /// (`layout.ts:268-355`), binomials stretch parentheses
    /// (`layout.ts:437-451`).
    #[test]
    fn md009_engine_matrix_and_binom_paint() {
        let layout = engine_layout("\\begin{pmatrix}a\\\\b\\end{pmatrix}").expect("matrix");
        assert_eq!(layout.height, 3);
        assert_eq!(rendered_rows("\\begin{pmatrix}a\\\\b\\end{pmatrix}"), {
            vec!["⎛a⎞", "⎜ ⎟", "⎝b⎠"].into_iter().map(String::from).collect::<Vec<_>>()
        });

        let layout = engine_layout("\\binom{n}{k}").expect("binom");
        assert_eq!(
            rendered_rows("\\binom{n}{k}"),
            vec!["⎛ n ⎞".to_owned(), "⎜   ⎟".to_owned(), "⎝ k ⎠".to_owned()]
        );
    }
}
