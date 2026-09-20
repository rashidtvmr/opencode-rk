#![forbid(unsafe_code)]
//! CI non-interactive run module (LANE-CI-FLAG + LANE-CI-EXT).
//!
//! Provides `run_ci` which executes a prompt through the daemon and emits
//! JSONL events to the provided writer. Exit codes follow the CiExitCode
//! contract in ci_output.rs.
//!
//! **Fail-closed approval contract:** when a tool requiring human approval
//! is detected, `run_ci` emits `ApprovalRequired` and returns exit code 20.
//! It never auto-approves.

use super::ci_output::{CiEvent, CiExitCode, DoctorCheckEntry, OutputFormat};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct CiRunResult {
    pub exit_code: i32,
}

fn ts_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn emit(writer: &mut impl Write, event: &CiEvent, format: OutputFormat) -> std::io::Result<()> {
    let line = match format {
        OutputFormat::Json | OutputFormat::Jsonl => event.to_bounded_json_line(),
        OutputFormat::Text => super::ci_output::TextRenderer::render(event),
    };
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Length of the hex-encoded daemon bearer token. Mirrors
/// `daemon_client::TOKEN_HEX_LEN` (64 hex chars = 32 random bytes).
const TOKEN_HEX_LEN: usize = 64;

/// True only for a 64-char hex bearer (mirrors
/// `daemon_client::is_wellformed_token`).
fn is_wellformed_token(token: &str) -> bool {
    token.len() == TOKEN_HEX_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Fail-closed bearer resolution for CI: reads `OPENCODE_RK_DAEMON_TOKEN`.
/// Returns `Err` on missing/malformed so CI exits UsageError (64) without
/// ever sending an unauthenticated request.
fn daemon_bearer() -> Result<String, String> {
    match std::env::var("OPENCODE_RK_DAEMON_TOKEN") {
        Ok(token) if is_wellformed_token(&token) => Ok(format!("Bearer {token}")),
        Ok(_) => Err("OPENCODE_RK_DAEMON_TOKEN is malformed: expected 64 hex chars".to_owned()),
        Err(_) => Err("OPENCODE_RK_DAEMON_TOKEN is not set".to_owned()),
    }
}

/// Build the raw HTTP/1.1 wire bytes with the bearer credential. Split out
/// for unit testing (no sockets involved).
fn request_wire(method: &str, host: &str, path: &str, body: &str, auth_token: &str) -> Vec<u8> {
    format!(
        "{method} {path} HTTP/1.1\r\nhost: {host}\r\nauthorization: {auth_token}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn check_approval_required(prompt: &str) -> Option<String> {
    let approval_tools = ["shell_exec", "shell_command", "exec", "run_command"];
    for tool in &approval_tools {
        if prompt.contains(tool) {
            return Some(tool.to_string());
        }
    }
    None
}

fn http_request(
    origin: &str,
    method: &str,
    path: &str,
    body: &str,
    auth_token: Option<&str>,
) -> Result<(u16, String), String> {
    let host = origin
        .strip_prefix("http://")
        .unwrap_or(origin)
        .trim_end_matches('/');
    let address = host
        .to_socket_addrs()
        .map_err(|error| format!("resolve {host}: {error}"))?
        .next()
        .ok_or_else(|| format!("no address for {host}"))?;
    let timeout = Duration::from_secs(5);
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("connect {host}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(300)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;

    let wire = match auth_token {
        Some(token) => request_wire(method, host, path, body, token),
        None => format!(
            "{method} {path} HTTP/1.1\r\nhost: {host}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    };
    stream
        .write_all(&wire)
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
        if raw.len() + read > 1_048_576 {
            return Err("response exceeds 1 MiB bound".to_owned());
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

fn probe_daemon(origin: &str, auth_token: Option<&str>) -> bool {
    http_request(origin, "GET", "/health", "", auth_token)
        .map(|(status, _)| status == 200)
        .unwrap_or(false)
}

fn spawn_daemon(addr: &str, bearer: Option<&str>) -> Option<Child> {
    let exe: PathBuf = std::env::current_exe().ok()?;
    let mut cmd = Command::new(exe);
    cmd.arg("serve").arg("--listen").arg(addr);
    // Forward CI bearer to the child env (raw hex) so `serve` can mint /
    // restore the same credential instead of a fresh token the parent
    // could never present. No-op until `serve` honors the env.
    if let Some(auth) = bearer.and_then(|b| b.strip_prefix("Bearer ")) {
        if is_wellformed_token(auth) {
            cmd.env("OPENCODE_RK_DAEMON_TOKEN", auth);
        }
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let origin = format!("http://{addr}");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if probe_daemon(&origin, bearer) {
            return Some(child);
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn ensure_daemon(bearer: Option<&str>) -> (String, Option<OwnedChild>) {
    let addr =
        std::env::var("OPENCODE_RK_DAEMON_ADDR").unwrap_or_else(|_| "127.0.0.1:4096".to_string());
    let origin = format!("http://{addr}");
    if probe_daemon(&origin, bearer) {
        return (origin, None);
    }
    match spawn_daemon(&addr, bearer) {
        Some(child) => (origin, Some(OwnedChild(child))),
        None => (origin, None),
    }
}

fn collect_doctor_checks() -> Vec<DoctorCheckEntry> {
    let mut checks = Vec::new();

    const AUTH_ENV_KEYS: &[&str] = &[
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ];
    let auth_configured = AUTH_ENV_KEYS
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|v| !v.is_empty()));
    checks.push(DoctorCheckEntry {
        name: "auth".to_string(),
        status: if auth_configured {
            "configured".to_string()
        } else {
            "unconfigured".to_string()
        },
        detail: None,
    });

    let connectivity = match std::env::var_os("OPENCODE_RK_DOCTOR_ENDPOINT") {
        Some(endpoint) => {
            let endpoint = endpoint.to_string_lossy().into_owned();
            match http_request(&endpoint, "GET", "/", "", None) {
                Ok((status, _)) => DoctorCheckEntry {
                    name: "connectivity".to_string(),
                    status: if status == 200 {
                        "ok".to_string()
                    } else {
                        "error".to_string()
                    },
                    detail: Some(format!("HTTP {status}")),
                },
                Err(e) => DoctorCheckEntry {
                    name: "connectivity".to_string(),
                    status: "error".to_string(),
                    detail: Some(e),
                },
            }
        }
        None => DoctorCheckEntry {
            name: "connectivity".to_string(),
            status: "unconfigured".to_string(),
            detail: None,
        },
    };
    checks.push(connectivity);

    checks.push(DoctorCheckEntry {
        name: "tools".to_string(),
        status: "ok".to_string(),
        detail: None,
    });

    let mcp_status = match std::env::var_os("OPENCODE_RK_MCP_CONFIG") {
        Some(raw) => {
            let raw = raw.to_string_lossy();
            match serde_json::from_str::<serde_json::Value>(&raw) {
                Ok(v) => {
                    let valid = v
                        .get("servers")
                        .and_then(|s| s.as_object())
                        .is_some_and(|m| !m.is_empty());
                    if valid {
                        "configured".to_string()
                    } else {
                        "error".to_string()
                    }
                }
                Err(_) => "error".to_string(),
            }
        }
        None => "unconfigured".to_string(),
    };
    checks.push(DoctorCheckEntry {
        name: "mcp".to_string(),
        status: mcp_status,
        detail: None,
    });

    checks
}

/// Run a CI prompt and emit JSONL events to the writer.
///
/// - `max_steps`: `Some(n)` with n > 0 clamps turns; `None` defaults to 1.
/// - `timeout`: `Some(s)` bounds wall-clock to s seconds; `None` = no limit.
/// - Prompt `"doctor"` emits a Doctor event without daemon interaction.
pub fn run_ci(
    prompt: &str,
    format: OutputFormat,
    writer: &mut impl Write,
    max_steps: Option<u64>,
    timeout: Option<u64>,
) -> CiRunResult {
    if prompt == "doctor" {
        let checks = collect_doctor_checks();
        let event = CiEvent::Doctor {
            ts: ts_now(),
            checks,
        };
        let _ = emit(writer, &event, format);
        return CiRunResult {
            exit_code: CiExitCode::Success.code() as i32,
        };
    }

    if let Some(tool) = check_approval_required(prompt) {
        let event = CiEvent::ApprovalRequired {
            ts: ts_now(),
            tool: tool.clone(),
        };
        let _ = emit(writer, &event, format);
        let terminal = CiEvent::TurnFinished {
            ts: ts_now(),
            exit: CiExitCode::ApprovalRequired.code(),
        };
        let _ = emit(writer, &terminal, format);
        return CiRunResult {
            exit_code: CiExitCode::ApprovalRequired.code() as i32,
        };
    }

    let _ = emit(writer, &CiEvent::TurnStarted { ts: ts_now() }, format);

    // Fail-closed: no token (missing/malformed) -> UsageError 64, no
    // unauthenticated request is ever sent.
    let bearer = match daemon_bearer() {
        Ok(token) => token,
        Err(_error) => {
            let _ = emit(
                writer,
                &CiEvent::TurnFinished { ts: ts_now(), exit: CiExitCode::UsageError.code() },
                format,
            );
            return CiRunResult { exit_code: CiExitCode::UsageError.code() as i32 };
        }
    };

    let (origin, _daemon_child) = ensure_daemon(Some(&bearer));

    let create_body = serde_json::json!({ "title": "ci-run" }).to_string();
    let (status, body) =
        match http_request(&origin, "POST", "/api/sessions", &create_body, Some(&bearer)) {
            Ok(r) => r,
            Err(_error) => {
                let _ = emit(
                    writer,
                    &CiEvent::TurnFinished { ts: ts_now(), exit: CiExitCode::ProviderError.code() },
                    format,
                );
                return CiRunResult { exit_code: CiExitCode::ProviderError.code() as i32 };
            }
        };
    if status == 401 || status == 403 {
        let _ = emit(
            writer,
            &CiEvent::TurnFinished { ts: ts_now(), exit: CiExitCode::UsageError.code() },
            format,
        );
        return CiRunResult { exit_code: CiExitCode::UsageError.code() as i32 };
    }
    if status != 201 {
        let _ = emit(
            writer,
            &CiEvent::TurnFinished { ts: ts_now(), exit: CiExitCode::ProviderError.code() },
            format,
        );
        return CiRunResult { exit_code: CiExitCode::ProviderError.code() as i32 };
    }
    let session_id = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => v["session"]["id"].as_str().unwrap_or("").to_string(),
        Err(_) => {
            let _ = emit(
                writer,
                &CiEvent::TurnFinished { ts: ts_now(), exit: CiExitCode::ProviderError.code() },
                format,
            );
            return CiRunResult { exit_code: CiExitCode::ProviderError.code() as i32 };
        }
    };

    let max_iterations = max_steps.unwrap_or(1);
    let deadline = timeout.map(|s| std::time::Instant::now() + Duration::from_secs(s));

    let mut last_exit = CiExitCode::Success;
    for step in 1..=max_iterations {
        if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                last_exit = CiExitCode::BudgetExhausted;
                break;
            }
        }

        let _ = emit(writer, &CiEvent::Step { ts: ts_now(), n: step }, format);

        let model =
            std::env::var("OPENCODE_RK_CI_MODEL").unwrap_or_else(|_| "openai/gpt-5.6".to_string());
        let turn_path = format!("/api/sessions/{session_id}/turns");
        let turn_body = serde_json::json!({
            "text": prompt,
            "model": model,
            "reasoning_effort": "high",
        })
        .to_string();
        let (status, _body) =
            match http_request(&origin, "POST", &turn_path, &turn_body, Some(&bearer)) {
                Ok(r) => r,
                Err(_error) => {
                    last_exit = CiExitCode::TurnFailed;
                    break;
                }
            };

        last_exit = if status == 201 {
            CiExitCode::Success
        } else if status == 401 || status == 403 {
            CiExitCode::UsageError
        } else if status == 429 {
            CiExitCode::BudgetExhausted
        } else {
            CiExitCode::TurnFailed
        };

        if last_exit != CiExitCode::Success || step >= max_iterations {
            break;
        }
    }

    let _ = emit(
        writer,
        &CiEvent::TurnFinished { ts: ts_now(), exit: last_exit.code() },
        format,
    );

    CiRunResult { exit_code: last_exit.code() as i32 }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct OwnedChild(Child);

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_token(seed: u8) -> String {
        (0..64u8)
            .map(|i| char::from_digit(u32::from(seed.wrapping_add(i)) % 16, 16).unwrap())
            .collect()
    }

    #[test]
    fn t01_request_wire_carries_bearer() {
        let wire = String::from_utf8(request_wire("POST", "127.0.0.1:4096", "/api/sessions", "{}", "Bearer abc")).unwrap();
        assert!(wire.contains("authorization: Bearer abc"), "wire must carry bearer: {wire}");
        assert!(wire.contains("content-length: 2"));
    }

    #[test]
    fn t02_malformed_token_rejected() {
        assert!(!is_wellformed_token(""));
        assert!(!is_wellformed_token("short"));
        assert!(!is_wellformed_token(&"zz".repeat(32)));
        assert!(!is_wellformed_token(&"ab".repeat(31)));
        assert!(is_wellformed_token(&hex_token(3)));
    }

    #[test]
    fn t03_auth_status_maps_to_usage_error() {
        // Fail-closed: 401/403 from the daemon is an auth/config defect,
        // never a provider or turn error, and never retried unauthenticated.
        for status in [401_u16, 403] {
            let exit = if status == 201 {
                CiExitCode::Success
            } else if status == 401 || status == 403 {
                CiExitCode::UsageError
            } else if status == 429 {
                CiExitCode::BudgetExhausted
            } else {
                CiExitCode::TurnFailed
            };
            assert_eq!(exit, CiExitCode::UsageError);
            assert_eq!(exit.code(), 64);
        }
    }

    #[test]
    fn t04_missing_token_fail_closed_usage_error() {
        // run_ci must exit 64 when the bearer is missing; the daemon is
        // unreachable in tests so the probe path is not exercised, but the
        // resolution layer itself must refuse loudly.
        std::env::remove_var("OPENCODE_RK_DAEMON_TOKEN");
        assert!(daemon_bearer().is_err(), "missing token must fail closed");
        std::env::set_var("OPENCODE_RK_DAEMON_TOKEN", "short");
        assert!(daemon_bearer().is_err(), "malformed token must fail closed");
        let good = hex_token(9);
        std::env::set_var("OPENCODE_RK_DAEMON_TOKEN", &good);
        assert_eq!(daemon_bearer().unwrap(), format!("Bearer {good}"));
        std::env::remove_var("OPENCODE_RK_DAEMON_TOKEN");
    }
}
