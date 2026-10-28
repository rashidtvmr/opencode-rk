mod native_view_pages {
    pub const MAX_ROWS: usize = 512;
}
mod native_workspace;

#[cfg(test)]
mod tests {
    use super::native_workspace::*;
    use serde_json::json;

    #[test]
    fn test_abbreviate_home() {
        assert_eq!(abbreviate_home("/home/user/projects", Some("/home/user")), "~/projects");
        assert_eq!(abbreviate_home("/home/user", Some("/home/user")), "~");
        assert_eq!(abbreviate_home("/other/path", Some("/home/user")), "/other/path");
    }

    #[test]
    fn test_truncate_left_basic() {
        assert_eq!(truncate_left("", 10), "");
        assert_eq!(truncate_left("abc", 10), "abc");
        assert_eq!(truncate_left("abcdefghij", 4), "…hij");
        assert_eq!(truncate_left("abcdefghij", 1), "…");
    }

    #[test]
    fn test_truncate_middle_basic() {
        assert_eq!(truncate_middle("abcdefghij", 7), "ab…hij");
        assert_eq!(truncate_middle("abcdefghij", 1), "…");
    }

    #[test]
    fn test_char_width() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('\u{3042}'), 2);
    }

    #[test]
    fn test_parse_workspace_rows_valid() {
        let value = json!([{"directory":"/home/user/projects","strategy":"git"}]);
        let rows = parse_workspace_rows(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].directory, "/home/user/projects");
        assert_eq!(rows[0].detail, "git");
    }

    #[test]
    fn test_workspace_picker_rows_current_first() {
        let rows = vec![
            WorkspaceRow { directory: "/projects/main".into(), label: String::new(), detail: String::new(), current: false, enabled: true },
            WorkspaceRow { directory: "/home/user/current".into(), label: String::new(), detail: String::new(), current: false, enabled: true },
        ];
        let result = workspace_picker_rows(rows, Some("/home/user/current"), Some("/home/user"), 80);
        assert_eq!(result.len(), 2);
        assert!(result[0].current);
        assert_eq!(result[0].label, "~/current");
    }

    #[test]
    fn test_workspace_picker_rows_subdirectory_suffix() {
        let rows = vec![
            WorkspaceRow { directory: "/projects/root".into(), label: String::new(), detail: String::new(), current: false, enabled: true },
            WorkspaceRow { directory: "/projects/root/sub/dir".into(), label: String::new(), detail: String::new(), current: false, enabled: true },
        ];
        let result = workspace_picker_rows(rows, Some("/projects/root"), None, 80);
        let sub = result.iter().find(|r| r.directory == "/projects/root/sub/dir").unwrap();
        assert_eq!(sub.detail, "/sub/dir");
    }

    #[test]
    fn test_relative_path() {
        assert_eq!(relative_path("/root", "/root"), Some(String::new()));
        assert_eq!(relative_path("/root", "/root/sub"), Some("sub".to_string()));
        assert_eq!(relative_path("/root", "/other"), None);
    }
}