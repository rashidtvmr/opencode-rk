//! Unmodified JSON theme assets from OpenCode pin
//! 95daf90670b7c039c436c85537da5fbfe2205b41, packages/tui/src/theme/assets.
pub const ASSETS: &[(&str, &str)] = include!("native_theme_assets/index.rs");

pub fn lookup(name: &str) -> Option<&'static str> {
    ASSETS
        .iter()
        .find_map(|(id, json)| (*id == name).then_some(*json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_theme_runtime::{palette_from_asset, ThemeMode};

    #[test]
    fn every_shipped_upstream_theme_resolves_in_both_modes() {
        assert_eq!(ASSETS.len(), 33);
        for (name, text) in ASSETS {
            let value = serde_json::from_str(text).expect("bundled JSON");
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                assert!(
                    palette_from_asset(&value, mode).is_ok(),
                    "{name} {mode}: {:?}",
                    palette_from_asset(&value, mode)
                );
            }
        }
    }
}
