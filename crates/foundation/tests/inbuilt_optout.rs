//! REL-007 frozen tests T01..T05: unified opt-out for four inbuilt subsystems.
//!
//! Lane seam: shared `lib.rs`/`config.rs` are integrator-owned, so this target
//! includes the fragment directly (`#[path]`) and shims `crate::config` from
//! the built library. The fragment itself uses lib-correct `crate::config`
//! paths, so it compiles unchanged once the integrator adds
//! `pub mod inbuilt_optout;` plus `pub inbuilt: InbuiltFeatures` on `Config`.
//! Post-integration these same asserts hold through `Config::resolve`, which
//! merges the identical `inbuilt` subtree with the same precedence.

mod config {
    pub use opencode_rk_foundation::config::{ConfigError, ConfigLayer};
}

#[path = "../src/inbuilt_optout.rs"]
mod inbuilt_optout;

use config::{ConfigError as CfgErr, ConfigLayer as Layer};
use inbuilt_optout::{InbuiltFeatures, OptOutFeature};

// T01: defaults all on (mirrors `Config::resolve(&[])` + `config.inbuilt.*`).
#[test]
fn rel007_t01_defaults_all_on() {
    let cfg = InbuiltFeatures::resolve(&[]).expect("empty layers must resolve");
    assert!(cfg.codebase_index && cfg.rtk_filter && cfg.terse_mode && cfg.telemetry);
}

// T02: each flag flips independently, others stay on, set(true) restores.
#[test]
fn rel007_t02_each_flag_flips_independently() {
    for f in OptOutFeature::ALL {
        let mut cfg = InbuiltFeatures::default();
        cfg.set(*f, false);
        assert!(!cfg.enabled(*f), "{} must turn off", f.name());
        for other in OptOutFeature::ALL {
            if other != f {
                assert!(cfg.enabled(*other), "{} must stay on", other.name());
            }
        }
        cfg.set(*f, true);
        assert!(cfg.enabled(*f), "{} must restore", f.name());
    }
}

// T03: layered precedence defaults < file < env < CLI; unrelated env and
// unknown keys ignored; bad layer values map to InvalidValue.
#[test]
fn rel007_t03_env_layer_wins_over_file() {
    let file = Layer::from_toml_str("[inbuilt]\ntelemetry = false\n").unwrap();
    let env = Layer::from_env_vars([(
        String::from("OPENCODE_RK_INBUILT__TELEMETRY"),
        String::from("true"),
    )])
    .unwrap();
    let cfg = InbuiltFeatures::resolve(&[file, env]).unwrap();
    assert!(cfg.telemetry, "env must win over file");

    let file = Layer::from_toml_str("[inbuilt]\ntelemetry = true\n").unwrap();
    let env = Layer::from_env_vars([(
        String::from("OPENCODE_RK_INBUILT__TELEMETRY"),
        String::from("false"),
    )])
    .unwrap();
    let cfg = InbuiltFeatures::resolve(&[file, env]).unwrap();
    assert!(!cfg.telemetry, "reverse polarity must also hold");

    let env = Layer::from_env_vars([(String::from("UNRELATED_VAR"), String::from("true"))]).unwrap();
    let cfg = InbuiltFeatures::resolve(&[env]).unwrap();
    assert!(cfg.telemetry, "unrelated env must be ignored");

    let file = Layer::from_json_str(r#"{"inbuilt":{"rtk_filter":false}}"#).unwrap();
    let env = Layer::from_env_vars([(
        String::from("OPENCODE_RK_INBUILT__RTK_FILTER"),
        String::from("true"),
    )])
    .unwrap();
    let cli = Layer::from_pairs([("inbuilt.rtk_filter", "false")]).unwrap();
    let cfg = InbuiltFeatures::resolve(&[file, env, cli]).unwrap();
    assert!(!cfg.rtk_filter, "CLI must win over env");
    assert!(cfg.telemetry && cfg.codebase_index && cfg.terse_mode);

    let unknown =
        Layer::from_json_str(r#"{"inbuilt":{"future_flag":false},"other":{}}"#).unwrap();
    let cfg = InbuiltFeatures::resolve(&[unknown]).unwrap();
    assert_eq!(cfg, InbuiltFeatures::default(), "unknown keys must be ignored");

    let wrong_type = Layer::from_json_str(r#"{"inbuilt":{"telemetry":42}}"#).unwrap();
    assert!(
        matches!(
            InbuiltFeatures::resolve(&[wrong_type]),
            Err(CfgErr::InvalidValue(_))
        ),
        "wrong-type layer value must be InvalidValue"
    );
}

// T04: disabled gate errors before init; gated constructor builds nothing.
struct FakeSubsystem {
    flag: OptOutFeature,
}

fn build_subsystem(cfg: &InbuiltFeatures, flag: OptOutFeature) -> Result<FakeSubsystem, CfgErr> {
    cfg.require(flag)?;
    Ok(FakeSubsystem { flag })
}

#[test]
fn rel007_t04_disabled_gate_returns_clean_error_before_init() {
    for f in OptOutFeature::ALL {
        let mut cfg = InbuiltFeatures::default();
        assert_eq!(cfg.require(*f), Ok(()));
        assert!(build_subsystem(&cfg, *f).is_ok());
        assert_eq!(build_subsystem(&cfg, *f).unwrap().flag, *f);
        cfg.set(*f, false);
        assert_eq!(cfg.require(*f), Err(CfgErr::FeatureDisabled(f.name())));
        assert!(
            build_subsystem(&cfg, *f).is_err(),
            "disabled subsystem must not be built"
        );
    }
}

// T05: exhaustive default-on guard; fifth flag without test update fails.
#[test]
fn rel007_t05_exhaustive_default_on() {
    assert_eq!(OptOutFeature::ALL.len(), 4);
    for f in OptOutFeature::ALL {
        assert!(
            InbuiltFeatures::default().enabled(*f),
            "default must enable {}",
            f.name()
        );
        assert_eq!(
            OptOutFeature::parse(f.name()),
            Some(*f),
            "parse must round-trip {}",
            f.name()
        );
    }
    assert_eq!(OptOutFeature::parse("nonexistent"), None);
    assert_eq!(OptOutFeature::parse(""), None);
}
