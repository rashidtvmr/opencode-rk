//! Pure network-proxy + translation-adapter routing decision (ROUTE-006).
//!
//! Pure function of caller-supplied inputs: no I/O, env, net, clock, or threads.

use thiserror::Error;

/// Whether the outbound request goes direct or through a configured proxy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyMode {
    Direct,
    ViaProxy,
}

/// Which translation adapter applies to the outbound request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationAdapter {
    Passthrough,
    NineRouter,
    OpenAiCompatible,
}

/// Caller-supplied routing inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyRouteInput {
    pub target_host: String,
    pub proxy_url: Option<String>,
    pub no_proxy: Vec<String>,
    pub provider_id: String,
    pub model_id: String,
}

/// Routing decision returned to the provider call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyRouteDecision {
    pub mode: ProxyMode,
    pub proxy_host: Option<String>,
    pub adapter: TranslationAdapter,
}

/// Typed routing failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProxyRouteError {
    #[error("empty target host")]
    EmptyTargetHost,
    #[error("empty provider id")]
    EmptyProviderId,
    #[error("too many no_proxy entries: {actual} exceeds max {max}")]
    TooManyBypassEntries { max: usize, actual: usize },
}

/// Maximum accepted `no_proxy` bypass entries.
pub const MAX_NO_PROXY_ENTRIES: usize = 64;

/// Provider id selecting the 9router translation adapter.
const NINE_ROUTER_PROVIDER_ID: &str = "9router";
/// Prefix selecting the OpenAI-compatible translation adapter.
const OPENAI_COMPAT_PROVIDER_PREFIX: &str = "openai-compat-";

fn matches_bypass(target_host: &str, entry: &str) -> bool {
    let entry = entry.trim();
    if entry.is_empty() {
        return false;
    }
    if target_host == entry {
        return true;
    }
    // Suffix `.domain` match: target must be a subdomain of the entry.
    entry.strip_prefix('.').map_or_else(
        || {
            target_host
                .strip_suffix(entry)
                .is_some_and(|prefix| prefix.ends_with('.'))
        },
        |bare| target_host == bare || target_host.ends_with(entry),
    )
}

fn proxy_host(proxy_url: &str) -> Option<String> {
    // Strip scheme (`scheme://`), keep authority up to first `/`.
    let without_scheme = proxy_url
        .split_once("://")
        .map_or(proxy_url, |(_, rest)| rest);
    let host = without_scheme.split('/').next().unwrap_or("");
    (!host.is_empty()).then(|| host.to_owned())
}

fn select_adapter(provider_id: &str) -> TranslationAdapter {
    if provider_id == NINE_ROUTER_PROVIDER_ID {
        TranslationAdapter::NineRouter
    } else if provider_id.starts_with(OPENAI_COMPAT_PROVIDER_PREFIX) {
        TranslationAdapter::OpenAiCompatible
    } else {
        TranslationAdapter::Passthrough
    }
}

/// Decide proxy mode and translation adapter as a pure function of `input`.
///
/// Order: validate empties, enforce the bypass-entry bound, apply the
/// `no_proxy` bypass, then resolve proxy mode and adapter.
pub fn decide_proxy_route(input: &ProxyRouteInput) -> Result<ProxyRouteDecision, ProxyRouteError> {
    if input.target_host.is_empty() {
        return Err(ProxyRouteError::EmptyTargetHost);
    }
    if input.provider_id.is_empty() {
        return Err(ProxyRouteError::EmptyProviderId);
    }
    if input.no_proxy.len() > MAX_NO_PROXY_ENTRIES {
        return Err(ProxyRouteError::TooManyBypassEntries {
            max: MAX_NO_PROXY_ENTRIES,
            actual: input.no_proxy.len(),
        });
    }

    let adapter = select_adapter(&input.provider_id);
    let bypassed = input
        .no_proxy
        .iter()
        .any(|entry| matches_bypass(&input.target_host, entry));
    let proxy_url = input.proxy_url.as_deref().filter(|url| !url.is_empty());

    match (bypassed, proxy_url) {
        (true, _) | (false, None) => Ok(ProxyRouteDecision {
            mode: ProxyMode::Direct,
            proxy_host: None,
            adapter,
        }),
        (false, Some(url)) => Ok(ProxyRouteDecision {
            mode: ProxyMode::ViaProxy,
            proxy_host: proxy_host(url),
            adapter,
        }),
    }
}
