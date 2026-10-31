#!/usr/bin/env python3
"""Negative-control self-test for the OC2 UI parity harness.

Proves the comparator catches:
  * a one-cell foreground color change,
  * a cursor move,
  * a width truncation (viewport and row width),
  * a focus move,
  * an extra semantic action trace,
and that recording is deterministic and rejects mixed pins.

Run: python3 tools/ui_parity/selftest.py
"""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent.parent
sys.path.insert(0, str(TOOLS))
import ui_parity as harness  # noqa: E402

PASSED = []


def ok(name: str, condition: bool, detail: str = "") -> None:
    if not condition:
        raise AssertionError(f"{name} failed: {detail}")
    PASSED.append(name)
    print(f"PASS {name}")


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=1, sort_keys=True) + "\n", encoding="utf-8")


def run(args):
    return subprocess.run(
        [sys.executable, str(TOOLS / "ui_parity.py")] + args,
        cwd=str(REPO),
        text=True,
        capture_output=True,
        timeout=120,
    )


def diff_kinds(report):
    result = report["results"][0]
    return set(result.get("diff_summary", {}).get("by_kind", {}).keys())


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="oc2-ui-parity-selftest-") as raw:
        root = Path(raw)
        cols, rows = 12, 4
        case = {
            "id": "selftest-static",
            "purpose": "deterministic comparator self-test",
            "viewport": {"columns": cols, "rows": rows},
            "theme": {"name": "opencode", "mode": "dark"},
            "settings": {},
            "state_overrides": {},
            "events": [],
            "clock_ms": 0,
            "expected_rules": ["self-test"],
            "compare": [
                "styled_cells",
                "cursor",
                "layout_and_clipping",
                "focus",
                "semantic_actions",
            ],
            "static_screen": {
                "default_style": {"fg": "#ffffff", "bg": "#000000"},
                "rows": [
                    {
                        "text": "HelloWorld",
                        "w": cols,
                        "spans": [
                            {"c": 0, "n": 5, "fg": "#ff0000", "bg": "#000000"}
                        ],
                    },
                    {"text": "abcdefghijkl", "w": cols, "spans": []},
                    {"text": "second row", "w": cols, "spans": []},
                    {"text": "tail", "w": cols, "spans": []},
                ],
                "cursor": {"row": 1, "col": 2, "visible": True},
                "focus": {"value": "composer", "source": "selftest"},
                "actions": [
                    {"seq": 0, "type": "key", "key": "x", "applied": True}
                ],
                "theme": {"name": "opencode", "mode": "dark"},
            },
        }
        manifest = {
            "schema": "oc2.ui-parity.manifest/v1",
            "oc2_bin": "{REPO}/target/release/oc2",
            "pins": {
                "product": "opencode-rk",
                "upstream_commit": "450e7b92412c780a7b5a75226bb8d49aaca24e22",
                "opentui_commit": "8d6065a3e60039d16c6ee697692dcfbc46d3c786",
            },
            "renderers": {
                "static": {
                    "command": [
                        "{PYTHON}",
                        "{TOOLS}/renderers/static_json.py",
                    ],
                    "timeout_s": 30,
                    "supports_events": True,
                    "supports_state": True,
                }
            },
            "fixtures": ["cases/selftest-static.json"],
        }
        write_json(root / "cases" / "selftest-static.json", case)
        write_json(root / "manifest.json", manifest)
        common = [
            "--fixture-manifest",
            str(root / "manifest.json"),
            "--golden-dir",
            str(root / "goldens"),
            "--renderer-profile",
            "static",
            "--filter",
            "selftest-static",
        ]
        record = run(["--record", "--force"] + common)
        ok("record exits 0", record.returncode == 0, record.stdout + record.stderr)
        golden_path = root / "goldens" / "selftest-static.golden.json"
        ok("golden written", golden_path.exists())
        first_bytes = golden_path.read_bytes()
        record_again = run(["--record", "--force"] + common)
        ok(
            "record is deterministic",
            record_again.returncode == 0
            and golden_path.read_bytes() == first_bytes,
            "golden bytes differ between identical recordings",
        )
        golden = json.loads(first_bytes)
        ok(
            "golden stores compact cell/style data",
            golden["rows"][0]["text"].startswith("HelloWorld")
            and golden["rows"][0]["spans"][0]["c"] == 0
            and '"\"_style\""' not in json.dumps(golden),
        )

        base_check = run(["--check", "--json-report", str(root / "base.json")] + common)
        ok("baseline check passes", base_check.returncode == 0, base_check.stdout)
        ok(
            "baseline report says pass",
            json.loads((root / "base.json").read_text())["results"][0]["status"] == "pass",
        )

        def mutated(mutation):
            report_path = root / f"{mutation}.json"
            command = json.dumps(
                [
                    sys.executable,
                    str(TOOLS / "renderers" / "static_json.py"),
                    "--mutation",
                    mutation,
                ]
            )
            result = run(
                [
                    "--check",
                    "--json-report",
                    str(report_path),
                    "--renderer-command",
                    command,
                ]
                + common
            )
            return result, json.loads(report_path.read_text())

        result, report = mutated("one_cell_color")
        kinds = diff_kinds(report)
        ok("one-cell color change fails", result.returncode == 1, result.stdout)
        ok("one-cell color reports exactly one fg diff", kinds == {"fg"}, kinds)

        result, report = mutated("cursor_move")
        ok("cursor move fails", result.returncode == 1, result.stdout)
        ok("cursor move reports cursor diff", "cursor" in diff_kinds(report), diff_kinds(report))

        result, report = mutated("width_truncate")
        kinds = diff_kinds(report)
        ok("width truncation fails", result.returncode == 1, result.stdout)
        ok(
            "width truncation reports viewport and row_width",
            {"viewport", "row_width"}.issubset(kinds),
            kinds,
        )

        result, report = mutated("focus_change")
        ok("focus move fails", result.returncode == 1, result.stdout)
        ok("focus move reports focus diff", "focus" in diff_kinds(report), diff_kinds(report))

        result, report = mutated("action_extra")
        ok("extra action fails", result.returncode == 1, result.stdout)
        ok(
            "extra action reports action_trace",
            "action_trace" in diff_kinds(report),
            diff_kinds(report),
        )

        # Pin drift must fail unless explicitly allowed.
        expected = harness.load_json(golden_path)
        screen = {
            "schema": harness.SCHEMA_SCREEN,
            "viewport": expected["viewport"],
            "default_style": expected["default_style"],
            "theme": expected["theme"],
            "cursor": expected["cursor"],
            "focus": expected["focus"],
            "rows": expected["rows"],
            "actions": expected["actions"],
            "environment": {
                "adapter": expected["capture"]["adapter"],
                "product": expected["capture"]["product"],
            },
        }

        class Options:
            max_diffs = 25
            allow_pin_drift = False

        diffs, _ = harness.compare_screens(
            expected, screen, case, {"product": "different"}, Options()
        )
        ok("mixed pins are rejected", "pin_mismatch" in diffs.counts, diffs.counts)

        wide, truncated = harness.expand_row(
            {"text": "界a", "w": 3, "spans": []},
            3,
            {"fg": "#ffffff", "bg": "#000000"},
        )
        ok("wide cell keeps a continuation column", wide[1]["cont"] and not truncated)

        kit = harness.load_json(
            REPO / "docs" / "ui-parity" / "opencode-ui-parity-kit" / "checklist.json"
        )
        ok(
            "kit obligations loaded",
            len(kit["items"]) == 223,
            len(kit["items"]),
        )
        fixtures = harness.load_json(
            REPO / "docs" / "ui-parity" / "opencode-ui-parity-kit" / "fixtures.json"
        )
        ok("kit fixture catalog loaded", len(fixtures["cases"]) == 196)

    print(f"\nSELFTEST PASS: {len(PASSED)} checks")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        print(f"SELFTEST FAIL: {error}", file=sys.stderr)
        sys.exit(1)
