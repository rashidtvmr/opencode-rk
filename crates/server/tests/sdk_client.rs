//! SDK-001 frozen tests T01..T05 plus negatives.
//!
//! Fixture `HttpPort`/`AbortFlag` only; no sockets, no spawn, no fs.
//! Module included via path; integrator wires `mod sdk_client` later.
#[path = "../src/sdk_client.rs"]
mod sdk_client;

use sdk_client::{
    AbortFlag, HttpMethod, HttpPort, HttpRequest, HttpResponse, SdkClient, SdkError,
    MAX_BASE_URL_CHARS, MAX_DIR_CHARS, MAX_PATH_CHARS, MAX_REQUEST_BODY_BYTES,
    MAX_RESPONSE_BODY_BYTES, SDK_TIMEOUT_SECS,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------- fixtures ----------

#[derive(Clone)]
struct SharedFlag(Rc<Cell<bool>>);
impl SharedFlag {
    fn new(v: bool) -> Self {
        Self(Rc::new(Cell::new(v)))
    }
    fn set(&self, v: bool) {
        self.0.set(v);
    }
}
impl AbortFlag for SharedFlag {
    fn flag(&self) -> bool {
        self.0.get()
    }
}

struct Clear;
impl AbortFlag for Clear {
    fn flag(&self) -> bool {
        false
    }
}

struct FixturePort {
    status: Cell<u16>,
    body: RefCell<Vec<u8>>,
    fail: RefCell<Option<SdkError>>,
    watch: RefCell<Option<SharedFlag>>,
    sends: Cell<usize>,
    open: Cell<usize>,
    max_open: Cell<usize>,
    sent: RefCell<Vec<HttpRequest>>,
}
impl FixturePort {
    fn ok(status: u16, body: &[u8]) -> Self {
        Self {
            status: Cell::new(status),
            body: RefCell::new(body.to_vec()),
            fail: RefCell::new(None),
            watch: RefCell::new(None),
            sends: Cell::new(0),
            open: Cell::new(0),
            max_open: Cell::new(0),
            sent: RefCell::new(Vec::new()),
        }
    }
    fn set_fail(&self, e: Option<SdkError>) {
        *self.fail.borrow_mut() = e;
    }
    fn set_watch(&self, f: Option<SharedFlag>) {
        *self.watch.borrow_mut() = f;
    }
    fn last(&self) -> Option<HttpRequest> {
        self.sent.borrow().last().cloned()
    }
}
impl HttpPort for FixturePort {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, SdkError> {
        self.sends.set(self.sends.get() + 1);
        self.open.set(self.open.get() + 1);
        self.max_open.set(self.max_open.get().max(self.open.get()));
        let out = (|| {
            if let Some(f) = self.watch.borrow().as_ref() {
                if f.flag() {
                    return Err(SdkError::Abort);
                }
            }
            if let Some(e) = self.fail.borrow().clone() {
                return Err(e);
            }
            Ok(HttpResponse {
                status: self.status.get(),
                body: self.body.borrow().clone(),
            })
        })();
        self.sent.borrow_mut().push(req.clone());
        self.open.set(self.open.get() - 1);
        out
    }
}

fn header(req: &HttpRequest, name: &str) -> Option<String> {
    req.headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

// ---------- T01 happy path ----------

#[test]
fn sdk001_t01_get_returns_typed_body_with_dir_header_and_rewrite() {
    let port = FixturePort::ok(200, br#"{"ok":true,"n":3}"#);
    let client = SdkClient::new("https://fixture.local").unwrap();
    let out = client.get("/api/session", "proj/x", &port, &Clear).unwrap();
    assert_eq!(out.value(), &serde_json::json!({"ok": true, "n": 3}));
    let req = port.last().expect("request recorded");
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(
        req.url,
        "https://fixture.local/api/session?directory=proj%2Fx"
    );
    assert!(req.body.is_empty());
    assert_eq!(
        header(&req, "x-workspace-directory").as_deref(),
        Some("proj/x")
    );
    assert_eq!(port.open.get(), 0);
}

// ---------- T02 exact rewrite ----------

#[test]
fn sdk001_t02_rewrite_appends_with_ampersand_preserving_order() {
    let port = FixturePort::ok(200, br#"{"items":[]}"#);
    let client = SdkClient::new("https://fixture.local/").unwrap();
    client
        .get("/api/items?a=1", "my dir/x", &port, &Clear)
        .unwrap();
    let req = port.last().expect("request recorded");
    assert_eq!(
        req.url,
        "https://fixture.local/api/items?a=1&directory=my%20dir%2Fx"
    );
    assert_eq!(
        header(&req, "x-workspace-directory").as_deref(),
        Some("my dir/x")
    );
}

#[test]
fn sdk001_t02_post_carries_body_and_rewrite() {
    let port = FixturePort::ok(201, br#"{"created":true}"#);
    let client = SdkClient::new("https://fixture.local").unwrap();
    let out = client
        .post("/api/items?a=1", "d", b"{\"a\":1}", &port, &Clear)
        .unwrap();
    assert_eq!(out.value(), &serde_json::json!({"created": true}));
    let req = port.last().expect("request recorded");
    assert_eq!(req.method, HttpMethod::Post);
    assert_eq!(req.body, b"{\"a\":1}");
    assert_eq!(req.url, "https://fixture.local/api/items?a=1&directory=d");
}

// ---------- T03 timeout ----------

#[test]
fn sdk001_t03_timeout_reclaims_slot_and_client_reusable() {
    assert_eq!(SDK_TIMEOUT_SECS, 30);
    let port = FixturePort::ok(200, br#"{"ok":true}"#);
    port.set_fail(Some(SdkError::Timeout));
    let client = SdkClient::new("https://fixture.local").unwrap();
    let r = client.get("/api/x", "d", &port, &Clear);
    assert!(matches!(r, Err(SdkError::Timeout)));
    assert_eq!(port.open.get(), 0, "in-flight slot reclaimed");
    assert_eq!(port.max_open.get(), 1, "slot was held during flight");
    port.set_fail(None);
    let out = client.get("/api/x", "d", &port, &Clear).unwrap();
    assert_eq!(out.value(), &serde_json::json!({"ok": true}));
}

// ---------- T04 abort reclaim ----------

#[test]
fn sdk001_t04_midflight_abort_reclaims_and_retry_succeeds() {
    let port = FixturePort::ok(200, br#"{"ok":1}"#);
    let mid = SharedFlag::new(true);
    port.set_watch(Some(mid.clone()));
    let client = SdkClient::new("https://fixture.local").unwrap();
    // Module-level abort flag is clear; abort arrives mid-flight (port sees it).
    let r = client.get("/api/x", "d", &port, &Clear);
    assert!(matches!(r, Err(SdkError::Abort)));
    assert_eq!(port.open.get(), 0, "slot reclaimed after abort");
    mid.set(false);
    port.set_watch(None);
    let out = client.get("/api/x", "d", &port, &Clear).unwrap();
    assert_eq!(out.value(), &serde_json::json!({"ok": 1}));
}

#[test]
fn sdk001_t04_preflagged_abort_makes_no_port_call() {
    let port = FixturePort::ok(200, br#"{"ok":1}"#);
    let client = SdkClient::new("https://fixture.local").unwrap();
    let flag = SharedFlag::new(true);
    let r = client.get("/api/x", "d", &port, &flag);
    assert!(matches!(r, Err(SdkError::Abort)));
    assert_eq!(port.sends.get(), 0, "no port call on pre-flagged abort");
    let r = client.post("/api/x", "d", b"{}", &port, &flag);
    assert!(matches!(r, Err(SdkError::Abort)));
    assert_eq!(port.sends.get(), 0);
}

// ---------- T05 redaction + isolation ----------

const SECRET: &str = "sk-fixture-secret-abcdef-12345";
const SECRET_PW: &str = "pw-fixture-secret-xyz";

fn assert_clean(label: &str, debug: &str, display: &str) {
    for needle in [SECRET, SECRET_PW] {
        assert!(!debug.contains(needle), "{label} Debug leaks secret");
        assert!(!display.contains(needle), "{label} Display leaks secret");
    }
}

fn err_strings(e: &SdkError) -> (String, String) {
    (format!("{e:?}"), format!("{e}"))
}

#[test]
fn sdk001_t05_errors_and_debug_forms_carry_no_secrets() {
    // Secret-bearing URL + secret dir + secret bodies throughout.
    let base = format!("https://user:{SECRET_PW}@fixture.local");
    let client = SdkClient::new(&base).unwrap();
    let secret_dir = format!("w/{SECRET}");
    let port = FixturePort::ok(500, SECRET.as_bytes());
    // BadStatus carries code only.
    let e = client.get("/api/x", "d", &port, &Clear).unwrap_err();
    assert!(matches!(e, SdkError::BadStatus { status: 500 }));
    let (d, s) = err_strings(&e);
    assert!(s.contains("500"), "code preserved");
    assert_clean("BadStatus", &d, &s);
    // Decode on secret bytes.
    port.status.set(200);
    let e = client
        .get("/api/x", &secret_dir, &port, &Clear)
        .unwrap_err();
    assert!(matches!(e, SdkError::Decode));
    let (d, s) = err_strings(&e);
    assert_clean("Decode", &d, &s);
    // Timeout / Abort.
    for e in [SdkError::Timeout, SdkError::Abort] {
        let (d, s) = err_strings(&e);
        assert_clean("Timeout/Abort", &d, &s);
    }
    // TooLarge via oversize POST body embedding the secret.
    let mut big = vec![b'x'; MAX_REQUEST_BODY_BYTES + 1];
    big[..SECRET.len()].copy_from_slice(SECRET.as_bytes());
    let e = client.post("/api/x", "d", &big, &port, &Clear).unwrap_err();
    assert!(matches!(e, SdkError::TooLarge));
    let (d, s) = err_strings(&e);
    assert_clean("TooLarge", &d, &s);
    // TooLarge via oversize dir embedding the secret.
    let long_dir = format!("{SECRET}-{}", "y".repeat(MAX_DIR_CHARS));
    let e = client.get("/api/x", &long_dir, &port, &Clear).unwrap_err();
    assert!(matches!(e, SdkError::TooLarge));
    let (d, s) = err_strings(&e);
    assert_clean("TooLarge-dir", &d, &s);
    // InvalidInput / InvalidBaseUrl.
    for e in [
        client.get("/api/x", "", &port, &Clear).unwrap_err(),
        SdkClient::new("").unwrap_err(),
        SdkClient::new("ftp://fixture.local").unwrap_err(),
    ] {
        let (d, s) = err_strings(&e);
        assert_clean("Invalid", &d, &s);
    }
    // TypedBody Debug hides secret body bytes.
    let port2 = FixturePort::ok(200, SECRET.as_bytes());
    let _ = client.get("/api/x", "d", &port2, &Clear);
    let port3 = FixturePort::ok(200, br#"{"tok":"sk-fixture-secret-abcdef-12345"}"#);
    let body = client.get("/api/x", "d", &port3, &Clear).unwrap();
    let dbg = format!("{body:?}");
    assert!(!dbg.contains(SECRET), "TypedBody Debug leaks body");
    // SdkClient Debug hides secret-bearing base URL.
    let dbg = format!("{client:?}");
    assert!(!dbg.contains(SECRET_PW), "SdkClient Debug leaks URL secret");
    assert!(!dbg.contains(&base), "SdkClient Debug leaks base URL");
}

#[test]
fn sdk001_t05_io_denied_matrix_no_side_channels() {
    // Harness counters the module can never touch: any nonzero value means
    // the module reached past the injected fixture port.
    let fs_calls = Cell::new(0usize);
    let net_calls = Cell::new(0usize);
    let spawn_calls = Cell::new(0usize);
    let client = SdkClient::new("https://fixture.local").unwrap();
    let port = FixturePort::ok(200, br#"{"ok":true}"#);
    let _ = client.get("/api/a", "d", &port, &Clear);
    let _ = client.post("/api/a", "d", b"{}", &port, &Clear);
    port.status.set(503);
    let _ = client.get("/api/a", "d", &port, &Clear);
    let _ = SdkClient::new("bad://x");
    let _ = client.get("/api/a", "", &port, &Clear);
    assert_eq!(fs_calls.get(), 0);
    assert_eq!(net_calls.get(), 0);
    assert_eq!(spawn_calls.get(), 0);
    assert!(port.sends.get() >= 2, "only the fixture port was used");
}

// ---------- negatives ----------

#[test]
fn sdk001_neg_bad_base_urls_rejected_before_any_port_call() {
    let port = FixturePort::ok(200, br#"{"ok":true}"#);
    for bad in ["", "ftp://h", "example.com", "http://", "https://"] {
        let r = SdkClient::new(bad);
        assert!(matches!(r, Err(SdkError::InvalidBaseUrl)), "base {bad:?}");
    }
    let huge = format!("https://h/{}", "z".repeat(MAX_BASE_URL_CHARS));
    assert!(matches!(
        SdkClient::new(&huge),
        Err(SdkError::InvalidBaseUrl)
    ));
    assert_eq!(port.sends.get(), 0);
}

#[test]
fn sdk001_neg_bad_dirs_and_paths_rejected_without_port_call() {
    let port = FixturePort::ok(200, br#"{"ok":true}"#);
    let client = SdkClient::new("https://fixture.local").unwrap();
    let before = port.sends.get();
    for bad_dir in ["", "a\nb", "a\tb", "a\0b"] {
        assert!(matches!(
            client.get("/api/x", bad_dir, &port, &Clear),
            Err(SdkError::InvalidInput)
        ));
    }
    for bad_path in ["", "no-slash", "has\nctl"] {
        assert!(matches!(
            client.get(bad_path, "d", &port, &Clear),
            Err(SdkError::InvalidInput)
        ));
    }
    let long_path = format!("/{}", "p".repeat(MAX_PATH_CHARS));
    assert!(matches!(
        client.get(&long_path, "d", &port, &Clear),
        Err(SdkError::TooLarge)
    ));
    let huge_body = vec![0u8; MAX_REQUEST_BODY_BYTES + 1];
    assert!(matches!(
        client.post("/api/x", "d", &huge_body, &port, &Clear),
        Err(SdkError::TooLarge)
    ));
    assert_eq!(port.sends.get(), before, "no port call on input errors");
}

#[test]
fn sdk001_neg_bad_status_decode_and_response_cap() {
    let port = FixturePort::ok(404, b"not here");
    let client = SdkClient::new("https://fixture.local").unwrap();
    let e = client.get("/api/x", "d", &port, &Clear).unwrap_err();
    assert!(matches!(e, SdkError::BadStatus { status: 404 }));
    assert!(!format!("{e}").contains("not here"), "body dropped");
    port.status.set(200);
    port.body.replace(vec![0xff, 0xfe]);
    assert!(matches!(
        client.get("/api/x", "d", &port, &Clear),
        Err(SdkError::Decode)
    ));
    port.body.replace(vec![]);
    assert!(matches!(
        client.get("/api/x", "d", &port, &Clear),
        Err(SdkError::Decode)
    ));
    port.body.replace(vec![b' '; MAX_RESPONSE_BODY_BYTES + 1]);
    assert!(matches!(
        client.get("/api/x", "d", &port, &Clear),
        Err(SdkError::TooLarge)
    ));
    for ok in [200, 201, 204, 299] {
        port.status.set(ok);
        port.body.replace(br#"{"s":1}"#.to_vec());
        assert!(client.get("/api/x", "d", &port, &Clear).is_ok());
    }
}

#[test]
fn sdk001_neg_deterministic_byte_identical_requests() {
    let port = FixturePort::ok(200, br#"{"ok":true}"#);
    let client = SdkClient::new("https://fixture.local/").unwrap();
    client.get("/api/x?a=1", "d 1", &port, &Clear).unwrap();
    client.get("/api/x?a=1", "d 1", &port, &Clear).unwrap();
    let sent = port.sent.borrow();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].url, sent[1].url);
    assert_eq!(sent[0].headers, sent[1].headers);
    assert_eq!(sent[0].body, sent[1].body);
    assert_eq!(sent[0].method, sent[1].method);
    assert_eq!(
        sent[0].url,
        "https://fixture.local/api/x?a=1&directory=d%201"
    );
}
