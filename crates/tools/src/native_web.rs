//! Bounded native network tools. Search uses the pinned upstream public Exa
//! MCP transport. Fetch pins a checked public address and never follows redirects.
use std::{net::{IpAddr, SocketAddr}, time::Duration};
use opencode_rk_security::OperationIntent;
use reqwest::{Client, Url};
use serde_json::{Value, json};

pub const SEARCH_URL: &str = "https://mcp.exa.ai/mcp";
const MAX_RESPONSE: usize = 512 * 1024;
const MAX_OUTPUT: usize = 16 * 1024;

pub fn intent(name: &str, input: &Value) -> Result<OperationIntent, String> {
    if input.to_string().len() > 8192 { return Err("web arguments exceed limit".into()); }
    match name {
        "websearch" => {
            let query = input.get("query").and_then(Value::as_str).unwrap_or("");
            if query.trim().is_empty() || query.len() > 4096 || query.chars().any(char::is_control) {
                return Err("query must be nonempty text up to 4096 bytes".into());
            }
            for (field, max) in [("numResults", 10), ("contextMaxCharacters", MAX_OUTPUT as u64)] {
                if let Some(value) = input.get(field) {
                    if !value.as_u64().is_some_and(|n| n > 0 && n <= max) {
                        return Err(format!("{field} is outside the supported range"));
                    }
                }
            }
            for (field, values) in [("type", &["auto", "fast", "deep"][..]), ("livecrawl", &["fallback", "preferred"][..])] {
                if input.get(field).is_some_and(|v| !v.as_str().is_some_and(|v| values.contains(&v))) {
                    return Err(format!("unsupported {field}"));
                }
            }
        }
        "webfetch" => {
            fetch_url(input)?;
            if input.get("format").is_some_and(|v| !v.as_str().is_some_and(|s| matches!(s, "text" | "html"))) {
                return Err("webfetch supports text or html format".into());
            }
        }
        _ => return Err("unsupported web tool".into()),
    }
    Ok(OperationIntent::Tool { name: name.into(), description: input.to_string() })
}

fn fetch_url(input: &Value) -> Result<Url, String> {
    let raw = input.get("url").and_then(Value::as_str).ok_or("url is required")?;
    if raw.len() > 4096 { return Err("url exceeds limit".into()); }
    let url = Url::parse(raw).map_err(|_| "invalid URL")?;
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() {
        return Err("only credential-free HTTP(S) URLs are supported".into());
    }
    let host = url.host_str().ok_or("URL requires a host")?;
    let host = host.trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err("local network addresses are not allowed".into());
    }
    if host.parse::<IpAddr>().is_ok_and(|ip| !public_ip(ip)) {
        return Err("non-public network addresses are not allowed".into());
    }
    Ok(url)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a,b,c,_] = ip.octets();
            !ip.is_private() && !ip.is_loopback() && !ip.is_link_local()
                && !ip.is_multicast() && !ip.is_unspecified() && !ip.is_broadcast()
                && a != 0 && a < 224 && !(a == 100 && (64..=127).contains(&b))
                && !(a == 192 && b == 0 && (c == 0 || c == 2))
                && !(a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                && !(a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Accept ordinary global unicast only, excluding special-use and
            // documentation prefixes, mapped IPv4, ULA and local link scopes.
            (s[0] & 0xe000) == 0x2000 && s[0] != 0x2002 && (s[0] & 0xfff0) != 0x3ff0
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
        }
    }
}

async fn client_for(url: &Url) -> Result<Client, String> {
    let host = url.host_str().ok_or("URL requires a host")?.trim_matches(['[', ']']);
    let port = url.port_or_known_default().ok_or("invalid URL port")?;
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await
        .map_err(|_| "web DNS lookup failed")?.take(33).collect();
    if addresses.is_empty() || addresses.len() > 32 || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err("web destination did not resolve exclusively to public addresses".into());
    }
    Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(25))
        .user_agent("OpenCode-RK/0.1 native-web")
        .resolve_to_addrs(host, &addresses).build().map_err(|_| "web client initialization failed".into())
}

pub async fn execute(name: &str, input: &Value) -> Result<String, String> {
    intent(name, input)?;
    tokio::time::timeout(Duration::from_secs(30), execute_inner(name, input)).await
        .map_err(|_| "web request timed out".to_owned())?
}

async fn execute_inner(name: &str, input: &Value) -> Result<String, String> {
    let url = if name == "websearch" { Url::parse(SEARCH_URL).expect("fixed HTTPS URL") } else { fetch_url(input)? };
    let client = client_for(&url).await?;
    let request = if name == "websearch" {
        client.post(url).header("accept", "application/json, text/event-stream").json(&json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call","params":{
                "name":"web_search_exa","arguments":{
                    "query":input["query"],"numResults":input.get("numResults").cloned().unwrap_or(json!(8)),
                    "type":input.get("type").cloned().unwrap_or(json!("auto")),
                    "livecrawl":input.get("livecrawl").cloned().unwrap_or(json!("fallback")),
                    "contextMaxCharacters":input.get("contextMaxCharacters").cloned().unwrap_or(json!(12000))
                }
            }
        }))
    } else { client.get(url) };
    let mut response = request.send().await.map_err(|_| "web request failed")?;
    if !response.status().is_success() { return Err(format!("web HTTP {} (redirects are not followed)", response.status().as_u16())); }
    if response.content_length().is_some_and(|n| n > MAX_RESPONSE as u64) { return Err("web response exceeds size limit".into()); }
    let html = response.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|v| v.to_ascii_lowercase().contains("html"));
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "web response interrupted")? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE { return Err("web response exceeds size limit".into()); }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&bytes);
    let text = if name == "websearch" { search_text(&body)? } else { fetch_text(&body, html, input["format"].as_str().unwrap_or("text")) };
    Ok(bound_output(&text))
}

fn fetch_text(body: &str, html: bool, format: &str) -> String {
    if !html || format == "html" { return body.to_owned(); }
    // Bounded plain-text extraction, not a browser or Markdown converter.
    // Skip non-visible blocks before the output cap so page chrome/scripts
    // cannot consume the entire excerpt and hide the actual document.
    let lower = body.to_ascii_lowercase();
    let mut output = String::new();
    let mut cursor = 0;
    while cursor < body.len() {
        let Some(offset) = body[cursor..].find('<') else { output.push_str(&body[cursor..]); break; };
        let start = cursor + offset;
        output.push_str(&body[cursor..start]);
        let Some(end) = body[start..].find('>') else { output.push_str(&body[start..]); break; };
        let end = start + end + 1;
        let tag = lower[start+1..end-1].trim();
        let name = tag.split_whitespace().next().unwrap_or("").trim_end_matches('/');
        if matches!(name, "head" | "script" | "style" | "noscript" | "svg") {
            if let Some(close) = lower[end..].find(&format!("</{name}")) {
                let close = end + close;
                cursor = lower[close..].find('>').map_or(body.len(), |n| close+n+1);
                continue;
            }
            break;
        }
        if matches!(name, "br" | "p" | "/p" | "div" | "/div" | "li" | "/li" | "h1" | "/h1" | "h2" | "/h2" | "h3" | "/h3" | "tr" | "/tr" | "pre" | "/pre") {
            output.push('\n');
        }
        cursor = end;
    }
    let mut text = String::new();
    let mut rest = output.as_str();
    while let Some(start) = rest.find('&') {
        text.push_str(&rest[..start]);
        let entity = &rest[start..];
        let end = entity.find(';').filter(|n| *n <= 12);
        let decoded = end.and_then(|end| match &entity[1..end] {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" | "#39" => Some('\''), "nbsp" => Some(' '),
            value if value.starts_with("#x") || value.starts_with("#X") => u32::from_str_radix(&value[2..],16).ok().and_then(char::from_u32),
            value if value.starts_with('#') => value[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        }).filter(|c| !c.is_control());
        if let (Some(end), Some(character)) = (end, decoded) {
            text.push(character); rest = &entity[end+1..];
        } else { text.push('&'); rest = &entity[1..]; }
    }
    text.push_str(rest);
    text.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n")
}

fn search_text(body: &str) -> Result<String, String> {
    fn decode(payload: &str) -> Option<String> {
        let value: Value = serde_json::from_str(payload).ok()?;
        if value.get("error").is_some() || value["result"]["isError"] == true { return None; }
        let content = value["result"]["content"].as_array()?;
        let texts: Vec<_> = content.iter().filter_map(|item| item["text"].as_str()).collect();
        (!texts.is_empty()).then(|| texts.join("\n"))
    }
    decode(body.trim()).or_else(|| body.lines().filter_map(|line| line.strip_prefix("data: ")).find_map(decode))
        .ok_or_else(|| "search service returned no usable results".into())
}

fn bound_output(text: &str) -> String {
    if text.len() <= MAX_OUTPUT { return text.to_owned(); }
    let mut end = MAX_OUTPUT;
    while !text.is_char_boundary(end) { end -= 1; }
    format!("{}\n[web output truncated]", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_private_literal_and_credential_urls_before_network() {
        for url in ["http://127.0.0.1", "http://2130706433", "http://[::1]", "http://[::ffff:127.0.0.1]", "http://169.254.169.254", "http://10.0.0.1", "http://user:secret@example.com", "file:///etc/passwd"] {
            assert!(intent("webfetch", &json!({"url":url})).is_err(), "{url}");
        }
    }
    #[test]
    fn search_binds_all_arguments_and_bounds_query_and_result_count() {
        assert!(intent("websearch", &json!({"query":"Rust"})).is_ok());
        for input in [json!({"query":""}), json!({"query":"x","numResults":11}), json!({"query":"x","type":"bogus"}), json!({"query":"x","contextMaxCharacters":100000})] {
            assert!(intent("websearch", &input).is_err());
        }
        assert_ne!(intent("websearch", &json!({"query":"a"})).unwrap(), intent("websearch", &json!({"query":"b"})).unwrap());
    }
    #[test]
    fn parses_real_mcp_json_and_sse_and_rejects_error_envelopes() {
        let payload = json!({"result":{"content":[{"type":"text","text":"Title: Rust\nURL: https://rust-lang.org"}]}}).to_string();
        assert!(search_text(&payload).unwrap().contains("https://rust-lang.org"));
        assert_eq!(search_text(&format!("event: message\ndata: {payload}\n\n")).unwrap(), search_text(&payload).unwrap());
        assert!(search_text(r#"{"error":{"message":"private error"}}"#).is_err());
    }
    #[test]
    fn public_address_policy_and_utf8_output_budget() {
        for ip in ["192.0.2.1", "198.18.0.1", "100.64.0.1", "224.0.0.1", "2001:db8::1", "fc00::1", "2002:7f00:1::1", "3fff::1"] { assert!(!public_ip(ip.parse().unwrap())); }
        for ip in ["8.8.8.8", "2606:4700:4700::1111"] { assert!(public_ip(ip.parse().unwrap())); }
        let output = bound_output(&"界".repeat(MAX_OUTPUT));
        assert!(output.len() < MAX_OUTPUT + 64);
        assert!(output.ends_with("[web output truncated]"));
    }
    #[test]
    fn text_fetch_extracts_readable_html_and_preserves_raw_mode() {
        let source = "<html><head><style>private css</style><script>hidden js</script></head><body><h1>Rust &amp; tools</h1><p>Real page content.</p></body></html>";
        let text = fetch_text(source, true, "text");
        assert!(text.contains("Rust & tools"));
        assert!(text.contains("Real page content."));
        assert!(!text.contains("hidden js") && !text.contains("private css") && !text.contains("<html>"));
        assert_eq!(fetch_text(source,true,"html"),source);
        assert_eq!(fetch_text("2 < 3",false,"text"),"2 < 3");
    }
}
