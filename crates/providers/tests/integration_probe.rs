use opencode_rk_providers::integration_probe::{plan_probes, ProbeError, MAX_PROBES};

#[test]
fn probe_t01_valid_plans() {
    let plans = plan_probes(&[
        ("openai", "https://api.openai.com/health", 1_000),
        ("anthropic", "https://api.anthropic.com/health", 2_000),
    ])
    .expect("valid probes should plan");
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].integration_id, "openai");
    assert_eq!(plans[0].endpoint, "https://api.openai.com/health");
    assert_eq!(plans[0].timeout_ms, 1_000);
    assert_eq!(plans[1].integration_id, "anthropic");
    assert_eq!(plans[1].endpoint, "https://api.anthropic.com/health");
    assert_eq!(plans[1].timeout_ms, 2_000);
}

#[test]
fn probe_t02_empty_id_rejected() {
    assert_eq!(
        plan_probes(&[("", "https://example.com/health", 1_000)]).expect_err("empty id"),
        ProbeError::EmptyId
    );
}

#[test]
fn probe_t03_bad_endpoint_rejected() {
    assert_eq!(
        plan_probes(&[("a", "", 1_000)]).expect_err("empty endpoint"),
        ProbeError::EmptyEndpoint
    );
    assert_eq!(
        plan_probes(&[("a", "http://example.com/health", 1_000)]).expect_err("non-https"),
        ProbeError::BadEndpoint
    );
}

#[test]
fn probe_t04_zero_timeout_rejected() {
    assert_eq!(
        plan_probes(&[("a", "https://example.com/health", 0)]).expect_err("zero timeout"),
        ProbeError::ZeroTimeout
    );
}

#[test]
fn probe_t05_overflow_rejected() {
    assert_eq!(MAX_PROBES, 32);
    let rows: Vec<(String, String, u64)> = (0..MAX_PROBES + 1)
        .map(|i| {
            (
                format!("id-{i}"),
                "https://example.com/health".to_owned(),
                1_000,
            )
        })
        .collect();
    let refs: Vec<(&str, &str, u64)> = rows
        .iter()
        .map(|(id, ep, t)| (id.as_str(), ep.as_str(), *t))
        .collect();
    assert_eq!(
        plan_probes(&refs).expect_err("overflow"),
        ProbeError::TooManyProbes {
            max: MAX_PROBES,
            actual: MAX_PROBES + 1,
        }
    );
}
