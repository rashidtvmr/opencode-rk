#![forbid(unsafe_code)]
//! Default chat TUI: `opencode-rk` with no subcommand.
//!
//! Attaches to the singleton daemon (auto-spawning one bound to this data
//! directory when none is running) and drives the existing HTTP surface:
//! sessions, models, and provider turns. Every line is bounded, degraded
//! states are explicit (`[offline]`, `[error]`), and nothing buffers without
//! a cap.
//!
//! Daemon ownership rule: a daemon this process spawned is owned by the
//! per-data-directory service and survives client exit; a pre-existing daemon
//! is left running. The default listen address
//! is discovered from this data directory's authenticated descriptor. New
//! services use an available loopback port unless `OPENCODE_RK_DAEMON_ADDR`
//! explicitly selects a listen address.

use std::{
    io::{BufRead, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

use serde_json::Value;

use crate::daemon_client;
use opencode_rk_server::daemon as server_daemon;

const DEFAULT_DAEMON_ADDR: &str = "127.0.0.1:4096";
const DEFAULT_MODEL: &str = "openai/gpt-5.6";
/// Turn requests against real providers can be slow; reads are still bounded.
const TURN_READ_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_millis(750);
const MAX_BODY_BYTES: usize = 1024 * 1024;
const HISTORY_LIMIT: usize = 20;

fn daemon_addr() -> String {
    std::env::var("OPENCODE_RK_DAEMON_ADDR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_DAEMON_ADDR.to_owned())
}

fn daemon_origin(addr: &str) -> String {
    format!("http://{addr}")
}

/// Entry bound from `main.rs` when no subcommand is given.
pub fn run(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lease = prepare_daemon(data_dir);
    let origin = lease.origin.clone().unwrap_or_else(|| daemon_origin(&daemon_addr()));
    let attached = lease.origin.is_some();
    if !attached {
        println!(
            "[offline] daemon unavailable ({origin}); \
             start it manually with: opencode-rk serve"
        );
    }
    let mut chat = Chat {
        origin: lease.origin.clone(),
        auth: lease.auth.clone(),
        session: None,
        model: DEFAULT_MODEL.to_owned(),
    };
    chat.banner(&origin, attached);
    chat.bind_recent_session();
    chat.loop_until_exit()
}

/// Owned daemon lease used by the native entrypoint.  The lease deliberately
/// reuses the same descriptor validation and readiness path as line chat.
pub struct DaemonLease {
    pub origin: Option<String>,
    pub auth: Option<String>,
    owned: Option<Child>,
}

impl DaemonLease {
    pub fn close(mut self) {
        if let Some(child) = self.owned.take() {
            std::thread::spawn(move || {
                let mut child = child;
                let _ = child.wait();
            });
        }
    }
}

impl Drop for DaemonLease {
    fn drop(&mut self) {
        if let Some(child) = self.owned.take() {
            // The daemon is a service singleton, not a client child.  Move
            // the handle to a bounded-lifetime reaper so dropping the client
            // neither kills the shared daemon nor leaves a zombie behind.
            // `Child` is the only process handle we ever reap here; a
            // descriptor/PID from another origin is never acted upon.
            std::thread::spawn(move || {
                let mut child = child;
                let _ = child.wait();
            });
        }
    }
}

/// Acquire or attach to the singleton daemon without opening a second
/// lifecycle implementation.  Foreign listeners are never given a bearer:
/// the descriptor origin must match the probed origin and the descriptor must
/// pass `decide_lifecycle_authed`.
pub fn prepare_daemon(data_dir: &Path) -> DaemonLease {
    let requested = std::env::var("OPENCODE_RK_DAEMON_ADDR").ok()
        .filter(|value| !value.trim().is_empty());
    if let Some(origin) = published_target(data_dir, requested.as_deref()) {
        if let Some(auth) = reuse_credential(data_dir, &origin, probe_daemon(&origin)) {
            return DaemonLease { origin: Some(origin), auth: Some(auth), owned: None };
        }
    }
    // A healthy public /health response on 4096 is not ownership evidence.
    // In particular, another workspace/test service must not strand Setup or
    // receive this data directory's bearer. The singleton lock still belongs
    // to the data directory; the OS selects only the HTTP port.
    let owned = spawn_daemon(requested.as_deref().unwrap_or("127.0.0.1:0"), data_dir);
    let origin = published_target(data_dir, requested.as_deref());
    let auth = origin.as_ref().and_then(|origin| reuse_credential(data_dir, origin, probe_daemon(origin)));
    DaemonLease {
        origin: origin.filter(|_| auth.is_some()),
        auth,
        owned,
    }
}

fn published_target(data_dir: &Path, requested: Option<&str>) -> Option<String> {
    let descriptor = server_daemon::read_backend_descriptor(data_dir).ok()??;
    if requested.is_some_and(|address| address != "127.0.0.1:0" && daemon_origin(address) != descriptor.http_origin) { return None; }
    Some(descriptor.http_origin)
}

#[cfg(test)]
mod daemon_target_tests {
    use super::*;
    #[test]
    fn absent_descriptor_never_trusts_a_default_port_listener() {
        let root = std::env::temp_dir().join(format!("oc2-no-descriptor-{}", std::process::id()));
        assert!(published_target(&root, None).is_none());
    }
    #[test]
    fn discovery_reuses_own_published_ephemeral_port_and_honors_explicit_target() {
        let root = std::env::temp_dir().join(format!("oc2-published-target-{}", std::process::id()));
        server_daemon::publish_backend_descriptor_with_auth(&root, "127.0.0.1:18579".parse().unwrap(), "a".repeat(64)).unwrap();
        assert_eq!(published_target(&root, None).as_deref(), Some("http://127.0.0.1:18579"));
        assert_eq!(published_target(&root, Some("127.0.0.1:0")).as_deref(), Some("http://127.0.0.1:18579"));
        assert!(published_target(&root, Some("127.0.0.1:18580")).is_none());
        assert_eq!(published_target(&root, Some("127.0.0.1:18579")).as_deref(), Some("http://127.0.0.1:18579"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

struct Chat {
    origin: Option<String>,
    /// Bearer for `/api/*` (`"Bearer <64-hex>"`), threaded from
    /// [`daemon_client::decide_lifecycle_authed`] via [`reuse_credential`].
    /// `None` means fail-closed: every `/api/*` call errors, nothing sends.
    auth: Option<String>,
    session: Option<String>,
    model: String,
}

impl Chat {
    fn banner(&self, origin: &str, attached: bool) {
        println!("OpenCode RK");
        if attached {
            println!("daemon: {origin}");
        }
        println!(
            "model: {} | type /help for commands; plain text sends a turn",
            self.model
        );
    }

    fn help(&self) {
        println!(
            "/new [title]  create a session\n\
             /sessions     list sessions\n\
             /open <id>    open a session (id prefix is enough)\n\
             /models       list models\n\
             /model <p/m>  switch model (provider/model)\n\
             /exit         quit"
        );
    }

    /// Bind the most recently updated session so returning users land in
    /// their last conversation instead of an empty shell.
    fn bind_recent_session(&mut self) {
        let Some(origin) = &self.origin else { return };
        let auth = self.auth.clone();
        let Ok((status, body)) = request(origin, "GET", "/api/sessions", None, auth.as_deref())
        else {
            return;
        };
        if status != 200 {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(&body) else {
            return;
        };
        let Some(sessions) = value.get("sessions").and_then(Value::as_array) else {
            return;
        };
        let Some(first) = sessions.first() else {
            println!("no sessions yet; type /new to start one");
            return;
        };
        let id = first.get("id").and_then(Value::as_str).unwrap_or_default();
        let title = first.get("title").and_then(Value::as_str).unwrap_or("?");
        if id.is_empty() {
            return;
        }
        self.session = Some(id.to_owned());
        println!("session: {id} ({title})");
        self.print_history();
    }

    fn print_history(&self) {
        let Some(origin) = &self.origin else { return };
        let Some(session) = &self.session else { return };
        let auth = self.auth.clone();
        let path = format!("/api/sessions/{session}/messages?limit={HISTORY_LIMIT}");
        let Ok((200, body)) = request(origin, "GET", &path, None, auth.as_deref()) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&body) else {
            return;
        };
        let Some(messages) = value.get("messages").and_then(Value::as_array) else {
            return;
        };
        for message in messages {
            if let (Some(role), Some(text)) = (
                message.get("role").and_then(Value::as_str),
                message_text(message),
            ) {
                println!("{role}: {text}");
            }
        }
    }

    fn create_session(&mut self, title: Option<&str>) {
        let Some(origin) = &self.origin else {
            println!("[error] daemon offline; cannot create a session");
            return;
        };
        let title = title.unwrap_or("Chat").trim();
        let title = if title.is_empty() { "Chat" } else { title };
        let body = serde_json::json!({ "title": title }).to_string();
        let auth = self.auth.clone();
        match request(
            origin,
            "POST",
            "/api/sessions",
            Some(&body),
            auth.as_deref(),
        ) {
            Ok((201, response)) => match serde_json::from_str::<Value>(&response) {
                Ok(value) => {
                    let id = value
                        .pointer("/session/id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    if id.is_empty() {
                        println!("[error] session create returned no id");
                        return;
                    }
                    self.session = Some(id.clone());
                    println!("created: {id} ({title})");
                }
                Err(error) => println!("[error] malformed session response: {error}"),
            },
            Ok((status, response)) => {
                println!("[error] session create failed: {status} {response}")
            }
            Err(error) => println!("[error] {error}"),
        }
    }

    fn list_sessions(&self) {
        let Some(origin) = &self.origin else {
            println!("[error] daemon offline; cannot list sessions");
            return;
        };
        let auth = self.auth.clone();
        match request(origin, "GET", "/api/sessions", None, auth.as_deref()) {
            Ok((200, body)) => match serde_json::from_str::<Value>(&body) {
                Ok(value) => {
                    for session in value
                        .get("sessions")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default()
                    {
                        let id = session.get("id").and_then(Value::as_str).unwrap_or("?");
                        let title = session.get("title").and_then(Value::as_str).unwrap_or("?");
                        println!("{id}  {title}");
                    }
                }
                Err(error) => println!("[error] malformed session list: {error}"),
            },
            Ok((status, body)) => println!("[error] session list failed: {status} {body}"),
            Err(error) => println!("[error] {error}"),
        }
    }

    fn open_session(&mut self, prefix: &str) {
        let Some(origin) = &self.origin else {
            println!("[error] daemon offline; cannot open a session");
            return;
        };
        let prefix = prefix.trim();
        if prefix.is_empty() {
            println!("[error] usage: /open <session-id>");
            return;
        }
        let auth = self.auth.clone();
        let Ok((200, body)) = request(origin, "GET", "/api/sessions", None, auth.as_deref()) else {
            println!("[error] could not list sessions to resolve {prefix}");
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&body) else {
            println!("[error] malformed session list");
            return;
        };
        let sessions = value
            .get("sessions")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let matches: Vec<&Value> = sessions
            .iter()
            .filter(|session| {
                session
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.starts_with(prefix))
            })
            .collect();
        match matches.as_slice() {
            [session] => {
                let id = session
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let title = session.get("title").and_then(Value::as_str).unwrap_or("?");
                self.session = Some(id.to_owned());
                println!("session: {id} ({title})");
                self.print_history();
            }
            [] => println!("[error] no session matches {prefix}"),
            _ => println!("[error] ambiguous prefix {prefix}; use /sessions"),
        }
    }

    fn list_models(&self) {
        let Some(origin) = &self.origin else {
            println!("[error] daemon offline; cannot list models");
            return;
        };
        let auth = self.auth.clone();
        match request(origin, "GET", "/api/models", None, auth.as_deref()) {
            Ok((200, body)) => match serde_json::from_str::<Value>(&body) {
                Ok(value) => {
                    let models = value
                        .get("models")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    if models.is_empty() {
                        println!(
                            "no models cached; run: opencode-rk models sync && opencode-rk models search --limit 25"
                        );
                    }
                    for model in models.iter().take(25) {
                        let provider = model.get("provider").and_then(Value::as_str).unwrap_or("?");
                        let id = model.get("id").and_then(Value::as_str).unwrap_or("?");
                        let name = model.get("name").and_then(Value::as_str).unwrap_or("?");
                        println!("{provider}/{id}  {name}");
                    }
                }
                Err(error) => println!("[error] malformed model list: {error}"),
            },
            Ok((status, body)) => println!("[error] model list failed: {status} {body}"),
            Err(error) => println!("[error] {error}"),
        }
    }

    fn set_model(&mut self, model: &str) {
        let model = model.trim();
        if model
            .split_once('/')
            .is_none_or(|(provider, id)| provider.is_empty() || id.is_empty())
        {
            println!("[error] model must use provider/model format");
            return;
        }
        self.model = model.to_owned();
        println!("model: {}", self.model);
    }

    fn send_turn(&mut self, text: &str) {
        let Some(origin) = &self.origin else {
            println!("[error] daemon offline; start it with: opencode-rk serve");
            return;
        };
        let Some(session) = &self.session else {
            println!("[error] no session; run /new first");
            return;
        };
        println!("you: {text}");
        let body = serde_json::json!({
            "text": text,
            "model": self.model,
            "reasoning_effort": "high",
        })
        .to_string();
        let path = format!("/api/sessions/{session}/turns");
        let auth = self.auth.clone();
        match request(origin, "POST", &path, Some(&body), auth.as_deref()) {
            Ok((201, response)) => match serde_json::from_str::<Value>(&response) {
                Ok(value) => {
                    let assistant = value
                        .get("assistant_message")
                        .and_then(message_text)
                        .unwrap_or_else(|| "(empty assistant reply)".to_owned());
                    println!("assistant: {assistant}");
                }
                Err(error) => println!("[error] malformed turn response: {error}"),
            },
            Ok((status, response)) => {
                let message = serde_json::from_str::<Value>(&response)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("message")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| response.clone());
                println!("[error] provider request failed ({status}): {message}");
            }
            Err(error) => println!("[error] provider request failed: {error}"),
        }
    }

    fn loop_until_exit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            match line.trim() {
                "/exit" | "/quit" | "/q" => break,
                "/help" | "?" => self.help(),
                "" => {}
                rest if rest.starts_with("/new") => {
                    self.create_session(rest.strip_prefix("/new").map(str::trim));
                }
                "/sessions" => self.list_sessions(),
                rest if rest.starts_with("/open") => {
                    let prefix = rest.strip_prefix("/open").unwrap_or("").trim();
                    self.open_session(prefix);
                }
                "/models" => self.list_models(),
                rest if rest.starts_with("/model") => {
                    let arg = rest.strip_prefix("/model").unwrap_or("").trim();
                    if arg.is_empty() {
                        println!("model: {}", self.model);
                    } else {
                        self.set_model(arg);
                    }
                }
                rest if rest.starts_with('/') => {
                    println!("[error] unknown command {rest}; /help lists commands");
                }
                text => self.send_turn(text),
            }
        }
        Ok(())
    }
}

/// Assistant/user inline message text from the wire shape
/// `{"role": ..., "body": {"storage": "inline", "text": ...}}`.
fn message_text(message: &Value) -> Option<String> {
    message
        .get("body")
        .and_then(|body| body.get("text"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// True only when a healthy daemon answers `GET /health` on the origin. A
/// bare TCP connect is not enough: an unrelated listener on the port must not
/// be mistaken for the daemon. `/health` stays public (`daemon_auth.rs:1-7`):
/// no bearer is sent here.
fn probe_daemon(origin: &str) -> bool {
    matches!(request(origin, "GET", "/health", None, None), Ok((200, _)))
}

/// Bearer for `/api/*` reuse, threaded from
/// [`daemon_client::decide_lifecycle_authed`] (`daemon_client.rs:722-735`).
/// `healthy` is the `/health` probe outcome (`true` == HTTP 200). Any
/// outcome but validated-descriptor-plus-healthy-probe yields `None`: the
/// `/api/*` calls below then fail closed without sending. Reads the
/// published descriptor via `server::daemon::read_backend_descriptor`
/// (`daemon.rs:137`), which already gates schema/PID/loopback/non-empty
/// token; the bearer shape is re-checked with
/// [`daemon_client::is_wellformed_token`] before it becomes a credential.
/// `probed_origin` is the origin the `/health` probe just hit: a published
/// descriptor bound to any other origin yields `None` so no bearer ever
/// travels to a foreign port that merely answered the probe.
fn reuse_credential(data_dir: &Path, probed_origin: &str, healthy: bool) -> Option<String> {
    let published = server_daemon::read_backend_descriptor(data_dir).ok()??;
    if published.http_origin != probed_origin {
        return None;
    }
    if !daemon_client::is_wellformed_token(&published.auth_token) {
        return None;
    }
    let authed = daemon_client::AuthenticatedDescriptor {
        descriptor: daemon_client::BackendDescriptor {
            pid: published.pid,
            http_origin: published.http_origin,
            schema_version: published.schema_version,
        },
        auth_token: published.auth_token,
    };
    let (_, credential) =
        daemon_client::decide_lifecycle_authed(Some(authed), healthy.then_some(200));
    credential
}

/// Spawn `serve` from this same binary and wait for readiness. Returns the
/// owned child process; the service remains alive after this client exits.
///
/// The child inherits this chat's data directory explicitly (`--data-dir`):
/// `serve` resolves its HOME/descriptor from it (`main.rs:resolve_data_dir`),
/// so the spawned daemon publishes the descriptor this chat reads via
/// `reuse_credential` instead of the default HOME. Environment fallback alone
/// cannot carry an explicit `--data-dir` override across `exec`.
fn spawn_daemon(addr: &str, data_dir: &Path) -> Option<Child> {
    let exe: PathBuf = std::env::current_exe().ok()?;
    let mut cmd = Command::new(exe);
    cmd.arg("--data-dir")
        .arg(data_dir)
        .arg("serve")
        .arg("--listen")
        .arg(addr);
    // Keep an explicit data-dir authoritative even when the parent ran under
    // a custom HOME: clap's `env = "OPENCODE_RK_HOME"` on `--data-dir`
    // overrides the env, so clearing it cannot regress the default path.
    cmd.env_remove("OPENCODE_RK_HOME");
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // Keep the frozen startup bound: at most 200 probes, 10ms apart.
    for _ in 0..200 {
        if server_daemon::read_backend_descriptor(data_dir).ok().flatten()
            .filter(|descriptor| descriptor.pid == child.id())
            .is_some_and(|descriptor| probe_daemon(&descriptor.http_origin)) {
            return Some(child);
        }
        if let Ok(Some(_status)) = child.try_wait() {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// Bounded std-only HTTP/1.1 request. Returns `(status, body)` with the body
/// capped at [`MAX_BODY_BYTES`]; larger responses are an error, never a hang.
///
/// `auth` is the `Authorization` header value (`"Bearer <64-hex>"`) threaded
/// from [`daemon_client::decide_lifecycle_authed`] via [`reuse_credential`].
/// Every `/api/*` path requires it: `None`/empty fails closed with `Err`
/// before any byte is sent, per the server gate (`daemon_auth.rs:1-7`).
/// `/health` stays public (liveness only) and never takes a credential.
fn request(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    auth: Option<&str>,
) -> Result<(u16, String), String> {
    let wire = build_request_wire(origin, method, path, body, auth)?;
    let host = origin
        .strip_prefix("http://")
        .unwrap_or(origin)
        .trim_end_matches('/');
    let timeout = if path == "/health" {
        PROBE_TIMEOUT
    } else {
        CONNECT_TIMEOUT
    };
    let address = host
        .to_socket_addrs()
        .map_err(|error| format!("resolve {host}: {error}"))?
        .next()
        .ok_or_else(|| format!("no address for {host}"))?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("connect {host}: {error}"))?;
    let read_timeout = if path.ends_with("/turns") {
        TURN_READ_TIMEOUT
    } else {
        CONNECT_TIMEOUT.max(Duration::from_secs(5))
    };
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;

    stream
        .write_all(wire.as_bytes())
        .map_err(|error| format!("write: {error}"))?;
    stream.flush().ok();

    let mut raw = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("read: {error}"))?;
        if read == 0 {
            break;
        }
        if raw.len() + read > MAX_BODY_BYTES {
            return Err(format!("response exceeds {MAX_BODY_BYTES} byte bound"));
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, payload) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "malformed response: no header terminator".to_owned())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "malformed response: no status line".to_owned())?;
    Ok((status, payload.to_owned()))
}

/// Pure wire builder for [`request`]: fail-closed bearer gate plus the
/// HTTP/1.1 bytes. `/health` never carries a credential; every `/api/*`
/// path requires `Some("Bearer <token>")` and errors otherwise. No socket
/// I/O here, so tests assert the gate without a network.
fn build_request_wire(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    auth: Option<&str>,
) -> Result<String, String> {
    let host = origin
        .strip_prefix("http://")
        .unwrap_or(origin)
        .trim_end_matches('/');
    let payload = body.unwrap_or("");
    if path.starts_with("/api/") {
        let credential = auth.unwrap_or("").trim();
        if credential.is_empty() {
            return Err(
                "missing daemon credential: refusing unauthenticated /api/* request".to_owned(),
            );
        }
        return Ok(format!(
            "{method} {path} HTTP/1.1\r\nhost: {host}\r\nauthorization: {credential}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        ));
    }
    Ok(format!(
        "{method} {path} HTTP/1.1\r\nhost: {host}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
        payload.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEARER: &str = "Bearer abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    #[test]
    fn api_wire_carries_bearer() {
        let wire = build_request_wire(
            "http://127.0.0.1:4096",
            "GET",
            "/api/sessions",
            None,
            Some(BEARER),
        )
        .expect("credentialed /api/* must build");
        assert!(
            wire.contains("authorization: Bearer "),
            "wire must carry the bearer: {wire:?}"
        );
        // Token bytes travel exactly once: in the header, never in the body.
        let body = wire.split("\r\n\r\n").nth(1).unwrap_or("");
        assert!(!body.contains("Bearer"), "bearer must not leak into body");
        assert_eq!(
            wire.matches("abcdef0123456789").count(),
            4,
            "token must appear once (4 x 16-hex chunks): {wire:?}"
        );
    }

    #[test]
    fn api_wire_missing_credential_fails_closed() {
        for auth in [None, Some(""), Some("   ")] {
            let error =
                build_request_wire("http://127.0.0.1:4096", "GET", "/api/sessions", None, auth)
                    .expect_err("credential-less /api/* must refuse");
            assert!(
                error.contains("refusing unauthenticated"),
                "fail-closed message expected, got: {error:?}"
            );
        }
    }

    #[test]
    fn health_probe_stays_unauthenticated() {
        let wire = build_request_wire("http://127.0.0.1:4096", "GET", "/health", None, None)
            .expect("public /health must build without a credential");
        assert!(
            !wire.to_ascii_lowercase().contains("authorization:"),
            "probe must not carry a bearer: {wire:?}"
        );
        assert!(
            wire.starts_with("GET /health HTTP/1.1\r\n"),
            "probe request line intact: {wire:?}"
        );
    }

    #[test]
    fn post_wire_bounds_body_without_credential_leak() {
        let wire = build_request_wire(
            "http://127.0.0.1:4096",
            "POST",
            "/api/sessions",
            Some(r#"{"title":"Chat"}"#),
            Some(BEARER),
        )
        .expect("credentialed POST must build");
        assert!(
            wire.contains("content-length: 16\r\n"),
            "content-length must bound the body: {wire:?}"
        );
        assert!(
            wire.ends_with(r#"{"title":"Chat"}"#),
            "body bytes intact: {wire:?}"
        );
    }
}
