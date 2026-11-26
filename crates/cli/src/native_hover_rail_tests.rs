#[cfg(test)]
mod owner_hover_rail_boundary {
    use super::*;
    #[test]
    fn rail_cell_does_not_admit_inner_card_hover() {
        let mut view = NativeView::new(80, 24);
        view.set_session_route(true);
        let messages = [crate::native_view_chat::ChatMessage::text(
            crate::native_view_chat::ChatRole::User, "rail-boundary-owner")];
        let frame = view.render_typed("", &messages, false, &[]);
        let row = frame.iter().position(|line| line.contains("rail-boundary-owner")).unwrap();
        let p = view.presentation();
        let left = p.main_offset + p.main_padding.min(p.main_width / 2);
        assert_eq!(frame[row].chars().nth(usize::from(left)), Some('┃'));
        view.set_chat_pointer(left, row as u16);
        view.render_typed("", &messages, false, &[]);
        assert!(!view.chat_hovered_rows().iter().any(|v| *v), "outer rail must not admit inner-box hover");
        view.set_chat_pointer(left + 1, row as u16);
        view.render_typed("", &messages, false, &[]);
        assert!(view.chat_hovered_rows()[row], "first inner-box cell admits hover");
    }
}
