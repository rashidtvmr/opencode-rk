#![forbid(unsafe_code)]

//! Installed default-entrypoint journey through the real macOS PTY.
//!
//! Source evidence (current code, 2026-09-23):
//! - `crates/cli/src/main.rs:225-271` - no-subcommand planning and native route.
//! - `crates/cli/src/app_start.rs:287-320` - TTY owner/attacher and setup/main view.
//! - `crates/cli/src/chat.rs:99-137,657-698` - authenticated singleton discovery,
//!   spawn, and client lease.
//! - `crates/cli/src/daemon_client.rs:661-743` - bounded descriptor, bearer,
//!   loopback, and PID validation.
//! - `crates/cli/src/tui_entry.rs:615-739` - native frame and provider dialog.
//!
//! Native readiness requires a closed synchronized frame containing the Home
//! composer and command hint. The obsolete `OpenCode RK (native)` body banner
//! is deliberately absent from the current layout. The assertions retain the
//! frame, auth, singleton, setup, and alternate-screen contracts.
//!
//! Approved native Home workflow: unlike upstream's automatic empty-provider
//! overlay, launch renders Home and explicit `/connect` opens the real dialog.
//! The independent contract decision is recorded in
//! `docs/worklogs/ui-parity-003/missing-provider-startup-contract-review.json`.
//! Setup availability, auth, terminal and secret assertions remain required;
//! the earlier automatic-overlay failures remain preserved.
//! - `crates/cli/tests/app001_repair_e2e.rs:18-124` - bounded macOS PTY
//!   capture, disposable roots, loopback selection, and descriptor helpers.

// Portable lifecycle contract: this remains compiled and runnable on every target.
// The journey itself is macOS-only because it drives the real `/usr/bin/script` PTY.
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static PORTABLE_NEXT: AtomicU64 = AtomicU64::new(0);

fn fresh_home(tag: &str) -> std::path::PathBuf {
    let counter = PORTABLE_NEXT.fetch_add(1, Ordering::Relaxed);
    let entropy = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "opencode2-e2e-{tag}-{}-{counter}-{entropy}",
        std::process::id()
    ))
}

#[test]
fn rerun_starts_from_empty_disposable_home() {
    let home = fresh_home("empty");
    fs::create_dir_all(&home).expect("create disposable HOME");
    assert!(home.is_dir());
    assert!(fs::read_dir(&home)
        .expect("read disposable HOME")
        .next()
        .is_none());
    fs::remove_dir_all(&home).expect("remove disposable HOME");
    assert!(!home.exists());
}

#[cfg(target_os = "macos")]
mod macos_pty {
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, ExitStatus, Stdio},
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    const CAP: usize = 64 * 1024;
    const WAIT: Duration = Duration::from_secs(12);
    const CLEANUP_WAIT: Duration = Duration::from_secs(2);
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Root(PathBuf);

    impl Root {
        fn new() -> Self {
            // Darwin's sockaddr-unix limit includes the terminating NUL.  Keep
            // the exclusive fixture root deliberately short: data/runtime and
            // the daemon socket are appended by the production server.
            let base = std::env::temp_dir();
            for _ in 0..64 {
                let n = NEXT.fetch_add(1, Ordering::Relaxed) % 36_u64.pow(6);
                let mut digits = [b'0'; 6];
                let mut value = n;
                for digit in digits.iter_mut().rev() {
                    let remainder = (value % 36) as u8;
                    *digit = match remainder {
                        0..=9 => b'0' + remainder,
                        _ => b'a' + remainder - 10,
                    };
                    value /= 36;
                }
                let name = format!("oc2e{}", String::from_utf8_lossy(&digits));
                let path = base.join(name);
                let socket = path.join("data/runtime/opencode-rk.sock");
                assert!(
                    socket.as_os_str().as_encoded_bytes().len() + 1 <= 104,
                    "daemon socket path exceeds Darwin limit: {} bytes",
                    socket.as_os_str().as_encoded_bytes().len() + 1
                );
                match fs::create_dir(&path) {
                    Ok(()) => {
                        fs::create_dir(path.join("home")).expect("disposable HOME");
                        fs::create_dir(path.join("data")).expect("disposable data");
                        return Self(path);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create exclusive disposable root: {error}"),
                }
            }
            panic!("could not allocate exclusive disposable root")
        }

        fn home(&self) -> PathBuf {
            self.0.join("home")
        }

        fn data(&self) -> PathBuf {
            self.0.join("data")
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            if let Some(desc) = bounded_file(&self.data().join("runtime/backend.json")) {
                if let Some(pid) = descriptor_pid(&desc) {
                    terminate_pid(pid);
                }
            }
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct Bytes(Vec<u8>, usize);

    struct Capture {
        state: Arc<Mutex<Bytes>>,
        join: Option<thread::JoinHandle<()>>,
    }

    impl Capture {
        fn new(mut reader: impl Read + Send + 'static) -> Self {
            let state = Arc::new(Mutex::new(Bytes::default()));
            let copy = Arc::clone(&state);
            let join = thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    let count = match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => count,
                    };
                    let mut output = copy.lock().expect("capture lock");
                    if output.0.len() + count > CAP {
                        let remove = (output.0.len() + count - CAP).min(output.0.len());
                        output.0.drain(..remove);
                        output.1 = output.1.saturating_add(remove);
                    }
                    output.0.extend_from_slice(&buffer[..count]);
                }
            });
            Self {
                state,
                join: Some(join),
            }
        }

        fn text(&self) -> String {
            String::from_utf8_lossy(&self.state.lock().expect("capture lock").0).into_owned()
        }

        fn byte_len(&self) -> usize {
            let state = self.state.lock().expect("capture lock");
            state.1.saturating_add(state.0.len())
        }

        fn snapshot_since(&self, offset: usize) -> (usize, Vec<u8>) {
            let state = self.state.lock().expect("capture lock");
            let retained_offset = offset
                .checked_sub(state.1)
                .expect("causal capture baseline evicted");
            let bytes = state
                .0
                .get(retained_offset..)
                .expect("capture baseline beyond output")
                .to_vec();
            (state.1.saturating_add(state.0.len()), bytes)
        }

        fn text_since(&self, offset: usize) -> String {
            let state = self.state.lock().expect("capture lock");
            let retained_offset = offset
                .checked_sub(state.1)
                .expect("causal capture baseline evicted");
            String::from_utf8_lossy(
                state
                    .0
                    .get(retained_offset..)
                    .expect("capture baseline beyond output"),
            )
            .into_owned()
        }

        fn join(&mut self) {
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    impl Drop for Capture {
        fn drop(&mut self) {
            self.join();
        }
    }

    struct Pty {
        child: Child,
        input: std::process::ChildStdin,
        out: Capture,
        err: Capture,
    }

    impl Pty {
        fn spawn(bin: &Path, root: &Root, addr: &str, key: Option<&str>) -> Self {
            let mut command = Command::new("/usr/bin/script");
            command
                .args(["-q", "/dev/null"])
                .arg(bin)
                .args(["--data-dir", root.data().to_str().expect("data path")])
                .env_clear()
                .env("HOME", root.home())
                .env("OPENCODE_RK_HOME", root.home())
                .env("PATH", "/usr/bin:/bin")
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .env("TERM", "xterm-256color")
                .env("OPENCODE_RK_DAEMON_ADDR", addr)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(secret) = key {
                command.env("OPENAI_API_KEY", secret);
            }
            let mut child = command
                .spawn()
                .expect("spawn executable under macOS script PTY");
            let input = child.stdin.take().expect("PTY input");
            let out = Capture::new(child.stdout.take().expect("PTY stdout"));
            let err = Capture::new(child.stderr.take().expect("PTY stderr"));
            Self {
                child,
                input,
                out,
                err,
            }
        }

        fn send(&mut self, input: &str) {
            self.input.write_all(input.as_bytes()).expect("PTY write");
            self.input.flush().expect("PTY flush");
        }

        fn wait_for_native_frame(&self) -> String {
            let deadline = Instant::now() + WAIT;
            loop {
                // The initial wait has no action baseline: a completed Home
                // frame anywhere in the retained capture is eligible. The
                // snapshot still fails closed if capture eviction removed the
                // causal beginning at offset zero.
                let (_, bytes) = self.out.snapshot_since(0);
                if initial_home_frame(&bytes)
                    .expect("decode completed native Home frame")
                    .is_some()
                {
                    return self.out.text();
                }
                assert!(
                    Instant::now() < deadline,
                    "missing completed native Home frame\n{}",
                    self.out.text()
                );
                thread::sleep(Duration::from_millis(25));
            }
        }

        fn open_provider_dialog(&mut self) -> String {
            let (draft_start, home_screen, home_generation) = self.terminal_baseline();
            self.send("/connect");
            let draft = self.wait_for_terminal_frame_after(
                draft_start,
                home_generation,
                &home_screen,
                "missing nonmodal /connect composer draft",
                |screen| screen.has_connect_draft() && !screen.contains_text("Connect a provider"),
            );
            assert!(
                draft.generation > home_generation,
                "/connect draft reused an old completed frame"
            );

            // Use the latest completed screen immediately before Enter. This
            // makes the next frame baseline causal, not a raw-substring match.
            let (submit_start, suffix) = self.out.snapshot_since(draft_start);
            let before_submit = scan_terminal_frames(
                &suffix,
                draft_start,
                home_screen,
                home_generation,
                submit_start,
                |_| false,
            )
            .expect("decode pre-submit terminal state");
            assert!(
                !before_submit.partial,
                "cannot establish /connect baseline inside a partial frame"
            );
            assert!(
                before_submit.screen.has_connect_draft(),
                "composer draft changed before Enter\n{}",
                before_submit.screen.text()
            );
            assert!(
                !before_submit.screen.contains_text("Connect a provider"),
                "provider dialog opened before Enter\n{}",
                before_submit.screen.text()
            );

            self.send("\r");
            let provider = self.wait_for_terminal_frame_after(
                submit_start,
                before_submit.generation,
                &before_submit.screen,
                "missing completed provider dialog after Enter",
                |screen| screen.is_provider_dialog(),
            );
            assert!(
                provider.generation > before_submit.generation,
                "provider dialog reused a pre-Enter completed frame"
            );
            provider.screen.text()
        }

        fn terminal_baseline(&self) -> (usize, TerminalScreen, u64) {
            let (offset, bytes) = self.out.snapshot_since(0);
            let trace =
                scan_terminal_frames(&bytes, 0, TerminalScreen::default(), 0, offset, |_| false)
                    .expect("decode terminal baseline");
            assert!(
                !trace.partial,
                "terminal baseline ends inside an incomplete synchronized frame"
            );
            assert!(
                trace.generation > 0,
                "terminal baseline has no completed frame"
            );
            (offset, trace.screen, trace.generation)
        }

        fn wait_for_terminal_frame_after(
            &self,
            byte_offset: usize,
            generation: u64,
            baseline: &TerminalScreen,
            description: &str,
            matches: impl Fn(&TerminalScreen) -> bool,
        ) -> CompletedTerminalFrame {
            let deadline = Instant::now() + WAIT;
            loop {
                let (_, suffix) = self.out.snapshot_since(byte_offset);
                let trace = scan_terminal_frames(
                    &suffix,
                    byte_offset,
                    baseline.clone(),
                    generation,
                    byte_offset,
                    &matches,
                )
                .expect("decode completed terminal frames");
                if let Some(frame) = trace.matched {
                    return frame;
                }
                assert!(
                    Instant::now() < deadline,
                    "{description}\n{}",
                    self.out.text_since(byte_offset)
                );
                thread::sleep(Duration::from_millis(25));
            }
        }

        fn wait_for_completed_frame(&self, needles: &[&str]) -> String {
            self.wait_for_completed_frame_after(needles, 0)
        }

        fn wait_for_completed_frame_after(&self, needles: &[&str], byte_offset: usize) -> String {
            let deadline = Instant::now() + WAIT;
            while Instant::now() < deadline {
                if let Some(frame) =
                    completed_frame_with(&self.out.text_since(byte_offset), needles)
                {
                    return frame.to_owned();
                }
                thread::sleep(Duration::from_millis(25));
            }
            panic!(
                "missing completed native frame containing {needles:?}\nstdout={}\nstderr={}",
                self.out.text(),
                self.err.text()
            );
        }

        fn exit(&mut self) -> ExitStatus {
            let deadline = Instant::now() + WAIT;
            loop {
                if let Some(status) = self.child.try_wait().expect("PTY wait") {
                    self.out.join();
                    self.err.join();
                    return status;
                }
                if Instant::now() >= deadline {
                    terminate_child(&mut self.child);
                    panic!(
                        "executable did not exit\nstdout={}\nstderr={}",
                        self.out.text(),
                        self.err.text()
                    );
                }
                thread::sleep(Duration::from_millis(25));
            }
        }

        fn text(&self) -> String {
            format!("{}\n{}", self.out.text(), self.err.text())
        }
    }

    impl Drop for Pty {
        fn drop(&mut self) {
            terminate_child(&mut self.child);
            self.out.join();
            self.err.join();
        }
    }

    fn binary() -> PathBuf {
        std::env::var_os("OC2_E2E_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
    }

    fn address() -> String {
        TcpListener::bind("127.0.0.1:0")
            .expect("loopback address")
            .local_addr()
            .expect("loopback local address")
            .to_string()
    }

    fn bounded_file(path: &Path) -> Option<String> {
        let file = fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        file.take((CAP + 1) as u64).read_to_end(&mut bytes).ok()?;
        if bytes.len() > CAP {
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    fn field(text: &str, name: &str) -> Option<String> {
        let marker = format!("\"{name}\":\"");
        let start = text.find(&marker)? + marker.len();
        let end = text[start..].find('"')? + start;
        Some(text[start..end].to_owned())
    }

    fn descriptor_pid(text: &str) -> Option<u32> {
        let marker = "\"pid\":";
        let start = text.find(marker)? + marker.len();
        let digits: String = text[start..]
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect();
        digits.parse().ok().filter(|pid| *pid > 1)
    }

    fn wait_for_descriptor(root: &Root, addr: &str) -> String {
        let path = root.data().join("runtime/backend.json");
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            if let Some(desc) = bounded_file(&path) {
                if field(&desc, "http_origin").as_deref() == Some(format!("http://{addr}").as_str())
                    && field(&desc, "auth_token").is_some()
                    && descriptor_pid(&desc).is_some()
                {
                    return desc;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("backend descriptor not published at {}", path.display());
    }

    fn request(addr: &str, path: &str, bearer: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect_timeout(
            &addr.parse().expect("loopback address"),
            Duration::from_secs(1),
        )
        .expect("daemon connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("daemon read timeout");
        let authorization = bearer
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let wire = format!(
            "GET {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}Connection: close\r\n\r\n"
        );
        stream.write_all(wire.as_bytes()).expect("daemon request");
        let mut bytes = Vec::new();
        stream
            .take((CAP + 1) as u64)
            .read_to_end(&mut bytes)
            .expect("daemon response");
        assert!(bytes.len() <= CAP, "daemon response exceeded 64KiB");
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse().ok())
            .expect("HTTP status");
        (status, text)
    }

    fn health(addr: &str) -> bool {
        request(addr, "/health", None).0 == 200
    }

    fn process_alive(pid: u32) -> bool {
        Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn signal(pid: u32, signal_name: &str) {
        let _ = Command::new("/bin/kill")
            .args([signal_name, &pid.to_string()])
            .stderr(Stdio::null())
            .status();
    }

    fn terminate_pid(pid: u32) {
        if pid <= 1 || !process_alive(pid) {
            return;
        }
        signal(pid, "-TERM");
        let deadline = Instant::now() + CLEANUP_WAIT;
        while process_alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        if process_alive(pid) {
            signal(pid, "-KILL");
            let deadline = Instant::now() + CLEANUP_WAIT;
            while process_alive(pid) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(25));
            }
        }
    }

    fn terminate_child(child: &mut Child) {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        signal(child.id(), "-TERM");
        let deadline = Instant::now() + CLEANUP_WAIT;
        while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        if child.try_wait().ok().flatten().is_none() {
            signal(child.id(), "-KILL");
        }
        let _ = child.wait();
    }

    fn revision_receipt() -> Option<String> {
        std::env::var("OC2_E2E_REVISION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| option_env!("GIT_COMMIT").map(str::to_owned))
    }

    /// Ignore partial output and setup/title text outside synchronized frames.
    /// Return one complete frame, never a concatenation of different renders.
    fn completed_home_frame(text: &str) -> Option<&str> {
        completed_frame_with(text, &["Ask anything", "ctrl+p commands"])
    }

    fn completed_frame_with<'a>(text: &'a str, needles: &[&str]) -> Option<&'a str> {
        completed_frame_matching(text, |frame| {
            needles.iter().all(|needle| frame.contains(needle))
        })
    }

    const MAX_TERMINAL_ROWS: usize = 40;
    const MAX_TERMINAL_COLS: usize = 160;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TerminalScreen {
        cells: Vec<Vec<char>>,
    }

    impl Default for TerminalScreen {
        fn default() -> Self {
            Self {
                cells: vec![vec![' '; MAX_TERMINAL_COLS]; MAX_TERMINAL_ROWS],
            }
        }
    }

    impl TerminalScreen {
        fn text(&self) -> String {
            self.cells
                .iter()
                .map(|row| row.iter().collect::<String>().trim_end().to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn contains_text(&self, needle: &str) -> bool {
            self.cells
                .iter()
                .any(|row| row.iter().collect::<String>().contains(needle))
        }

        fn has_home_frame(&self) -> bool {
            self.contains_text("Ask anything") && self.contains_text("ctrl+p commands")
        }

        fn has_connect_draft(&self) -> bool {
            self.cells.iter().any(|row| {
                let row = row.iter().collect::<String>();
                ['┃', '│'].iter().any(|rail| {
                    row.split_once(*rail).is_some_and(|(_, tail)| {
                        tail.trim_start()
                            .strip_prefix("/connect")
                            .is_some_and(|rest| rest.trim().is_empty())
                    })
                })
            })
        }

        fn is_provider_dialog(&self) -> bool {
            self.contains_text("Connect a provider")
        }

        fn write(&mut self, row: usize, col: usize, text: &str) -> Result<usize, String> {
            if row >= MAX_TERMINAL_ROWS || col >= MAX_TERMINAL_COLS {
                return Err(format!(
                    "terminal CUP outside bounded grid: {};{}",
                    row + 1,
                    col + 1
                ));
            }
            let mut cursor = col;
            for character in text.chars() {
                if cursor >= MAX_TERMINAL_COLS {
                    return Err("terminal paint exceeded bounded columns".to_owned());
                }
                self.cells[row][cursor] = character;
                cursor += 1;
            }
            Ok(cursor)
        }

        fn erase_line(&mut self, row: usize, col: usize, mode: usize) {
            let (start, end) = match mode {
                1 => (0, col.saturating_add(1)),
                2 => (0, MAX_TERMINAL_COLS),
                _ => (col, MAX_TERMINAL_COLS),
            };
            if let Some(cells) = self.cells.get_mut(row) {
                for cell in cells.iter_mut().take(end).skip(start) {
                    *cell = ' ';
                }
            }
        }

        fn erase_display(&mut self, row: usize, col: usize, mode: usize) {
            match mode {
                1 => {
                    for line in self.cells.iter_mut().take(row) {
                        line.fill(' ');
                    }
                    self.erase_line(row, col, 1);
                }
                2 | 3 => self.cells.iter_mut().for_each(|line| line.fill(' ')),
                _ => {
                    self.erase_line(row, col, 0);
                    for line in self.cells.iter_mut().skip(row.saturating_add(1)) {
                        line.fill(' ');
                    }
                }
            }
        }
    }

    struct CompletedTerminalFrame {
        generation: u64,
        screen: TerminalScreen,
    }

    struct TerminalTrace {
        generation: u64,
        completed_end: usize,
        screen: TerminalScreen,
        partial: bool,
        matched: Option<CompletedTerminalFrame>,
    }

    /// Test-only bounded CUP screen projection. Predicates rely on ASCII
    /// command text and the single-cell `\xE2\x94\x83`/`│` rails only; no general Unicode
    /// display-width claim is made. Only fully closed synchronized frames paint.
    fn scan_terminal_frames(
        bytes: &[u8],
        absolute_offset: usize,
        mut screen: TerminalScreen,
        mut generation: u64,
        after_offset: usize,
        matches: impl Fn(&TerminalScreen) -> bool,
    ) -> Result<TerminalTrace, String> {
        const OPEN: &[u8] = b"\x1b[?2026h";
        const CLOSE: &[u8] = b"\x1b[?2026l";
        if bytes.len() > CAP {
            return Err("terminal capture exceeded 64KiB".to_owned());
        }
        let mut cursor = 0;
        let mut completed_end = absolute_offset;
        let mut matched = None;
        let mut partial = false;
        while let Some(relative_start) = find_bytes(&bytes[cursor..], OPEN) {
            let start = cursor + relative_start + OPEN.len();
            let Some(relative_end) = find_bytes(&bytes[start..], CLOSE) else {
                partial = true;
                break;
            };
            let end = start + relative_end;
            paint_terminal_frame(&mut screen, &bytes[start..end])?;
            generation = generation.saturating_add(1);
            completed_end = absolute_offset
                .saturating_add(end)
                .saturating_add(CLOSE.len());
            if completed_end > after_offset && matches(&screen) && matched.is_none() {
                matched = Some(CompletedTerminalFrame {
                    generation,
                    screen: screen.clone(),
                });
            }
            cursor = end + CLOSE.len();
        }
        Ok(TerminalTrace {
            generation,
            completed_end,
            screen,
            partial,
            matched,
        })
    }

    fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn paint_terminal_frame(screen: &mut TerminalScreen, bytes: &[u8]) -> Result<(), String> {
        let mut row = 0;
        let mut col = 0;
        let mut cursor = 0;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\x1b' if bytes.get(cursor + 1) == Some(&b'[') => {
                    let start = cursor + 2;
                    let Some(relative_end) = bytes[start..]
                        .iter()
                        .position(|byte| (0x40..=0x7e).contains(byte))
                    else {
                        return Err("incomplete CSI inside closed terminal frame".to_owned());
                    };
                    let end = start + relative_end;
                    let parameters = std::str::from_utf8(&bytes[start..end])
                        .map_err(|_| "non-ASCII CSI parameter")?;
                    let final_byte = bytes[end];
                    if matches!(final_byte, b'H' | b'f') {
                        let mut parts = parameters.split(';');
                        let parsed_row = parse_csi_position(parts.next())?;
                        let parsed_col = parse_csi_position(parts.next())?;
                        row = parsed_row.saturating_sub(1);
                        col = parsed_col.saturating_sub(1);
                        if row >= MAX_TERMINAL_ROWS || col >= MAX_TERMINAL_COLS {
                            return Err(format!(
                                "terminal CUP outside bounded grid: {parsed_row};{parsed_col}"
                            ));
                        }
                    } else if final_byte == b'K' {
                        screen.erase_line(row, col, parse_csi_mode(parameters)?);
                    } else if final_byte == b'J' {
                        screen.erase_display(row, col, parse_csi_mode(parameters)?);
                    }
                    // SGR and other CSI styling/control modes do not change text.
                    cursor = end + 1;
                }
                b'\x1b' if bytes.get(cursor + 1) == Some(&b']') => {
                    cursor += 2;
                    while cursor < bytes.len() {
                        if bytes[cursor] == 0x07 {
                            cursor += 1;
                            break;
                        }
                        if bytes[cursor] == b'\x1b' && bytes.get(cursor + 1) == Some(&b'\\') {
                            cursor += 2;
                            break;
                        }
                        cursor += 1;
                    }
                }
                b'\x1b' => cursor = (cursor + 2).min(bytes.len()),
                b'\r' => {
                    col = 0;
                    cursor += 1;
                }
                b'\n' => {
                    row = row.saturating_add(1).min(MAX_TERMINAL_ROWS - 1);
                    cursor += 1;
                }
                b'\t' => {
                    col = ((col / 8) + 1) * 8;
                    if col >= MAX_TERMINAL_COLS {
                        return Err("terminal tab exceeded bounded columns".to_owned());
                    }
                    cursor += 1;
                }
                byte if byte >= 0x20 && byte != 0x7f => {
                    let width = utf8_scalar_width(bytes, cursor)?;
                    let text = std::str::from_utf8(&bytes[cursor..cursor + width])
                        .map_err(|_| "invalid UTF-8 in terminal frame")?;
                    col = screen.write(row, col, text)?;
                    cursor += width;
                }
                _ => cursor += 1,
            }
        }
        Ok(())
    }

    fn parse_csi_position(value: Option<&str>) -> Result<usize, String> {
        match value.filter(|value| !value.is_empty()) {
            Some(value) => value
                .parse::<usize>()
                .map(|value| value.max(1))
                .map_err(|_| "invalid CUP coordinate".to_owned()),
            None => Ok(1),
        }
    }

    fn parse_csi_mode(parameters: &str) -> Result<usize, String> {
        match parameters
            .split(';')
            .next()
            .filter(|value| !value.is_empty())
        {
            Some(value) => value
                .parse::<usize>()
                .map_err(|_| "invalid erase mode".to_owned()),
            None => Ok(0),
        }
    }

    fn utf8_scalar_width(bytes: &[u8], offset: usize) -> Result<usize, String> {
        let first = *bytes.get(offset).ok_or("missing UTF-8 scalar")?;
        let width = if first < 0x80 {
            1
        } else if first & 0xe0 == 0xc0 {
            2
        } else if first & 0xf0 == 0xe0 {
            3
        } else if first & 0xf8 == 0xf0 {
            4
        } else {
            return Err("invalid UTF-8 leading byte in terminal frame".to_owned());
        };
        if offset.saturating_add(width) > bytes.len() {
            return Err("truncated UTF-8 scalar in closed terminal frame".to_owned());
        }
        std::str::from_utf8(&bytes[offset..offset + width])
            .map_err(|_| "invalid UTF-8 scalar in terminal frame")?;
        Ok(width)
    }

    fn completed_frame_matching(text: &str, matches: impl Fn(&str) -> bool) -> Option<&str> {
        const OPEN: &str = "\x1b[?2026h";
        const CLOSE: &str = "\x1b[?2026l";
        let mut rest = text;
        while let Some(start) = rest.find(OPEN) {
            let body = &rest[start + OPEN.len()..];
            let end = body.find(CLOSE)?;
            let frame = &body[..end];
            if matches(frame) {
                return Some(frame);
            }
            rest = &body[end + CLOSE.len()..];
        }
        None
    }

    /// Decode the initial retained PTY output with an action threshold of
    /// zero. Unlike post-action waits, startup has no pre-action byte offset;
    /// a closed Home frame in the retained capture is therefore eligible.
    fn initial_home_frame(bytes: &[u8]) -> Result<Option<CompletedTerminalFrame>, String> {
        Ok(scan_terminal_frames(
            bytes,
            0,
            TerminalScreen::default(),
            0,
            0,
            TerminalScreen::has_home_frame,
        )?
        .matched)
    }

    #[test]
    fn native_readiness_requires_both_markers_in_one_completed_frame() {
        let frame = "\x1b[?2026hAsk anything… ctrl+p commands\x1b[?2026l";
        assert_eq!(
            completed_home_frame(frame),
            Some("Ask anything… ctrl+p commands")
        );
        assert_eq!(completed_home_frame("Ask anything ctrl+p commands"), None);
        assert_eq!(
            completed_home_frame("\x1b[?2026hAsk anything ctrl+p commands"),
            None
        );
        assert_eq!(
            completed_home_frame(
                "\x1b[?2026hAsk anything\x1b[?2026l\x1b[?2026hctrl+p commands\x1b[?2026l"
            ),
            None
        );
        assert_eq!(
            completed_home_frame(&format!("\x1b[?2026hirrelevant\x1b[?2026l{frame}")),
            Some("Ask anything… ctrl+p commands")
        );
    }

    #[test]
    fn initial_home_wait_uses_zero_threshold_and_rejects_partial_frame() {
        let frame = b"\x1b[?2026h\x1b[2J\x1b[5;8HAsk anything\x1b[6;8Hctrl+p commands\x1b[?2026l";
        let completed = initial_home_frame(frame)
            .expect("decode initial Home fixture")
            .expect("closed Home frame must match initial wait");
        assert_eq!(completed.generation, 1);
        assert!(completed.screen.has_home_frame());

        let partial = b"\x1b[?2026h\x1b[2J\x1b[5;8HAsk anything\x1b[6;8Hctrl+p commands";
        let trace = scan_terminal_frames(
            partial,
            0,
            TerminalScreen::default(),
            0,
            0,
            TerminalScreen::has_home_frame,
        )
        .expect("decode partial initial Home fixture");
        assert!(trace.matched.is_none());
        assert!(trace.partial);
        assert!(initial_home_frame(partial)
            .expect("initial wait must decode partial fixture")
            .is_none());
    }

    #[test]
    fn connect_frame_wait_excludes_output_before_each_action() {
        let old = "\x1b[?2026h/connect Connect a provider\x1b[?2026l";
        let capture = Capture {
            state: Arc::new(Mutex::new(Bytes(old.as_bytes().to_vec(), 0))),
            join: None,
        };
        let baseline = capture.byte_len();
        assert!(
            completed_frame_with(&capture.text_since(baseline), &["Connect a provider"]).is_none()
        );
        capture
            .state
            .lock()
            .unwrap()
            .0
            .extend_from_slice(b"\x1b[?2026hConnect a provider");
        assert!(
            completed_frame_with(&capture.text_since(baseline), &["Connect a provider"]).is_none()
        );
        capture
            .state
            .lock()
            .unwrap()
            .0
            .extend_from_slice(b"\x1b[?2026l");
        assert_eq!(
            completed_frame_with(&capture.text_since(baseline), &["Connect a provider"]),
            Some("Connect a provider")
        );
    }

    #[test]
    fn connect_transition_requires_nonmodal_composer_before_enter() {
        let stale_modal = b"\x1b[?2026h\x1b[2J\x1b[3;5HConnect a provider\x1b[?2026l";
        let draft = b"\x1b[?2026h\x1b[2J\x1b[4;7H\xE2\x94\x83  /connect     \x1b[?2026l";
        let output = [stale_modal.as_slice(), draft.as_slice()].concat();
        let trace = scan_terminal_frames(&output, 0, TerminalScreen::default(), 0, 0, |screen| {
            screen.has_connect_draft() && !screen.is_provider_dialog()
        })
        .expect("decode full rail/CUP composer fixture");
        let observed = trace.matched.expect("decoded composer frame");
        assert_eq!(observed.generation, 2);
        assert!(observed.screen.has_connect_draft());
        assert!(!observed.screen.is_provider_dialog());

        let modal = scan_terminal_frames(
            b"\x1b[?2026h\x1b[2J\x1b[3;5HConnect a provider\x1b[4;7H\xE2\x94\x83  /connect \x1b[?2026l",
            0,
            TerminalScreen::default(),
            0,
            0,
            |screen| screen.has_connect_draft() && !screen.is_provider_dialog(),
        )
        .expect("decode modal negative fixture");
        assert!(modal.matched.is_none());

        let listing = scan_terminal_frames(
            b"\x1b[?2026h\x1b[2J\x1b[3;5HCommands /connect Connect provider\x1b[?2026l",
            0,
            TerminalScreen::default(),
            0,
            0,
            |screen| screen.has_connect_draft() && !screen.is_provider_dialog(),
        )
        .expect("decode command-listing negative fixture");
        assert!(listing.matched.is_none());

        let prefix = scan_terminal_frames(
            b"\x1b[?2026h\x1b[2J\x1b[4;7H\xE2\x94\x83 /connect-extra \x1b[?2026l",
            0,
            TerminalScreen::default(),
            0,
            0,
            |screen| screen.has_connect_draft() && !screen.is_provider_dialog(),
        )
        .expect("decode command-prefix negative fixture");
        assert!(prefix.matched.is_none());

        let provider_open = b"\x1b[?2026h\x1b[2J\x1b[3;5HConnect a provider";
        let incomplete = scan_terminal_frames(
            provider_open,
            0,
            observed.screen.clone(),
            observed.generation,
            output.len(),
            TerminalScreen::is_provider_dialog,
        )
        .expect("decode incomplete post-Enter provider frame");
        assert!(incomplete.matched.is_none());
        assert!(incomplete.partial);

        let provider = [provider_open.as_slice(), b"\x1b[?2026l"].concat();
        let completed = scan_terminal_frames(
            &provider,
            output.len(),
            observed.screen,
            observed.generation,
            output.len(),
            TerminalScreen::is_provider_dialog,
        )
        .expect("decode completed post-Enter provider frame");
        let provider_frame = completed.matched.expect("decoded provider dialog frame");
        assert!(provider_frame.generation > observed.generation);
        assert!(provider_frame.screen.is_provider_dialog());
    }

    fn strip_terminal_controls(text: &str) -> String {
        let mut output = String::with_capacity(text.len());
        let mut chars = text.chars();
        while let Some(character) = chars.next() {
            if character != '\x1b' {
                output.push(character);
                continue;
            }
            if chars.next() == Some('[') {
                for control in chars.by_ref() {
                    if ('@'..='~').contains(&control) {
                        break;
                    }
                }
            }
        }
        output.replace('\r', "")
    }

    /// Normalize only the wall-clock field emitted by the live snapshot.
    /// Typed transcript lines and all other frame assertions remain byte exact.
    fn normalize_dynamic_frame(text: &str) -> String {
        text.lines()
            .map(|line| {
                if let Some((prefix, _)) = line.split_once(", updated: ") {
                    format!("{prefix}, updated: <dynamic>")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn bare_no_command_enters_planned_native_route() {
        let root = Root::new();
        let addr = address();
        let mut pty = Pty::spawn(&binary(), &root, &addr, None);
        pty.wait_for_native_frame();
        let initial = pty.text();
        let provider_frame = pty.open_provider_dialog();
        assert!(
            provider_frame.contains("Connect a provider"),
            "provider dialog missing\n{initial}"
        );
        assert!(
            completed_home_frame(&initial).is_some(),
            "completed native Home frame missing\n{initial}"
        );
        assert!(
            !initial.contains("type /help for commands"),
            "compatibility chat route\n{initial}"
        );
        assert!(
            !initial.contains("manual") && !initial.contains("serve"),
            "manual serve path\n{initial}"
        );
        pty.send("\x1b:q\n");
        let status = pty.exit();
        assert!(
            status.success(),
            "planned native route exit: {status}\n{}",
            pty.text()
        );
    }

    #[test]
    fn single_authenticated_daemon_reused_by_second_client() {
        let root = Root::new();
        let addr = address();
        let bin = binary();
        let mut first = Pty::spawn(&bin, &root, &addr, Some("fixture-secret"));
        first.wait_for_native_frame();
        let first_desc = wait_for_descriptor(&root, &addr);
        let token = field(&first_desc, "auth_token").expect("bearer descriptor token");
        assert!(token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let first_pid = descriptor_pid(&first_desc).expect("daemon PID");
        assert_eq!(
            request(&addr, "/api/sessions", None).0,
            401,
            "API accepted no bearer"
        );
        assert_eq!(
            request(&addr, "/api/sessions", Some(&token)).0,
            200,
            "bearer rejected"
        );
        assert!(health(&addr), "daemon health failed");

        first.send("\x1b:q\n");
        assert!(
            first.exit().success(),
            "first client exit failed\n{}",
            first.text()
        );
        assert!(health(&addr), "first client exit killed shared daemon");

        let mut second = Pty::spawn(&bin, &root, &addr, Some("fixture-secret"));
        second.wait_for_native_frame();
        let second_desc = wait_for_descriptor(&root, &addr);
        assert_eq!(
            descriptor_pid(&second_desc).expect("second daemon PID"),
            first_pid,
            "second client started a second daemon"
        );
        assert_eq!(field(&second_desc, "auth_token"), Some(token));
        assert!(health(&addr), "second client daemon unhealthy");
        second.send("\x1b:q\n");
        assert!(
            second.exit().success(),
            "second client exit failed\n{}",
            second.text()
        );
    }

    #[test]
    fn missing_credentials_open_setup_without_offline_instruction_or_secret() {
        let root = Root::new();
        let addr = address();
        let mut pty = Pty::spawn(&binary(), &root, &addr, None);
        pty.wait_for_native_frame();
        let provider_frame = pty.open_provider_dialog();
        pty.send("\x1b:q\n");
        let status = pty.exit();
        let text = pty.text();
        assert!(status.success(), "setup exit: {status}\n{text}");
        assert!(
            provider_frame.contains("Connect a provider"),
            "provider dialog missing\n{text}"
        );
        assert!(!text.contains("offline"), "offline path exposed\n{text}");
        assert!(
            !text.contains("serve") && !text.contains("session create"),
            "manual instruction\n{text}"
        );
        assert!(
            !text.contains("OPENAI_API_KEY"),
            "raw credential name exposed\n{text}"
        );
        assert!(
            !text.contains("fixture-secret") && !text.contains("Bearer "),
            "raw secret exposed\n{text}"
        );
    }

    #[test]
    fn native_renderer_emits_frame_and_restores_alternate_screen() {
        let root = Root::new();
        let addr = address();
        let mut pty = Pty::spawn(&binary(), &root, &addr, Some("fixture-secret"));
        pty.wait_for_native_frame();
        let frame = pty.text();
        assert!(
            completed_home_frame(&frame).is_some(),
            "completed native Home frame missing\n{frame}"
        );
        assert!(
            strip_terminal_controls(&frame)
                .lines()
                .any(|line| !line.trim().is_empty()),
            "native frame is empty\n{frame}"
        );
        assert!(
            frame.contains("\x1b[?1049h"),
            "native renderer did not enter alternate screen\n{frame}"
        );
        assert!(
            !frame.to_ascii_lowercase().contains("legacy"),
            "fallback claim in native frame\n{frame}"
        );
        pty.send("\x1b:q\n");
        let status = pty.exit();
        let final_text = pty.text();
        assert!(
            !final_text.contains("fixture-secret")
                && !final_text.contains("Bearer ")
                && !final_text.contains("OPENAI_API_KEY"),
            "native renderer exposed injected credentials"
        );
        assert!(
            status.success(),
            "native renderer exit: {status}\n{final_text}"
        );
        assert!(
            final_text.contains("\x1b[?1049l"),
            "alternate screen was not restored\n{final_text}"
        );
    }

    #[test]
    fn rerun_same_executable_and_revision_is_deterministic() {
        let revision = revision_receipt().expect(
        "revision receipt missing: set OC2_E2E_REVISION in packaging or provide compile-time GIT_COMMIT",
    );
        assert!(!revision.trim().is_empty(), "revision receipt is empty");
        let bin = binary();
        assert!(
            bin.is_file(),
            "supplied executable is missing: {}",
            bin.display()
        );

        let first_root = Root::new();
        let first_addr = address();
        let mut first = Pty::spawn(&bin, &first_root, &first_addr, None);
        let first_frame =
            normalize_dynamic_frame(&strip_terminal_controls(&first.wait_for_native_frame()));
        first.send("\x1b:q\n");
        assert!(
            first.exit().success(),
            "first deterministic run failed\n{}",
            first.text()
        );

        let second_root = Root::new();
        let second_addr = address();
        let mut second = Pty::spawn(&bin, &second_root, &second_addr, None);
        let second_frame =
            normalize_dynamic_frame(&strip_terminal_controls(&second.wait_for_native_frame()));
        second.send("\x1b:q\n");
        assert!(
            second.exit().success(),
            "second deterministic run failed\n{}",
            second.text()
        );

        assert_eq!(
            first_frame, second_frame,
            "same executable/revision changed native frame"
        );
    }
}
