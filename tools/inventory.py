"""Inventory and coverage checks. Candidate mappings are never accepted reviews."""
from __future__ import annotations
import argparse, fnmatch, hashlib, json, pathlib, re, subprocess, sys
from typing import Any, Iterable

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
from tools.source_lock import validate_lock, verify_checkout


def parse_tree_record(record: bytes) -> dict[str, Any]:
    try:
        meta, name = record.split(b"\t", 1)
        mode, kind, blob, size = meta.decode("ascii").split()
        if not re.fullmatch(r"[0-7]{6}", mode) or kind not in {"blob", "commit"}:
            raise ValueError("Unexpected tracked entry")
        if not re.fullmatch(r"[0-9a-f]{40,64}", blob):
            raise ValueError("Invalid object ID")
        return {"path": name.decode("utf-8", errors="surrogateescape"), "mode": mode, "kind": kind, "blob": blob, "bytes": None if size == "-" else int(size)}
    except (ValueError, UnicodeDecodeError) as exc:
        raise ValueError("Malformed git ls-tree record") from exc


def tracked(repo: pathlib.Path, commit: str) -> Iterable[dict[str, Any]]:
    process = subprocess.Popen(["git", "-C", str(repo), "ls-tree", "-r", "-l", "-z", commit], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None
    pending = b""
    try:
        while True:
            chunk = process.stdout.read(65536)
            if not chunk:
                break
            pending += chunk
            while b"\0" in pending:
                record, pending = pending.split(b"\0", 1)
                yield parse_tree_record(record)
        if pending:
            raise ValueError("Truncated ls-tree output")
        if process.wait() != 0:
            stderr = process.stderr.read().decode("utf-8", errors="replace") if process.stderr else ""
            raise RuntimeError(f"git ls-tree failed: {stderr[:512]}")
    finally:
        process.stdout.close()
        if process.stderr:
            process.stderr.close()
        if process.poll() is None:
            process.kill(); process.wait()


def check_coverage(inventory: list[dict], reviews: list[dict], task_ids: set[str]) -> list[str]:
    errors = []
    if not inventory:
        return ["Full source inventory is absent"]
    by_key = {}
    for review in reviews:
        key = review.get("key")
        if key in by_key:
            errors.append(f"Duplicate review: {key}")
        by_key[key] = review
    seen = set()
    for entry in inventory:
        key = entry["key"]
        if key in seen:
            errors.append(f"Duplicate inventory key: {key}")
        seen.add(key)
        review = by_key.get(key)
        if not review:
            errors.append(f"Unreviewed source: {key}"); continue
        if review.get("blob") != entry["blob"]:
            errors.append(f"Stale blob review: {key}")
        if review.get("status") != "reviewed" or not review.get("reviewReceipt"):
            errors.append(f"Missing trusted review receipt: {key}")
        disposition = review.get("disposition")
        owners = review.get("featureIds", [])
        if disposition == "product":
            if not owners or not set(owners) <= task_ids:
                errors.append(f"Unowned product source: {key}")
            if not review.get("behaviorIds"):
                errors.append(f"Missing behavior surfaces: {key}")
        elif disposition == "nonproduct":
            if not review.get("rationale"):
                errors.append(f"Unjustified scope disposition: {key}")
        else:
            errors.append(f"Unresolved scope: {key}")
    for extra in set(by_key) - seen:
        errors.append(f"Review not in frozen inventory: {extra}")
    return errors


def read_jsonl(path: pathlib.Path) -> list[dict]:
    if not path.is_file():
        return []
    with path.open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def create_inventory(checkout_root: pathlib.Path, output: pathlib.Path) -> dict:
    lock = json.loads((ROOT / "sources/upstream.lock.json").read_text(encoding="utf-8"))
    sources = validate_lock(lock)
    selectors = json.loads((ROOT / "sources/ownership-candidates.json").read_text(encoding="utf-8"))["groups"]
    output.mkdir(parents=True, exist_ok=True)
    manifest = {"schemaVersion": 2, "repositories": []}
    for source in sources:
        repo = checkout_root / str(source["id"])
        identity = verify_checkout(repo, source)
        commit = str(source["commit"])
        out = output / (str(source["id"]) + ".jsonl")
        count = byte_count = 0
        with out.open("w", encoding="utf-8") as handle:
            for entry in tracked(repo, commit):
                entry.update(repository=source["id"], commit=commit, key=f"{source['id']}:{commit}:{entry['path']}", status="unreviewed", candidateGroups=[group for group, patterns in selectors.items() if group != "discovery" and any(fnmatch.fnmatchcase(entry["path"], pattern) for pattern in patterns)])
                handle.write(json.dumps(entry, ensure_ascii=True) + "\n")
                count += 1; byte_count += entry["bytes"] or 0
        manifest["repositories"].append({"repository": source["id"], "commit": commit, "tree": identity["treeSha"], "originUrl": identity["originUrl"], "licensePath": identity["licensePath"], "licenseBlobSha": identity["licenseBlobSha"], "connectivityVerified": identity["connectivityVerified"], "detachedHead": identity["branch"] == "", "cleanWorktree": not identity["dirty"], "entries": count, "sourceBytes": byte_count, "inventoryFile": out.name, "inventorySha256": hashlib.sha256(out.read_bytes()).hexdigest()})
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkouts", type=pathlib.Path, default=ROOT / ".upstream")
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "sources/inventory")
    args = parser.parse_args()
    print(json.dumps(create_inventory(args.checkouts, args.output), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
