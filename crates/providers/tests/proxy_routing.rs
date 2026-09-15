use opencode_rk_providers::proxy_route::{
    decide_proxy_route, ProxyMode, ProxyRouteError, ProxyRouteInput, TranslationAdapter,
    MAX_NO_PROXY_ENTRIES,
};

fn base_input() -> ProxyRouteInput {
    ProxyRouteInput {
        target_host: "api.example.com".to_owned(),
        proxy_url: None,
        no_proxy: Vec::new(),
        provider_id: "anthropic".to_owned(),
        model_id: "claude-x".to_owned(),
    }
}

#[test]
fn route_006_t01_direct_when_no_proxy_configured() {
    let decision = decide_proxy_route(&base_input()).expect("direct route");
    assert_eq!(decision.mode, ProxyMode::Direct);
    assert_eq!(decision.proxy_host, None);
    assert_eq!(decision.adapter, TranslationAdapter::Passthrough);
}

#[test]
fn route_006_t02_via_proxy_when_configured() {
    let input = ProxyRouteInput {
        proxy_url: Some("http://proxy.internal:8080".to_owned()),
        ..base_input()
    };
    let decision = decide_proxy_route(&input).expect("proxy route");
    assert_eq!(decision.mode, ProxyMode::ViaProxy);
    assert_eq!(decision.proxy_host.as_deref(), Some("proxy.internal:8080"));

    let with_path = ProxyRouteInput {
        proxy_url: Some("https://proxy.internal:3128/proxy".to_owned()),
        ..base_input()
    };
    let decision = decide_proxy_route(&with_path).expect("proxy route with path");
    assert_eq!(decision.mode, ProxyMode::ViaProxy);
    assert_eq!(decision.proxy_host.as_deref(), Some("proxy.internal:3128"));
}

#[test]
fn route_006_t03_no_proxy_bypass_matches() {
    let exact = ProxyRouteInput {
        target_host: "internal.example.com".to_owned(),
        proxy_url: Some("http://proxy.internal:8080".to_owned()),
        no_proxy: vec!["internal.example.com".to_owned()],
        ..base_input()
    };
    let decision = decide_proxy_route(&exact).expect("bypassed route");
    assert_eq!(decision.mode, ProxyMode::Direct);
    assert_eq!(decision.proxy_host, None);

    let suffix = ProxyRouteInput {
        target_host: "api.internal.example.com".to_owned(),
        proxy_url: Some("http://proxy.internal:8080".to_owned()),
        no_proxy: vec!["internal.example.com".to_owned()],
        ..base_input()
    };
    let decision = decide_proxy_route(&suffix).expect("suffix bypassed route");
    assert_eq!(decision.mode, ProxyMode::Direct);
    assert_eq!(decision.proxy_host, None);
}

#[test]
fn route_006_t04_adapter_selection_by_provider() {
    let nine = ProxyRouteInput {
        provider_id: "9router".to_owned(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&nine).expect("9router").adapter,
        TranslationAdapter::NineRouter
    );

    let compat = ProxyRouteInput {
        provider_id: "openai-compat-acme".to_owned(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&compat).expect("compat").adapter,
        TranslationAdapter::OpenAiCompatible
    );

    let other = ProxyRouteInput {
        provider_id: "anthropic".to_owned(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&other).expect("other").adapter,
        TranslationAdapter::Passthrough
    );
}

#[test]
fn route_006_t05_empty_and_overflow_are_typed_errors() {
    let empty_host = ProxyRouteInput {
        target_host: String::new(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&empty_host),
        Err(ProxyRouteError::EmptyTargetHost)
    );

    let empty_provider = ProxyRouteInput {
        provider_id: String::new(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&empty_provider),
        Err(ProxyRouteError::EmptyProviderId)
    );

    let overflow = ProxyRouteInput {
        no_proxy: (0..MAX_NO_PROXY_ENTRIES + 1)
            .map(|i| format!("host{i}.example.com"))
            .collect(),
        ..base_input()
    };
    assert_eq!(
        decide_proxy_route(&overflow),
        Err(ProxyRouteError::TooManyBypassEntries {
            max: MAX_NO_PROXY_ENTRIES,
            actual: MAX_NO_PROXY_ENTRIES + 1,
        })
    );
}
