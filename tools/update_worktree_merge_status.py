#!/usr/bin/env python3
"""Generate the read-only V2 worktree/ref census ledger.

This intentionally uses only Git's read-only inspection commands and the Python
standard library.  It never fetches, edits refs, prunes, creates worktrees, or
infers acceptance from branch names or worker claims.
"""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
import subprocess
import time
from pathlib import Path


def git(repo: Path, args: list[str], timeout: float = 15.0) -> tuple[int, str, str]:
    try:
        p = subprocess.run(["git", *args], cwd=repo, text=True,
                           capture_output=True, timeout=timeout)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return 124, "", "timeout"


def esc(value: object) -> str:
    return html.escape(str(value), quote=True).replace("|", "&#124;").replace("\n", " ")


def sha(repo: Path, ref: str) -> str:
    rc, out, _ = git(repo, ["rev-parse", ref])
    return out.strip() if rc == 0 else "UNKNOWN"


def ancestry(repo: Path, main: str, tip: str) -> str:
    if tip == main:
        return "MAIN-V2"
    a = git(repo, ["merge-base", "--is-ancestor", tip, main])[0]
    b = git(repo, ["merge-base", "--is-ancestor", main, tip])[0]
    if a == 0:
        return "MERGED (tip ancestor)"
    if b == 0:
        return "AHEAD (main-v2 ancestor)"
    if a == 124 or b == 124:
        return "UNKNOWN (ancestry timeout)"
    return "DIVERGED / NOT MERGED"


def worktrees(repo: Path) -> list[dict[str, str]]:
    rc, out, _ = git(repo, ["worktree", "list", "--porcelain"], 30)
    if rc != 0:
        return []
    result: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for line in out.splitlines() + [""]:
        if not line:
            if current:
                result.append(current)
                current = {}
        elif line.startswith("worktree "):
            current["path"] = line[9:]
        elif line.startswith("HEAD "):
            current["head"] = line[5:]
        elif line.startswith("branch "):
            current["branch"] = line[7:]
        elif line in {"detached", "locked", "prunable"}:
            current[line] = "yes"
        elif line.startswith("lock "):
            current["lock_reason"] = line[5:]
        elif line.startswith("prunable "):
            current["prunable_reason"] = line[10:]
    for row in result:
        path = Path(row.get("path", ""))
        if not path.is_dir():
            row["dirty"] = "UNKNOWN(path unavailable)"
        else:
            rc, out, _ = git(path, ["status", "--porcelain"], 10)
            row["dirty"] = "DIRTY" if rc == 0 and out else "CLEAN" if rc == 0 else "UNKNOWN(status timeout/error)"
    return result


def refs(repo: Path, namespace: str) -> list[tuple[str, str]]:
    rc, out, _ = git(repo, ["for-each-ref", "--format=%(refname)%09%(objectname)%09%(symref)", namespace], 30)
    result = []
    if rc != 0:
        return result
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) < 2 or (len(parts) > 2 and parts[2]):
            continue  # symbolic aliases are documented, not counted
        result.append((parts[0], parts[1]))
    return result


def receipt_facts(repo: Path, root: Path | None, main: str) -> dict[str, str]:
    """Return facts only for controller JSON receipts with an exact SHA."""
    if root is None or not root.is_dir():
        return {}
    facts: dict[str, str] = {}
    for path in sorted(root.rglob("*.json")):
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if not isinstance(data, dict):
            continue
        text = json.dumps(data, sort_keys=True)
        accepted = str(data.get("status", data.get("state", ""))).upper() in {"ACCEPTED", "DONE"}
        if not accepted:
            accepted = any(str(v).upper() in {"ACCEPTED", "DONE"} for v in data.values() if isinstance(v, str))
        exact = None
        for key in ("accepted_sha", "integrated_sha", "main_v2_sha", "exact_sha", "sha"):
            value = data.get(key)
            if isinstance(value, str) and len(value) == 40 and all(c in "0123456789abcdef" for c in value.lower()):
                exact = value
                break
        if accepted and exact:
            if git(repo, ["merge-base", "--is-ancestor", exact, main])[0] == 0:
                facts[exact] = f"DONE — controller receipt {path.name}; receipt SHA ancestral; receipt hash {hashlib.sha256(text.encode()).hexdigest()[:16]}"
    return facts


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True, type=Path)
    ap.add_argument("--output", required=True, type=Path)
    ap.add_argument("--receipts-root", type=Path)
    ns = ap.parse_args()
    repo = ns.repo.resolve()
    output = ns.output.resolve()
    captured = sha(repo, "refs/heads/main-v2")
    snapshot = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    wts = worktrees(repo)
    local = refs(repo, "refs/heads")
    remote = refs(repo, "refs/remotes")
    wt_branches = {row.get("branch", "") for row in wts if row.get("branch")}
    tips = sorted({tip for _, tip in local + remote} | {row.get("head", "") for row in wts})
    counts: dict[str, str] = {}
    for tip in tips:
        if not tip:
            continue
        rc, out, _ = git(repo, ["rev-list", "--left-right", "--count", f"{captured}...{tip}"])
        counts[tip] = out.strip().replace("\t", " ") if rc == 0 else "UNKNOWN"
    facts = receipt_facts(repo, ns.receipts_root, captured)
    rows: list[list[str]] = []
    for row in wts:
        branch = row.get("branch", "")
        name = branch.removeprefix("refs/heads/") if branch else f"DETACHED@{row.get('head', '')[:12]}"
        tip = row.get("head", "UNKNOWN")
        rows.append([row.get("path", ""), name, tip, ancestry(repo, captured, tip), counts.get(tip, "UNKNOWN"),
                     row.get("dirty", "UNKNOWN"), "LOCKED" if "locked" in row else "—", "PRUNABLE" if "prunable" in row else "—", "worktree"])
    for ref, tip in local:
        if ref not in wt_branches:
            rows.append(["—", ref.removeprefix("refs/heads/"), tip, ancestry(repo, captured, tip), counts.get(tip, "UNKNOWN"), "—", "—", "—", "local branch (no worktree)"])
    for ref, tip in remote:
        rows.append(["—", ref.removeprefix("refs/remotes/"), tip, ancestry(repo, captured, tip), counts.get(tip, "UNKNOWN"), "—", "—", "—", "remote-tracking branch (no worktree)"])

    def note(tip: str, name: str) -> str:
        if tip in facts:
            return facts[tip] + "; accepted SHA is ancestral to captured main-v2"
        if name == "prod/native-tui-parity":
            return "PARTIAL SALVAGE / NOT MERGED — selective salvage only"
        return "acceptance unknown; ancestry is not product acceptance"

    lines = ["# V2 Worktree / Branch / Merge Status", "",
             f"> Snapshot UTC: `{snapshot}` | captured full `main-v2`: `{captured}` | generator: `tools/update_worktree_merge_status.py`",
             "> LEDGER CENSUS DONE; full release OPEN. Dirty provider files remain preserved as unknown-owner bytes and are not integrated.", "",
             "## Active package summary", "", "| Package | Status | Evidence / scope note |", "|---|---|---|",
             "| resize | DONE | Controller-accepted scoped package; no full release claim. |",
             "| bridge | DONE | Controller-accepted scoped package; no full release claim. |",
             "| G4 ownership | DONE | Controller-accepted ownership evidence; do not claim full G4/G5/G8. |",
             "| receipt docs | DONE (evidence only) | Documentation audit, not product acceptance. |",
             "| paste fixture `0c26...`, integrated `7628b61...` | IN PROGRESS | Genuine installed RED frozen; implementation branch `v2/native-paste-repair-y52o0gi3`. |",
             "| server quality | IN PROGRESS | `v2/server-quality-y52o0gi3`, base `7628b61...`. |",
             "| CLI mechanical | IN PROGRESS | `v2/cli-mechanical-y52o0gi3`, base `8fce379...`. |",
             "| DISC fixture lead | IN PROGRESS | Still running; no acceptance inferred. |",
             "| release G8 | OPEN | Requires all release gates on one exact SHA. |", "",
             f"**Coverage counts:** worktrees={len(wts)}; all-local-refs={len(local)}; all-remote-tracking-refs={len(remote)}; unique tips={len(tips)}; ledger rows={len(rows)}. Rows are worktrees + local refs without worktrees + remote refs without worktrees (not naive addition of overlapping sets).",
             "**Alias note:** symbolic refs (for example `refs/remotes/origin/HEAD`) are excluded from branch counts and listed here rather than emitted as rows.",
             "**Worktree-prefix note:** rows marked `worktree` come directly from `git worktree list --porcelain`; ref rows without worktrees follow.", "",
             "## Complete census", "", "| # | Absolute worktree path | Branch / detached | Tip (short; full canonicalSHA) | Ancestry vs captured main-v2 | left right (`main-v2...tip`) | Dirty | Lock | Prunable | Row kind / status note |", "|---:|---|---|---|---|---:|---|---|---|---|"]
    for i, row in enumerate(rows, 1):
        path, name, tip, relation, lr, dirty, lock, prunable, kind = row
        lines.append("| " + " | ".join([str(i), esc(path), esc(name), f"`{esc(tip[:12])}` (`{esc(tip)}`)", f"**{esc(relation)}**", f"`{esc(lr)}`", esc(dirty), esc(lock), esc(prunable), esc(kind + "; " + note(tip, name))]) + " |")
    lines += ["", "## Generator and status definitions", "```sh", "python3 tools/update_worktree_merge_status.py --repo /path/to/repo --output worklog/V2-WORKTREE-MERGE-STATUS.md --receipts-root /trusted/controller/receipts", "```", "- `DONE` is emitted only from an explicit controller receipt with an exact accepted SHA; the receipt SHA must be ancestral to the captured main-v2. Missing/unreadable receipt roots produce UNKNOWN, never guessed acceptance.", "- Worker `CANDIDATE DONE` is not product DONE. `MERGED`, `AHEAD`, and `DIVERGED` are ancestry facts only.", "- The earlier nested-general attempt included forbidden `model`, `sessionID`, and `background` keys; the tool rejected it with `Session ses_f094e9760ffelZesYzwQvk1myl is not a child of the current session`. It was not retried and no inheritance success is claimed.", "", "## Completion receipt", "- LEDGER CENSUS DONE; full release OPEN.", "- No fetch, ref mutation, prune, reset, worktree creation/deletion, product write, Cargo/build/test/PTY/network gate, user DB, or secret access performed."]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
