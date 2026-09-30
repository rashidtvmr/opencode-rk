//! PROV-030 auth-source contract: inline content, persisted auth, and ambient key.
//! Source contract: upstream packages/opencode/src/auth/index.ts:58-67, 23-27.
//! This is an installed-daemon test. It deliberately owns no production code.
#![forbid(unsafe_code)]

use std::{
    fs::File,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};
use serde_json::{json, Value};
use tempfile::TempDir;

const INLINE_KEY: &str = "synthetic-prov030-inline-key";
const PERSISTED_KEY: &str = "synthetic-prov030-persisted-key";
const AMBIENT_KEY: &str = "synthetic-prov030-ambient-key";
const IO: Duration = Duration::from_secs(4);
const READY: Duration = Duration::from_secs(10);
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn read_http(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    stream.set_read_timeout(Some(IO))?;
    let mut out = Vec::new(); let mut buf = [0u8; 4096];
    loop { match stream.read(&mut buf) { Ok(0) => break, Ok(n) => { out.extend_from_slice(&buf[..n]); if out.len() > 256*1024 { return Err(io::Error::other("wire cap")); } }, Err(e) if e.kind() == io::ErrorKind::TimedOut => break, Err(e) => return Err(e) } }
    Ok(out)
}

struct Provider { url: String, stop: Option<mpsc::Sender<()>>, join: Option<thread::JoinHandle<Vec<Vec<u8>>>> }
impl Provider {
    fn new(body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("provider bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let addr = listener.local_addr().expect("provider address");
        let (tx, rx) = mpsc::channel();
        let join = thread::spawn(move || {
            let mut got = Vec::new(); let until = Instant::now() + READY;
            loop {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        let request = read_http(&mut socket).expect("provider request");
                        let bytes = body.as_bytes();
                        let head = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", bytes.len());
                        socket.write_all(head.as_bytes()).expect("provider response"); socket.write_all(bytes).expect("provider body");
                        got.push(request); return got;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => { if rx.recv_timeout(Duration::from_millis(20)).is_ok() { return got; } }
                    Err(_) => return got,
                }
                if Instant::now() >= until { return got; }
            }
        });
        Self { url: format!("http://{addr}/v1"), stop: Some(tx), join: Some(join) }
    }
    fn finish(mut self) -> Vec<Vec<u8>> { if let Some(tx) = self.stop.take() { let _ = tx.send(()); } self.join.take().unwrap().join().unwrap() }
}
impl Drop for Provider { fn drop(&mut self) { if let Some(tx) = self.stop.take() { let _ = tx.send(()); } if let Some(j) = self.join.take() { let _ = j.join(); } } }

struct Server { child: Child, origin: String, token: String, _home: TempDir }
impl Server {
    fn new(provider: &str, inline: Option<&str>, file: Option<Value>, ambient: Option<&str>, project: &Path) -> Self {
        let binary = PathBuf::from(std::env::var_os("OC2_TEST_BINARY").expect("OC2_TEST_BINARY required"));
        assert!(binary.is_file(), "OC2_TEST_BINARY is not a file");
        let home = tempfile::tempdir().expect("disposable HOME"); let xdg = home.path().join("x"); let data = home.path().join("d");
        assert!(data.join("runtime/opencode-rk.sock").to_string_lossy().len() < 104, "short socket root required");
        if let Some(value) = file { let dir = xdg.join("opencode"); std::fs::create_dir_all(&dir).unwrap(); let path = dir.join("auth.json"); std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap(); #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap(); } }
        let _lock = ENV_LOCK.lock().unwrap(); let mut cmd = Command::new(binary);
        cmd.env_clear().env("HOME", home.path()).env("USERPROFILE", home.path()).env("XDG_DATA_HOME", &xdg).env("OPENCODE_RK_HOME", &data).env("PATH", std::env::var_os("PATH").unwrap_or_default()).env("OPENAI_BASE_URL", provider).current_dir(project).arg("--data-dir").arg(&data).arg("serve").arg("--listen").arg("127.0.0.1:0").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(v) = inline { cmd.env("OPENCODE_AUTH_CONTENT", v); } if let Some(v) = ambient { cmd.env("OPENAI_API_KEY", v); }
        let child = cmd.spawn().expect("spawn daemon"); let descriptor = data.join("runtime/backend.json"); let deadline = Instant::now() + READY; let (origin, token);
        loop { if let Ok(bytes) = std::fs::read(&descriptor) { if bytes.len() < 8192 { if let Ok(v) = serde_json::from_slice::<Value>(&bytes) { if let (Some(o), Some(t)) = (v["http_origin"].as_str(), v["auth_token"].as_str()) { if !o.is_empty() && !t.is_empty() { origin=o.to_owned(); token=t.to_owned(); break; } } } } } assert!(Instant::now() < deadline, "bounded daemon readiness"); thread::yield_now(); }
        Self { child, origin, token, _home: home }
    }
}
impl Drop for Server { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }

fn daemon_request(s: &Server, method: &str, path: &str, body: Value) -> (u16, Vec<u8>) {
    let addr = s.origin.strip_prefix("http://").unwrap(); let mut stream = TcpStream::connect_timeout(&addr.parse().unwrap(), IO).unwrap(); let body = serde_json::to_vec(&body).unwrap();
    let h = format!("{method} {path} HTTP/1.1\r\nhost: {addr}\r\nauthorization: Bearer {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", s.token, body.len()); stream.write_all(h.as_bytes()).unwrap(); stream.write_all(&body).unwrap(); let bytes = read_http(&mut stream).unwrap(); let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap(); let status = String::from_utf8_lossy(&bytes[..split]).lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap(); (status, bytes[split+4..].to_vec())
}
fn session(s: &Server) -> String { let (status, body) = daemon_request(s, "POST", "/api/sessions", json!({"title":"auth source"})); assert_eq!(status, 201); serde_json::from_slice::<Value>(&body).unwrap()["session"]["id"].as_str().unwrap().to_owned() }
fn turn(s: &Server, id: &str) -> (u16, Vec<u8>) { daemon_request(s, "POST", &format!("/api/sessions/{id}/turns"), json!({"text":"source contract","model":"openai/gpt-5.6","reasoning_effort":"high"})) }
fn auth(req: &[u8]) -> Option<String> { let end=req.windows(4).position(|w|w==b"\r\n\r\n").unwrap(); String::from_utf8_lossy(&req[..end]).lines().skip(1).find_map(|line| { let (n,v)=line.split_once(':')?; if n.eq_ignore_ascii_case("authorization") { Some(v.trim().strip_prefix("Bearer ").unwrap().to_owned()) } else { None } }) }
fn records(key: &str, metadata: Option<Value>) -> Value { let mut v=json!({"openai":{"type":"api","key":key}}); if let Some(m)=metadata { v["openai"]["metadata"]=m; } v }
fn response() -> &'static str { r#"{"id":"resp030","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"source-ok"}]}]}"# }
fn success(body: &[u8]) { assert!(String::from_utf8_lossy(body).contains("source-ok")); }

#[test] fn invalid_inline_falls_through_to_persisted() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let s=Server::new(&p.url,Some("{"),Some(records(PERSISTED_KEY,None)),Some(AMBIENT_KEY),project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(PERSISTED_KEY)); }
#[test] fn empty_inline_falls_through_to_persisted() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let s=Server::new(&p.url,Some(""),Some(records(PERSISTED_KEY,None)),None,project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(PERSISTED_KEY)); }
#[test] fn valid_inline_api_key_wins_over_file_and_ambient() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let inline=serde_json::to_string(&records(INLINE_KEY,None)).unwrap(); let s=Server::new(&p.url,Some(&inline),Some(records(PERSISTED_KEY,None)),Some(AMBIENT_KEY),project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(INLINE_KEY)); }
#[test] fn valid_empty_object_inline_does_not_merge_file_and_uses_ambient() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let s=Server::new(&p.url,Some("{}"),Some(records(PERSISTED_KEY,None)),Some(AMBIENT_KEY),project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(AMBIENT_KEY)); }
#[test] fn schema_invalid_selected_file_record_falls_to_ambient() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let s=Server::new(&p.url,None,Some(records(PERSISTED_KEY,Some(json!({"bad":7})))),Some(AMBIENT_KEY),project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(AMBIENT_KEY)); }
#[test] fn valid_file_metadata_string_survives_malformed_sibling() { let p=Provider::new(response()); let project=tempfile::tempdir().unwrap(); let mut file=records(PERSISTED_KEY,Some(json!({"team":"test"}))); file["malformed"]=json!({"type":"api","key":7}); let s=Server::new(&p.url,None,Some(file),Some(AMBIENT_KEY),project.path()); let id=session(&s); let (st,b)=turn(&s,&id); assert_eq!(st,201); success(&b); let r=p.finish(); assert_eq!(auth(&r[0]).as_deref(),Some(PERSISTED_KEY)); }
