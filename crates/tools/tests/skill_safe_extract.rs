use std::{fs, path::Path};

use opencode_rk_tools::skill_gate::extract_skill_file;

#[test]
fn ext_013_t01_safe_relative_extraction_preserves_exact_bytes() {
    let root = tempfile::tempdir().expect("temporary extraction root");
    let bytes = b"# bundled skill\n\x00exact-bytes\r\n";

    extract_skill_file(root.path(), Path::new("skill.md"), bytes)
        .expect("safe relative skill path should extract");

    assert_eq!(
        fs::read(root.path().join("skill.md")).expect("extracted skill should be readable"),
        bytes
    );
}

#[test]
fn ext_013_t02_absolute_and_traversal_paths_are_rejected_without_writes() {
    let outer = tempfile::tempdir().expect("temporary containment root");
    let root = outer.path().join("skills");
    fs::create_dir(&root).expect("create extraction root");

    let traversal_target = outer.path().join("escaped.txt");
    let absolute_target = outer.path().join("absolute.txt");

    let traversal = extract_skill_file(&root, Path::new("../escaped.txt"), b"escape");
    let absolute = extract_skill_file(&root, &absolute_target, b"absolute");

    assert!(traversal.is_err(), "parent traversal must be rejected");
    assert!(absolute.is_err(), "absolute paths must be rejected");
    assert!(
        !traversal_target.exists(),
        "traversal rejection must not write outside root"
    );
    assert!(
        !absolute_target.exists(),
        "absolute-path rejection must not create the target"
    );
    assert_eq!(
        fs::read_dir(&root)
            .expect("extraction root should remain readable")
            .count(),
        0,
        "rejected paths must not leave entries in the extraction root"
    );
}

#[test]
fn ext_013_t03_existing_file_is_not_overwritten() {
    let root = tempfile::tempdir().expect("temporary extraction root");
    let target = root.path().join("skill.md");
    fs::write(&target, b"existing-content").expect("seed existing file");

    let result = extract_skill_file(root.path(), Path::new("skill.md"), b"replacement");

    assert!(
        result.is_err(),
        "exclusive creation must refuse an existing file"
    );
    assert_eq!(
        fs::read(&target).expect("existing file should remain readable"),
        b"existing-content",
        "refused extraction must preserve the existing bytes"
    );
}

#[test]
fn ext_013_t04_final_symlink_target_is_refused() {
    let root = tempfile::tempdir().expect("temporary extraction root");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let backing = root.path().join("backing.txt");
        let target = root.path().join("skill.md");
        fs::write(&backing, b"sentinel").expect("seed symlink backing file");
        symlink(&backing, &target).expect("create final-component symlink");

        let result = extract_skill_file(root.path(), Path::new("skill.md"), b"replacement");

        assert!(result.is_err(), "final symlink must not be followed");
        assert_eq!(
            fs::read(&backing).expect("backing file should remain readable"),
            b"sentinel",
            "refused extraction must not modify the symlink target"
        );
        assert!(
            fs::symlink_metadata(&target)
                .expect("symlink should still exist")
                .file_type()
                .is_symlink(),
            "refused extraction must leave the symlink itself intact"
        );
    }
}

#[test]
fn ext_013_t05_created_file_is_owner_only_mode_0600_on_unix() {
    let root = tempfile::tempdir().expect("temporary extraction root");

    extract_skill_file(root.path(), Path::new("skill.md"), b"private")
        .expect("safe skill should extract");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(root.path().join("skill.md"))
            .expect("extracted skill metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "extracted skill must be owner-read/write only");
    }
}
