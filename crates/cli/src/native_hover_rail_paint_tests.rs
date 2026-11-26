#[cfg(all(test, feature = "native"))]
mod owner_hover_rail_paint {
    use super::*;
    #[test]
    fn hovered_rail_retains_panel_background() {
        let view = crate::native_view::NativeView::new(80, 24);
        let p = view.presentation();
        let left = u32::from(p.main_offset + p.main_padding.min(p.main_width / 2));
        let root = Rgba::rgb(10, 10, 10);
        let panel = Rgba::rgb(20, 20, 20);
        let element = Rgba::rgb(30, 30, 30);
        let class = Some(&crate::native_view_chat::ChatRowClass::UserCard);
        assert_eq!(chat_row_background_at(class, p, left, root, panel, element, None, true), Some(panel), "outer rail keeps unhovered panel color");
        assert_eq!(chat_row_background_at(class, p, left + 1, root, panel, element, None, true), Some(element));
    }
}
