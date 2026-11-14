#!/usr/bin/env python3
"""Run the pinned OpenCode/OpenTUI oracle capture adapter.

The adapter emits the same ``oc2.ui-parity.screen/v1`` protocol as the local
oc2 renderers.  Production requests mount the *pinned upstream* app through
its own ``createAppFixture`` and capture real frames; synthetic requests keep
the labelled diagnostic paths.  Expected identities (upstream production file
hashes, native dylib hash, fixture entry hash) arrive inside the request
provenance and are verified fail-closed by both this wrapper and the adapter;
nothing here substitutes a fabricated pin or hash to make output look matched.

Trust boundary (F4): only the provenance-listed files are authenticity-checked
against the pinned upstream checkout.  The pinned checkout itself and every
transitive node_modules dependency are external trust boundaries — dependency
closure is NOT verified by these hashes, and the output says so.

Compatibility (B1): the documented renderer protocol
(``tools/ui_parity/README.md`` "Renderer protocol") — as emitted by the real
consumers ``ui_parity.py build_request`` and ``par03_capture.py`` — remains
accepted for legacy (non-production) requests, with shape validation.
Production requests keep a strict allowlist.

Child isolation (F1): the Bun child never inherits ``os.environ``.  It receives
a minimal allowlisted environment: isolated HOME/TMPDIR/XDG fixture dirs,
PATH, request-validated TERM/COLORTERM, and NO_COLOR.  Ambient secrets and
OPENCODE_* variables cannot reach the child.

Launch handoff (F3): there is NO staging write and NO mutable-path handoff.
The exact candidate TS source bytes are read once, hashed, and passed to the
child as an ``bun --eval <source>`` argv literal (request JSON stays on stdin;
cwd is the oracle's ``packages/tui`` so imports resolve against the pinned
workspace — Bun 1.4.4 sets ``import.meta.dir`` to the cwd in eval mode, the
same semantics the staged layout had). A same-directory hostile writer
therefore cannot swap what the child executes, and no automatic unlink of any
oracle entry exists; foreign and legacy entries are never touched.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve()
REPO = HERE.parents[3]
CAPTURE = HERE.with_name("upstream_capture.ts")
ORACLE = Path("/Users/mymac/Projects/.oc2-features-20261103/artifacts/upstream-opentui068")

SCHEMA_REQUEST = "oc2.ui-parity.request/v1"
EXPECTED_UPSTREAM_COMMIT = "450e7b92412c780a7b5a75226bb8d49aaca24e22"
EXPECTED_OPENTUI_COMMIT = "8d6065a3e60039d16c6ee697692dcfbc46d3c786"
EXPECTED_BUN_RUNTIME = "1.4.4"  # manifest reference_runtime.bun
SUPPORTED_SCENARIOS = {"home-command-palette-v1"}
SCENARIO_CONTRACTS = {
    "home-command-palette-v1": {"search_text": "new", "expected_visible": "New session"},
}
REQUEST_BUDGET_BYTES = 1_048_576
MAX_TIMEOUT_S = 120
STAGE_DIR_REL = "packages/tui"  # bun cwd: bare @opentui/* and relative fixture imports resolve here
SYNTHETIC_FLAGS = {"--component", "--synthetic"}
FIXTURE_CLASSES = ("baseline", "optional", "experimental", "extension")  # ui_parity.py:FIXTURE_CLASSES
# Authority identities for the production request envelope. These are measured
# values, not fabrications: file digests are sha256 of the exact production
# files (and the fixture entry that executes) in the pinned upstream checkout,
# the native dylib digest is sha256 of the installed @opentui/
# core-darwin-arm64 0.6.8 library inside the oracle workspace. Requests
# claiming different expected identities are denied instead of being allowed
# to self-certify a mismatched runtime.
EXPECTED_OPENTUI_CORE_VERSION = "0.6.8"
EXPECTED_FILES = {
    "src/app.tsx": "b97ebed0dea62f1eda897ddde4268b0d30caf271bff74f04c5dc11fe4345faf3",
    "src/routes/home.tsx": "f1c6ae4c353549f5afe57b81ebe807562c5a3ba3539f568e4632b509c3da7ccb",
    "src/component/command-palette.tsx": "61fb73798ad39822e1a7a950b086a10f1540a67621f49a81a64e13fc10b29224",
    "src/ui/dialog.tsx": "33cc6de6fc6107a14f266177ca2d288dd69e3ce9e8f0aa205ce3918cc046f690",
    "src/ui/dialog-select.tsx": "86f43fa17785e1b935fb3d484eba9f079f5252b77adeae40b260d920370a3052",
    "src/config/keybind.ts": "49c8a69149811fee3bf57e1f9104f885d20910c594cae8ba3e99f14c8f454fd8",
    "src/context/keymap.tsx": "d81a9a515f73b6e4b5cc15f6aabe06e51ffb5471ebaa133bb25e791356e9f812",
    "test/fixture/app.ts": "66ce0d97112a8e12b03bf117f6a5a0abf6a4760355cd460e61cb454716ffcacd",
}
EXPECTED_NATIVE_DYLIB_SHA256 = "a2013c73d6bc9835aa59c7aa4ae6d58b0a1b6e0822a7f44ef9e21644bac72910"
# Documented renderer-protocol request keys (legacy consumers).
LEGACY_REQUEST_KEYS = {
    "schema", "fixture", "fixture_class", "viewport", "theme", "clock_ms",
    "pins", "environment", "renderer", "events", "state_overrides", "synthetic",
}
PRODUCTION_REQUEST_KEYS = {
    "schema", "fixture", "viewport", "theme", "clock_ms", "pins", "production",
    "synthetic", "scenario", "search_text", "events", "state_overrides", "provenance",
}
# Child environment allowlist (F1): only explicitly controlled values.
CHILD_ENV_ALLOWED_KEYS = {
    "PATH", "HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME",
    "NO_COLOR", "TERM", "COLORTERM",
}
CHILD_ENV_TERM_KEYS = {"TERM", "COLORTERM"}
CHILD_ENV_VALUE_PATTERN_RE = r"^[A-Za-z0-9_.:+-]{1,64}$"
DEFAULT_TERM = "xterm-256color"
HEX_DIGITS = set("0123456789abcdef")

_CHILD_VALUE_RE = re.compile(CHILD_ENV_VALUE_PATTERN_RE)


class RequestDenied(Exception):
    """The request itself is invalid; no command runs and nothing is written."""


class StageDenied(Exception):
    """Staging refused; the oracle workspace is untouched."""


class WrapperError(Exception):
    """The capture run failed; the denied or failed state is reported for repair."""


def _deny(reason: str):
    raise RequestDenied(reason)


def _is_sha256(value) -> bool:
    return isinstance(value, str) and len(value) == 64 and set(value.lower()) <= HEX_DIGITS


def _bounded_int(value, low, high, what: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        _deny(f"invalid {what}")
    return value


def _validate_legacy_request(raw: dict) -> dict:
    """Documented renderer-protocol shape (README 'Renderer protocol')."""
    unknown = sorted(set(raw) - LEGACY_REQUEST_KEYS)
    if unknown:
        _deny(f"unsupported request keys: {', '.join(unknown)}")
    if raw.get("schema") != SCHEMA_REQUEST:
        _deny("unsupported request schema")

    pins = raw.get("pins")
    if not isinstance(pins, dict) or pins.get("upstream_commit") != EXPECTED_UPSTREAM_COMMIT \
            or pins.get("opentui_commit") != EXPECTED_OPENTUI_COMMIT:
        _deny("pin mismatch")
    # Manifest-style extras are documented and allowed, but must keep shape.
    for key, value in pins.items():
        if key in ("upstream_commit", "opentui_commit"):
            continue
        if isinstance(value, dict):
            for inner_key, inner_value in value.items():
                if not isinstance(inner_key, str) or not isinstance(inner_value, (str, int)) \
                        or len(str(inner_value)) > 128:
                    _deny(f"invalid pin detail: {key}.{inner_key}")
        elif not isinstance(value, (str, int)) or len(str(value)) > 128:
            _deny(f"invalid pin detail: {key}")

    if "fixture_class" in raw:
        fixture_class = raw.get("fixture_class")
        if not isinstance(fixture_class, str) or fixture_class not in FIXTURE_CLASSES:
            _deny("unsupported fixture_class")

    environment = raw.get("environment")
    if environment is not None:
        if not isinstance(environment, dict) or len(environment) > 16:
            _deny("invalid environment map")
        for name, value in environment.items():
            if not isinstance(name, str) or not isinstance(value, str) \
                    or not name or len(name) > 64 or len(value) > 256 \
                    or not name.replace("_", "").replace("-", "").isalnum():
                _deny(f"invalid environment entry: {name!r}")

    renderer = raw.get("renderer")
    if renderer is not None:
        if not isinstance(renderer, dict) or not isinstance(renderer.get("name"), str) \
                or len(renderer.get("name")) > 64:
            _deny("invalid renderer name")
        command = renderer.get("command")
        if not isinstance(command, list) or len(command) > 64 \
                or not all(isinstance(part, str) and len(part) <= 4096 for part in command):
            _deny("invalid renderer command")

    events = raw.get("events", [])
    state = raw.get("state_overrides", {})
    if not isinstance(events, list) or len(events) > 256:
        _deny("invalid events")
    if not isinstance(state, dict) or len(state) > 64:
        _deny("invalid state_overrides")
    for event in events:
        if not isinstance(event, dict) or len(event) > 32:
            _deny("invalid event entry")
    for key, value in state.items():
        if not isinstance(key, str) or not key or len(key) > 128 or len(json.dumps(value)) > 4096:
            _deny(f"invalid state override: {key!r}")

    fixture = raw.get("fixture")
    if not isinstance(fixture, dict) or not isinstance(fixture.get("id"), str) \
            or not 1 <= len(fixture["id"]) <= 200:
        _deny("invalid fixture")
    viewport = raw.get("viewport")
    if not isinstance(viewport, dict):
        _deny("invalid viewport")
    _bounded_int(viewport.get("cols"), 1, 500, "viewport cols")
    _bounded_int(viewport.get("rows"), 1, 200, "viewport rows")
    fixture_viewport = fixture.get("viewport")
    if not isinstance(fixture_viewport, dict) \
            or _bounded_int(fixture_viewport.get("columns"), 1, 500, "fixture columns") != viewport["cols"] \
            or _bounded_int(fixture_viewport.get("rows"), 1, 200, "fixture rows") != viewport["rows"]:
        _deny("fixture/viewport mismatch")

    theme = raw.get("theme")
    if not isinstance(theme, dict) or set(theme) - {"name", "mode", "new_designs"} \
            or theme.get("mode") not in ("dark", "light"):
        _deny("invalid theme")

    clock_ms = _bounded_int(raw.get("clock_ms", 0), 0, 10 ** 12, "clock_ms")

    if raw.get("production"):
        _deny("production mode is not part of the legacy protocol shape")
    synthetic = raw.get("synthetic", False)
    if not isinstance(synthetic, bool):
        _deny("synthetic flag must be boolean")

    parsed = dict(raw)
    parsed["production"] = False
    parsed["synthetic"] = synthetic
    parsed["events"] = events
    parsed["state_overrides"] = state
    parsed["clock_ms"] = clock_ms
    return parsed


def _validate_production_provenance(raw: dict) -> dict:
    provenance = raw.get("provenance")
    if not isinstance(provenance, dict):
        _deny("production request requires provenance")
    expected = provenance.get("expected")
    if not isinstance(expected, dict):
        _deny("production provenance requires expected identities")
    files = expected.get("files")
    if not isinstance(files, dict):
        _deny("provenance files must be a map")
    for rel in files:
        if not isinstance(rel, str) or not rel or rel.startswith("/") \
                or ".." in Path(rel).parts or "\\" in rel:
            _deny("provenance file path traversal")
    if files != EXPECTED_FILES:
        _deny("provenance files do not match the pinned authority identities")
    if expected.get("native_dylib_sha256") != EXPECTED_NATIVE_DYLIB_SHA256:
        _deny("provenance native dylib identity does not match the pinned authority")
    if not _is_sha256(expected.get("excluded_bridge_dylib_sha256")) \
            or expected.get("excluded_bridge_dylib_sha256") == EXPECTED_NATIVE_DYLIB_SHA256:
        _deny("provenance excluded bridge identity shape")
    version = expected.get("opentui_core_version")
    if version != EXPECTED_OPENTUI_CORE_VERSION:
        _deny("provenance opentui core version does not match the pinned authority")
    return provenance


def _validate_production_request(raw: dict) -> dict:
    unknown = sorted(set(raw) - PRODUCTION_REQUEST_KEYS)
    if unknown:
        _deny(f"unsupported request keys: {', '.join(unknown)}")
    if raw.get("schema") != SCHEMA_REQUEST:
        _deny("unsupported request schema")

    pins = raw.get("pins")
    if not isinstance(pins, dict) or set(pins) != {"upstream_commit", "opentui_commit"} \
            or pins.get("upstream_commit") != EXPECTED_UPSTREAM_COMMIT \
            or pins.get("opentui_commit") != EXPECTED_OPENTUI_COMMIT:
        _deny("pin mismatch")

    production = raw.get("production")
    synthetic = raw.get("synthetic", False)
    if production is not True or not isinstance(synthetic, bool):
        _deny("production requests require production: true")
    if synthetic:
        _deny("mode masquerade: production and synthetic are exclusive")

    scenario = raw.get("scenario")
    if scenario not in SUPPORTED_SCENARIOS:
        _deny(f"unsupported scenario: {scenario!r}")
    contract = SCENARIO_CONTRACTS[scenario]
    if raw.get("search_text") != contract["search_text"]:
        _deny("search_text must match the scenario contract")

    events = raw.get("events", [])
    state = raw.get("state_overrides", {})
    if events != [] or state != {}:
        _deny("unsupported state/event replay for this capture")

    fixture = raw.get("fixture")
    if not isinstance(fixture, dict) or not isinstance(fixture.get("id"), str) \
            or not 1 <= len(fixture["id"]) <= 200:
        _deny("invalid fixture")
    viewport = raw.get("viewport")
    if not isinstance(viewport, dict):
        _deny("invalid viewport")
    _bounded_int(viewport.get("cols"), 1, 500, "viewport cols")
    _bounded_int(viewport.get("rows"), 1, 200, "viewport rows")
    fixture_viewport = fixture.get("viewport")
    if not isinstance(fixture_viewport, dict) \
            or _bounded_int(fixture_viewport.get("columns"), 1, 500, "fixture columns") != viewport["cols"] \
            or _bounded_int(fixture_viewport.get("rows"), 1, 200, "fixture rows") != viewport["rows"]:
        _deny("fixture/viewport mismatch")

    theme = raw.get("theme")
    if not isinstance(theme, dict) or set(theme) - {"name", "mode", "new_designs"} \
            or theme.get("mode") not in ("dark", "light"):
        _deny("invalid theme")

    clock_ms = _bounded_int(raw.get("clock_ms", 0), 0, 10 ** 12, "clock_ms")

    _validate_production_provenance(raw)

    parsed = dict(raw)
    parsed["production"] = True
    parsed["synthetic"] = False
    parsed["events"] = events
    parsed["state_overrides"] = state
    parsed["clock_ms"] = clock_ms
    return parsed


def parse_request(text: str) -> dict:
    if len(text.encode("utf-8", "surrogatepass")) > REQUEST_BUDGET_BYTES:
        _deny("request exceeds budget")
    try:
        raw = json.loads(text)
    except (json.JSONDecodeError, UnicodeDecodeError):
        _deny("bad json")
    if not isinstance(raw, dict):
        _deny("request is not an object")
    if raw.get("production") is True:
        return _validate_production_request(raw)
    return _validate_legacy_request(raw)


def validate_forward(request: dict, extra: list) -> None:
    forwarded = [arg for arg in extra if arg in SYNTHETIC_FLAGS]
    if request.get("production") and forwarded:
        raise RequestDenied("mode masquerade: production request cannot use synthetic flags")
    if any(arg not in SYNTHETIC_FLAGS for arg in extra):
        raise RequestDenied("unsupported forwarded flags")
    # B1: legacy (non-production) requests may forward --component/--synthetic
    # for ANY fixture — the legacy adapter defaults the component text when the
    # fixture carries none (base behaviour, used by par03_capture.py for all
    # nine local fixtures). The production masquerade denial above is the
    # actual protection and is kept.


def child_environment(request: dict, home: Path, tmp: Path) -> dict:
    """Minimal allowlisted child environment (F1).

    Nothing is inherited from ``os.environ``: ambient secrets and OPENCODE_*
    variables (which the upstream app reads: OPENCODE_DRIVE/STORY/ROUTE/
    FAST_BOOT) cannot reach the Bun child.  Only PATH (explicitly controlled),
    the isolated fixture dirs, NO_COLOR, and request-validated TERM/COLORTERM
    are set.  Locale variables are deliberately absent (C default).
    """
    environment = request.get("environment") or {}
    env: dict[str, str] = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "XDG_CONFIG_HOME": str(Path(home) / "cfg"),
        "XDG_DATA_HOME": str(Path(home) / "data"),
        "XDG_CACHE_HOME": str(Path(home) / "cache"),
        "NO_COLOR": "1",
    }
    for name in CHILD_ENV_TERM_KEYS:
        if name in environment:
            value = environment[name]
            if not isinstance(value, str) or not _CHILD_VALUE_RE.match(value):
                _deny(f"invalid environment value: {name}")
            env[name] = value
    if "TERM" not in env:
        env["TERM"] = DEFAULT_TERM
    return env



def resolve_bun(override: str | None) -> str:
    bun = override or shutil.which("bun")
    if not bun:
        raise WrapperError("bun runtime not found on PATH")
    return bun


def run_capture(bun: str, oracle: Path, source_text: str, request: dict,
                extra: list, timeout: float) -> dict:
    """Launch the capture with NO mutable-path handoff (F3).

    The verified candidate TS source travels as an argv element to
    ``bun --eval <source>`` (argv literal data — no shell, no concatenation);
    the request JSON travels on stdin; the cwd stays the oracle's
    ``packages/tui`` so bare ``@opentui/*`` and relative fixture imports
    resolve against the pinned workspace (Bun 1.4.4 sets ``import.meta.dir``
    to the cwd in eval mode, which is exactly the staged layout's semantics).
    Nothing is written into the oracle and nothing is unlinked there.
    """
    timeout = _bounded_float(timeout, 0.0, float(MAX_TIMEOUT_S), "timeout")
    home = Path(tempfile.mkdtemp(prefix="oc2-upstream-child-home-"))
    tmp = Path(tempfile.mkdtemp(prefix="oc2-upstream-child-tmp-"))
    # F1: the child never inherits the caller's environment.
    env = child_environment(request, home=home, tmp=tmp)
    # Runtime-verified bun 1.4.4 semantics: `bun --eval <src> --flag` silently
    # consumes the program flag; `-- <flags>` forwards them to the child's
    # process.argv (the separator itself is excluded there).
    cmd = [bun, "--eval", source_text, "--", *extra]
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(Path(oracle) / STAGE_DIR_REL),
            input=json.dumps(request),
            text=True,
            capture_output=True,
            env=env,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise WrapperError(f"capture timeout after {timeout}s") from error
    except OSError as error:
        raise WrapperError(f"bun runtime not launchable: {error}") from error
    finally:
        shutil.rmtree(home, ignore_errors=True)
        shutil.rmtree(tmp, ignore_errors=True)
    if proc.returncode:
        raise WrapperError(f"capture exited {proc.returncode}: {proc.stderr.strip()[-2000:]}")
    try:
        result = json.loads(proc.stdout)
    except json.JSONDecodeError as error:
        raise WrapperError(f"capture did not emit JSON: {error}") from error
    if not isinstance(result, dict):
        raise WrapperError("capture emitted non-object JSON")
    return result


def _bounded_float(value, low: float, high: float, what: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not low < value <= high:
        _deny(f"invalid {what}")
    return float(value)


def validate_production_result(result: dict) -> None:
    """Post-run validation of measured identity (N4/N5)."""
    if result.get("schema") != "oc2.ui-parity.screen.flow/v1":
        raise WrapperError(f"unexpected production result schema: {result.get('schema')!r}")
    if result.get("evidence_class") != "production_flow":
        raise WrapperError("production result is not labelled production_flow")
    provenance = result.get("provenance") or {}
    if provenance.get("bun_version") != EXPECTED_BUN_RUNTIME:
        raise WrapperError(
            f"measured bun runtime {provenance.get('bun_version')!r} != pinned "
            f"reference_runtime.bun {EXPECTED_BUN_RUNTIME!r}"
        )
    if not provenance.get("verified_files"):
        raise WrapperError("production result carries no verified production files")


def read_stdin_payload(reader) -> str:
    text = reader()
    if len(text.encode("utf-8", "surrogatepass")) > REQUEST_BUDGET_BYTES:
        _deny("request exceeds budget")
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", default=str(ORACLE))
    parser.add_argument("--capture", default=str(CAPTURE))
    parser.add_argument("--bun", default=None, help="bun executable override")
    parser.add_argument("--timeout", type=float, default=MAX_TIMEOUT_S)
    args, extra = parser.parse_known_args()

    try:
        request = parse_request(read_stdin_payload(sys.stdin.read))
        validate_forward(request, extra)
        bun = resolve_bun(args.bun)
        # F3: the exact candidate TS source bytes are read once, hashed, and
        # passed to the child as an argv literal — no staging write, no
        # mutable pathname handoff, no automatic unlink anywhere in the
        # oracle workspace. Foreign and legacy entries are never touched.
        source_text = Path(args.capture).read_text(encoding="utf-8")
        source_sha256 = hashlib.sha256(source_text.encode("utf-8")).hexdigest()
        sys.stderr.write(
            f"capture_sha256={source_sha256} handoff=argv-eval staging=none\n"
        )
        result = run_capture(bun, Path(args.oracle), source_text, request, extra, args.timeout)
        if request.get("production"):
            validate_production_result(result)
    except (RequestDenied, StageDenied) as error:
        sys.stderr.write(f"denied: {error}\n")
        return 2
    except WrapperError as error:
        sys.stderr.write(f"{error}\n")
        return 3
    sys.stderr.write(f"capture_sha256={source_sha256} launch=argv-eval\n")
    sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
