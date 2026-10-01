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
import re
import subprocess
import time
from pathlib import Path


def git(repo: Path, args: list[str], timeout: float = 15.0) -> tuple[int, str, str]:
    try:
        env = os.environ.copy()
        env["GIT_OPTIONAL_LOCKS"] = "0"
        p = subprocess.run(["git", *args], cwd=repo, text=True, env=env,
                           capture_output=True, timeout=timeout)
        return p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return 124, "", "timeout"


def esc(value: object) -> str:
    return html.escape(str(value), quote=True).replace("|", "&#124;").replace("\n", " ")


def sha(repo: Path, ref: str) -> str:
    rc, out, _ = git(repo, ["rev-parse", ref])
    if rc != 0 or len(out.strip()) != 40:
        raise RuntimeError(f"cannot capture {ref}: {out.strip()} {_}")
    return out.strip()


def ancestry(repo: Path, main: str, tip: str) -> str:
    if tip == main:
        return "MAIN-V2"
    a = git(repo, ["merge-base", "--is-ancestor", tip, main])[0]
    b = git(repo, ["merge-base", "--is-ancestor", main, tip])[0]
    if a == 0:
        return "MERGED (tip ancestor)"
    if b == 0:
        return "AHEAD (main-v2 ancestor)"
    if a not in (1,) or b not in (1,):
        return "UNKNOWN (ancestry command error)"
    return "DIVERGED / NOT MERGED"


def worktrees(repo: Path) -> list[dict[str, str]]:
    rc, out, error = git(repo, ["worktree", "list", "--porcelain", "-z"], 30)
    if rc != 0:
        raise RuntimeError(f"worktree census failed: {error}")
    result: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for line in out.split("\0") + [""]:
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
        elif line.startswith("locked "):
            current["locked"] = line[7:]
        elif line.startswith("prunable "):
            current["prunable"] = line[9:]
    for row in result:
        path = Path(row.get("path", ""))
        if not path.is_dir():
            row["dirty"] = "UNKNOWN(path unavailable)"
        else:
            rc, out, _ = git(path, ["status", "--porcelain"], 10)
            row["dirty"] = "DIRTY" if rc == 0 and out else "CLEAN" if rc == 0 else "UNKNOWN(status timeout/error)"
    return result


def refs(repo: Path, namespace: str) -> list[tuple[str, str]]:
    rc, out, error = git(repo, ["for-each-ref", "--format=%(refname)%09%(objectname)%09%(symref)", namespace], 30)
    result = []
    if rc != 0:
        raise RuntimeError(f"ref census failed for {namespace}: {error}")
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) < 2 or (len(parts) > 2 and parts[2]):
            continue  # symbolic aliases are documented, not counted
        result.append((parts[0], parts[1]))
    return result


def hashed_json(path: Path, expected: str) -> object:
    raw = path.read_bytes()
    if not isinstance(expected, str) or hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError(f"artifact hash mismatch: {path.name}")
    return json.loads(raw)


def verified_commands(commands: object, directory: Path, revision: str) -> bool:
    if not isinstance(commands, list) or not commands:
        return False
    for item in commands:
        if not isinstance(item, dict) or item.get("source_sha") != revision:
            return False
        if type(item.get("exit")) is not int or item["exit"] != 0:
            return False
        if any(item.get(key) for key in ("timeout", "limit_failure", "failed", "ignored")):
            return False
        label, expected = item.get("label"), item.get("log_sha256")
        if not isinstance(label, str) or not re.fullmatch(r"[a-z0-9-]+", label):
            return False
        if not isinstance(expected, str) or hashlib.sha256((directory / (label + ".log")).read_bytes()).hexdigest() != expected:
            return False
    return True


def receipt_facts(repo: Path, root: Path | None, main: str) -> dict[str, dict[str, str]]:
    """Accept only hash-linked, successful receipts in the controller's root.

    Worker completion files are deliberately outside this allowlisted shape.
    An ancestry claim or ACCEPTED label without runtime evidence is insufficient.
    """
    if root is None or not root.is_dir():
        return {}
    facts: dict[str, dict[str, str]] = {}
    for path in sorted(root.glob("*-accepted.json")):
        try:
            raw = path.read_bytes()
            data = json.loads(raw)
            if not isinstance(data, dict) or data.get("lifecycle") != "ACCEPTED":
                continue
            package = data.get("package")
            candidate, integrated = data.get("candidate_sha"), data.get("integrated_main_v2_sha")
            if not isinstance(package, str) or not package:
                continue
            if not all(isinstance(value, str) and re.fullmatch(r"[a-f0-9]{40}", value) for value in (candidate, integrated)):
                continue
            if git(repo, ["merge-base", "--is-ancestor", candidate, integrated])[0] != 0 or git(repo, ["merge-base", "--is-ancestor", integrated, main])[0] != 0:
                continue
            evidence = 0
            for path_key, hash_key in (("integrated_receipt", "receipt_sha256"), ("quality_receipt", "quality_receipt_sha256")):
                if path_key not in data:
                    continue
                nested_path = Path(data[path_key])
                nested = hashed_json(nested_path, data.get(hash_key))
                if not isinstance(nested, dict) or nested.get("source_sha", nested.get("product_sha")) != integrated:
                    raise ValueError("receipt revision mismatch")
                if "commands" in nested:
                    if not verified_commands(nested["commands"], nested_path.parent, integrated):
                        raise ValueError("unsuccessful or unverified command logs")
                else:
                    if type(nested.get("exit")) is not int or nested["exit"] != 0 or nested.get("timeout") or nested.get("limit_failure"):
                        raise ValueError("unsuccessful runtime gate")
                    result = hashed_json(Path(nested["result_path"]), nested["result_sha256"])
                    if not isinstance(result, dict):
                        raise ValueError("missing runtime result")
                    if hashlib.sha256((nested_path.parent / "pty.log").read_bytes()).hexdigest() != nested.get("log_sha256"):
                        raise ValueError("runtime log hash mismatch")
                    if data.get("result_sha256", nested["result_sha256"]) != nested["result_sha256"]:
                        raise ValueError("result hash mismatch")
                evidence += 1
            if "native_commands_sha256" in data:
                native = Path(data["native_release"])
                commands = hashed_json(native / "commands.json", data["native_commands_sha256"])
                if not verified_commands(commands, native, integrated):
                    raise ValueError("unverified native command logs")
                evidence += 1
            if not evidence:
                continue
            facts[package] = {"candidate": candidate, "integrated": integrated, "scope": str(data.get("scope", "scoped gate")), "receipt": path.name, "hash": hashlib.sha256(raw).hexdigest()}
        except (OSError, ValueError, TypeError, KeyError):
            # Missing or corrupted evidence never produces DONE.
            continue
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
    relations = {tip: ancestry(repo, captured, tip) for tip in tips if tip}
    facts = receipt_facts(repo, ns.receipts_root, captured)
    rc, alias_output, error = git(repo, ["for-each-ref", "--format=%(refname)%09%(symref)", "refs/heads", "refs/remotes"])
    if rc != 0:
        raise RuntimeError(f"symbolic-ref census failed: {error}")
    aliases = [f"{ref} -> {target}" for ref, target in (line.split("\t", 1) for line in alias_output.splitlines()) if target]
    rows: list[list[str]] = []
    for row in wts:
        branch = row.get("branch", "")
        name = branch.removeprefix("refs/heads/") if branch else f"DETACHED@{row.get('head', '')[:12]}"
        tip = row.get("head", "UNKNOWN")
        rows.append([row.get("path", ""), name, tip, relations.get(tip, "UNKNOWN"), counts.get(tip, "UNKNOWN"),
                     row.get("dirty", "UNKNOWN"), row.get("locked", "—"), row.get("prunable", "—"), "worktree"])
    for ref, tip in local:
        if ref not in wt_branches:
            rows.append(["—", ref.removeprefix("refs/heads/"), tip, relations.get(tip, "UNKNOWN"), counts.get(tip, "UNKNOWN"), "—", "—", "—", "local branch (no worktree)"])
    for ref, tip in remote:
        if ref not in wt_branches:
            rows.append(["—", ref.removeprefix("refs/remotes/"), tip, relations.get(tip, "UNKNOWN"), counts.get(tip, "UNKNOWN"), "—", "—", "—", "remote-tracking branch (no worktree)"])

    def fact_note(package: str, fact: dict[str, str]) -> str:
        return f"DONE — {package}; candidate {fact['candidate'][:12]} accepted on integrated {fact['integrated'][:12]}; receipt {fact['receipt']} SHA-256 {fact['hash']}"

    def note(tip: str, name: str) -> str:
        matching = [fact_note(package, fact) for package, fact in facts.items() if tip in (fact["candidate"], fact["integrated"])]
        if matching:
            return "; ".join(matching) + "; scoped acceptance, not full release"
        if name in ("prod/native-tui-parity", "origin/prod/native-tui-parity"):
            return "PARTIAL SALVAGE / NOT MERGED — selective salvage only"
        return "acceptance unknown; ancestry is not product acceptance"

    def package_status(package: str) -> str:
        fact = facts.get(package)
        return fact_note(package, fact) if fact else "UNKNOWN — no verified controller acceptance receipt available"

    def integration_status(filename: str) -> str:
        try:
            if ns.receipts_root is None:
                return "UNKNOWN — integration receipt unavailable"
            record = json.loads((ns.receipts_root / filename).read_bytes())
            revision = record["main_v2_sha"]
            if not isinstance(revision, str) or not re.fullmatch(r"[a-f0-9]{40}", revision):
                return "UNKNOWN — invalid integration receipt"
            if git(repo, ["merge-base", "--is-ancestor", revision, captured])[0] != 0:
                return "UNKNOWN — integration revision not ancestral"
            return f"INTEGRATED on {revision[:12]}; gate acceptance pending"
        except (OSError, ValueError, TypeError, KeyError):
            return "UNKNOWN — integration receipt unavailable"

    main_wt = next((wt for wt in wts if wt.get("branch") == "refs/heads/main-v2"), None)
    dirty_note = f"Canonical worktree status: {main_wt['dirty']}. Dirty work is preserved; this census does not establish its owner." if main_wt else "Canonical worktree unavailable."
    active = [
        ("G5-NATIVE-LIVE-RESIZE", package_status("G5-NATIVE-LIVE-RESIZE"), "Scoped native resize and cleanup."),
        ("G5-BRIDGE-LIBRARY-MAINTENANCE", package_status("G5-BRIDGE-LIBRARY-MAINTENANCE"), "Scoped library quality/lifecycle."),
        ("G4-SESSION-EXECUTION-SERIALIZATION", package_status("G4-SESSION-EXECUTION-SERIALIZATION"), "Per-session ownership only; full G4 remains open."),
        ("receipt docs", integration_status("docs-integration.json"), "Evidence-only documentation; no product acceptance."),
        ("G5-NATIVE-BRACKETED-PASTE", package_status("G5-NATIVE-BRACKETED-PASTE"), "Frozen installed gate; repair branch v2/native-paste-repair-y52o0gi3."),
        ("V2-SERVER-LIBRARY-QUALITY", package_status("V2-SERVER-LIBRARY-QUALITY") if "V2-SERVER-LIBRARY-QUALITY" in facts else integration_status("server-quality-integration.json"), "Library quality and focused regressions; full server gate remains open."),
        ("CLI mechanical", integration_status("cli-mechanical-integration.json"), "Global CLI quality remains open."),
        ("DISC-101 identity", package_status("DISC-101-IDENTITY"), "Actual native caller identity contract; no worker acceptance inferred."),
        ("Live interruption", package_status("G4-LIVE-SESSION-INTERRUPTION"), "Authenticated daemon cancellation and cleanup."),
        ("Interactive permissions", "OPEN", "Real request/reply approval and denial journey."),
        ("Mac + Web + Ubuntu release G8", "OPEN", "Requires all release gates on one exact SHA."),
    ]

    lines = ["# V2 Worktree / Branch / Merge Status", "",
             f"> Snapshot UTC: `{snapshot}` | captured full `main-v2`: `{captured}` | generator: `tools/update_worktree_merge_status.py`",
             f"> LEDGER CENSUS DONE; full release OPEN. {dirty_note}", "",
             "## Active package summary", "", "| Package | Status | Evidence / scope note |", "|---|---|---|",
             *["| " + " | ".join(esc(cell) for cell in row) + " |" for row in active], "",
             f"**Coverage counts:** worktrees={len(wts)}; all-local-refs={len(local)}; all-remote-tracking-refs={len(remote)}; unique tips={len(tips)}; ledger rows={len(rows)}. Rows are worktrees + local refs without worktrees + remote refs without worktrees (not naive addition of overlapping sets).",
             "**Excluded symbolic aliases:** " + ("; ".join(esc(alias) for alias in aliases) if aliases else "none"),
             "**Worktree-prefix note:** rows marked `worktree` come directly from `git worktree list --porcelain`; ref rows without worktrees follow.", "",
             "## Complete census", "", "| # | Absolute worktree path | Branch / detached | Tip (short; full canonicalSHA) | Ancestry vs captured main-v2 | left right (`main-v2...tip`) | Dirty | Lock | Prunable | Row kind / status note |", "|---:|---|---|---|---|---:|---|---|---|---|"]
    for i, row in enumerate(rows, 1):
        path, name, tip, relation, lr, dirty, lock, prunable, kind = row
        lines.append("| " + " | ".join([str(i), esc(path), esc(name), f"`{esc(tip[:12])}` (`{esc(tip)}`)", f"**{esc(relation)}**", f"`{esc(lr)}`", esc(dirty), esc(lock), esc(prunable), esc(kind + "; " + note(tip, name))]) + " |")
    lines += ["", "## Generator and status definitions", "```sh", "python3 tools/update_worktree_merge_status.py --repo /path/to/repo --output worklog/V2-WORKTREE-MERGE-STATUS.md --receipts-root /trusted/controller/receipts", "```", "- `DONE` is emitted only from an explicit controller receipt with an exact accepted SHA; the receipt SHA must be ancestral to the captured main-v2. Missing/unreadable receipt roots produce UNKNOWN, never guessed acceptance.", "- Worker `CANDIDATE DONE` is not product DONE. `MERGED`, `AHEAD`, and `DIVERGED` are ancestry facts only.", "- The earlier nested-general attempt included forbidden `model`, `sessionID`, and `background` keys; the tool rejected it with `Session ses_f094e9760ffelZesYzwQvk1myl is not a child of the current session`. It was not retried and no inheritance success is claimed.", "", "## Completion receipt", "- LEDGER CENSUS DONE; full release OPEN.", "- No fetch, ref mutation, prune, reset, worktree creation/deletion, product write, Cargo/build/test/PTY/network gate, user DB, or secret access performed."]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n")
    print(f"Ledger {captured[:12]}: {len(wts)} worktrees, {len(local)} local refs, {len(remote)} remote refs, {len(rows)} census rows; {len(facts)} verified scoped acceptances.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
