"""Explicit delivery session model binding (pre-dispatch fail-closed contract).

RED on the pre-patch adapter (dispatches `run --agent` with no --model, so the
installed runner silently resolves a foreign default provider; see
V/wave3/C02/cli-runtime-differential-v1/report.md) and GREEN when
tools/delivery/runtime.py binds the configured model. Source authorities
(pin 450e7b92412c780a7b5a75226bb8d49aaca24e22):

- packages/cli/src/commands/commands.ts `Spec.make("run")`: `--model` accepts
  "provider/model#variant".
- packages/schema/src/model.ts `Model.Ref.parse`: provider ends at the FIRST
  "/"; the variant follows the FIRST "#" after it; the model id between them
  may contain further slashes.
- packages/schema/src/config/model.ts `ConfigModel.Selection`: agents accept a
  short string or an object {providerID, model, variant?}.
- packages/core/src/session/runner/model.ts: an explicit session.model that is
  unavailable raises ModelUnavailableError; only the agent-model path falls
  back to the default resolver. The error is raised while loading session
  context, before any provider HTTP request.

No live provider, daemon, network or canonical writes are involved: the fake
executable is a deterministic subprocess double, never presented as a live run.
"""
import json
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.delivery import runtime

ROUTE = "9router-river-glm-5-3-flash"
SLASHY = "9router/river/zai-org/GLM-5.3-Flash"
OBJECT_FORM = {"providerID": "9router", "model": "river/zai-org/GLM-5.3-Flash"}

FAKE = '''#!/usr/bin/env python3
"""Deterministic OpenCode CLI double; never a live model run."""
import json, pathlib, sys

workspace = pathlib.Path.cwd()
kind = sys.argv[1] if len(sys.argv) > 1 else ""

if kind == "debug" and sys.argv[2:3] == ["agents"]:
    cfg = json.loads((workspace / ".opencode" / "opencode.json").read_text())
    rows = [{"id": name, "permissions": row["permissions"], "model": row.get("model")}
            for name, row in cfg.get("agents", {}).items() if isinstance(row, dict)]
    print(json.dumps({"agents": rows}))
elif kind == "run":
    capture = {"argv": sys.argv, "model": None, "agent": None, "format": None, "auto": False}
    rest = sys.argv[2:]
    positional = []
    take = False
    for index, value in enumerate(rest):
        if take:
            capture[flag[2:]] = value
            take = False
        elif value in ("--agent", "--model", "--format"):
            flag = value
            take = True
        elif value == "--standalone":
            capture["standalone"] = True
        elif value == "--auto":
            capture["auto"] = True
        else:
            positional.append(value)
    capture["prompt"] = " ".join(positional)
    (workspace / "fake-run-capture.json").write_text(json.dumps(capture))
    if not capture["model"]:
        print(json.dumps({"type": "error", "error": {
            "type": "provider.internal",
            "message": "fallback default provider reached: HTTP 526 foreign",
            "status": 526}}))
        raise SystemExit(1)
    print(json.dumps({"type": "text", "part": {"type": "text", "id": "part-1",
        "messageID": "message-1", "text": "fixture implementation with explicit model"}}))
'''


class RuntimeModelBindTest(unittest.TestCase):
    def setUp(self):
        self._temp = tempfile.TemporaryDirectory()
        root = pathlib.Path(self._temp.name)
        self.root = root
        self.fake = root / "fake-opencode.py"
        self.fake.write_text(FAKE)
        self.fake.chmod(0o755)
        self.cfg_path = root / "agent-catalog.json"
        self.workspace = root / "workspace"
        self.workspace.mkdir()
        self.sentinel = self.workspace / "fake-run-capture.json"

    def tearDown(self):
        self._temp.cleanup()

    def agent_config(self, model, row_variant=None):
        row = {"description": "route row", "mode": "primary", "system": "worker", "model": model}
        if row_variant is not None:
            row["variant"] = row_variant
        self.cfg_path.write_text(json.dumps({"agents": {ROUTE: row}}))

    def dispatch(self):
        cfg = {"stage_timeout": 30, "max_output_bytes": 65536,
               "runtime": {"kind": "opencode", "executable": str(self.fake), "standalone": False,
                           "agent_config": str(self.cfg_path), "forward_env": []}}
        payload = {"stage": "implement", "base": "base", "report": "",
                   "task": {"id": "MODEL-BIND", "goal": "fixture", "source": "fixture",
                            "paths": [str(self.workspace / "impl.py")], "checks": []}}
        return runtime.agent(payload, cfg, self.workspace, ROUTE, lambda: True, None)

    def captured(self):
        return json.loads(self.sentinel.read_text())

    def test_dispatch_binds_exact_slash_heavy_model_reference(self):
        self.agent_config(SLASHY)
        self.dispatch()
        self.assertEqual(self.captured()["model"], SLASHY)
        self.assertEqual(self.captured()["agent"], ROUTE)

    def test_row_variant_is_appended_to_the_bound_reference(self):
        self.agent_config(SLASHY, row_variant="high")
        self.dispatch()
        self.assertEqual(self.captured()["model"], SLASHY + "#high")

    def test_in_string_variant_is_preserved_verbatim(self):
        self.agent_config("9router/an/deepseek-v4-pro-0813#high")
        self.dispatch()
        self.assertEqual(self.captured()["model"], "9router/an/deepseek-v4-pro-0813#high")

    def test_object_selection_form_binds_and_applies_row_variant(self):
        self.agent_config(dict(OBJECT_FORM), row_variant="max")
        self.dispatch()
        self.assertEqual(self.captured()["model"], SLASHY + "#max")

    def test_object_selection_with_matching_variant_stays_single(self):
        self.agent_config(dict(OBJECT_FORM, variant="max"), row_variant="max")
        self.dispatch()
        self.assertEqual(self.captured()["model"], SLASHY + "#max")

    def test_dispatch_model_identity_matches_the_configured_route(self):
        # RED witness: pre-patch the CLI is dispatched with no --model (the
        # capture records the foreign-default route to oxionai-style fallback);
        # post-patch the exact configured reference must be bound.
        self.agent_config(SLASHY)
        self.dispatch()
        captured = self.captured()
        self.assertIsNotNone(captured["model"])
        self.assertEqual(captured["model"], SLASHY)

    def test_foreign_default_failure_is_never_silent_success(self):
        self.agent_config(SLASHY)
        result = self.dispatch()
        self.assertEqual(self.captured()["model"], SLASHY)
        self.assertIn("explicit model", result["summary"])

    def assert_rejected_before_dispatch(self, model, row_variant=None):
        self.agent_config(model, row_variant=row_variant)
        with self.assertRaises(runtime.Failure) as cm:
            self.dispatch()
        self.assertEqual(cm.exception.kind, "runtime")
        self.assertFalse(self.sentinel.exists(), "the CLI must never run for a malformed model binding")

    def test_missing_model_is_refused_before_dispatch(self):
        row = {"description": "route row", "mode": "primary", "system": "worker"}
        self.cfg_path.write_text(json.dumps({"agents": {ROUTE: row}}))
        with self.assertRaises(runtime.Failure) as cm:
            self.dispatch()
        self.assertEqual(cm.exception.kind, "runtime")
        self.assertFalse(self.sentinel.exists())

    def test_malformed_models_are_refused_before_dispatch(self):
        cases = ["", 7, None, "noslash", "/leadless", "9router/", "9router/#high",
                 "9router/id#", "9router/id##tail", "pro#vider/mid",
                 {"model": "mid"}, {"providerID": "9router"},
                 {"providerID": "9router", "model": ""},
                 dict(OBJECT_FORM, variant=""), dict(OBJECT_FORM, variant="a#b")]
        for case in cases:
            with self.subTest(model=case):
                self.assert_rejected_before_dispatch(case)

    def test_malformed_row_variants_are_refused_before_dispatch(self):
        for case in ("", 7, "a#b"):
            with self.subTest(variant=case):
                self.assert_rejected_before_dispatch(SLASHY, row_variant=case)

    def test_model_and_row_variant_conflict_is_refused(self):
        self.assert_rejected_before_dispatch(SLASHY + "#low", row_variant="max")

    def test_command_kind_stays_untouched_without_model_leakage(self):
        echo = self.root / "echo-adapter.py"
        echo.write_text("import json,sys\nprint(json.dumps({'summary': ' '.join(sys.argv[1:])}))\n")
        cfg = {"stage_timeout": 30, "max_output_bytes": 65536,
               "runtime": {"kind": "command", "argv": [sys.executable, str(echo), "{prompt}"],
                           "forward_env": [], "agent_config": str(self.cfg_path)}}
        payload = {"stage": "implement", "base": "base", "report": "",
                   "task": {"id": "CMD", "goal": "fixture", "source": "fixture",
                            "paths": [str(self.workspace / "impl.py")], "checks": []}}
        result = runtime.agent(payload, cfg, self.workspace, ROUTE, lambda: True, None)
        self.assertIn("Task: CMD\n", json.loads(result["summary"])["summary"])
        self.assertNotIn("--model", result["summary"])
        self.assertFalse(self.sentinel.exists())


if __name__ == "__main__":
    unittest.main()
