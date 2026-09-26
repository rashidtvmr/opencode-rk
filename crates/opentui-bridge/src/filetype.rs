#![forbid(unsafe_code)]
//! File extension -> language id (mirrors
//! `packages/tui/src/util/filetype.ts` `LANGUAGE_EXTENSIONS`).
//!
//! Keys are lowercase with a leading dot, except the bare `makefile` entry
//! copied verbatim from TS. Table is sorted for `binary_search`; multi-dot
//! keys (`.html.erb`) resolve via longest-suffix match. Raw table values are
//! returned as-is (TS `filetype()` collapses react/js variants to
//! `typescript`; that collapse lives with the caller).

/// Sorted extension -> language table (118 dot keys + bare `makefile`).
pub const EXTENSIONS: &[(&str, &str)] = &[
    (".abap", "abap"),
    (".astro", "astro"),
    (".bash", "shellscript"),
    (".bat", "bat"),
    (".bib", "bibtex"),
    (".bibtex", "bibtex"),
    (".c", "c"),
    (".c++", "cpp"),
    (".cc", "cpp"),
    (".cjs", "javascript"),
    (".clj", "clojure"),
    (".cljc", "clojure"),
    (".cljs", "clojure"),
    (".coffee", "coffeescript"),
    (".cpp", "cpp"),
    (".cs", "csharp"),
    (".cshtml", "razor"),
    (".css", "css"),
    (".css.erb", "erb"),
    (".cts", "typescript"),
    (".ctsx", "typescriptreact"),
    (".cxx", "cpp"),
    (".d", "d"),
    (".dart", "dart"),
    (".diff", "diff"),
    (".dockerfile", "dockerfile"),
    (".edn", "clojure"),
    (".erb", "erb"),
    (".erl", "erlang"),
    (".ex", "elixir"),
    (".exs", "elixir"),
    (".fs", "fsharp"),
    (".fsi", "fsharp"),
    (".fsscript", "fsharp"),
    (".fsx", "fsharp"),
    (".gemspec", "ruby"),
    (".gitcommit", "git-commit"),
    (".gitrebase", "git-rebase"),
    (".gleam", "gleam"),
    (".go", "go"),
    (".groovy", "groovy"),
    (".handlebars", "handlebars"),
    (".hbs", "handlebars"),
    (".hcl", "hcl"),
    (".hrl", "erlang"),
    (".hs", "haskell"),
    (".htm", "html"),
    (".html", "html"),
    (".html.erb", "erb"),
    (".ini", "ini"),
    (".jade", "jade"),
    (".java", "java"),
    (".jl", "julia"),
    (".js", "javascript"),
    (".js.erb", "erb"),
    (".json", "json"),
    (".json.erb", "erb"),
    (".jsx", "javascriptreact"),
    (".ksh", "shellscript"),
    (".kt", "kotlin"),
    (".kts", "kotlin"),
    (".less", "less"),
    (".lhs", "haskell"),
    (".lua", "lua"),
    (".m", "objective-c"),
    (".makefile", "makefile"),
    (".markdown", "markdown"),
    (".md", "markdown"),
    (".mjs", "javascript"),
    (".ml", "ocaml"),
    (".mli", "ocaml"),
    (".mm", "objective-cpp"),
    (".mts", "typescript"),
    (".mtsx", "typescriptreact"),
    (".nix", "nix"),
    (".pas", "pascal"),
    (".pascal", "pascal"),
    (".patch", "diff"),
    (".php", "php"),
    (".pl", "perl"),
    (".pm", "perl"),
    (".pm6", "perl6"),
    (".ps1", "powershell"),
    (".psm1", "powershell"),
    (".pug", "jade"),
    (".py", "python"),
    (".r", "r"),
    (".rake", "ruby"),
    (".razor", "razor"),
    (".rb", "ruby"),
    (".rs", "rust"),
    (".ru", "ruby"),
    (".sass", "sass"),
    (".scala", "scala"),
    (".scss", "scss"),
    (".sh", "shellscript"),
    (".shader", "shaderlab"),
    (".sql", "sql"),
    (".svelte", "svelte"),
    (".swift", "swift"),
    (".tex", "latex"),
    (".tf", "terraform"),
    (".tfvars", "terraform-vars"),
    (".ts", "typescript"),
    (".tsx", "typescriptreact"),
    (".typ", "typst"),
    (".typc", "typst"),
    (".vue", "vue"),
    (".xml", "xml"),
    (".xsl", "xsl"),
    (".yaml", "yaml"),
    (".yml", "yaml"),
    (".zig", "zig"),
    (".zon", "zig"),
    (".zsh", "shellscript"),
    ("makefile", "makefile"),
];

fn lookup(key: &str) -> Option<&'static str> {
    EXTENSIONS
        .binary_search_by(|(k, _)| k.cmp(&key))
        .ok()
        .map(|i| EXTENSIONS[i].1)
}

/// Language id for a full path, bare extension, or bare filename.
/// Case-insensitive; longest-suffix wins (`.html.erb` beats `.erb`).
/// Fail-closed `None` for unknown / extensionless / dotfiles.
#[must_use]
pub fn language_of(path_or_ext: &str) -> Option<&'static str> {
    if path_or_ext.is_empty() {
        return None;
    }
    // ponytail: one alloc via to_ascii_lowercase; upgrade: stack buf when hot.
    let lower = path_or_ext.to_ascii_lowercase();
    let base = lower.rsplit(['/', '\\']).next().unwrap_or(&lower);
    if base.is_empty() {
        return None;
    }
    if let Some(lang) = lookup(base) {
        return Some(lang);
    }
    let mut best: Option<&'static str> = None;
    let mut best_len = 0usize;
    for (ext, lang) in EXTENSIONS {
        if ext.len() > best_len && ext.starts_with('.') && base.ends_with(ext) {
            best = Some(*lang);
            best_len = ext.len();
        }
    }
    if best.is_some() {
        return best;
    }
    if !base.contains('.') {
        return lookup(&format!(".{base}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_maps_typescript() {
        assert_eq!(language_of(".ts"), Some("typescript"));
    }

    #[test]
    fn uppercase_ext() {
        assert_eq!(language_of(".TS"), Some("typescript"));
        assert_eq!(language_of("RS"), Some("rust"));
    }

    #[test]
    fn full_path() {
        assert_eq!(language_of("foo/bar.rs"), Some("rust"));
        assert_eq!(language_of("C:\\src\\app.TSX"), Some("typescriptreact"));
    }

    #[test]
    fn longest_suffix_wins() {
        assert_eq!(language_of("view.html.erb"), Some("erb"));
    }

    #[test]
    fn unknown_ext_none() {
        assert_eq!(language_of("foo.xyz"), None);
        assert_eq!(language_of(".xyz"), None);
    }

    #[test]
    fn no_ext_none() {
        assert_eq!(language_of("README"), None);
        assert_eq!(language_of(""), None);
    }

    #[test]
    fn dotfile_none() {
        assert_eq!(language_of(".gitignore"), None);
    }

    #[test]
    fn bare_makefile() {
        assert_eq!(language_of("makefile"), Some("makefile"));
        assert_eq!(language_of("Makefile"), Some("makefile"));
    }
}
