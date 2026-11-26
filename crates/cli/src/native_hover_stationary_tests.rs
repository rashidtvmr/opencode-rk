#[cfg(test)]
mod owner_stationary_hover {
    use super::*;
    use crate::native_view_chat::{ChatMessage, ChatRole, NO_MESSAGE_GROUP};

    fn messages() -> Vec<ChatMessage> {
        (0..20).map(|i| ChatMessage::text(ChatRole::User, format!("stationary-{i:02}"))).collect()
    }
    fn setup() -> (NativeView, Vec<ChatMessage>, u16, usize) {
        let mut view = NativeView::new(80, 30);
        view.set_session_route(true);
        let messages = messages();
        let frame = view.render_typed("", &messages, false, &[]);
        let row = frame.iter().position(|s| s.contains("stationary-18")).unwrap();
        let p = view.presentation();
        let x = p.main_offset + p.main_padding.min(p.main_width / 2) + 1;
        view.set_chat_pointer(x, row as u16);
        view.render_typed("", &messages, false, &[]);
        assert!(view.chat_hovered_rows()[row], "fixture starts over a user card");
        (view, messages, x, row)
    }
    #[test]
    fn stationary_pointer_rechecks_card_after_scroll() {
        let (mut view, messages, _, row) = setup();
        let old_group = view.chat_row_groups()[row];
        view.scroll_up(4);
        assert!(view.chat_hovered_rows().is_empty(), "old painted mask invalidates immediately");
        view.render_typed("", &messages, false, &[]);
        let new_group = view.chat_row_groups()[row];
        assert_ne!(new_group, NO_MESSAGE_GROUP, "fixture lands on another card");
        assert_ne!(new_group, old_group, "fixture changes identity under stationary pointer");
        assert!(view.chat_hovered_rows()[row], "stationary pointer hovers current card without motion");
        for (index, hovered) in view.chat_hovered_rows().iter().enumerate() {
            assert_eq!(*hovered, view.chat_row_groups()[index] == new_group,
                "only the new current card's rows hover");
        }
    }
    #[test]
    fn stationary_pointer_rechecks_card_after_resize() {
        let (mut view, messages, _, row) = setup();
        view.resize(70, 30);
        assert!(view.chat_hovered_rows().is_empty(), "resize invalidates old mask");
        view.render_typed("", &messages, false, &[]);
        assert_ne!(view.chat_row_groups()[row], NO_MESSAGE_GROUP);
        assert!(view.chat_hovered_rows()[row], "stationary pointer rechecks resized hit grid without motion");
    }
}
