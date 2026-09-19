#!/usr/bin/env python3
"""Fail-closed convergence checks for the executable application spine.

This gate is intentionally structural. It catches recurring false-completion patterns
before an orchestrator spends another wave on isolated leaf modules. Passing it is
necessary but never sufficient for release; behavioral E2E and independent verification
remain mandatory.
"""
from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
BAD_NOTE_MARKERS = (
    "repair child",
    "follow-up",
    "follow up",
    "unwired",
    "unproven",
    "state-only",
    "state only",
    "missing",
    "partial",
    "no acceptance",
    "not accepted",
    "out of scope",
)


def read(rel: str) -> str:
    path = ROOT / rel
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def _main_none_arm(main: str) -> str:
    marker = "None =>"
    start = main.find(marker)
    if start < 0:
        return ""
    end = main.find("Some(Command::Doctor", start)
    return main[start : end if end >= 0 else len(main)]


def structural_errors() -> list[str]:
    errors: list[str] = []
    main = read("crates/cli/src/main.rs")
    none_arm = _main_none_arm(main)

    if "plan_default_launch" not in none_arm or "daemon_client" not in none_arm:
        errors.append(
            "ENTRYPOINT: no-subcommand path still bypasses plan_default_launch/daemon discovery"
        )

    serve_start = main.find("async fn serve")
    serve = main[serve_start:] if serve_start >= 0 else ""
    if "router_with_auth" not in serve:
        errors.append(
            "AUTH: serve path still does not call router_with_auth; minted token is not enforcement"
        )

    cli_rs = "\n".join(
        p.read_text(encoding="utf-8", errors="replace")
        for p in (ROOT / "crates/cli/src").glob("*.rs")
        if p.is_file()
    )
    if "opencode_rk_opentui_bridge" not in cli_rs:
        errors.append(
            "NATIVE_TUI: opentui bridge is declared/built but has no Rust CLI caller"
        )

    server_toml = read("crates/server/Cargo.toml")
    if "opencode-rk-security" not in server_toml:
        errors.append("RUNTIME: server crate does not depend on opencode-rk-security")
    if "opencode-rk-agents" not in server_toml:
        errors.append("RUNTIME: server crate does not depend on opencode-rk-agents")

    server_rs = "\n".join(
        p.read_text(encoding="utf-8", errors="replace")
        for p in (ROOT / "crates/server/src").glob("*.rs")
        if p.is_file()
    )
    if "opencode_rk_security" not in server_rs:
        errors.append("SECURITY: live server has no import/use of the security crate")
    if "opencode_rk_agents" not in server_rs:
        errors.append("AGENTS: live server has no import/use of the agents crate")

    lib = read("crates/server/src/lib.rs")
    if "ToolExecutor::new()" in lib and not re.search(
        r"(PermissionBroker|ToolAuthorizer|authorize)", lib
    ):
        errors.append(
            "TOOLS: live server constructs ToolExecutor directly with no visible authorization path"
        )

    e2e = read("crates/cli/tests/default_tui.rs")
    required_markers = (
        "bare_launch_opens_chat_tui",
        "provider",
        "persist",
    )
    if any(marker not in e2e for marker in required_markers):
        errors.append("E2E: default-entrypoint provider/persistence test spine is missing")
    if "native" not in e2e.lower() or "opentui" not in e2e.lower():
        errors.append(
            "E2E: default-entrypoint tests do not prove the real native OpenTUI path"
        )

    return errors


def plan_ids() -> set[str]:
    sys.path.insert(0, str(ROOT / "tools"))
    try:
        import completion_plan  # type: ignore
        plan = completion_plan.load(ROOT)
        return set(plan.get("stories", {}))
    except Exception:
        return set()


def ledger_errors() -> list[str]:
    path = ROOT / "tasks/completion/claims.json"
    if not path.is_file():
        return ["LEDGER: tasks/completion/claims.json is missing"]
    try:
        claims = json.loads(path.read_text(encoding="utf-8")).get("claims", {})
    except Exception as exc:
        return [f"LEDGER: cannot parse claims.json: {type(exc).__name__}"]

    known = plan_ids()
    errors: list[str] = []
    for tid, row in claims.items():
        status = row.get("status")
        if status == "completed":
            if known and tid not in known:
                errors.append(f"LEDGER: completed off-plan task {tid}")
            note = str(row.get("completedNote", "")).lower()
            marker = next((m for m in BAD_NOTE_MARKERS if m in note), None)
            if marker:
                errors.append(
                    f"LEDGER: {tid} is completed but its own note admits '{marker}'"
                )
    return errors


def main() -> int:
    errors = structural_errors() + ledger_errors()
    if errors:
        print("CONVERGENCE BLOCKED")
        for error in errors:
            print(f"- {error}")
        print(f"total={len(errors)}")
        return 1
    print(
        "CONVERGENCE STRUCTURE GREEN: run frozen installed E2E and independent "
        "post-integration verification before accepting parent tasks."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
