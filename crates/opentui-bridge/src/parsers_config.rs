#![forbid(unsafe_code)]
//! Static tree-sitter parser URL table.
//! Mirrors `packages/tui/src/parsers-config.ts` (TS checkout a0d9b6c).
//! DIVERGENCE: static table only, no network fetch. `query_url` is the first
//! active `highlights` URL per entry; `locals`, extra highlight URLs, and
//! commented-out/broken alternates are omitted. Builtin langs
//! (`markdown`, `javascript`, `typescript`) need no wasm fetch (see
//! `renderer_config.rs:20-23`) and are intentionally absent here.

/// (filetype, wasm_url, first-highlights query_url), URLs verbatim from TS.
pub const LANG_PARSERS: &[(&str, &str, &str)] = &[
    (
        "python",
        "https://github.com/tree-sitter/tree-sitter-python/releases/download/v0.23.6/tree-sitter-python.wasm",
        "https://github.com/tree-sitter/tree-sitter-python/raw/refs/heads/master/queries/highlights.scm",
    ),
    (
        "rust",
        "https://github.com/tree-sitter/tree-sitter-rust/releases/download/v0.24.0/tree-sitter-rust.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/rust/highlights.scm",
    ),
    (
        "go",
        "https://github.com/tree-sitter/tree-sitter-go/releases/download/v0.25.0/tree-sitter-go.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/go/highlights.scm",
    ),
    (
        "cpp",
        "https://github.com/tree-sitter/tree-sitter-cpp/releases/download/v0.23.4/tree-sitter-cpp.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/cpp/highlights.scm",
    ),
    (
        "csharp",
        "https://github.com/tree-sitter/tree-sitter-c-sharp/releases/download/v0.23.1/tree-sitter-c_sharp.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/c_sharp/highlights.scm",
    ),
    (
        "bash",
        "https://github.com/tree-sitter/tree-sitter-bash/releases/download/v0.25.0/tree-sitter-bash.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/bash/highlights.scm",
    ),
    (
        "c",
        "https://github.com/tree-sitter/tree-sitter-c/releases/download/v0.24.1/tree-sitter-c.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/c/highlights.scm",
    ),
    (
        "java",
        "https://github.com/tree-sitter/tree-sitter-java/releases/download/v0.23.5/tree-sitter-java.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/java/highlights.scm",
    ),
    (
        "kotlin",
        "https://github.com/fwcd/tree-sitter-kotlin/releases/download/0.3.8/tree-sitter-kotlin.wasm",
        "https://raw.githubusercontent.com/fwcd/tree-sitter-kotlin/0.3.8/queries/highlights.scm",
    ),
    (
        "ruby",
        "https://github.com/tree-sitter/tree-sitter-ruby/releases/download/v0.23.1/tree-sitter-ruby.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/ruby/highlights.scm",
    ),
    (
        "php",
        "https://github.com/tree-sitter/tree-sitter-php/releases/download/v0.24.2/tree-sitter-php.wasm",
        "https://github.com/tree-sitter/tree-sitter-php/raw/refs/heads/master/queries/highlights.scm",
    ),
    (
        "scala",
        "https://github.com/tree-sitter/tree-sitter-scala/releases/download/v0.24.0/tree-sitter-scala.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/scala/highlights.scm",
    ),
    (
        "html",
        "https://github.com/tree-sitter/tree-sitter-html/releases/download/v0.23.2/tree-sitter-html.wasm",
        "https://github.com/tree-sitter/tree-sitter-html/raw/refs/heads/master/queries/highlights.scm",
    ),
    (
        "vue",
        "https://github.com/anomalyco/tree-sitter-vue/releases/download/v0.1.2/tree-sitter-vue.wasm",
        "https://raw.githubusercontent.com/anomalyco/tree-sitter-vue/v0.1.2/queries/html_tags/highlights.scm",
    ),
    (
        "hcl",
        "https://github.com/tree-sitter-grammars/tree-sitter-hcl/releases/download/v1.2.0/tree-sitter-hcl.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/master/queries/hcl/highlights.scm",
    ),
    (
        "json",
        "https://github.com/tree-sitter/tree-sitter-json/releases/download/v0.24.8/tree-sitter-json.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/json/highlights.scm",
    ),
    (
        "yaml",
        "https://github.com/tree-sitter-grammars/tree-sitter-yaml/releases/download/v0.7.2/tree-sitter-yaml.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/yaml/highlights.scm",
    ),
    (
        "haskell",
        "https://github.com/tree-sitter/tree-sitter-haskell/releases/download/v0.23.1/tree-sitter-haskell.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/haskell/highlights.scm",
    ),
    (
        "css",
        "https://github.com/tree-sitter/tree-sitter-css/releases/download/v0.25.0/tree-sitter-css.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/css/highlights.scm",
    ),
    (
        "julia",
        "https://github.com/tree-sitter/tree-sitter-julia/releases/download/v0.23.1/tree-sitter-julia.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/julia/highlights.scm",
    ),
    (
        "lua",
        "https://github.com/tree-sitter-grammars/tree-sitter-lua/releases/download/v0.5.0/tree-sitter-lua.wasm",
        "https://raw.githubusercontent.com/tree-sitter-grammars/tree-sitter-lua/v0.5.0/queries/highlights.scm",
    ),
    (
        "ocaml",
        "https://github.com/tree-sitter/tree-sitter-ocaml/releases/download/v0.24.2/tree-sitter-ocaml.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/ocaml/highlights.scm",
    ),
    (
        "clojure",
        "https://github.com/anomalyco/tree-sitter-clojure/releases/download/v0.0.1/tree-sitter-clojure.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/clojure/highlights.scm",
    ),
    (
        "swift",
        "https://github.com/alex-pinkus/tree-sitter-swift/releases/download/0.7.1/tree-sitter-swift.wasm",
        "https://raw.githubusercontent.com/alex-pinkus/tree-sitter-swift/main/queries/highlights.scm",
    ),
    (
        "toml",
        "https://github.com/tree-sitter-grammars/tree-sitter-toml/releases/download/v0.7.0/tree-sitter-toml.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/master/queries/toml/highlights.scm",
    ),
    (
        "nix",
        "https://github.com/ast-grep/ast-grep.github.io/raw/40b84530640aa83a0d34a20a2b0623d7b8e5ea97/website/public/parsers/tree-sitter-nix.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/nix/highlights.scm",
    ),
    (
        "diff",
        "https://github.com/tree-sitter-grammars/tree-sitter-diff/releases/download/v0.1.0/tree-sitter-diff.wasm",
        "https://raw.githubusercontent.com/tree-sitter-grammars/tree-sitter-diff/master/queries/highlights.scm",
    ),
    (
        "elixir",
        "https://github.com/elixir-lang/tree-sitter-elixir/releases/download/v0.3.5/tree-sitter-elixir.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/elixir/highlights.scm",
    ),
    (
        "fsharp",
        "https://github.com/ionide/tree-sitter-fsharp/releases/download/0.3.0/tree-sitter-fsharp.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/fsharp/highlights.scm",
    ),
    (
        "r",
        "https://github.com/r-lib/tree-sitter-r/releases/download/v1.2.0/tree-sitter-r.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/r/highlights.scm",
    ),
    (
        "make",
        "https://github.com/tree-sitter-grammars/tree-sitter-make/releases/download/v1.1.1/tree-sitter-make.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/make/highlights.scm",
    ),
    (
        "vim",
        "https://github.com/tree-sitter-grammars/tree-sitter-vim/releases/download/v0.8.1/tree-sitter-vim.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/vim/highlights.scm",
    ),
    (
        "xml",
        "https://github.com/tree-sitter-grammars/tree-sitter-xml/releases/download/v0.7.0/tree-sitter-xml.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/xml/highlights.scm",
    ),
    (
        "agda",
        "https://github.com/tree-sitter/tree-sitter-agda/releases/download/v1.3.3/tree-sitter-agda.wasm",
        "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/agda/highlights.scm",
    ),
];

/// (alias, canonical filetype), verbatim from TS `aliases` fields.
pub const ALIASES: &[(&str, &str)] = &[("udiff", "diff"), ("patch", "diff"), ("makefile", "make")];

/// Resolve alias to canonical filetype; identity if none.
#[must_use]
pub fn canonical(lang: &str) -> &str {
    for (alias, target) in ALIASES {
        if *alias == lang {
            return target;
        }
    }
    lang
}

/// Alias-resolving lookup -> (wasm_url, query_url).
#[must_use]
pub fn lookup(lang: &str) -> Option<(&'static str, &'static str)> {
    let canon = canonical(lang);
    LANG_PARSERS.iter().find(|(l, _, _)| *l == canon).map(|(_, w, q)| (*w, *q))
}

/// Number of parser entries (TS `parsers` array length).
#[must_use]
pub fn count() -> usize {
    LANG_PARSERS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_resolves() {
        assert_eq!(lookup("udiff"), lookup("diff"));
        assert_eq!(lookup("patch"), lookup("diff"));
        assert_eq!(lookup("makefile"), lookup("make"));
        assert!(lookup("udiff").is_some());
    }

    #[test]
    fn unknown_none() {
        assert_eq!(lookup("typescript"), None);
        assert_eq!(lookup("nope"), None);
        assert_eq!(lookup(""), None);
    }

    #[test]
    fn count_matches_ts() {
        assert_eq!(count(), 34);
        assert_eq!(count(), LANG_PARSERS.len());
    }

    #[test]
    fn sample_url_verbatim() {
        let (wasm, query) = lookup("rust").unwrap();
        assert_eq!(
            wasm,
            "https://github.com/tree-sitter/tree-sitter-rust/releases/download/v0.24.0/tree-sitter-rust.wasm"
        );
        assert_eq!(
            query,
            "https://raw.githubusercontent.com/nvim-treesitter/nvim-treesitter/refs/heads/master/queries/rust/highlights.scm"
        );
        let (nix_wasm, _) = lookup("nix").unwrap();
        assert_eq!(
            nix_wasm,
            "https://github.com/ast-grep/ast-grep.github.io/raw/40b84530640aa83a0d34a20a2b0623d7b8e5ea97/website/public/parsers/tree-sitter-nix.wasm"
        );
    }

    #[test]
    fn no_empty_urls() {
        for (lang, wasm, query) in LANG_PARSERS {
            assert!(!lang.is_empty(), "empty lang");
            assert!(!wasm.is_empty(), "{lang} empty wasm");
            assert!(!query.is_empty(), "{lang} empty query");
            assert!(wasm.starts_with("https://"), "{lang} wasm not https");
            assert!(query.starts_with("https://"), "{lang} query not https");
        }
        for (alias, target) in ALIASES {
            assert!(!alias.is_empty() && !target.is_empty());
            assert!(lookup(target).is_some(), "alias target {target} missing");
        }
    }
}
