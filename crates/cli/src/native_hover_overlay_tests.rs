#[cfg(all(test, feature = "native"))]
mod owner_external_overlay_hover {
    use super::*;
    use crate::native_view::{NativeView};
    use crate::native_view_chat::{ChatMessage, ChatRole};

    fn hovered_view() -> (NativeView, Vec<ChatMessage>, u16, u16) {
        let mut view = NativeView::new(80, 30);
        view.set_session_route(true);
        let messages = vec![ChatMessage::text(ChatRole::User, "overlay-owned-hover")];
        let frame = view.render_typed("", &messages, false, &[]);
        let row = frame.iter().position(|line| line.contains("overlay-owned-hover")).unwrap() as u16;
        let p = view.presentation();
        let x = p.main_offset + p.main_padding.min(p.main_width / 2) + 1;
        assert!(HoverOverlayOwnership::default().record_chat_pointer(&mut view, x, row));
        view.render_typed("", &messages, false, &[]);
        assert!(view.chat_hovered_rows()[usize::from(row)]);
        (view, messages, x, row)
    }
    fn assert_owned_frame_clears(state: HoverOverlayOwnership) {
        let (mut view, messages, _, _) = hovered_view();
        state.clear_chat_if_blocked(&mut view);
        view.render_typed("", &messages, false, &[]);
        assert!(!view.chat_hovered_rows().iter().any(|hit| *hit),
            "external overlay owns the frame; underlying chat must not remain hovered");
    }
    #[test]
    fn setup_overlay_clears_existing_chat_hover() {
        assert_owned_frame_clears(HoverOverlayOwnership { setup: true, ..Default::default() });
    }
    #[test]
    fn import_overlay_clears_existing_chat_hover() {
        assert_owned_frame_clears(HoverOverlayOwnership { import: true, ..Default::default() });
    }
    #[test]
    fn import_overlay_refuses_new_chat_pointer() {
        let (mut view, messages, x, row) = hovered_view();
        view.clear_chat_hover();
        let state = HoverOverlayOwnership { import: true, ..Default::default() };
        assert!(!state.record_chat_pointer(&mut view, x, row), "import consumes pointer ownership");
        view.render_typed("", &messages, false, &[]);
        assert!(!view.chat_hovered_rows().iter().any(|hit| *hit));
    }
    #[test]
    fn existing_overlay_owners_and_clear_frame_remain_distinct() {
        for state in [
            HoverOverlayOwnership { dialog: true, ..Default::default() },
            HoverOverlayOwnership { help: true, ..Default::default() },
            HoverOverlayOwnership { palette: true, ..Default::default() },
            HoverOverlayOwnership { permission_editor: true, ..Default::default() },
            HoverOverlayOwnership { appearance: true, ..Default::default() },
            HoverOverlayOwnership { children: true, ..Default::default() },
        ] {
            assert_owned_frame_clears(state);
            let (mut view, _, x, row) = hovered_view();
            assert!(!state.record_chat_pointer(&mut view, x, row));
        }
        let (mut view, messages, _, row) = hovered_view();
        HoverOverlayOwnership::default().clear_chat_if_blocked(&mut view);
        view.render_typed("", &messages, false, &[]);
        assert!(view.chat_hovered_rows()[usize::from(row)], "no overlay retains normal hover");
    }
}
