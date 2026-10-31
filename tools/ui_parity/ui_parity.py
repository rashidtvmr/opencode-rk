#!/usr/bin/env python3
"""OC2 UI parity harness: render, record goldens, structured diff.

Stdlib only.  The renderer is an injectable executable that receives a JSON
request on stdin and writes a JSON screen document on stdout.  See
tools/ui_parity/README.md for the protocol and exact commands.

Exit codes: 0 = all checked fixtures passed, 1 = a comparison failed,
2 = usage/configuration/renderer error.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import subprocess
import sys
import unicodedata
from pathlib import Path

SCHEMA_GOLDEN = "oc2.ui-parity.golden/v1"
SCHEMA_SCREEN = "oc2.ui-parity.screen/v1"
SCHEMA_REPORT = "oc2.ui-parity.report/v1"
SCHEMA_REQUEST = "oc2.ui-parity.request/v1"

ATTR_BITS = (
    "bold",
    "dim",
    "italic",
    "underline",
    "blink",
    "reverse",
    "hidden",
    "strike",
)


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def load_json(path: Path):
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def dump_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        json.dump(value, handle, indent=1, sort_keys=True, ensure_ascii=False)
        handle.write("\n")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def char_width(ch: str) -> int:
    """Terminal columns for one code point (combining = 0, wide/full = 2)."""
    if not ch:
        return 0
    if unicodedata.combining(ch):
        return 0
    if unicodedata.category(ch) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def normalize_attrs(attrs) -> tuple:
    if attrs is None:
        return ()
    if isinstance(attrs, str):
        attrs = [part for part in attrs.replace(",", " ").split() if part]
    known = set(ATTR_BITS)
    return tuple(sorted({str(item) for item in attrs if str(item) in known}))


def blank_cell(default_style):
    return {
        "ch": " ",
        "fg": default_style["fg"],
        "bg": default_style["bg"],
        "attrs": (),
        "cont": False,
    }


def validate_row(row, where: str) -> None:
    if not isinstance(row, dict):
        raise ValueError(f"{where}: row must be an object")
    if not isinstance(row.get("text"), str):
        raise ValueError(f"{where}: row.text must be a string")
    width = row.get("w")
    if not isinstance(width, int) or width <= 0:
        raise ValueError(f"{where}: row.w must be a positive integer")
    spans = row.get("spans", [])
    if not isinstance(spans, list):
        raise ValueError(f"{where}: row.spans must be a list")
    for span in spans:
        if not isinstance(span, dict):
            raise ValueError(f"{where}: span must be an object")
        for key in ("c", "n"):
            if not isinstance(span.get(key), int) or span[key] < 0:
                raise ValueError(f"{where}: span.{key} must be a non-negative integer")
        if span.get("n", 0) == 0:
            raise ValueError(f"{where}: span.n must be positive")


def validate_screen(screen, where: str = "screen") -> None:
    if not isinstance(screen, dict):
        raise ValueError(f"{where}: root must be an object")
    if screen.get("schema") != SCHEMA_SCREEN:
        raise ValueError(f"{where}: schema must be {SCHEMA_SCREEN}")
    viewport = screen.get("viewport") or {}
    for key in ("cols", "rows"):
        if not isinstance(viewport.get(key), int) or viewport[key] <= 0:
            raise ValueError(f"{where}: viewport.{key} must be a positive integer")
    default = screen.get("default_style") or {}
    for key in ("fg", "bg"):
        if not isinstance(default.get(key), str):
            raise ValueError(f"{where}: default_style.{key} must be a string")
    rows = screen.get("rows")
    if not isinstance(rows, list) or len(rows) != viewport["rows"]:
        raise ValueError(
            f"{where}: rows must contain exactly viewport.rows entries "
            f"({viewport['rows']})"
        )
    for index, row in enumerate(rows):
        validate_row(row, f"{where}: rows[{index}]")


def expand_row(row, cols: int, default_style):
    """Expand a golden row (text + style spans) to one cell per column."""
    cells = []
    for ch in row.get("text", ""):
        width = char_width(ch)
        if width == 0:
            if cells:
                cells[-1]["ch"] += ch
            continue
        cells.append(
            {
                "ch": ch,
                "fg": default_style["fg"],
                "bg": default_style["bg"],
                "attrs": (),
                "cont": False,
            }
        )
        if width == 2:
            cells.append(
                {
                    "ch": "",
                    "fg": default_style["fg"],
                    "bg": default_style["bg"],
                    "attrs": (),
                    "cont": True,
                }
            )
    while len(cells) < cols:
        cells.append(blank_cell(default_style))
    truncated = len(cells) > cols
    if truncated:
        cells = cells[:cols]
    for span in row.get("spans", []):
        start = span.get("c", 0)
        count = span.get("n", 0)
        fg = span.get("fg", default_style["fg"])
        bg = span.get("bg", default_style["bg"])
        attrs = normalize_attrs(span.get("attrs"))
        for col in range(start, min(start + count, cols)):
            cells[col]["fg"] = fg
            cells[col]["bg"] = bg
            cells[col]["attrs"] = attrs
    return cells, truncated


def canonical_cursor(cursor):
    if not isinstance(cursor, dict):
        return {"visible": False, "row": None, "col": None}
    visible = bool(cursor.get("visible"))
    return {
        "visible": visible,
        "row": cursor.get("row") if visible else None,
        "col": cursor.get("col") if visible else None,
    }


def compact_theme(theme):
    if not isinstance(theme, dict):
        return {"name": None, "mode": None}
    return {"name": theme.get("name"), "mode": theme.get("mode")}


class Diff:
    def __init__(self, limit: int):
        self.limit = limit
        self.items = []
        self.counts = {}
        self.truncated = 0

    def add(self, kind: str, **fields) -> None:
        self.counts[kind] = self.counts.get(kind, 0) + 1
        if len(self.items) >= self.limit:
            self.truncated += 1
            return
        item = {"kind": kind}
        item.update(fields)
        self.items.append(item)

    def summary(self):
        return {
            "total": sum(self.counts.values()),
            "by_kind": dict(sorted(self.counts.items())),
            "truncated": self.truncated,
        }


def compare_screens(expected, actual, fixture, pins, options) -> tuple:
    """Return (diffs, warnings). expected is a recorded golden."""
    diffs = Diff(options.max_diffs)
    warnings = []
    compare = set(fixture.get("compare") or [])
    exp_view = expected.get("viewport") or {}
    act_view = actual.get("viewport") or {}
    if exp_view != act_view:
        diffs.add(
            "viewport",
            expected=dict(exp_view),
            actual=dict(act_view),
        )

    exp_default = expected.get("default_style") or {"fg": "#ffffff", "bg": "#000000"}
    act_default = actual.get("default_style") or {"fg": "#ffffff", "bg": "#000000"}
    if exp_default.get("fg") != act_default.get("fg") or exp_default.get(
        "bg"
    ) != act_default.get("bg"):
        diffs.add("default_style", expected=exp_default, actual=act_default)

    # PAR-01: pins travel with every golden.
    capture = expected.get("capture") or {}
    golden_pins = capture.get("pins") or {}
    if golden_pins != pins:
        if options.allow_pin_drift:
            warnings.append("pin drift allowed: golden pins differ from manifest pins")
        else:
            diffs.add("pin_mismatch", expected=golden_pins, actual=pins)
    golden_product = capture.get("product") or {}
    actual_env = actual.get("environment") or {}
    actual_product = actual_env.get("product") or {}
    for field in ("version", "sha256"):
        if golden_product.get(field) != actual_product.get(field):
            if options.allow_pin_drift:
                warnings.append(f"pin drift allowed: product.{field}")
            else:
                diffs.add(
                    f"product_{field}",
                    expected=golden_product.get(field),
                    actual=actual_product.get(field),
                )
    if capture.get("adapter") != actual_env.get("adapter"):
        if options.allow_pin_drift:
            warnings.append("pin drift allowed: renderer adapter")
        else:
            diffs.add(
                "renderer_adapter",
                expected=capture.get("adapter"),
                actual=actual_env.get("adapter"),
            )

    exp_theme = compact_theme(expected.get("theme"))
    act_theme = compact_theme(actual.get("theme"))
    if exp_theme != act_theme:
        diffs.add("theme", expected=exp_theme, actual=act_theme)

    if "cursor" in compare:
        exp_cursor = canonical_cursor(expected.get("cursor"))
        act_cursor = canonical_cursor(actual.get("cursor"))
        if exp_cursor != act_cursor:
            diffs.add("cursor", expected=exp_cursor, actual=act_cursor)

    if "focus" in compare:
        exp_focus = expected.get("focus")
        act_focus = actual.get("focus")
        if exp_focus is not None and exp_focus != act_focus:
            diffs.add("focus", expected=exp_focus, actual=act_focus)

    if "semantic_actions" in compare:
        exp_actions = expected.get("actions") or []
        act_actions = actual.get("actions") or []
        if exp_actions != act_actions:
            diffs.add(
                "action_trace",
                expected=exp_actions,
                actual=act_actions,
            )

    if "styled_cells" in compare or "layout_and_clipping" in compare:
        exp_rows = expected.get("rows") or []
        act_rows = actual.get("rows") or []
        act_cols = int(act_view.get("cols") or 0)
        cols = min(int(exp_view.get("cols") or 0), act_cols)
        row_count = min(len(exp_rows), len(act_rows))
        for index in range(row_count):
            exp_row = exp_rows[index]
            act_row = act_rows[index]
            if exp_row.get("w") != act_row.get("w"):
                diffs.add(
                    "row_width",
                    row=index,
                    expected=exp_row.get("w"),
                    actual=act_row.get("w"),
                )
            exp_cells, exp_trunc = expand_row(exp_row, cols, exp_default)
            act_cells, act_trunc = expand_row(act_row, cols, act_default)
            if exp_trunc != act_trunc:
                diffs.add("row_overflow", row=index)
            for col in range(cols):
                exp_cell = exp_cells[col]
                act_cell = act_cells[col]
                if exp_cell["ch"] != act_cell["ch"]:
                    diffs.add(
                        "char",
                        row=index,
                        col=col,
                        expected=exp_cell["ch"],
                        actual=act_cell["ch"],
                    )
                if exp_cell["fg"] != act_cell["fg"]:
                    diffs.add(
                        "fg",
                        row=index,
                        col=col,
                        expected=exp_cell["fg"],
                        actual=act_cell["fg"],
                    )
                if exp_cell["bg"] != act_cell["bg"]:
                    diffs.add(
                        "bg",
                        row=index,
                        col=col,
                        expected=exp_cell["bg"],
                        actual=act_cell["bg"],
                    )
                if exp_cell["attrs"] != act_cell["attrs"]:
                    diffs.add(
                        "attrs",
                        row=index,
                        col=col,
                        expected=list(exp_cell["attrs"]),
                        actual=list(act_cell["attrs"]),
                    )
        for index in range(row_count, max(len(exp_rows), len(act_rows))):
            diffs.add("missing_row", row=index, side="actual" if index >= len(act_rows) else "expected")
    return diffs, warnings


def resolve_fixtures(args, manifest, kit_fixtures):
    fixtures = {}
    for case in kit_fixtures:
        case = dict(case)
        case.setdefault("source", "kit")
        fixtures[case["id"]] = case
    for relative in manifest.get("fixtures", []):
        path = (manifest["__dir__"] / relative).resolve()
        case = load_json(path)
        case = dict(case)
        case["source"] = "local"
        case["__path__"] = str(path)
        fixtures[case["id"]] = case
    selected = []
    patterns = []
    for raw in args.filter or []:
        patterns.extend(part for part in raw.split(",") if part)
    for fixture_id in sorted(fixtures):
        if patterns and not any(
            fnmatch.fnmatch(fixture_id, pattern) or pattern in fixture_id
            for pattern in patterns
        ):
            continue
        selected.append(fixtures[fixture_id])
    return selected


def profile_command(args, manifest, profile_name):
    renderers = manifest.get("renderers") or {}
    if args.renderer_command:
        try:
            command = json.loads(args.renderer_command)
        except json.JSONDecodeError as error:
            print(f"error: --renderer-command is not valid JSON: {error}", file=sys.stderr)
            raise SystemExit(2) from error
        if not isinstance(command, list) or not all(isinstance(x, str) for x in command):
            print(
                "error: --renderer-command must be a JSON array of strings",
                file=sys.stderr,
            )
            raise SystemExit(2)
        profile = {"command": command, "timeout_s": args.timeout or 60, "name": "custom"}
    else:
        profile = dict(renderers.get(profile_name) or {})
        if not profile:
            print(f"error: unknown renderer profile: {profile_name}", file=sys.stderr)
            raise SystemExit(2)
        profile["name"] = profile_name
    substitutions = {
        "{PYTHON}": sys.executable,
        "{TOOLS}": str(repo_root() / "tools" / "ui_parity"),
        "{REPO}": str(repo_root()),
        "{OC2_BIN}": args.bin,
    }
    command = []
    for part in profile["command"]:
        for needle, value in substitutions.items():
            part = part.replace(needle, value)
        command.append(part)
    return profile, command


def build_request(fixture, args, pins, profile, command):
    environment = dict(fixture.get("environment") or {})
    viewport = fixture.get("viewport") or {"columns": 80, "rows": 24}
    return {
        "schema": SCHEMA_REQUEST,
        "fixture": fixture,
        "viewport": {
            "cols": int(viewport.get("columns") or 80),
            "rows": int(viewport.get("rows") or 24),
        },
        "theme": fixture.get("theme") or {"name": "opencode", "mode": "dark"},
        "pins": pins,
        "environment": environment,
        "renderer": {"name": profile.get("name"), "command": command},
    }


def run_renderer(profile, command, request, timeout):
    proc = subprocess.run(
        command,
        input=json.dumps(request),
        text=True,
        capture_output=True,
        timeout=timeout,
        cwd=str(repo_root()),
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"renderer exited {proc.returncode}: {proc.stderr.strip()[:2000]}"
        )
    try:
        screen = json.loads(proc.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"renderer produced invalid JSON: {error}") from error
    return screen


def golden_from_screen(fixture, screen, pins, profile):
    environment = screen.get("environment") or {}
    return {
        "schema": SCHEMA_GOLDEN,
        "fixture_id": fixture["id"],
        "capture": {
            "adapter": environment.get("adapter") or profile.get("name"),
            "pins": pins,
            "product": environment.get("product") or {},
            "environment": environment.get("terminal") or {},
            "theme": compact_theme(screen.get("theme") or fixture.get("theme")),
            "normalized": environment.get("normalized") or [],
            "clock_ms": fixture.get("clock_ms", 0),
            "expected_rules": fixture.get("expected_rules", []),
        },
        "viewport": screen.get("viewport"),
        "default_style": screen.get("default_style"),
        "theme": screen.get("theme"),
        "cursor": screen.get("cursor"),
        "focus": screen.get("focus"),
        "rows": screen.get("rows"),
        "actions": screen.get("actions") or [],
    }


def golden_path(args, fixture_id):
    return Path(args.golden_dir) / f"{fixture_id}.golden.json"


def print_result(result):
    line = f"{result['status']:<15} {result['id']}"
    if result.get("detail"):
        line += f"  [{result['detail']}]"
    print(line)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="OC2 UI parity runner (renderer protocol, goldens, structured diff)"
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_const", const="check", dest="mode")
    mode.add_argument("--record", action="store_const", const="record", dest="mode")
    parser.set_defaults(mode="check")
    parser.add_argument("--filter", action="append", help="fixture id glob/substring; repeatable")
    parser.add_argument(
        "--fixture-manifest",
        default=str(repo_root() / "tests" / "fixtures" / "ui-parity" / "manifest.json"),
    )
    parser.add_argument(
        "--golden-dir",
        default=str(repo_root() / "tests" / "fixtures" / "ui-parity" / "goldens"),
    )
    parser.add_argument(
        "--kit-dir",
        default=str(repo_root() / "docs" / "ui-parity" / "opencode-ui-parity-kit"),
    )
    parser.add_argument("--renderer-profile", default="oc2-pty")
    parser.add_argument("--renderer-command", help="JSON array argv; overrides profile")
    parser.add_argument("--bin", default=os.environ.get("OC2_BIN") or "")
    parser.add_argument("--timeout", type=float, default=None)
    parser.add_argument("--max-diffs", type=int, default=25)
    parser.add_argument("--force", action="store_true", help="overwrite existing goldens")
    parser.add_argument(
        "--require-golden",
        action="store_true",
        help="treat kit fixtures without a golden as failures",
    )
    parser.add_argument(
        "--allow-kit-record",
        action="store_true",
        help="allow recording kit fixtures (default: local fixtures only)",
    )
    parser.add_argument(
        "--allow-unsupported",
        action="store_true",
        help="record fixtures with events/state even if the adapter cannot replay them",
    )
    parser.add_argument(
        "--allow-pin-drift",
        action="store_true",
        help="warn instead of fail when product/renderer pins changed",
    )
    parser.add_argument("--json-report", help="write machine-readable report")
    parser.add_argument("--list", action="store_true", help="list fixtures and golden state")
    parser.add_argument("--validate-kit", action="store_true", help="validate kit counts/ids")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args(argv)

    manifest_path = Path(args.fixture_manifest).resolve()
    if not manifest_path.exists():
        print(f"error: fixture manifest not found: {manifest_path}", file=sys.stderr)
        return 2
    manifest = load_json(manifest_path)
    manifest["__dir__"] = manifest_path.parent
    kit_dir = Path(args.kit_dir).resolve()
    kit_fixtures_doc = load_json(kit_dir / "fixtures.json")
    kit_checklist = load_json(kit_dir / "checklist.json")
    kit_fixtures = kit_fixtures_doc.get("cases") or []

    if args.validate_kit:
        counts = kit_checklist.get("counts") or {}
        errors = []
        if len(kit_fixtures) != 196:
            errors.append(f"fixtures.json cases={len(kit_fixtures)} (expected 196)")
        if len(kit_checklist.get("items") or []) != 223:
            errors.append(
                f"checklist items={len(kit_checklist.get('items') or [])} (expected 223)"
            )
        if counts.get("PAR") != 24:
            errors.append(f"PAR count={counts.get('PAR')} (expected 24)")
        ids = [case.get("id") for case in kit_fixtures]
        if len(ids) != len(set(ids)):
            errors.append("duplicate fixture ids in kit")
        par_ids = [item.get("id") for item in kit_checklist.get("items", []) if str(item.get("id", "")).startswith("PAR-")]
        if par_ids != [f"PAR-{index:02d}" for index in range(1, 25)]:
            errors.append("PAR ids are not PAR-01..PAR-24 in order")
        if errors:
            for error in errors:
                print(f"KIT FAIL: {error}")
            return 1
        print("KIT OK: 223 obligations (PAR-01..PAR-24), 196 fixtures")
        return 0

    fixtures = resolve_fixtures(args, manifest, kit_fixtures)
    if not fixtures:
        print("error: no fixtures selected", file=sys.stderr)
        return 2

    pins = dict(manifest.get("pins") or {})
    if not args.bin:
        args.bin = (manifest.get("oc2_bin") or "{REPO}/target/release/oc2").replace(
            "{REPO}", str(repo_root())
        )
    profile, command = profile_command(args, manifest, args.renderer_profile)

    if args.list:
        for fixture in fixtures:
            path = golden_path(args, fixture["id"])
            print(
                f"{fixture['id']:<48} {fixture['source']:<5} "
                f"{fixture['viewport'].get('columns')}x{fixture['viewport'].get('rows')} "
                f"events={len(fixture.get('events') or [])} "
                f"golden={'yes' if path.exists() else 'no'}"
            )
        return 0

    results = []
    failures = 0
    checked = 0
    for fixture in fixtures:
        fixture_id = fixture["id"]
        path = golden_path(args, fixture_id)
        if args.mode == "record":
            if fixture["source"] == "kit" and not args.allow_kit_record:
                results.append(
                    {"id": fixture_id, "status": "skipped", "detail": "kit fixture; use --allow-kit-record"}
                )
                continue
            unsupported = []
            if fixture.get("events") and not profile.get("supports_events"):
                unsupported.append("events")
            if fixture.get("state_overrides") and not profile.get("supports_state"):
                unsupported.append("state_overrides")
            if unsupported and not args.allow_unsupported:
                results.append(
                    {
                        "id": fixture_id,
                        "status": "skipped",
                        "detail": "adapter does not replay " + ",".join(unsupported),
                    }
                )
                continue
            if path.exists() and not args.force:
                results.append(
                    {"id": fixture_id, "status": "error", "detail": f"golden exists: {path}"}
                )
                failures += 1
                continue
            request = build_request(fixture, args, pins, profile, command)
            try:
                screen = run_renderer(
                    profile, command, request, args.timeout or profile.get("timeout_s", 60)
                )
                validate_screen(screen, f"{fixture_id} screen")
                if screen.get("viewport") != request["viewport"]:
                    raise ValueError(
                        "renderer viewport "
                        f"{screen.get('viewport')} != requested {request['viewport']}"
                    )
                rejected = [
                    action
                    for action in (screen.get("actions") or [])
                    if action.get("applied") is False
                ]
                if rejected:
                    raise RuntimeError(
                        "adapter could not replay every fixture action: "
                        + ", ".join(str(action.get("key") or action.get("type")) for action in rejected)
                    )
                golden = golden_from_screen(fixture, screen, pins, profile)
                dump_json(path, golden)
            except Exception as error:  # noqa: BLE001 - report as fixture error
                results.append({"id": fixture_id, "status": "error", "detail": str(error)})
                failures += 1
                if args.verbose:
                    print(f"renderer error for {fixture_id}: {error}", file=sys.stderr)
                continue
            results.append(
                {
                    "id": fixture_id,
                    "status": "recorded",
                    "detail": f"{path} sha256={sha256_file(path)[:12]}",
                }
            )
            continue

        # check mode
        if not path.exists():
            status = "fail" if (fixture["source"] == "local" or args.require_golden) else "missing_golden"
            if status == "fail":
                failures += 1
            results.append({"id": fixture_id, "status": status})
            continue
        request = build_request(fixture, args, pins, profile, command)
        try:
            expected = load_json(path)
            if expected.get("schema") != SCHEMA_GOLDEN:
                raise ValueError(f"unsupported golden schema in {path}")
            if expected.get("fixture_id") != fixture_id:
                raise ValueError(
                    f"golden fixture_id {expected.get('fixture_id')!r} != {fixture_id!r}"
                )
            validate_screen({**expected, "schema": SCHEMA_SCREEN}, f"{fixture_id} golden")
            screen = run_renderer(
                profile, command, request, args.timeout or profile.get("timeout_s", 60)
            )
            validate_screen(screen, f"{fixture_id} screen")
        except Exception as error:  # noqa: BLE001
            failures += 1
            results.append({"id": fixture_id, "status": "error", "detail": str(error)})
            if args.verbose:
                print(f"renderer error for {fixture_id}: {error}", file=sys.stderr)
            continue
        checked += 1
        diffs, warnings = compare_screens(expected, screen, fixture, pins, args)
        if diffs.items or diffs.counts:
            failures += 1
            results.append(
                {
                    "id": fixture_id,
                    "status": "fail",
                    "detail": f"{sum(diffs.counts.values())} diffs",
                    "diffs": diffs.items,
                    "diff_summary": diffs.summary(),
                    "warnings": warnings,
                }
            )
        else:
            results.append(
                {
                    "id": fixture_id,
                    "status": "pass",
                    "warnings": warnings,
                }
            )

    for result in results:
        if result["status"] in ("skipped", "missing_golden") and not args.verbose:
            continue
        print_result(result)
        if result["status"] == "fail" and args.verbose:
            for diff in result.get("diffs", []):
                print(json.dumps(diff, ensure_ascii=False, sort_keys=True))
            for warning in result.get("warnings", []):
                print(f"warning: {warning}")

    summary = {
        "fixtures": len(results),
        "checked": checked,
        "passed": sum(1 for r in results if r["status"] == "pass"),
        "failed": sum(1 for r in results if r["status"] == "fail"),
        "errors": sum(1 for r in results if r["status"] == "error"),
        "recorded": sum(1 for r in results if r["status"] == "recorded"),
        "missing_golden": sum(1 for r in results if r["status"] == "missing_golden"),
        "skipped": sum(1 for r in results if r["status"] == "skipped"),
    }
    by_id = {result["id"]: result for result in results}
    coverage = {}
    for fixture in fixtures:
        result = by_id.get(fixture["id"])
        if not result or result["status"] not in ("pass", "fail"):
            continue
        for checklist_id in fixture.get("checklist_ids") or []:
            coverage.setdefault(checklist_id, set()).add(result["status"])
    report = {
        "schema": SCHEMA_REPORT,
        "mode": args.mode,
        "renderer": profile.get("name"),
        "pins": pins,
        "summary": summary,
        "checklist_coverage": {
            "referenced": len(coverage),
            "passed": sum(1 for states in coverage.values() if states == {"pass"}),
            "ids": sorted(coverage),
        },
        "results": results,
    }
    if args.json_report:
        dump_json(Path(args.json_report), report)
    print(
        "summary: "
        + " ".join(f"{key}={value}" for key, value in summary.items())
    )
    if coverage:
        print(
            "checklist: "
            f"{len(coverage)} obligation ids referenced; "
            f"{sum(1 for states in coverage.values() if states == {'pass'})} passed"
        )
    if any(result["status"] == "error" for result in results):
        return 2
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
