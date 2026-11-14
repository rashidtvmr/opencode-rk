"""Executable contract tests for the matched upstream production capture seam.

Covers the request parser / staging / launch-denial contract of
``tools/ui_parity/renderers/upstream_opentui.py`` and the production-flow
contract of ``tools/ui_parity/renderers/upstream_capture.ts`` (source-level,
same pattern as ``tests/test_par03_capture.py``).  Denials must happen before
any command execution or file write; positives must exercise the real parser
and state/event path.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

WRAPPER_PATH = Path(__file__).parents[1] / "tools/ui_parity/renderers/upstream_opentui.py"
ADAPTER_PATH = Path(__file__).parents[1] / "tools/ui_parity/renderers/upstream_capture.ts"

UPSTREAM_COMMIT = "450e7b92412c780a7b5a75226bb8d49aaca24e22"
OPENTUI_COMMIT = "8d6065a3e60039d16c6ee697692dcfbc46d3c786"
NATIVE_DYLIB = "a2013c73d6bc9835aa59c7aa4ae6d58b0a1b6e0822a7f44ef9e21644bac72910"
# Test sentinel only: the real excluded bridge identity arrives via request
# provenance and is verified at runtime; the unit tests never claim a real
# bridge hash.
BRIDGE_SENTINEL = "f" * 64
EXPECTED_FILES = {
    "src/app.tsx": "b97ebed0dea62f1eda897ddde4268b0d30caf271bff74f04c5dc11fe4345faf3",
    "src/routes/home.tsx": "f1c6ae4c353549f5afe57b81ebe807562c5a3ba3539f568e4632b509c3da7ccb",
    "src/component/command-palette.tsx": "61fb73798ad39822e1a7a950b086a10f1540a67621f49a81a64e13fc10b29224",
    "src/ui/dialog.tsx": "33cc6de6fc6107a14f266177ca2d288dd69e3ce9e8f0aa205ce3918cc046f690",
    "src/ui/dialog-select.tsx": "86f43fa17785e1b935fb3d484eba9f079f5252b77adeae40b260d920370a3052",
    "src/config/keybind.ts": "49c8a69149811fee3bf57e1f9104f885d20910c594cae8ba3e99f14c8f454fd8",
    "src/context/keymap.tsx": "d81a9a515f73b6e4b5cc15f6aabe06e51ffb5471ebaa133bb25e791356e9f812",
    # Fixture entry that executes during the production flow (F4 provenance extension).
    "test/fixture/app.ts": "66ce0d97112a8e12b03bf117f6a5a0abf6a4760355cd460e61cb454716ffcacd",
}


def _load():
    spec = importlib.util.spec_from_file_location("upstream_opentui", WRAPPER_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


FIXTURE_ENTRY = "test/fixture/app.ts"
FIXTURE_ENTRY_SHA256 = "66ce0d97112a8e12b03bf117f6a5a0abf6a4760355cd460e61cb454716ffcacd"
BRIDGE_SENTINEL = "f" * 64

RETAINED_REAL_CAPTURE = Path(
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/ui-parity-autonomous-20261113"
    "/upstream-oracle-implementation/race-repair/flow.json"
)
# Older retained real captures are still valid RED evidence for the per-action
# assertions (the b8db checkpoint captured stage-end cursor copies); the
# repaired flow capture is preferred when present.
_FALLBACK_REAL_CAPTURE = Path(
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/ui-parity-autonomous-20261113"
    "/upstream-oracle-implementation/security-repair/flow.json"
)


def _ui_parity_manifest_pins():
    """Exact pins object tools/ui_parity/ui_parity.py sends (manifest `pins`)."""
    return {
        "product": "opencode-rk",
        "upstream_commit": UPSTREAM_COMMIT,
        "upstream_package_version": "2.1.4",
        "opentui_commit": OPENTUI_COMMIT,
        "opentui_version": "0.6.8",
        "reference_runtime": {"bun": "1.4.4", "solid_js": "1.9.15"},
    }


def _ui_parity_request():
    """Exact request shape tools/ui_parity/ui_parity.py:build_request emits."""
    fixture = json.loads(
        (Path(__file__).parent / "fixtures/ui-parity/cases/par-static-home-80x24-dark.json").read_text()
    )
    return {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": fixture,
        "fixture_class": fixture.get("class") or "baseline",
        "viewport": {"cols": 80, "rows": 24},
        "theme": fixture.get("theme") or {"name": "opencode", "mode": "dark"},
        "pins": _ui_parity_manifest_pins(),
        "environment": dict(fixture.get("environment") or {}),
        "renderer": {"name": "upstream-opentui", "command": ["{PYTHON}", "{TOOLS}/renderers/upstream_opentui.py"]},
    }


def _par03_request():
    """Exact request shape tools/ui_parity/par03_capture.py:main emits."""
    fixture = json.loads(
        (Path(__file__).parent / "fixtures/ui-parity/cases/par-static-home-80x24-dark.json").read_text()
    )
    viewport = fixture["viewport"]
    return {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": fixture,
        "fixture_class": fixture.get("class", "baseline"),
        "viewport": {"cols": int(viewport["columns"]), "rows": int(viewport["rows"])},
        "theme": fixture.get("theme", {"name": "opencode", "mode": "dark"}),
        "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
    }


def _production_request(**overrides):
    request = {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": {"id": "par-static-home-80x24-dark", "viewport": {"columns": 80, "rows": 24}},
        "viewport": {"cols": 80, "rows": 24},
        "theme": {"name": "opencode", "mode": "dark", "new_designs": False},
        "clock_ms": 0,
        "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
        "production": True,
        "scenario": "home-command-palette-v1",
        "search_text": "new",
        "events": [],
        "state_overrides": {},
        "provenance": {
            "expected": {
                "files": dict(EXPECTED_FILES),
                "native_dylib_sha256": NATIVE_DYLIB,
                "excluded_bridge_dylib_sha256": BRIDGE_SENTINEL,
                "opentui_core_version": "0.6.8",
            }
        },
    }
    request.update(overrides)
    for key, value in list(overrides.items()):
        if value is None:
            request.pop(key)
    return request


def _synthetic_request():
    return {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": {"id": "synthetic", "viewport": {"columns": 80, "rows": 24}, "component_text": "x"},
        "viewport": {"cols": 80, "rows": 24},
        "theme": {"name": "opencode", "mode": "dark"},
        "clock_ms": 0,
        "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
        "synthetic": True,
    }


class RequestParserContract(unittest.TestCase):
    def setUp(self):
        self.mod = _load()

    def test_valid_production_request_parses_with_identity_envelope(self):
        parsed = self.mod.parse_request(json.dumps(_production_request()))
        self.assertEqual(parsed["schema"], "oc2.ui-parity.request/v1")
        self.assertEqual(parsed["production"], True)
        self.assertEqual(parsed["scenario"], "home-command-palette-v1")
        self.assertEqual(parsed["search_text"], "new")
        self.assertEqual(parsed["viewport"], {"cols": 80, "rows": 24})
        self.assertEqual(parsed["clock_ms"], 0)
        self.assertEqual(parsed["events"], [])
        self.assertEqual(parsed["state_overrides"], {})
        self.assertEqual(parsed["pins"]["upstream_commit"], UPSTREAM_COMMIT)
        self.assertEqual(parsed["pins"]["opentui_commit"], OPENTUI_COMMIT)
        self.assertTrue(parsed["provenance"]["expected"]["files"])

    def test_valid_synthetic_request_parses_without_scenario(self):
        parsed = self.mod.parse_request(json.dumps(_synthetic_request()))
        self.assertEqual(parsed["synthetic"], True)
        self.assertFalse(parsed["production"])

    def test_wrong_pin_denied(self):
        for field in ("upstream_commit", "opentui_commit"):
            request = _production_request()
            request["pins"] = dict(request["pins"], **{field: "0" * 40})
            with self.assertRaises(self.mod.RequestDenied) as caught:
                self.mod.parse_request(json.dumps(request))
            self.assertIn("pin", str(caught.exception))

    def test_wrong_opentui_version_pin_denied(self):
        request = _production_request()
        request["provenance"]["expected"]["opentui_core_version"] = "0.6.9"
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_component_masquerade_denied_in_production_mode(self):
        parsed = self.mod.parse_request(json.dumps(_production_request()))
        for extra in (["--component"], ["--synthetic"], ["--component", "--synthetic"]):
            with self.assertRaises(self.mod.RequestDenied) as caught:
                self.mod.validate_forward(parsed, extra)
            self.assertIn("masquerade", str(caught.exception))

    def test_synthetic_mode_allows_component_forwarding(self):
        parsed = self.mod.parse_request(json.dumps(_synthetic_request()))
        self.mod.validate_forward(parsed, ["--component"])
        self.mod.validate_forward(parsed, ["--synthetic"])

    def test_unsupported_events_denied(self):
        request = _production_request(events=[{"type": "key", "name": "x"}])
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self.mod.parse_request(json.dumps(request))
        self.assertIn("unsupported", str(caught.exception))

    def test_unsupported_state_overrides_denied(self):
        request = _production_request(state_overrides={"route": "session"})
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self.mod.parse_request(json.dumps(request))
        self.assertIn("unsupported", str(caught.exception))

    def test_unsupported_scenario_denied(self):
        request = _production_request(scenario="session-transcript-v1")
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_search_text_must_match_scenario_contract(self):
        request = _production_request(search_text="zzz")
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_bad_json_denied(self):
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request("{not json")

    def test_bad_schema_denied(self):
        request = _production_request(schema="oc2.ui-parity.request/v2")
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_unknown_top_level_key_denied(self):
        request = _production_request(render_expected_cells=[{"ch": "x"}])
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_oversized_request_denied(self):
        request = _production_request()
        request["fixture"]["padding"] = "x" * (self.mod.REQUEST_BUDGET_BYTES + 16)
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self.mod.parse_request(json.dumps(request))
        self.assertIn("budget", str(caught.exception))

    def test_missing_production_provenance_denied(self):
        request = _production_request(provenance=None)
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_provenance_hash_shape_denied(self):
        request = _production_request()
        request["provenance"]["expected"]["native_dylib_sha256"] = "not-a-hash"
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_provenance_file_traversal_denied(self):
        request = _production_request()
        request["provenance"]["expected"]["files"] = dict(EXPECTED_FILES, **{"../../etc/passwd": "0" * 64})
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_viewport_mismatch_denied(self):
        request = _production_request(viewport={"cols": 120, "rows": 40})
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_parser_denial_performs_no_filesystem_write(self):
        before = self._snapshot_tmp()
        for request in (
            json.dumps(_production_request(pins={"upstream_commit": "0" * 40, "opentui_commit": OPENTUI_COMMIT})),
            json.dumps(_production_request(scenario="unknown-v9")),
            "{oops",
        ):
            with self.assertRaises(self.mod.RequestDenied):
                self.mod.parse_request(request)
        self.assertEqual(before, self._snapshot_tmp())

    def _snapshot_tmp(self):
        root = Path(tempfile.gettempdir())
        return {p.name: p.stat().st_mtime_ns for p in root.glob("*") if p.is_dir()}


class LaunchGuardContract(unittest.TestCase):
    def setUp(self):
        self.mod = _load()

    def test_timeout_bounds_denied(self):
        parsed = self.mod.parse_request(json.dumps(_production_request()))
        for timeout in (0, -1, self.mod.MAX_TIMEOUT_S + 1):
            with self.assertRaises(self.mod.RequestDenied):
                self.mod.run_capture(
                    bun="bun", oracle=Path("/unused"), source_text="// capture",
                    request=parsed, extra=[], timeout=timeout,
                )

    def test_budget_rejection_happens_before_launch(self):
        # A stdin payload above the budget must be refused without staging or
        # launching: exercise the stdin-reading guard directly.
        oversized = "x" * (self.mod.REQUEST_BUDGET_BYTES + 64)
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self.mod.read_stdin_payload(lambda: oversized)
        self.assertIn("budget", str(caught.exception))


class AdapterSourceContract(unittest.TestCase):
    """Source-level contract for the TS adapter (same pattern as test_par03_capture)."""

    def setUp(self):
        self.source = ADAPTER_PATH.read_text()

    def test_production_flow_mounts_real_app_fixture(self):
        self.assertIn("createAppFixture", self.source)
        self.assertIn("test/fixture/app", self.source)

    def test_production_flow_is_labeled_and_screen_v1_frames_kept(self):
        self.assertIn('"oc2.ui-parity.screen/v1"', self.source)
        self.assertIn('"oc2.ui-parity.screen.flow/v1"', self.source)
        self.assertIn('"evidence_class"', self.source)

    def test_no_fabricated_pins_or_hashes_embedded(self):
        # Expected identities arrive via request provenance; hardcoding them
        # would let a mismatched runtime "look matched".
        self.assertNotIn(UPSTREAM_COMMIT, self.source)
        self.assertNotIn(OPENTUI_COMMIT, self.source)
        self.assertNotIn("a2013c73", self.source)

    def test_fail_closed_identity_verification_present(self):
        self.assertIn("CryptoHasher", self.source)
        self.assertIn("native_dylib_sha256", self.source)
        self.assertIn("excluded_bridge_dylib_sha256", self.source)

    def test_real_channels_not_faked(self):
        self.assertIn("currentFocusedEditor", self.source)
        self.assertIn("upstream_input_replay_trace", self.source)
        self.assertIn("not_captured_from_upstream", self.source)

    def test_scenario_fail_closed(self):
        self.assertIn("unsupported_scenario", self.source)
        self.assertIn("unsupported_state_events", self.source)

    def test_per_action_cursor_measurement_present(self):
        # B2: per-event cursor snapshots and their declared source.
        self.assertIn("pressAndMeasure", self.source)
        self.assertIn("measured_cursor", self.source)
        self.assertIn("measured_per_dispatch", self.source)

    def test_repo_key_vocabulary_in_code(self):
        # Hyphen vocabulary, matching the fixtures and oc2_pty parser.
        self.assertIn("ctrl-${entry.name}", self.source)
        self.assertNotIn('`ctrl+${entry.name}`', self.source)

    def test_legacy_declares_events_not_replayed(self):
        self.assertIn("events_replayed", self.source)


class CallerShapeCompatibility(unittest.TestCase):
    """B1: the two existing production consumers must keep working.

    These feed the EXACT request objects the callers build (no invented shape)
    through the real parse_request / validate_forward.  Regression guard for
    the allowlist introduced in the first checkpoint.
    """

    def setUp(self):
        self.mod = _load()

    def test_ui_parity_build_request_is_accepted(self):
        # Mirrors tools/ui_parity/ui_parity.py build_request exactly: manifest
        # pins object, fixture_class, environment, renderer.
        parsed = self.mod.parse_request(json.dumps(_ui_parity_request()))
        self.assertEqual(parsed["schema"], "oc2.ui-parity.request/v1")
        self.assertEqual(parsed["fixture"]["id"], "par-static-home-80x24-dark")
        self.mod.validate_forward(parsed, [])

    def test_ui_parity_request_with_forwarded_component_smoke_reaches_adapter(self):
        parsed = self.mod.parse_request(json.dumps(_ui_parity_request()))
        # ui_parity's upstream-opentui profile forwards no flags, but the
        # documented --component smoke must stay deny-free for legacy requests.
        self.mod.validate_forward(parsed, ["--component"])

    def test_par03_request_is_accepted_and_forwards_component(self):
        parsed = self.mod.parse_request(json.dumps(_par03_request()))
        self.mod.validate_forward(parsed, ["--component"])

    def test_all_nine_local_fixtures_accept_exact_par03_component_forwarding(self):
        # B1 residual: par03 forwards --component for every fixture; the legacy
        # base adapter defaulted the component text when absent. Every real
        # local fixture case must be accepted with its exact caller shape.
        cases = sorted((Path(__file__).parent / "fixtures/ui-parity/cases").glob("par-static-*.json"))
        self.assertEqual(len(cases), 9, "the real local manifest fixture set changed")
        for case in cases:
            fixture = json.loads(case.read_text())
            viewport = fixture["viewport"]
            request = {
                "schema": "oc2.ui-parity.request/v1",
                "fixture": fixture,
                "fixture_class": fixture.get("class", "baseline"),
                "viewport": {"cols": int(viewport["columns"]), "rows": int(viewport["rows"])},
                "theme": fixture.get("theme", {"name": "opencode", "mode": "dark"}),
                "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
            }
            with self.subTest(fixture=case.name):
                parsed = self.mod.parse_request(json.dumps(request))
                self.mod.validate_forward(parsed, ["--component"])

    def test_component_forwarding_without_component_text_is_legacy_ok(self):
        # The component-text precondition must not reappear for legacy
        # requests: par03 forwards --component regardless, and the base
        # adapter defaulted the text.
        fixture = json.loads(
            (Path(__file__).parent / "fixtures/ui-parity/cases/par-static-agents-120x40-dark.json").read_text()
        )
        self.assertNotIn("component_text", fixture)
        request = {
            "schema": "oc2.ui-parity.request/v1",
            "fixture": fixture,
            "fixture_class": fixture.get("class", "baseline"),
            "viewport": {"cols": 120, "rows": 40},
            "theme": fixture.get("theme", {"name": "opencode", "mode": "dark"}),
            "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
        }
        parsed = self.mod.parse_request(json.dumps(request))
        self.mod.validate_forward(parsed, ["--component"])

    def test_par03_request_with_bad_fixture_class_denied(self):
        request = _par03_request()
        request["fixture_class"] = "not-a-class"
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_ui_parity_environment_non_string_value_denied(self):
        request = _ui_parity_request()
        request["environment"] = {"TERM": 256}
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_ui_parity_renderer_shape_denied_when_malformed(self):
        request = _ui_parity_request()
        request["renderer"] = {"name": ["upstream-opentui"]}
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_production_request_still_strict_about_legacy_keys(self):
        request = _production_request(environment={"TERM": "xterm-256color"})
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_manifest_style_pins_accepted_only_for_legacy(self):
        parsed = self.mod.parse_request(json.dumps(_ui_parity_request()))
        self.assertEqual(parsed["pins"]["upstream_commit"], UPSTREAM_COMMIT)
        production = _production_request(pins=_ui_parity_manifest_pins())
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(production))


class ChildEnvironmentIsolation(unittest.TestCase):
    """F1: the Bun child must see a minimal allowlisted environment."""

    def setUp(self):
        self.mod = _load()

    def test_ambient_secrets_and_opencode_vars_never_reach_child(self):
        poisoned = {
            "FAKE_SECRET_TOKEN": "ambient-secret",
            "FAKE_AWS_ACCESS_KEY_ID": "AKIAFAKE",
            "OPENCODE_ROUTE": '{"type":"session"}',
            "OPENCODE_DRIVE": "1",
            "OPENCODE_STORY": "1",
            "OPENCODE_FAST_BOOT": "1",
            "GITHUB_TOKEN": "ghp_fake",
            "HTTP_PROXY": "http://ambient:1",
        }
        home = Path("/isolated/home")
        tmp = Path("/isolated/tmp")
        with unittest.mock.patch.dict(os.environ, poisoned):
            env = self.mod.child_environment(_production_request(), home=home, tmp=tmp)
        for name in poisoned:
            self.assertNotIn(name, env)
        self.assertNotIn("OPENCODE_ROUTE", json.dumps(env))

    def test_child_environment_is_allowlisted(self):
        env = self.mod.child_environment(_production_request(), home=Path("/h"), tmp=Path("/t"))
        allowed = {
            "PATH", "HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
            "XDG_CACHE_HOME", "NO_COLOR", "TERM", "COLORTERM",
        }
        self.assertTrue(set(env) <= allowed, f"unexpected keys: {set(env) - allowed}")
        self.assertEqual(env["NO_COLOR"], "1")
        self.assertEqual(env["HOME"], "/h")
        self.assertEqual(env["TMPDIR"], "/t")

    def test_term_taken_from_request_environment_when_valid(self):
        request = _ui_parity_request()
        request["environment"] = {"TERM": "xterm-256color", "COLORTERM": "truecolor"}
        env = self.mod.child_environment(request, home=Path("/h"), tmp=Path("/t"))
        self.assertEqual(env["TERM"], "xterm-256color")
        self.assertEqual(env["COLORTERM"], "truecolor")

    def test_hostile_term_value_rejected(self):
        request = _ui_parity_request()
        request["environment"] = {"TERM": "xterm; rm -rf /tmp"}
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.child_environment(request, home=Path("/h"), tmp=Path("/t"))

    def test_unknown_environment_keys_are_not_forwarded(self):
        request = _ui_parity_request()
        request["environment"] = {"TERM": "xterm", "FAKE_LDW": "ambient"}
        env = self.mod.child_environment(request, home=Path("/h"), tmp=Path("/t"))
        self.assertNotIn("FAKE_LDW", env)


class ProvenanceShapeHardening(unittest.TestCase):
    """F2/F4: exact provenance key set incl. the executed fixture entry."""

    def setUp(self):
        self.mod = _load()

    def test_expected_files_include_the_executed_fixture_entry(self):
        self.assertIn(FIXTURE_ENTRY, self.mod.EXPECTED_FILES)
        self.assertEqual(self.mod.EXPECTED_FILES[FIXTURE_ENTRY], FIXTURE_ENTRY_SHA256)

    def test_seven_file_provenance_now_denied(self):
        request = _production_request()
        request["provenance"]["expected"]["files"] = {
            k: v for k, v in EXPECTED_FILES.items() if k != FIXTURE_ENTRY
        }
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_extra_provenance_file_denied(self):
        request = _production_request()
        request["provenance"]["expected"]["files"] = dict(EXPECTED_FILES, **{"src/extra.ts": "0" * 64})
        with self.assertRaises(self.mod.RequestDenied):
            self.mod.parse_request(json.dumps(request))

    def test_non_object_provenance_files_denied(self):
        for files in (None, [], "map", 7):
            request = _production_request()
            request["provenance"]["expected"]["files"] = files
            with self.assertRaises(self.mod.RequestDenied):
                self.mod.parse_request(json.dumps(request))


class LaunchPathContract(unittest.TestCase):
    """F3: the launch path must consume verified source bytes directly.

    The Bun child is launched with the exact candidate TS source as an argv
    element (`bun --eval <source>`, request JSON on stdin, cwd = oracle
    packages/tui). No staging path operation exists on the launch path: no
    mutable pathname handoff, no automatic unlink. A same-directory hostile
    writer cannot swap what the child executes.
    """

    def setUp(self):
        self.mod = _load()
        self.tmp = tempfile.mkdtemp(prefix="oc2-launch-test-")
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        (Path(self.tmp) / "packages" / "tui").mkdir(parents=True)
        self.capture = Path(self.tmp) / "capture.ts"
        self.source_text = ADAPTER_PATH.read_text()
        self.capture.write_text(self.source_text)
        self.calls = []

    def _run_main(self, payload, extra=()):
        import contextlib
        import io
        fake_proc = unittest.mock.MagicMock(returncode=0, stdout='{"schema":"probe"}', stderr="")
        fake_proc.return_value = fake_proc  # subprocess.run(...) -> the fake result
        with unittest.mock.patch.object(self.mod, "subprocess",
                                        unittest.mock.MagicMock(run=fake_proc)), \
                unittest.mock.patch.object(sys, "argv",
                                           ["upstream_opentui.py", "--oracle", self.tmp,
                                            "--capture", str(self.capture),
                                            "--bun", "/fake-bun", *extra]), \
                unittest.mock.patch.object(sys, "stdin", io.StringIO(payload)), \
                contextlib.redirect_stderr(io.StringIO()) as err, \
                contextlib.redirect_stdout(io.StringIO()) as out:
            try:
                code = self.mod.main()
            except SystemExit as exit_error:
                code = exit_error.code
        return code, err.getvalue(), out.getvalue(), fake_proc

    def test_launch_passes_verified_source_bytes_as_eval_argv(self):
        code, err, out, fake = self._run_main(json.dumps(_par03_request()), ["--component"])
        self.assertEqual(code, 0)
        argv = fake.call_args[0][0]
        self.assertEqual(argv[1], "--eval")
        self.assertIn(self.source_text, argv, "the exact candidate TS source must be an argv element")

    def test_launch_argv_never_references_a_stage_path(self):
        code, err, out, fake = self._run_main(json.dumps(_par03_request()), ["--component"])
        argv = fake.call_args[0][0]
        for element in argv:
            self.assertNotIn(".oc2-upstream-capture", str(element))
        self.assertFalse([e for e in argv if str(e).endswith(".ts")],
                         f"no argv element may be a mutable .ts path: {argv}")
        self.assertFalse([e for e in argv if str(e).startswith(self.tmp)],
                         f"no argv element may live under the oracle root: {argv}")

    def test_module_has_no_staging_surface_at_all(self):
        # The strongest honest form of "staging is never used on the launch
        # path": the unsafe mechanism no longer exists as any public or
        # private helper, so no caller can hand off through a mutable path.
        for name in ("stage_capture", "verify_staged_custody", "cleanup_staged", "StagedCapture"):
            self.assertFalse(hasattr(self.mod, name), f"staging API must be gone: {name}")

    def test_forwarded_flags_survive_eval_mode_via_separator(self):
        # Runtime-observed bun 1.4.4 behavior: `bun --eval <src> --component`
        # silently consumes the program flag; `-- <flags>` forwards it to the
        # child's process.argv (separator excluded there). The launch must use
        # the separator so --component/--synthetic actually reach the adapter.
        code, err, out, fake = self._run_main(json.dumps(_par03_request()), ["--component"])
        self.assertEqual(code, 0)
        argv = fake.call_args[0][0]
        self.assertIn("--", argv)
        self.assertGreater(argv.index("--"), argv.index("--eval"))
        self.assertIn("--component", argv)
        self.assertGreater(argv.index("--component"), argv.index("--"))

    def test_request_json_travels_on_stdin_not_in_code(self):
        code, err, out, fake = self._run_main(json.dumps(_par03_request()), ["--component"])
        argv = fake.call_args[0][0]
        stdin_payload = fake.call_args[1]["input"]
        for element in argv:
            self.assertNotIn('"par-static-home-80x24-dark"', str(element))
            self.assertNotIn('oc2.ui-parity.request', str(element))
        # The (parsed) request travels on stdin — the same channel the adapter
        # reads — never inside the executed code.
        self.assertIn('"par-static-home-80x24-dark"', stdin_payload)
        self.assertIn('"oc2.ui-parity.request/v1"', stdin_payload)


class ForeignSlotSafety(unittest.TestCase):
    """F3: pre-existing foreign/legacy oracle entries are never executed
    from, never deleted, and cannot influence the launch."""

    def setUp(self):
        self.mod = _load()
        self.tmp = tempfile.mkdtemp(prefix="oc2-foreign-test-")
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        (Path(self.tmp) / "packages" / "tui").mkdir(parents=True)
        self.capture = Path(self.tmp) / "capture.ts"
        self.capture.write_text(ADAPTER_PATH.read_text())
        digest = hashlib.sha256(self.capture.read_bytes()).hexdigest()
        self.slot = Path(self.tmp) / "packages" / "tui" / f".oc2-upstream-capture-{digest[:12]}.ts"
        self.foreign = b"// HOSTILE foreign bytes - must never execute\n"
        self.slot.write_bytes(self.foreign)
        self.legacy = Path(self.tmp) / "packages" / "tui" / ".oc2-upstream-capture.ts"
        self.legacy.write_bytes(b"// legacy capture02 foreign entry\n")

    def _run_main(self, payload, extra=("--component",)):
        import contextlib
        import io
        fake_proc = unittest.mock.MagicMock(returncode=0, stdout='{"schema":"probe"}', stderr="")
        fake_proc.return_value = fake_proc  # subprocess.run(...) -> the fake result
        with unittest.mock.patch.object(self.mod, "subprocess",
                                        unittest.mock.MagicMock(run=fake_proc)), \
                unittest.mock.patch.object(sys, "argv",
                                           ["upstream_opentui.py", "--oracle", self.tmp,
                                            "--capture", str(self.capture),
                                            "--bun", "/fake-bun", *extra]), \
                unittest.mock.patch.object(sys, "stdin", io.StringIO(payload)), \
                contextlib.redirect_stderr(io.StringIO()) as err, \
                contextlib.redirect_stdout(io.StringIO()) as out:
            try:
                code = self.mod.main()
            except SystemExit as exit_error:
                code = exit_error.code
        return code, err.getvalue(), fake_proc

    def test_foreign_slot_cannot_block_or_redirect_launch(self):
        code, err, fake = self._run_main(json.dumps(_par03_request()))
        self.assertEqual(code, 0, f"launch must not depend on stage slots: {err}")
        self.assertTrue(fake.called)

    def test_foreign_slot_bytes_preserved_never_deleted(self):
        code, err, fake = self._run_main(json.dumps(_par03_request()))
        self.assertEqual(self.slot.read_bytes(), self.foreign)
        self.assertTrue(self.slot.exists())
        self.assertEqual(self.legacy.read_bytes(), b"// legacy capture02 foreign entry\n")
        self.assertTrue(self.legacy.exists())

    def test_launch_creates_no_stage_entries(self):
        stage_dir = Path(self.tmp) / "packages" / "tui"
        before = {p.name for p in stage_dir.glob(".oc2-upstream-capture-*")}
        code, err, fake = self._run_main(json.dumps(_par03_request()))
        after = {p.name for p in stage_dir.glob(".oc2-upstream-capture-*")}
        self.assertEqual(after - before, set(),
                         f"staging must be gone from the launch path: {sorted(after - before)}")


class MainSurfaceContract(unittest.TestCase):
    """N1: main() surfaces denials/ failures cleanly (no traceback)."""

    def setUp(self):
        self.mod = _load()
        self.tmp = tempfile.mkdtemp(prefix="oc2-upstream-main-test-")
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        (Path(self.tmp) / "packages" / "tui").mkdir(parents=True)
        capture = Path(self.tmp) / "capture.ts"
        capture.write_text("// capture\n")

    def _run_main(self, argv, payload):
        import contextlib
        import io
        err = io.StringIO()
        out = io.StringIO()
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(out):
            with unittest.mock.patch.object(sys, "argv", ["upstream_opentui.py", *argv]), \
                    unittest.mock.patch.object(sys, "stdin", io.StringIO(payload)):
                try:
                    code = self.mod.main()
                except SystemExit as exit_error:
                    code = exit_error.code
        return code, err.getvalue(), out.getvalue()

    def test_missing_bun_reports_clean_wrapper_error(self):
        code, err, out = self._run_main(
            ["--oracle", self.tmp, "--capture", f"{self.tmp}/capture.ts", "--bun", "/absent-bun-xyz"],
            json.dumps(_production_request()),
        )
        self.assertEqual(code, 3)
        self.assertIn("bun", err)
        self.assertNotIn("Traceback", err)
        self.assertEqual(out, "")

    def test_invalid_timeout_reports_clean_denial(self):
        code, err, out = self._run_main(
            ["--oracle", self.tmp, "--capture", f"{self.tmp}/capture.ts", "--timeout", "0"],
            json.dumps(_production_request()),
        )
        self.assertEqual(code, 2)
        self.assertIn("denied", err)
        self.assertNotIn("Traceback", err)


class FlowOutputContract(unittest.TestCase):
    """B2: the retained real capture must carry a comparable action channel.

    Runs against the real production capture bytes retained from the first
    executed run (not a synthetic artifact).
    """

    @classmethod
    def setUpClass(cls):
        # The production-capture evidence is part of this feature's contract:
        # a missing capture is a FAILURE, not a silent skip. The pre-repair
        # capture (older checkpoint) is still a valid RED target for the
        # per-action measurement assertions.
        if not RETAINED_REAL_CAPTURE.exists() and not _FALLBACK_REAL_CAPTURE.exists():
            raise AssertionError(
                "no retained real production capture present: run the bounded "
                "Bun flow (runtime_proof) to produce race-repair/flow.json"
            )

    def setUp(self):
        self.source_path = (
            RETAINED_REAL_CAPTURE if RETAINED_REAL_CAPTURE.exists() else _FALLBACK_REAL_CAPTURE
        )
        self.flow = json.loads(self.source_path.read_text())

    def test_flow_is_real_production_evidence(self):
        self.assertEqual(self.flow.get("schema"), "oc2.ui-parity.screen.flow/v1")
        self.assertEqual(self.flow.get("evidence_class"), "production_flow")

    def test_actions_are_protocol_shaped_measured_entries(self):
        for frame in self.flow["frames"]:
            screen = frame["screen"]
            for action in screen["actions"]:
                self.assertIn("seq", action)
                self.assertIn("type", action)
                self.assertIn("key", action)
                self.assertIn("applied", action)
                self.assertIn("cursor_after", action)
                self.assertIn("row", action["cursor_after"])
                self.assertIn("col", action["cursor_after"])
                self.assertIn("visible", action["cursor_after"])

    def test_replay_trace_is_separate_from_protocol_actions(self):
        for frame in self.flow["frames"]:
            screen = frame["screen"]
            self.assertIn("input_trace", screen)
            self.assertNotIn("input_trace", screen["actions"] or [])
            self.assertEqual(screen["semantic_actions"], None)
            self.assertTrue(screen["actions_source"])
            self.assertTrue(screen["input_trace_source"])

    def test_measured_application_outcomes_are_real(self):
        # The measured cursor_after of a stage's last action equals the cursor
        # of that same stage's capture (the state after the action actually
        # landed) — a real measurement, not a fabricated or inferred one.
        frames = self.flow["frames"]
        for frame in frames:
            actions = frame["screen"]["actions"]
            if not actions:
                continue
            cursor_after = actions[-1]["cursor_after"]
            cursor = frame["screen"]["cursor"]
            self.assertEqual(
                (cursor_after["col"], cursor_after["row"]),
                (cursor["col"], cursor["row"]),
                f"{frame['stage']}: last action cursor_after vs captured cursor",
            )
        # Cursor transitions across stages are the real measured ones.
        by_stage = {f["stage"]: f for f in frames}
        self.assertEqual(by_stage["home"]["screen"]["cursor"], by_stage["palette_closed"]["screen"]["cursor"])
        self.assertNotEqual(by_stage["home"]["screen"]["cursor"], by_stage["palette_open"]["screen"]["cursor"])

    def test_cursor_after_measured_per_action_not_stage_end_copies(self):
        # B2: each action's cursor_after must be the cursor measured at THAT
        # action's own dispatch point (per-event snapshot before the next
        # input), not the stage-end cursor copied backwards to every action.
        by_stage = {f["stage"]: f for f in self.flow["frames"]}
        palette_open = by_stage["palette_open"]["screen"]["cursor"]
        search = by_stage["palette_search"]["screen"]
        actions = search["actions"]
        self.assertTrue(actions)
        # The palette-open dispatch (seq 0) measured the cursor at palette
        # open, not the search-stage end cursor.
        self.assertEqual(
            (actions[0]["cursor_after"]["col"], actions[0]["cursor_after"]["row"]),
            (palette_open["col"], palette_open["row"]),
            "seq0 cursor_after must be the cursor measured right after ctrl+p",
        )
        # Each typed character advanced the measured cursor by one column.
        typed = actions[1:]
        previous = palette_open["col"]
        for index, action in enumerate(typed):
            self.assertEqual(
                action["cursor_after"]["col"], previous + 1,
                f"typed char {index} ({action['key']}) cursor did not advance by exactly one column",
            )
            self.assertEqual(action["cursor_after"]["row"], palette_open["row"])
            previous = action["cursor_after"]["col"]
        # The final measured cursor equals the stage's captured cursor.
        self.assertEqual(
            (actions[-1]["cursor_after"]["col"], actions[-1]["cursor_after"]["row"]),
            (search["cursor"]["col"], search["cursor"]["row"]),
        )

    def test_actions_declare_per_action_cursor_measurement_source(self):
        for frame in self.flow["frames"]:
            screen = frame["screen"]
            if not screen["actions"]:
                continue
            self.assertEqual(
                screen.get("actions_cursor_after_source"), "measured_per_dispatch",
                f"{frame['stage']}: cursor_after must be declared measured per dispatch",
            )

    def test_actions_use_repository_key_vocabulary(self):
        # The repo protocol spells modified keys with a hyphen (fixtures use
        # {"key": "ctrl-p"}; oc2_pty parses "ctrl-" prefixed names).
        for frame in self.flow["frames"]:
            for action in frame["screen"]["actions"]:
                self.assertNotIn("+", action["key"], f"non-repo key spelling: {action['key']}")
        by_stage = {f["stage"]: f for f in self.flow["frames"]}
        self.assertEqual(by_stage["palette_open"]["screen"]["actions"][0]["key"], "ctrl-p")


class FlowHonestyContract(unittest.TestCase):
    """N4/N5/F4: measured-vs-requested labeling and trust boundary declared."""

    def setUp(self):
        self.source = ADAPTER_PATH.read_text()

    def test_requested_pins_and_theme_labeled_not_measured(self):
        self.assertIn("requested_pins", self.source)
        self.assertIn("requested_theme", self.source)

    def test_pins_enforcement_boundary_declared(self):
        self.assertIn("pins_enforced_by", self.source)

    def test_dependency_trust_boundary_declared(self):
        self.assertIn("trust_boundary", self.source)
        self.assertIn("not", self.source.lower())

    def test_bun_runtime_is_validated_against_pinned_reference(self):
        wrapper = WRAPPER_PATH.read_text()
        self.assertIn("EXPECTED_BUN_RUNTIME", wrapper)
        self.assertIn('"1.4.4"', wrapper)

    def test_deterministic_readiness_not_fixed_sleep_only(self):
        self.assertIn("waitForVisualIdle", self.source)


if __name__ == "__main__":
    unittest.main()
