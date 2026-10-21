#[path = "../src/native_text_selection.rs"]
mod selection;
#[path = "../src/native_view_layout.rs"]
mod native_view_layout;

use selection::TextSelection;

fn rows(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn left_and_right_padding_are_not_selectable() {
    let mut s = TextSelection::default();
    s.set_rows(&rows(&["  abc   "]), 0, 0, 12);
    assert!(!s.press(0, 0));
    assert!(s.press(2, 0));
    s.drag(11, 0);
    s.release();
    assert_eq!(s.selected_text().as_deref(), Some("abc"));
    assert_eq!(s.spans(), vec![(0, 2, 1), (0, 3, 1), (0, 4, 1)]);
}

#[test]
fn identical_set_rows_preserves_selection() {
    let mut s = TextSelection::default();
    s.set_rows(&rows(&["abc"]), 0, 0, 8);
    assert!(s.press(0, 0));
    s.drag(2, 0);
    s.release();
    s.set_rows(&rows(&["abc"]), 0, 0, 8);
    assert_eq!(s.selected_text().as_deref(), Some("abc"));
}

#[test]
fn changed_rows_clear_selection() {
    let mut s = TextSelection::default();
    s.set_rows(&rows(&["abc"]), 0, 0, 8);
    assert!(s.press(0, 0));
    s.drag(2, 0);
    s.release();
    s.set_rows(&rows(&["abd"]), 0, 0, 8);
    assert_eq!(s.selected_text(), None);
}

#[test]
fn flag_and_skin_tone_clusters_share_the_renderers_cell_boundaries() {
    let mut selection=TextSelection::default();
    selection.set_rows(&rows(&["🇺🇸👍🏽x"]),0,0,12);
    assert!(selection.press(1,0));selection.drag(4,0);selection.release();
    assert_eq!(selection.selected_text().as_deref(),Some("🇺🇸👍🏽x"));
    assert_eq!(selection.spans(),vec![(0,0,2),(0,2,2),(0,4,1)]);
}
