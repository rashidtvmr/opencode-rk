//! RED skeleton: tests reference items not yet declared.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_options_match_renderer_values() {
        assert_eq!(
            pack_options(BorderSides::ALL, false, TitleAlign::Left, TitleAlign::Left),
            0b1111
        );
        assert_eq!(
            pack_options(BorderSides::ALL, true, TitleAlign::Center, TitleAlign::Right),
            0b1111 | (1 << 4) | (1 << 5) | (2 << 7)
        );
    }

    #[test]
    fn handle_widths() {
        assert_eq!(core::mem::size_of::<NativeHandle>(), 4);
        assert_eq!(INVALID_HANDLE, 0);
    }

    #[test]
    fn coverage_table_self_consistent() {
        let sum = DOMAIN_AUDIO
            + DOMAIN_CLIPBOARD
            + DOMAIN_EMBEDDED_TERMINAL
            + DOMAIN_EDIT_BUFFER
            + DOMAIN_EDITOR_VIEW
            + DOMAIN_TEXT_BUFFER
            + DOMAIN_BUFFER
            + DOMAIN_HITGRID
            + DOMAIN_SYNTAX_STYLE
            + DOMAIN_LINK
            + DOMAIN_ATTRIBUTES
            + DOMAIN_RENDER
            + DOMAIN_OTHER;
        assert_eq!(sum, ALL_SYMBOLS.len());
        assert_eq!(ALL_SYMBOLS.len(), ZIGDECL_TOTAL);
        assert_eq!(ZIGDECL_TOTAL, 337);
    }

    #[test]
    fn status_enum_has_skipped_failed() {
        assert_eq!(NATIVE_RENDER_STATUS_RENDERED, 0);
        assert_eq!(NATIVE_RENDER_STATUS_SKIPPED, 1);
        assert_eq!(NATIVE_RENDER_STATUS_FAILED, 2);
        assert_eq!(NativeRenderStatus::from_u8(1), NativeRenderStatus::Skipped);
        assert_eq!(NativeRenderStatus::from_u8(9), NativeRenderStatus::Failed);
    }

    #[test]
    fn marker_validation() {
        assert_eq!(DEST_STDOUT, 0);
        assert_eq!(DEST_MEMORY, 1);
        assert_eq!(REMOTE_AUTO, 0);
        assert!(create_renderer(80, 24, DEST_STDOUT).is_ok());
        assert_eq!(create_renderer(0, 24, DEST_STDOUT), Err(DeclError::ZeroSize));
        assert_eq!(create_renderer(80, 24, 9), Err(DeclError::BadDestination));
        assert!(resize_renderer(80, 24).is_ok());
        assert_eq!(resize_renderer(0, 24), Err(DeclError::ZeroSize));
    }

    #[test]
    fn render_native_mapping() {
        assert_eq!(render_native(0), NativeRenderStatus::Rendered);
        assert_eq!(render_native(1), NativeRenderStatus::Skipped);
        assert_eq!(render_native(2), NativeRenderStatus::Failed);
        assert_eq!(render_native(255), NativeRenderStatus::Failed);
    }

    #[test]
    fn all_symbols_sorted_unique() {
        let mut sorted = ALL_SYMBOLS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, ALL_SYMBOLS);
        sorted.dedup();
        assert_eq!(sorted.len(), ALL_SYMBOLS.len());
    }
}
