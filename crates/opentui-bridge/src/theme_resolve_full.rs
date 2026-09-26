//! Theme ref-chain resolver with dark/light variant select.
//!
//! ponytail: fixed 8-hop cap, linear scan; upgrade to arena if theme graphs grow.

#![forbid(unsafe_code)]

/// Named theme with optional parent ref and per-variant values.
/// Empty variant string means "inherit from parent".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDef {
    pub name: String,
    pub extends: Option<String>,
    pub dark: String,
    pub light: String,
}

impl ThemeDef {
    pub fn new(name: &str, extends: Option<&str>, dark: &str, light: &str) -> Self {
        Self {
            name: name.to_string(),
            extends: extends.map(str::to_string),
            dark: dark.to_string(),
            light: light.to_string(),
        }
    }
}

/// Pick the active variant reference for one def.
pub fn variant_pick(def: &ThemeDef, is_dark: bool) -> &str {
    if is_dark {
        &def.dark
    } else {
        &def.light
    }
}

const MAX_HOPS: usize = 8;

fn find<'a>(defs: &'a [ThemeDef], name: &str) -> Option<&'a ThemeDef> {
    defs.iter().find(|d| d.name == name)
}

/// Resolve `name` to its active variant value.
///
/// Walks `extends` up to 8 hops; first non-empty variant from the
/// start wins. Fail-closed (`None`) on: unknown name, dangling parent
/// ref, cycle, chain longer than 8, or no non-empty variant in chain.
pub fn resolve_chain(defs: &[ThemeDef], name: &str, is_dark: bool) -> Option<String> {
    let mut chain: Vec<&ThemeDef> = Vec::new();
    let mut visited: Vec<&str> = Vec::new();
    let mut current: &str = name;
    loop {
        if visited.contains(&current) {
            return None; // cycle
        }
        visited.push(current);
        let def = find(defs, current)?;
        chain.push(def);
        match def.extends.as_deref() {
            Some(parent) => {
                if chain.len() >= MAX_HOPS {
                    return None; // depth cap
                }
                current = parent;
            }
            None => break,
        }
    }
    for def in chain {
        let v = variant_pick(def, is_dark);
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Vec<ThemeDef> {
        vec![ThemeDef::new("base", None, "b-dark", "b-light")]
    }

    #[test]
    fn direct_dark() {
        assert_eq!(
            resolve_chain(&base(), "base", true),
            Some("b-dark".to_string())
        );
    }

    #[test]
    fn direct_light() {
        assert_eq!(
            resolve_chain(&base(), "base", false),
            Some("b-light".to_string())
        );
    }

    #[test]
    fn chain_two_hop() {
        let d = vec![
            ThemeDef::new("child", Some("base"), "", ""),
            ThemeDef::new("base", None, "b-dark", "b-light"),
        ];
        assert_eq!(
            resolve_chain(&d, "child", false),
            Some("b-light".to_string())
        );
    }

    #[test]
    fn child_value_wins() {
        let d = vec![
            ThemeDef::new("child", Some("base"), "c-dark", "c-light"),
            ThemeDef::new("base", None, "b-dark", "b-light"),
        ];
        assert_eq!(resolve_chain(&d, "child", true), Some("c-dark".to_string()));
    }

    #[test]
    fn cycle_none() {
        let d = vec![
            ThemeDef::new("a", Some("b"), "a-dark", "a-light"),
            ThemeDef::new("b", Some("a"), "b-dark", "b-light"),
        ];
        assert_eq!(resolve_chain(&d, "a", true), None);
    }

    #[test]
    fn missing_none() {
        assert_eq!(resolve_chain(&base(), "nope", true), None);
    }

    #[test]
    fn depth_cap_none() {
        let mut d: Vec<ThemeDef> = (0..10)
            .map(|i| {
                let parent = if i == 9 {
                    None
                } else {
                    Some(format!("t{}", i + 1))
                };
                ThemeDef {
                    name: format!("t{i}"),
                    extends: parent,
                    dark: String::new(),
                    light: String::new(),
                }
            })
            .collect();
        d[9].dark = "root-dark".to_string();
        assert_eq!(resolve_chain(&d, "t0", true), None);
    }
}
