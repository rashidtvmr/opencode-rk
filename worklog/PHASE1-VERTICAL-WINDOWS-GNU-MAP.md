{
  "document": "PHASE1-VERTICAL-WINDOWS-GNU",
  "kind": "two-to-three-wave implementation and proof map",
  "planningStatus": "proposal-only; no product behavior or acceptance claimed",
  "repository": {
    "commit": "8a91a7b49a5a1c948218ad8f176d44e015530dcb",
    "target": "x86_64-pc-windows-gnu",
    "excluded": [
      "MSVC target",
      "automatic signing identity or certificate",
      "hosted cloud infrastructure",
      "changes to controller, verifier, frozen tests, PLAN.md, or task-card state"
    ],
    "policy": [
      "Only x86_64-pc-windows-gnu is a supported Windows target in this map.",
      "Native Rust owns orchestration; no hidden JS runtime starts in native mode.",
      "Every release claim requires real Windows runner evidence, not source compilation alone."
    ]
  },
  "waves": [
    {
      "wave": 1,
      "goal": "Make the GNU Windows artifact, runtime boundaries, installer, and native terminal path implementable.",
      "tasks": [
        {
          "id": "WGNU-001",
          "title": "GNU native build, OpenTUI DLL, and loader closure",
          "proofClass": "runner-required",
          "outcome": "A clean x86_64-pc-windows-gnu build links the native OpenTUI artifact, stages the exact DLL/import dependency beside the executable, and fails closed when the target artifact is absent. No Unix .so is silently selected on Windows.",
          "caller": "Cargo feature `native` invokes `crates/opentui-bridge/build.rs`; `crates/cli/src/tui_entry.rs::print_native_or_legacy` invokes `opencode_rk_opentui_bridge::Renderer`.",
          "sourceEvidence": [
            {
              "path": "crates/opentui-bridge/build.rs",
              "lines": "3-24",
              "symbol": "main",
              "behavior": "Checks only libopentui.a and libopentui.so, then links dylib or panics; no Windows DLL/import-archive branch."
            },
            {
              "path": "crates/opentui-bridge/src/renderer.rs",
              "lines": "45-114",
              "symbol": "unsafe extern \"C\"",
              "behavior": "Native renderer ABI is linked as opentui and exposes renderer lifetime, terminal, buffer, and render calls."
            },
            {
              "path": "crates/opentui-bridge/Cargo.toml",
              "lines": "9-14",
              "symbol": "native feature",
              "behavior": "Native bridge is an optional feature, so release wiring must select and receipt that feature explicitly."
            },
            {
              "path": "docs/architecture/COMPLETION_NATIVE_TUI.md",
              "lines": "24-38",
              "symbol": "Boundaries and safety",
              "behavior": "Requires audited FFI, ABI checks, deterministic native libraries, loader paths, SBOM, and licenses."
            }
          ],
          "implementationOwner": {
            "path": "crates/opentui-bridge/build.rs",
            "mode": "single-file primary; serialized integration with Cargo feature and release staging",
            "boundary": "Do not broaden target support or add an unapproved dependency; define the accepted GNU DLL/import naming and adjacent-loader contract."
          },
          "testOwner": "crates/opentui-bridge/tests/windows_gnu_build.rs",
          "verifier": "Independent verifier checks clean-tree build, PE imports, adjacent DLL hash, missing-artifact failure, and frozen test hash.",
          "dependencies": [],
          "runner": {
            "required": true,
            "platform": "Protected Windows x64 runner with GNU/MinGW toolchain and no MSVC fallback",
            "proof": "Build and launch the packaged executable from a clean PATH with only receipt-listed runtime DLLs."
          },
          "negativeResourceSecurity": [
            "Missing DLL or import archive fails before a misleading successful package is emitted.",
            "Wrong target triple, wrong architecture, stale developer absolute path, and Unix .so are rejected.",
            "DLL search-path hijack fixture cannot load an unlisted DLL from the working directory.",
            "Native handle remains single-owner; terminal restoration runs on normal exit, error, and cancellation.",
            "Build and runtime receipts include bounded dependency names and hashes, never secrets or inherited environment."
          ],
          "commandsEvidence": [
            {
              "command": "cargo build --locked --target x86_64-pc-windows-gnu -p opencode-rk-cli --features native",
              "evidence": "receipts/windows-gnu/WGNU-001-build.json: target, rustc, linker, executable hash, DLL/import hashes, exit code"
            },
            {
              "command": "objdump -p target/x86_64-pc-windows-gnu/release/opencode2.exe",
              "evidence": "Import table names only receipt-listed GNU/OpenTUI runtime DLLs"
            },
            {
              "command": "cargo test --locked -p opencode-rk-opentui-bridge",
              "evidence": "ABI/layout unit result; supplementary only, never Windows runtime acceptance"
            }
          ],
          "blocker": "No Windows runner is available in this workspace; Linux source checks cannot prove PE linking, DLL loading, or terminal behavior."
        },
        {
          "id": "WGNU-002",
          "title": "Deterministic ZIP, manifest, SBOM, and license bundle",
          "proofClass": "source-ready plus runner-required artifact proof",
          "outcome": "Package the GNU executable, native DLLs, licenses, manifest, SBOM, and provenance in a deterministic ZIP whose file order, timestamps, permissions, and metadata are reproducible from the pinned revision and toolchain.",
          "caller": "Release workflow consumes the GNU build output and emits the installable Windows artifact used by the PowerShell installer and clean-machine E2E.",
          "sourceEvidence": [
            {
              "path": "PLAN.md",
              "lines": "227-248",
              "symbol": "Resource acceptance and release completion certificate",
              "behavior": "Whole-process measurements, platform capabilities, license/SBOM data, exact revisions, and unresolved blockers are mandatory evidence."
            },
            {
              "path": "docs/architecture/COMPLETION_NATIVE_TUI.md",
              "lines": "33-38",
              "symbol": "Audit native dependency closure",
              "behavior": "Native libraries require deterministic packaging with fork commit, Zig version, flags, SBOM, and licenses."
            },
            {
              "path": ".github/workflows/ci.yml",
              "lines": "29-60",
              "symbol": "rust job",
              "behavior": "Current CI has generic Windows compilation/tests but no GNU target artifact, package, SBOM, or receipt job."
            },
            {
              "path": "tasks/completion/delivery.json",
              "lines": "12-13",
              "symbol": "SHIP-001 and SHIP-002",
              "behavior": "Release contract requires reproducible artifacts, integrity/provenance, and clean installed acceptance."
            }
          ],
          "implementationOwner": {
            "path": "release/windows-gnu-package.py",
            "mode": "single-file primary; serialized workflow owner for artifact upload",
            "boundary": "Stdlib-only deterministic archive/manifest generation; package only declared files and never read user data or credentials."
          },
          "testOwner": "tests/release/windows_gnu_package.py",
          "verifier": "Independent verifier reruns packaging twice from identical inputs, compares ZIP and manifest hashes, checks SBOM closure, and rejects undeclared files.",
          "dependencies": ["WGNU-001"],
          "runner": {
            "required": true,
            "platform": "Protected Windows GNU build runner for binary closure; deterministic packaging test may run on Linux",
            "proof": "Two clean builds/packages at the same revision produce byte-identical archive, manifest, SBOM, and checksum receipts."
          },
          "negativeResourceSecurity": [
            "Archive traversal names, duplicate paths, absolute paths, symlinks/reparse points, missing DLLs, and wrong architecture fail closed.",
            "Nondeterministic timestamps, ZIP order, host paths, usernames, and environment values are absent.",
            "SBOM must identify every native/runtime input and license; unlisted dependency or license omission blocks release.",
            "Archive member count and total uncompressed bytes are bounded before extraction."
          ],
          "commandsEvidence": [
            {
              "command": "python3 release/windows-gnu-package.py --target x86_64-pc-windows-gnu --input-dir <staged-build> --output <artifact.zip>",
              "evidence": "receipts/windows-gnu/WGNU-002-package.json: revision, input hashes, member list, ZIP/SHA256/manifest/SBOM hashes"
            },
            {
              "command": "cmp <artifact-a.zip> <artifact-b.zip>",
              "evidence": "zero exit on repeated packaging with identical inputs"
            },
            {
              "command": "python3 -m zipfile -l <artifact.zip>",
              "evidence": "Only expected bounded members; no traversal or development toolchain files"
            }
          ],
          "blocker": "The release workflow and package writer do not exist yet; Windows binary closure remains unprovable without WGNU-001 on a real runner."
        },
        {
          "id": "WGNU-003",
          "title": "Windows OS RNG for daemon bearer credentials",
          "proofClass": "runner-required",
          "outcome": "DaemonAuth::mint produces 32 bytes from the Windows OS CNG RNG on GNU Windows, preserving token format, uniqueness, constant-time verification, and fail-closed error handling.",
          "caller": "Daemon startup mints DaemonAuth; the token is published in BackendDescriptor and required by every /api route.",
          "sourceEvidence": [
            {
              "path": "crates/server/src/daemon_auth.rs",
              "lines": "42-60",
              "symbol": "DaemonAuth::mint and from_published",
              "behavior": "Minting is the auth root; restored tokens must be 64 hex characters."
            },
            {
              "path": "crates/server/src/daemon_auth.rs",
              "lines": "123-147",
              "symbol": "random_bytes",
              "behavior": "Unix reads /dev/urandom; Windows deliberately returns NoRandomness because the OS backend is absent."
            },
            {
              "path": "crates/server/src/daemon_auth.rs",
              "lines": "102-110",
              "symbol": "constant_time_eq",
              "behavior": "Credential comparison walks the longer input and must remain unchanged by the Windows backend."
            }
          ],
          "implementationOwner": {
            "path": "crates/server/src/daemon_auth.rs",
            "mode": "single-file platform backend; serialized dependency/FFI review if a Windows API crate is needed",
            "boundary": "Use the approved Windows CNG primitive, not a pseudo-random fallback, clock, PID, hash, or inherited secret."
          },
          "testOwner": "crates/server/tests/windows_rng_auth.rs",
          "verifier": "Independent Windows verifier checks repeated minting, token shape, auth route behavior, RNG failure propagation fixture, and no secret logging.",
          "dependencies": [],
          "runner": {
            "required": true,
            "platform": "Windows x64 GNU runner",
            "proof": "Run the cfg(windows) tests and a daemon startup/auth smoke from a clean HOME."
          },
          "negativeResourceSecurity": [
            "Unavailable CNG or malformed API result aborts credential minting; no weak fallback or automatic replay.",
            "Blank, short, non-hex, wrong, and missing bearer values remain rejected without handler side effects.",
            "Token bytes never enter logs, command arguments, URLs, transcripts, or artifact receipts.",
            "Credential length and error buffers remain bounded."
          ],
          "commandsEvidence": [
            {
              "command": "cargo test --locked -p opencode-rk-server --lib daemon_auth",
              "evidence": "receipts/windows-gnu/WGNU-003-auth-unit.json"
            },
            {
              "command": "cargo test --locked -p opencode-rk-server --test windows_rng_auth",
              "evidence": "receipts/windows-gnu/WGNU-003-auth-windows.json: real cfg(windows) pass counts and daemon route statuses"
            }
          ],
          "blocker": "Current Windows implementation is an explicit NoRandomness error; no Windows runner or approved dependency/FFI review is present."
        },
        {
          "id": "WGNU-004",
          "title": "Authenticated Windows daemon IPC and descriptor discovery",
          "proofClass": "runner-required",
          "outcome": "The singleton daemon and clients use a bounded loopback Windows transport with authenticated version negotiation, safe descriptor ownership, concurrent launch election, and no Unix-only socket or /proc assumptions.",
          "caller": "CLI discovery/startup calls daemon_client; server startup binds SingletonDaemon, publishes BackendDescriptor, and gates /api through require_bearer.",
          "sourceEvidence": [
            {
              "path": "crates/server/src/daemon.rs",
              "lines": "1-18",
              "symbol": "module imports",
              "behavior": "Current transport is tokio UnixListener/UnixStream and PID liveness is based on /proc."
            },
            {
              "path": "crates/server/src/daemon.rs",
              "lines": "138-190",
              "symbol": "read_backend_descriptor",
              "behavior": "Descriptor reads are bounded, symlink-rejecting, owner-checked, loopback-checked, schema-checked, and token-checked, but platform ownership/liveness is Unix-specific."
            },
            {
              "path": "crates/server/src/daemon.rs",
              "lines": "267-299",
              "symbol": "publish_backend_descriptor_with_auth",
              "behavior": "Descriptor publication is temporary-file plus rename and carries the bearer token."
            },
            {
              "path": "crates/cli/src/daemon_client.rs",
              "lines": "703-760",
              "symbol": "discover_from_path and discover_presence",
              "behavior": "Client discovery uses Unix MetadataExt and /proc, so Windows needs a real platform branch rather than a mocked liveness answer."
            },
            {
              "path": "crates/server/src/lib.rs",
              "lines": "112-119",
              "symbol": "router and router_with_auth",
              "behavior": "Authenticated routing exists as an option; the Windows daemon caller must pass Some(DaemonAuth)."
            }
          ],
          "implementationOwner": {
            "path": "crates/server/src/daemon.rs",
            "mode": "serialized shared transport owner with crates/cli/src/daemon_client.rs",
            "boundary": "Preserve descriptor byte caps, reparse/symlink defenses, loopback-only binding, PID/lock identity, and bearer semantics; do not kill by PID alone."
          },
          "testOwner": "crates/server/tests/windows_daemon_ipc.rs",
          "verifier": "Independent verifier starts twenty disposable concurrent clients, forges descriptors, collides ports, restarts the daemon, and confirms exact HTTP auth statuses and no duplicate owner.",
          "dependencies": ["WGNU-003"],
          "runner": {
            "required": true,
            "platform": "Windows x64 GNU runner with disposable HOME and no privileged service",
            "proof": "Real daemon/client processes communicate over the selected Windows loopback transport; receipts include process tree, endpoint, token-redacted descriptor hash, and cleanup state."
          },
          "negativeResourceSecurity": [
            "Forged, stale, reparse-point, wrong-user, malformed, oversized, wrong-schema, and foreign-origin descriptors cannot authorize or redirect a client.",
            "Twenty concurrent starts yield one lock owner and one store owner; occupied port never kills an unrelated process.",
            "Missing or wrong bearer returns 401/403 before state mutation; /health remains limited to liveness fields.",
            "Connection count, request body, descriptor reads, retry count, and retained output are bounded; shutdown reclaims listeners and children."
          ],
          "commandsEvidence": [
            {
              "command": "cargo test --locked -p opencode-rk-server --test windows_daemon_ipc",
              "evidence": "receipts/windows-gnu/WGNU-004-ipc.json: launch race, descriptor, auth, restart, and cleanup counts"
            },
            {
              "command": "cargo test --locked -p opencode-rk-cli --test windows_daemon_client",
              "evidence": "receipts/windows-gnu/WGNU-004-client.json: Windows discovery/liveness and refusal cases"
            }
          ],
          "blocker": "Current daemon transport and client liveness are Unix-only; a real Windows implementation and runner proof are absent."
        },
        {
          "id": "WGNU-005",
          "title": "Windows argv-direct process bounds and cleanup",
          "proofClass": "runner-required",
          "outcome": "Tool process execution selects an explicit Windows program and argv, applies broker authorization before spawn, bounds output, timeout, cancellation, environment, and process-tree cleanup without shell-string concatenation.",
          "caller": "ToolExecutor and shell tool call the bounded process owner for shell/tool execution.",
          "sourceEvidence": [
            {
              "path": "crates/tools/src/shell_bounds.rs",
              "lines": "119-158",
              "symbol": "ShellBounds::run",
              "behavior": "Authorization-before-spawn, timeout/cancel/output bounds, and denial events are specified, but the implementation is Unix-shaped."
            },
            {
              "path": "crates/tools/src/shell_bounds.rs",
              "lines": "169-190",
              "symbol": "kill_tree",
              "behavior": "Current cleanup invokes the Unix kill command and negative process-group IDs."
            },
            {
              "path": "crates/tools/src/shell_bounds.rs",
              "lines": "245-307",
              "symbol": "spawn_capped",
              "behavior": "Current spawn applies process_group(0), which has no Windows equivalent and must become a Windows job/process-tree boundary."
            },
            {
              "path": "docs/SECURITY.md",
              "lines": "9-22,49-56",
              "symbol": "capability and denial policy",
              "behavior": "Wildcard permissions cannot bypass mandatory controls; denial must leave no side effect and cancellation must reclaim children."
            }
          ],
          "implementationOwner": {
            "path": "crates/tools/src/shell_bounds.rs",
            "mode": "single-file platform implementation; caller wiring serialized with executor owner",
            "boundary": "Use direct argv and an OS process-job boundary; never emulate a sandbox with a regex or prompt."
          },
          "testOwner": "crates/tools/tests/windows_process_bounds.rs",
          "verifier": "Independent verifier runs nested child fixtures, cancellation and timeout races, output floods, denied spawn, environment filtering, and process-tree scans.",
          "dependencies": [],
          "runner": {
            "required": true,
            "platform": "Windows x64 GNU runner",
            "proof": "Every timeout/cancel case records no surviving child or grandchild after a bounded grace interval."
          },
          "negativeResourceSecurity": [
            "Denied command starts no process and writes a durable bounded denial event.",
            "Shell metacharacters, nested child processes, hostile argv, inherited secret environment, and alternate working directories are tested.",
            "Timeout/cancel kills and reaps the full owned tree; no detached process, job, handle, reader thread, or retained output remains.",
            "Output retention is byte-capped, not only line/count-capped."
          ],
          "commandsEvidence": [
            {
              "command": "cargo test --locked -p opencode-rk-tools --test windows_process_bounds",
              "evidence": "receipts/windows-gnu/WGNU-005-process.json: allow/deny, argv, byte, timeout, cancellation, and process-scan results"
            },
            {
              "command": "cargo test --locked -p opencode-rk-tools shell_bounds",
              "evidence": "Existing bounded Unix regression result; does not substitute for Windows proof"
            }
          ],
          "blocker": "Current kill and process-group code is Unix-only; no Windows job/process-tree implementation or runner evidence exists."
        },
        {
          "id": "WGNU-006",
          "title": "ConPTY lifecycle and native TUI caller wiring",
          "proofClass": "runner-required",
          "outcome": "The native TUI owns a real Windows ConPTY or an explicit supported-terminal fallback, starts child programs with direct argv, handles resize/input/interrupt, paints bounded application state, and restores the terminal on every exit path. `--native` is not an offline line-mode claim.",
          "caller": "Bare launch in `main.rs` selects chat/native mode; `tui_entry` owns interaction; `NativeHost` is the single state-loop owner; OpenTUI bridge owns renderer handles.",
          "sourceEvidence": [
            {
              "path": "crates/cli/src/main.rs",
              "lines": "220-258",
              "symbol": "run None branch",
              "behavior": "Native dispatch calls tui_entry, while the default non-native path calls chat; daemon lifecycle parity must be preserved."
            },
            {
              "path": "crates/cli/src/tui_entry.rs",
              "lines": "119-186",
              "symbol": "http_request",
              "behavior": "Live TUI HTTP calls require a validated bearer and bound response cap, but the visible interaction path remains line-oriented."
            },
            {
              "path": "crates/cli/src/tui_entry.rs",
              "lines": "599-615",
              "symbol": "print_native_or_legacy",
              "behavior": "Renderer call is feature-gated and falls back to text on error; installed Windows proof must distinguish intentional fallback from missing native runtime."
            },
            {
              "path": "crates/cli/src/native_host.rs",
              "lines": "14-26",
              "symbol": "NativeHost ownership contract",
              "behavior": "The file explicitly records that tui_entry must construct and drive NativeHost; current wiring is an integration boundary."
            },
            {
              "path": "docs/architecture/COMPLETION_NATIVE_TUI.md",
              "lines": "44-56",
              "symbol": "Installed app lifecycle",
              "behavior": "Bare installed invocation must start/reuse one authenticated daemon and open native TUI without manual setup."
            }
          ],
          "implementationOwner": {
            "path": "crates/cli/src/windows_conpty.rs",
            "mode": "single-file new platform owner; serialized caller wiring in tui_entry.rs/native_host.rs",
            "boundary": "ConPTY handles, input, resize, and cleanup are bounded and owned; no per-agent OS process and no shell command string."
          },
          "testOwner": "crates/cli/tests/windows_conpty.rs",
          "verifier": "Independent verifier launches the installed binary under a real Windows terminal/ConPTY, submits input, resizes, interrupts, exits through errors, and checks terminal/process restoration.",
          "dependencies": ["WGNU-001", "WGNU-004", "WGNU-005"],
          "runner": {
            "required": true,
            "platform": "Windows x64 GNU runner with a real interactive console or ConPTY-capable harness",
            "proof": "Terminal recording, renderer/runtime hashes, bounded event counts, child-process receipt, and restored console state."
          },
          "negativeResourceSecurity": [
            "Piped/noninteractive input follows a bounded documented headless path and never hangs or enables raw mode indefinitely.",
            "ConPTY creation, resize, broken pipe, Ctrl-C, panic/error, and child exit restore terminal modes and release handles.",
            "Renderer failure is explicit and receipt-classified; it cannot silently pass as native UI.",
            "Transcript, input, and event queues are bounded by bytes and items; no hidden JS/Bun/Node process starts."
          ],
          "commandsEvidence": [
            {
              "command": "cargo test --locked -p opencode-rk-cli --test windows_conpty",
              "evidence": "receipts/windows-gnu/WGNU-006-conpty.json: lifecycle, input, resize, interrupt, fallback, terminal restoration"
            },
            {
              "command": "opencode2 --once --native",
              "evidence": "Installed receipt shows renderer-backed bounded frame and authenticated daemon state, not line fallback"
            }
          ],
          "blocker": "No ConPTY implementation or Windows native-TUI proof exists; current native-host comments explicitly leave caller integration to an integrator."
        },
        {
          "id": "WGNU-007",
          "title": "Bounded transactional PowerShell installer with ACL and junction hardening",
          "proofClass": "runner-required",
          "outcome": "Installer verifies archive, checksum, manifest, architecture, member bounds, and native DLL closure before writes; extracts into a private bounded staging directory; atomically installs with rollback; rejects symlink/reparse/junction/ACL attacks; preserves existing opencode and user data.",
          "caller": "Clean user invokes scripts/install-opencode2.ps1 against the WGNU-002 artifact; oc2 alias follows the same contract through the serialized installer owner.",
          "sourceEvidence": [
            {
              "path": "scripts/install-opencode2.ps1",
              "lines": "1-70",
              "symbol": "installer script",
              "behavior": "Current script has checksum and AMD64 gates but expands/copies directly, has no manifest/file/byte bounds, ACL/reparse checks, rollback, or atomic transaction."
            },
            {
              "path": "scripts/install-oc2.ps1",
              "lines": "1-70",
              "symbol": "installer alias",
              "behavior": "Duplicate installer has the same transactional and junction-hardening gaps."
            },
            {
              "path": "tasks/completion/local.json",
              "lines": "13-15",
              "symbol": "APP-010 and APP-012",
              "behavior": "Install must be side-by-side, architecture-safe, corruption-rejecting, and usable without Cargo/Zig/Bun/Node."
            }
          ],
          "implementationOwner": {
            "path": "scripts/install-opencode2.ps1",
            "mode": "serialized shared owner with scripts/install-oc2.ps1",
            "boundary": "Both names must invoke equivalent bounded logic; never write outside the requested install directory or mutate existing opencode/data."
          },
          "testOwner": "tests/release/install_opencode2.ps1",
          "verifier": "Independent Windows verifier runs install, upgrade, rollback, uninstall, path, corruption, reparse-point, ACL, wrong-arch, and preexisting-opencode cases in disposable profiles.",
          "dependencies": ["WGNU-002"],
          "runner": {
            "required": true,
            "platform": "Windows x64 GNU clean profile with standard user privileges",
            "proof": "Receipt includes before/after tree hashes, ACL/reparse inspection, archive hash, installed binary/DLL hashes, rollback result, and no user-data diff."
          },
          "negativeResourceSecurity": [
            "Checksum, manifest, architecture, path traversal, duplicate member, file-count, byte-budget, or DLL mismatch leaves install target unchanged.",
            "Existing target remains usable after a failed replacement; interrupted transaction rolls back without deleting history.",
            "Install directory, stage directory, archive, and every extracted member reject symlink/reparse/junction redirection.",
            "ACL grants only the intended user/system identities; untrusted write access and inherited broad permissions block installation.",
            "PowerShell arguments are passed as values, not shell-concatenated commands; cleanup is bounded and best effort after durable rollback."
          ],
          "commandsEvidence": [
            {
              "command": "pwsh -NoProfile -File tests/release/install_opencode2.ps1 -Archive <artifact.zip> -Checksum <sha256> -InstallDir <disposable-dir>",
              "evidence": "receipts/windows-gnu/WGNU-007-install.json: positive install and exact file/ACL/hash receipt"
            },
            {
              "command": "pwsh -NoProfile -File tests/release/install_opencode2.ps1 -Case all-negative",
              "evidence": "Corruption, junction, ACL, rollback, wrong-arch, and preexisting-opencode cases show zero forbidden side effects"
            }
          ],
          "blocker": "Current scripts perform only checksum/architecture checks; ACL, reparse, transactional, rollback, and Windows runner evidence are absent."
        }
      ]
    },
    {
      "wave": 2,
      "goal": "Prove the installed native app journey across TUI, authenticated web, approval, persistence, and restart.",
      "tasks": [
        {
          "id": "WGNU-008",
          "title": "APP012 installed Windows file approval and restart journey",
          "proofClass": "runner-required",
          "outcome": "From a clean disposable Windows home, install the release artifact, launch opencode2 without manual serve/database/session setup, complete provider setup, stream a fixture turn, approve a real file operation through the broker, restart, and resume the same durable session.",
          "caller": "Installed opencode2 no-subcommand journey invokes daemon discovery, native TUI, provider fixture, broker, tools, storage, and restart recovery.",
          "sourceEvidence": [
            {
              "path": "tasks/completion/local.json",
              "lines": "4-15",
              "symbol": "APP-001 through APP-012",
              "behavior": "Contracts require one authenticated daemon, in-app setup, native UI, brokered tool turn, persistence, second client, interruption, restart, and clean install proof."
            },
            {
              "path": "tasks/completion/local.json",
              "lines": "15",
              "symbol": "APP-012",
              "behavior": "Golden journey explicitly requires clean home, release artifact only, real UI/broker/tools/storage, second client, failure injection, recordings, process resources, hashes."
            },
            {
              "path": "docs/CONVERGENCE.md",
              "lines": "10-24",
              "symbol": "hard product boundary",
              "behavior": "Bare launch, authenticated daemon, native UI, real provider/tool/approval, persistence, second client, denial, interruption, restart, and exact-revision rerun are mandatory."
            }
          ],
          "implementationOwner": {
            "path": "tests/e2e/local_application_windows_gnu.rs",
            "mode": "single-file journey harness; product callers remain owned by Wave 1 integration owners",
            "boundary": "Fixture provider is the only substitute; no mocked UI, broker, tool, storage, daemon, or process success."
          },
          "testOwner": "tests/e2e/local_application_windows_gnu.rs",
          "verifier": "Independent verifier installs from the exact WGNU-002 ZIP, reruns the frozen journey on the exact integrated revision, and rejects recordings without positive event/process/hash counts.",
          "dependencies": ["WGNU-001", "WGNU-003", "WGNU-004", "WGNU-005", "WGNU-006", "WGNU-007"],
          "runner": {
            "required": true,
            "platform": "Clean Windows x64 GNU runner/profile with real terminal, no Cargo/Zig/Bun/Node in PATH",
            "proof": "Redacted terminal recording, event/test counts, process tree, RSS/CPU/disk bounds, artifact hashes, and session restart/resume receipt."
          },
          "negativeResourceSecurity": [
            "Approval denial leaves fixture file unchanged and emits durable denial state.",
            "Startup, approval, partial stream, persistence, daemon restart, and terminal restoration failures produce recoverable truthful state, never fabricated completion.",
            "Second client observes one turn/session and no duplicate provider/tool execution.",
            "Fixture data, HOME, provider credentials, logs, recordings, and receipts contain no real secrets and remain bounded."
          ],
          "commandsEvidence": [
            {
              "command": "cargo test --locked --test local_application_windows_gnu --manifest-path tests/e2e/Cargo.toml",
              "evidence": "receipts/windows-gnu/WGNU-008-app012.json: positive/negative journey counts and exact binary/archive hashes"
            },
            {
              "command": "pwsh -NoProfile -File tests/release/inspect_process_receipt.ps1 -Receipt <receipt>",
              "evidence": "No unowned daemon/tool/ConPTY child, no duplicate store owner, bounded RSS/CPU/handles"
            }
          ],
          "blocker": "No Windows installed runner or release artifact exists; current source has partial TUI/daemon paths but no integrated APP012 Windows proof."
        },
        {
          "id": "WGNU-009",
          "title": "Native TUI and authenticated web parity on one installed daemon",
          "proofClass": "runner-required",
          "outcome": "The installed native TUI and served web client attach to the same authenticated daemon/session; web API calls carry a non-URL, non-localStorage credential mechanism, unauthorized API access is rejected, and restart/resume remains shared and durable.",
          "caller": "Native TUI calls daemon APIs; installed web UI calls `web/src/lib/api.ts`; server serves assets and applies router_with_auth to /api routes.",
          "sourceEvidence": [
            {
              "path": "web/src/lib/api.ts",
              "lines": "171-187",
              "symbol": "request",
              "behavior": "Web fetch currently adds JSON content type but no bearer credential, so authenticated installed web proof requires explicit token/cookie/bootstrap wiring."
            },
            {
              "path": "crates/server/src/lib.rs",
              "lines": "116-119",
              "symbol": "router_with_auth",
              "behavior": "Server has an auth-gated router option; release serving must use it rather than legacy router(None)."
            },
            {
              "path": "crates/cli/src/tui_entry.rs",
              "lines": "119-161",
              "symbol": "http_request",
              "behavior": "Native TUI refuses unauthenticated /api requests and sends a validated Authorization header."
            },
            {
              "path": "tasks/completion/delivery.json",
              "lines": "13",
              "symbol": "SHIP-002",
              "behavior": "Clean installed acceptance requires two terminal clients sharing one authenticated daemon and correct workspace state."
            }
          ],
          "implementationOwner": {
            "path": "web/src/lib/api.ts",
            "mode": "serialized shared auth owner with server web host and descriptor/bootstrap owner",
            "boundary": "Use a same-origin bounded credential channel, preferably an HttpOnly scoped cookie or equivalent brokered bootstrap; never put bearer tokens in URLs, browser storage, logs, or transcripts."
          },
          "testOwner": "tests/release/windows_web_e2e.rs",
          "verifier": "Independent browser/terminal verifier checks unauthenticated and wrong-token API responses, same-session observations, approval/tool state, restart/resume, and no token leakage.",
          "dependencies": ["WGNU-004", "WGNU-006", "WGNU-008"],
          "runner": {
            "required": true,
            "platform": "Installed Windows x64 GNU artifact with real browser automation and native terminal",
            "proof": "Browser/network receipt redacts credential values while proving auth status, session/turn IDs, event counts, and same-daemon identity."
          },
          "negativeResourceSecurity": [
            "Direct unauthenticated, malformed, expired, wrong-origin, and wrong-token /api calls have no state side effects.",
            "Browser refresh/restart does not create a second daemon or duplicate turn; stale credential recovery is explicit and bounded.",
            "No bearer appears in URL query, localStorage, page text, network logs, screenshots, or exported transcript.",
            "Web event and request bodies, reconnect retries, and retained UI history have byte/item limits."
          ],
          "commandsEvidence": [
            {
              "command": "pnpm --dir web test --run",
              "evidence": "Web component/API compatibility regression only; does not substitute for installed Windows auth proof"
            },
            {
              "command": "cargo test --locked --test windows_web_e2e --manifest-path tests/release/Cargo.toml",
              "evidence": "receipts/windows-gnu/WGNU-009-web.json: browser auth, shared session, restart, denial, and leakage scan"
            }
          ],
          "blocker": "Web request code currently sends no bearer and no installed Windows browser/terminal runner is available; server auth wiring must be independently verified."
        }
      ]
    },
    {
      "wave": 3,
      "goal": "Protect the Windows proof path, bind receipts to exact revisions, and close release status honestly.",
      "tasks": [
        {
          "id": "WGNU-010",
          "title": "Protected GNU Windows workflow and artifact receipts",
          "proofClass": "runner-required",
          "outcome": "A protected workflow runs only trusted Windows GNU jobs, builds/tests/packages the exact revision, executes installer/runtime/E2E/resource/security checks, and publishes bounded redacted receipts bound to commit, target, toolchain, artifact hashes, frozen test hashes, and runner identity.",
          "caller": "GitHub Actions release/completion workflow invokes the build, package, installer, APP012, web, and receipt validators after policy checks.",
          "sourceEvidence": [
            {
              "path": ".github/workflows/ci.yml",
              "lines": "15-60",
              "symbol": "planning and rust jobs",
              "behavior": "Current workflow uses generic ubuntu-latest/windows-latest jobs and workspace tests; it has no GNU target, protected runner, artifact, installer, E2E, or receipt contract."
            },
            {
              "path": ".github/workflows/completion.yml",
              "lines": "1-20",
              "symbol": "completion-specification",
              "behavior": "Current completion workflow validates planning primitives only, not product or Windows release behavior."
            },
            {
              "path": "tasks/completion/delivery.json",
              "lines": "12-19",
              "symbol": "SHIP-001, SHIP-002, SHIP-006, SHIP-008",
              "behavior": "Release contracts require reproducible artifacts, clean installs, whole-app resource/security soak, and independent exact-proof completion."
            },
            {
              "path": "docs/TDD.md",
              "lines": "43-75",
              "symbol": "RED/GREEN and independent verification",
              "behavior": "Compiling RED, frozen hashes, exact command manifests, GREEN, and verifier rerun are required; worker reports do not count."
            }
          ],
          "implementationOwner": {
            "path": ".github/workflows/release.yml",
            "mode": "serialized workflow owner with release receipt validator",
            "boundary": "Do not expose protected runner or signing secrets to untrusted fork/PR code; pin actions and fail closed on missing receipt inputs."
          },
          "testOwner": "tests/release/windows_gnu_receipts.py",
          "verifier": "Independent verifier checks workflow policy, receipt schema/hash bindings, positive counts, resource metrics, redaction, artifact closure, and no privileged fork execution.",
          "dependencies": ["WGNU-002", "WGNU-007", "WGNU-008", "WGNU-009"],
          "runner": {
            "required": true,
            "platform": "Protected, labeled Windows x64 GNU runner; ordinary PRs cannot execute privileged jobs or read signing secrets",
            "proof": "Workflow run URL plus immutable receipts; each receipt binds exact commit, source tree hash, frozen test hash, command, exit code, counts, and artifact hashes."
          },
          "negativeResourceSecurity": [
            "Fork and untrusted pull requests cannot reach protected runner, credentials, signing identity, or artifact publication.",
            "Changed source/test/receipt bytes, wrong revision, stale artifact, missing positive count, or fabricated PASS text fails validation.",
            "Only one resource-heavy validation runs at a time; RSS/process-tree/CPU/disk/handle bounds preserve the 8 GiB host policy.",
            "Receipts redact tokens, provider credentials, user paths where not needed, transcript content, and runner secrets; output size is bounded."
          ],
          "commandsEvidence": [
            {
              "command": "python3 tests/release/windows_gnu_receipts.py --revision <commit> --receipts <dir>",
              "evidence": "receipts/windows-gnu/WGNU-010-receipt-validation.json: schema, hashes, commands, counts, redaction, policy decisions"
            },
            {
              "command": "python3 tools/validate_repository.py",
              "evidence": "Canonical repository guard result captured separately; failure remains a blocker and is not bypassed"
            },
            {
              "command": "python3 tools/convergence_gate.py",
              "evidence": "Convergence result captured separately; current known ledger/convergence blockers remain visible"
            }
          ],
          "blocker": "No protected Windows GNU runner or workflow receipt system is available here; current convergence gate also reports pre-existing repository blockers."
        },
        {
          "id": "WGNU-011",
          "title": "Independent Windows GNU release decision and external signing disposition",
          "proofClass": "externally-blocked until authorized signing evidence exists",
          "outcome": "An independent verifier issues a Windows GNU release disposition only when build, package, installer, daemon/auth, process/ConPTY, native TUI, authenticated web, APP012, resource, security, and receipt gates all pass on one integrated revision. Missing Authenticode evidence remains an explicit external blocker, never an implied pass.",
          "caller": "Trusted release verifier consumes WGNU-010 receipts plus authorized signing receipt and emits pass, blocked, or incomplete status.",
          "sourceEvidence": [
            {
              "path": "tasks/completion/delivery.json",
              "lines": "12-19",
              "symbol": "SHIP-001 and SHIP-008",
              "behavior": "Signing may remain blocked without an authorized identity; independent completion must fail closed on missing platform proof or stale/unintegrated evidence."
            },
            {
              "path": "PLAN.md",
              "lines": "241-252",
              "symbol": "Release completion certificate",
              "behavior": "Certificate must include exact pins, revision, source hashes, tests/logs, platform capabilities, resource measurements, SBOM, and unresolved blockers; no percentage or clean compile substitutes."
            },
            {
              "path": "docs/CONVERGENCE.md",
              "lines": "99-105",
              "symbol": "Acceptance",
              "behavior": "Claims ledger is not acceptance authority; unintegrated branches, failed reruns, and off-plan self-reports cannot complete a parent."
            },
            {
              "path": "docs/SECURITY.md",
              "lines": "62-72",
              "symbol": "Human authority",
              "behavior": "Signing identities and human authority cannot be fabricated by workers; unavailable authority is a blocker."
            }
          ],
          "implementationOwner": {
            "path": "tests/release/windows_gnu_completion.py",
            "mode": "single-file independent decision validator; serialized consumption of WGNU-010 receipts",
            "boundary": "Validator is read-only over source/receipts except its bounded output; it cannot mint signatures, weaken gates, edit tests, or alter controller state."
          },
          "testOwner": "tests/release/test_windows_gnu_completion.py",
          "verifier": "Separate verifier identity reruns the frozen Windows suite on the exact integrated revision, validates Authenticode certificate chain/timestamp/publisher against policy, and records blocked status when authority is absent.",
          "dependencies": ["WGNU-010"],
          "runner": {
            "required": true,
            "platform": "Protected Windows GNU release runner plus authorized signing environment, if signing is required for the declared distribution",
            "proof": "Immutable completion report references all receipts, source/tree hashes, signing certificate fingerprint/timestamp when available, and every unresolved blocker."
          },
          "negativeResourceSecurity": [
            "Missing, expired, wrong-publisher, wrong-architecture, unsigned, or unverifiable Authenticode evidence yields blocked, never pass.",
            "Receipt from another revision, edited frozen test, worker self-report, zero-test success, or unintegrated branch fails closed.",
            "No live credential, signing private key, provider secret, user database, or real approval is copied into evidence.",
            "No claim of Windows support is made from generic windows-latest CI or source compilation alone."
          ],
          "commandsEvidence": [
            {
              "command": "python3 tests/release/windows_gnu_completion.py --revision <integrated-commit> --receipts <receipts-dir> --out <report.json>",
              "evidence": "completion report contains exact pins, artifact/test/log/resource/SBOM hashes, positive counts, platform status, and blockers"
            },
            {
              "command": "Get-AuthenticodeSignature <installed-or-shipped-exe> | Format-List",
              "evidence": "Authorized Windows signing receipt, or explicit blocked status with no fabricated identity"
            }
          ],
          "blocker": "Signing identity/certificate and a protected Windows GNU release runner are external authorities unavailable in this workspace; release remains unaccepted until independently supplied and rerun."
        }
      ]
    }
  ],
  "crossCuttingRules": [
    "Use disposable HOME, workspace, provider fixture, and install directory. Never mutate the user's OpenCode database or credentials.",
    "Write RED tests before product implementation, freeze test hashes and command manifests, then run GREEN and independent reruns without test edits.",
    "Treat current source claims as partial until the named caller is wired and the named verifier observes behavior on the actual Windows target.",
    "Preserve bounded bytes, queue items, process count, retries, archive members, descriptor reads, and receipt output; record measurements for the whole process tree.",
    "No shell-string concatenation, broad inherited environment, secret logging, detached process, automatic replay of ambiguous side effects, or prompt-as-sandbox behavior.",
    "Task IDs in this document are proposed decomposition IDs only. The controller must reconcile them with the canonical plan before scheduling and must not infer acceptance from this map."
  ],
  "currentBlockers": [
    "No Windows x64 GNU runner is available in the current environment.",
    "Current OpenTUI build script has no Windows DLL/import-archive handling.",
    "Windows daemon auth RNG returns NoRandomness; daemon IPC/client discovery are Unix-specific.",
    "Windows process-group cleanup and ConPTY lifecycle are not implemented or proven.",
    "PowerShell installers lack bounded transactional extraction, ACL, reparse/junction, rollback, and DLL receipt checks.",
    "Current web request helper does not send bearer auth; installed web auth wiring remains unproven.",
    "Protected runner policy, release receipts, and authorized signing evidence are external/release-owner work.",
    "python3 tools/convergence_gate.py currently reports pre-existing ledger/convergence blockers; this map does not suppress or reinterpret them."
  ],
  "verificationOfThisMap": {
    "parser": "python3 -m json.tool worklog/PHASE1-VERTICAL-WINDOWS-GNU-MAP.md",
    "format": "git diff --check",
    "acceptance": "None. This artifact is a source-grounded proposal and cannot certify implementation, Windows support, signing, or release completion."
  }
}
