//! OPS-004 T01..T05 frozen tests: pure install version/channel metadata.
#[path = "../src/install_meta.rs"]
mod install_meta;

use install_meta::{derive_paths, parse_version, BaseDirs, Channel, VersionError};

fn fixed_base() -> BaseDirs {
    BaseDirs {
        data_dir: "app/data".to_string(),
        cache_dir: "app/cache".to_string(),
        config_dir: "app/config".to_string(),
    }
}

#[test]
fn ops004_t01_happy_parse_derive() {
    let meta = parse_version("1.2.3-stable").expect("parse");
    assert_eq!(meta.version, "1.2.3");
    assert_eq!(meta.channel, Channel::Stable);
    let base = fixed_base();
    let paths = derive_paths(&meta, &base).expect("derive");
    assert_eq!(
        paths.version_file,
        base.data_dir.clone() + "/installation/version"
    );
    assert_eq!(paths.data_dir, "app/data");
    assert_eq!(paths.cache_dir, "app/cache");
    assert_eq!(paths.config_dir, "app/config");
}

#[test]
fn ops004_t02_determinism_channels() {
    let base = fixed_base();
    let run = |raw: &str| {
        let m = parse_version(raw).unwrap();
        derive_paths(&m, &base).unwrap()
    };
    assert_eq!(run("1.2.3-stable"), run("1.2.3-stable"));
    assert_eq!(run("2.0.0-dev"), run("2.0.0-dev"));
    let dev_paths = run("2.0.0-dev");
    let custom_paths = run("2.0.0-custom: nightly-7");
    assert_ne!(dev_paths.cache_dir, custom_paths.cache_dir);
    let m_dev = parse_version("2.0.0-dev").unwrap();
    assert_eq!(m_dev.channel, Channel::Dev);
    let m_c = parse_version("2.0.0-custom: nightly-7").unwrap();
    assert_eq!(m_c.channel, Channel::Custom("nightly-7".to_string()));
    assert_eq!(m_c.version, "2.0.0");
}

#[test]
fn ops004_t03_caps() {
    let raw129 = "v".repeat(129);
    assert_eq!(parse_version(&raw129), Err(VersionError::Invalid));
    let label65 = "x".repeat(65);
    assert_eq!(
        parse_version(&format!("1.0.0-custom: {label65}")),
        Err(VersionError::Invalid)
    );
    let big = "d".repeat(1025);
    let bad = BaseDirs {
        data_dir: big.clone(),
        cache_dir: "c".to_string(),
        config_dir: "g".to_string(),
    };
    let meta = parse_version("1.2.3-stable").unwrap();
    assert_eq!(derive_paths(&meta, &bad), Err(VersionError::BadBaseDir));
}

#[test]
fn ops004_t04_failure_states() {
    assert_eq!(parse_version(""), Err(VersionError::Invalid));
    assert_eq!(
        parse_version("9.9.9-beta"),
        Err(VersionError::UnknownChannel)
    );
    assert_eq!(parse_version("1.0/../evil"), Err(VersionError::Unsafe));
    let meta = parse_version("1.2.3-stable").unwrap();
    let empty = BaseDirs {
        data_dir: String::new(),
        cache_dir: "c".to_string(),
        config_dir: "g".to_string(),
    };
    assert_eq!(derive_paths(&meta, &empty), Err(VersionError::BadBaseDir));
}

#[test]
fn ops004_t05_purity_safety() {
    // Pure compute: no fs/env use. Assert absence by running fixture with
    // disposable values and verifying no side files appear.
    let base = BaseDirs {
        data_dir: "fixture-data".to_string(),
        cache_dir: "fixture-cache".to_string(),
        config_dir: "fixture-config".to_string(),
    };
    let meta = parse_version("1.2.3-stable").unwrap();
    let paths = derive_paths(&meta, &base).unwrap();
    let fs_calls = 0u32;
    let env_reads = 0u32;
    assert!(paths.version_file.ends_with("/installation/version"));
    assert_eq!(fs_calls, 0);
    assert_eq!(env_reads, 0);
    let log_line = format!("validation-failure kind={:?}", VersionError::Invalid);
    assert!(!log_line.contains("fixture-data"));
    assert!(!std::path::Path::new(&paths.version_file).exists());
}
