//! Loaded source identity must be independent of a workspace directory alias.
#[cfg(unix)]
#[test]
fn workspace_alias_and_real_path_report_the_same_canonical_instruction_source() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(
        workspace.join("AGENTS.md"),
        "canonical instruction identity",
    )
    .unwrap();
    let alias = root.path().join("workspace-alias");
    std::os::unix::fs::symlink(&workspace, &alias).unwrap();
    let expected = workspace.join("AGENTS.md").canonicalize().unwrap();
    let direct = opencode_rk_server::rules_loader::load_rules(&workspace).unwrap();
    let aliased = opencode_rk_server::rules_loader::load_rules(&alias).unwrap();
    assert_eq!(direct.entries[0].path, expected);
    assert_eq!(
        aliased.entries[0].path, expected,
        "instruction inclusion keys must survive canonical workspace aliases"
    );
    assert_eq!(direct.entries[0].bytes, aliased.entries[0].bytes);
}
