#!/usr/bin/env python3
"""Materialize a trusted release template using one attested build.json."""
from __future__ import annotations
import argparse, copy, pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import receipts

COPY = ("profile", "native", "target", "binary_sha256", "native_library_sha256",
        "archive_sha256", "runner_sha256", "frozen_tests", "platform")

def materialize(template_path: pathlib.Path, build_path: pathlib.Path, source: str, output: pathlib.Path) -> dict:
    template = receipts.load_json(template_path)
    build = receipts.load_json(build_path)
    if template.get("source_sha") != receipts.TEMPLATE_SHA:
        raise receipts.ReceiptError("trusted template source_sha is not sentinel")
    contract = template.get("artifact_contract")
    if not isinstance(contract, dict) or contract.get("source_sha") != receipts.TEMPLATE_SHA:
        raise receipts.ReceiptError("trusted template artifact contract is mixed")
    if build.get("source_sha") != source:
        raise receipts.ReceiptError("build.json source_sha does not equal requested source")
    receipts.validate_manifest(build)
    for key in ("runner_sha256", "frozen_tests"):
        if build[key] != contract[key]:
            raise receipts.ReceiptError(f"build.json {key} differs from trusted template")
    concrete = copy.deepcopy(template)
    concrete["source_sha"] = source
    out_contract = concrete["artifact_contract"]
    out_contract["source_sha"] = source
    for key in COPY:
        if key in build:
            out_contract[key] = copy.deepcopy(build[key])
    receipts.validate_manifest(out_contract)
    receipts.write_json_atomic_exclusive(output, concrete)
    return concrete

def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--template", required=True); p.add_argument("--build", required=True)
    p.add_argument("--source-sha", required=True); p.add_argument("--output", required=True)
    args = p.parse_args()
    try:
        materialize(pathlib.Path(args.template), pathlib.Path(args.build), args.source_sha, pathlib.Path(args.output))
    except (receipts.ReceiptError, KeyError) as exc:
        print(f"MATERIALIZATION FAIL: {exc}", file=sys.stderr); return 1
    return 0
if __name__ == "__main__": sys.exit(main())
