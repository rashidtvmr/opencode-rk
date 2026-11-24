"""Executable contract tests for the upstream dialog capture seam.

Covers the dialog (permission prompt + required-multiselect form) request
contract of ``tools/ui_parity/renderers/upstream_opentui.py`` and the production
dialog-flow contract of ``tools/ui_parity/renderers/upstream_capture.ts``.

Requirement anchors (all in-workspace, and their presence is itself tested in
``RequirementAnchorContract``):

* ``docs/ui-parity/opencode-ui-parity-kit/checklist.json`` FORM-001 —
  "Permission once/always/reject variants; missing save rules omit always;
  changing selected option changes the body; nested rejection reason is
  correctly scoped." The captured permission journey exercises the option set,
  the always-body swap while the prompt is still open, and the wire reply.
* ``docs/ui-parity/opencode-ui-parity-kit/checklist.json`` FORM-003 —
  "Text/number/integer/boolean/single/multi/external/conditional/default
  fields, validation, review, and draft restoration behave identically."
  Coverage boundary: this feature captures the *required-multiselect* slice of
  FORM-003 only (the task's stated goal); the full field-type matrix and draft
  restoration are NOT claimed by these scenarios.
* ``docs/ui-parity/opencode-ui-parity-kit/audit-messages.md`` §8 — the
  permission keymap ("Arrow-left/h and arrow-right/l cycle options; Enter
  confirms", "Selecting always shows the rules it will persist"), the review
  surface ("marks invalid/required/missing external acknowledgements using
  error color"), and FORM-006 ("validation waits until the matching submit
  point").
* The ported product code cites the pinned upstream lines for every asserted
  string and wire shape: ``crates/cli/src/native_permissions.rs`` (option
  labels 159-161, cycling 640-655, reply body 565, always body 182-188 ←
  ``util/permission.ts:154-160``) and ``crates/cli/src/native_view_dialogs.rs``
  ("Answer required" 4903-4923 ← ``util/form.ts:72-110``, "(not answered)"
  review rows 6069-6100 ← ``form.tsx:1063-1123``, submit block 6031-6067 ←
  ``form.tsx:513-534``, multiselect checkbox rows 6367-6384), plus
  ``crates/server/src/lib.rs:326`` (reply route) and ``:690-693`` (reply keys).

The task brief also cited ``resume-20261114/dialogs/confirmation-3319802.json``
as a source-confirmed divergence record. That file is NOT present in this
checkout, so no assertion here pretends to quote it; the anchors above are the
sources that exist and each is verified to exist and to carry the quoted
obligation.

The core goal is EXECUTED, not merely described: ``GenuineDialogCaptureContract``
runs the real wrapper (``upstream_opentui.py``) as a subprocess against the
pinned oracle workspace, which launches the real Bun adapter, mounts the pinned
upstream app through its own ``createAppFixture``, injects the dialog events,
drives the production keymap, and returns the measured frames. The returned
flows are validated with the real postcondition validator and with journey
assertions on the genuine bytes. When the pinned runtime is genuinely
unavailable or a run fails, the retained real captures from a previous executed
run are used instead (the same policy as
``tests/test_upstream_matched_capture.py:FlowOutputContract``). When neither a
live run nor a retained real capture is obtainable the suite FAILS, exactly as
that sibling suite does: a source-anchored flow that was never executed cannot
satisfy an assertion demanding ``evidence_class == "production_flow"``, so
offering it as a fallback only manufactured a passing run with zero genuine
captures. ``_authentic_flow`` remains only as an explicitly-built input for the
validator unit tests below.

Every request and every hand-written flow payload used in the validator unit
tests is constructed in this file from the same authority identities the
production wrapper enforces, so those suites are reproducible from the delivered
bytes alone. Denials happen at the request boundary; positives exercise the real
parser and the real postcondition validator.
"""
from __future__ import annotations

import importlib.util
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

ROOT = Path(__file__).parents[1]
WRAPPER_PATH = ROOT / "tools/ui_parity/renderers/upstream_opentui.py"
ADAPTER_PATH = ROOT / "tools/ui_parity/renderers/upstream_capture.ts"
CHECKLIST_PATH = ROOT / "docs/ui-parity/opencode-ui-parity-kit/checklist.json"
AUDIT_PATH = ROOT / "docs/ui-parity/opencode-ui-parity-kit/audit-messages.md"
# Persistent evidence root (outside the checkout, same artifacts tree the
# sibling matched-capture suite reads from). Best-effort retention target for
# the executed dialog captures.
RETAINED_DIR = Path(
    "/Users/mymac/Projects/.oc2-features-20261103/artifacts/ui-parity-dialog-capture-20261114"
)

UPSTREAM_COMMIT = "450e7b92412c780a7b5a75226bb8d49aaca24e22"
OPENTUI_COMMIT = "8d6065a3e60039d16c6ee697692dcfbc46d3c786"
NATIVE_DYLIB = "a2013c73d6bc9835aa59c7aa4ae6d58b0a1b6e0822a7f44ef9e21644bac72910"
# Test sentinel only: the real excluded bridge identity arrives via request
# provenance and is verified at runtime; the unit tests never claim a real
# bridge hash.
BRIDGE_SENTINEL = "f" * 64
BUN_RUNTIME = "1.4.4"
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

DIALOG_SCENARIOS = ("form-permission-v1", "form-multiselect-v1")
PERMISSION_STAGES = ("permission_initial", "permission_always_body", "permission_submitted")
FORM_STAGES = (
    "form_initial", "form_required_selected", "form_required_cleared",
    "form_review", "form_submit_error",
)

# The ported upstream always-body wording (`util/permission.ts:154-160` via
# permission_always_lines, native_permissions.rs:182-188).
ALWAYS_BODY_MARKER = "This will always allow the following patterns for this project."
REQUIRED_MARKER = "Answer required"
OPTIONAL_MARKER = "Optional: (not answered)"


def _load():
    spec = importlib.util.spec_from_file_location("upstream_opentui_dialog", WRAPPER_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _dialog_request(scenario, **overrides):
    """Exact production request shape with a dialog scenario selected."""
    request = {
        "schema": "oc2.ui-parity.request/v1",
        "fixture": {"id": "par-static-home-80x24-dark", "viewport": {"columns": 80, "rows": 24}},
        "viewport": {"cols": 80, "rows": 24},
        "theme": {"name": "opencode", "mode": "dark", "new_designs": False},
        "clock_ms": 0,
        "pins": {"upstream_commit": UPSTREAM_COMMIT, "opentui_commit": OPENTUI_COMMIT},
        "production": True,
        "scenario": scenario,
        "search_text": "",
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


def _screen(rows):
    return {"rows": [{"text": text, "w": len(text), "spans": [
        {"c": 0, "n": len(text), "fg": "#ffffff", "bg": "#000000", "attrs": []}
    ] if text else []} for text in rows]}


def _flow(scenario, frames, replies=None):
    """A production flow payload with the measured-identity envelope the
    validator requires, built entirely from in-file literals."""
    return {
        "schema": "oc2.ui-parity.screen.flow/v1",
        "evidence_class": "production_flow",
        "scenario": scenario,
        # A real capture carries the requested viewport; every journey assertion
        # reads it, so the hand-written flows used by the validator unit tests
        # must carry it too.
        "viewport": {"cols": 80, "rows": 24},
        "replies": [] if replies is None else replies,
        "frames": [{"stage": stage, "screen": _screen(rows)} for stage, rows in frames],
        "provenance": {"bun_version": BUN_RUNTIME, "verified_files": {"src/app.tsx": "0" * 64}},
    }


# ---------------------------------------------------------------------------
# Hand-written dialog frames for the VALIDATOR unit tests. They mirror the
# ported upstream render (native_view_dialogs.rs / native_permissions.rs) so
# the negative cases are realistic: multiselect checkbox rows `1. [ ] Label`
# with a 7-space description indent, single-space footer joins, the tabbed bar
# `Targets  Optional  Submit`, review rows `Label: display`, and the
# submit-point validation error painted on the footer row only AFTER Enter.
# ---------------------------------------------------------------------------
FORM_INITIAL = [
    "Choose targets",
    "Targets  Optional  Submit",
    "1. [ ] Staging",
    "       Deploy to staging",
    "⇆ tab ↑↓ select enter toggle esc dismiss",
]
FORM_SELECTED = [
    "Choose targets",
    "Targets  Optional  Submit",
    "1. [✓] Staging",
    "       Deploy to staging",
    "⇆ tab ↑↓ select enter toggle esc dismiss",
]
FORM_REVIEW = [
    "Choose targets",
    "Targets  Optional  Submit",
    "Targets: Answer required",
    "Optional: (not answered)",
    "⇆ tab enter submit esc dismiss",
]
FORM_SUBMIT_ERROR = [
    "Choose targets",
    "Targets  Optional  Submit",
    "Targets: Answer required",
    "Optional: (not answered)",
    "⇆ tab enter submit esc dismiss              Answer required",
]

PERMISSION_INITIAL = [
    "Permission required",
    "$ echo oracle",
    "→ Allow once",
    "  Always allow",
    "  Reject",
    "ctrl+f fullscreen ⇆ select enter confirm",
]
PERMISSION_ALWAYS_BODY = [
    "Permission required",
    ALWAYS_BODY_MARKER,
    "- echo oracle",
    # The real `l` cycle moves the arrow onto Always allow; the genuine
    # capture test asserts the same "→ Always allow" on production bytes.
    "  Allow once",
    "→ Always allow",
    "  Reject",
    "ctrl+f fullscreen ⇆ select enter confirm",
]
PERMISSION_SUBMITTED = [
    "Ask anything",
    "commands",
]

# The real wire body: the decision travels under `reply` (with an optional
# `scope`). `{"reply":"always","scope":"always"}` is what the local CLI posts
# (native_permissions.rs:565); both it and the bare `{"reply":"always"}` shape
# are genuine and must be accepted.
PERMISSION_REPLY_BODY = '{"reply":"always","scope":"always"}'
PERMISSION_REPLY_PATH = "/api/session/ses_oracle_dialog/permission/per_oracle/reply"


def _form_flow(**overrides):
    frames = [
        ("form_initial", list(FORM_INITIAL)),
        ("form_required_selected", list(FORM_SELECTED)),
        ("form_required_cleared", list(FORM_INITIAL)),
        ("form_review", list(FORM_REVIEW)),
        ("form_submit_error", list(FORM_SUBMIT_ERROR)),
    ]
    flow = _flow("form-multiselect-v1", frames, replies=[])
    flow.update(overrides)
    return flow


def _permission_flow(**overrides):
    frames = [
        ("permission_initial", list(PERMISSION_INITIAL)),
        ("permission_always_body", list(PERMISSION_ALWAYS_BODY)),
        ("permission_submitted", list(PERMISSION_SUBMITTED)),
    ]
    replies = [{"path": PERMISSION_REPLY_PATH, "body": PERMISSION_REPLY_BODY, "status": 204}]
    flow = _flow("form-permission-v1", frames, replies=replies)
    flow.update(overrides)
    return flow


def _home_flow():
    return _flow(
        "home-command-palette-v1",
        [("home", ["Ask anything", "commands"]), ("palette_closed", ["Ask anything", "commands"])],
    )


def _authentic_flow(scenario):
    """Full-protocol flow built from the source-anchored authentic frames.

    Last-resort evidence for ``GenuineDialogCaptureContract`` when neither a live
    pinned capture nor a retained real capture can be obtained. It is NOT used by
    the validator unit tests (which use the leaner ``_permission_flow`` /
    ``_form_flow``); it augments those same frames with the measured-channel
    members (``actions``, ``input_trace``, their declared sources, and the full
    verified-file identity set) that the genuine-capture journey assertions read.
    Every literal row is anchored to the ported upstream render by
    ``PortedDialogSourceAnchorContract`` and the payload must still pass the real
    ``validate_production_result`` — so no assertion is skipped or weakened.
    """
    if scenario == "form-permission-v1":
        flow = _permission_flow()
        # Cumulative real keymap dispatches measured at each captured stage.
        # `permission_initial` is captured BEFORE any key, `permission_always_body`
        # after the single `l` cycle, `permission_submitted` after Enter. This
        # mirrors the genuine adapter exactly (one `l`, then one Enter), so a
        # per-frame action audit sees the same sequence a real capture carries.
        trace_keys = ([], ["l"], ["l", "enter"])
    elif scenario == "form-multiselect-v1":
        flow = _form_flow()
        # Same cumulative-at-capture discipline: initial is before any key;
        # selected after the first space; cleared after the second; review after
        # both tabs; submit_error after Enter (two spaces, two tabs, one Enter).
        trace_keys = (
            [], ["space"], ["space", "space"],
            ["space", "space", "tab", "tab"], ["space", "space", "tab", "tab", "enter"],
        )
    else:
        raise AssertionError(f"no authentic flow for scenario {scenario!r}")
    # The genuine-capture tests compare the verified-file identity set.
    flow["provenance"]["verified_files"] = {rel: "0" * 64 for rel in EXPECTED_FILES}
    for frame, keys in zip(flow["frames"], trace_keys):
        screen = frame["screen"]
        screen.update({
            "actions_source": "upstream_input_replay_measured",
            "actions_cursor_after_source": "measured_per_dispatch",
            "input_trace_source": "upstream_input_replay_trace",
            "semantic_actions": None,
        })
        screen["actions"] = [
            {
                "seq": index, "type": "key", "key": key, "applied": True,
                "cursor_after": {"row": 0, "col": index, "visible": True},
            }
            for index, key in enumerate(keys)
        ]
        screen["input_trace"] = [
            {"input": "key", "name": key, "seq": index, "measured_cursor": [index, 0]}
            for index, key in enumerate(keys)
        ]
    return flow


def _stage_text(flow):
    """Stage name -> joined row text for a flow payload."""
    return {
        frame["stage"]: "\n".join(row["text"] for row in frame["screen"]["rows"])
        for frame in flow["frames"]
    }


class DialogRequestContract(unittest.TestCase):
    """The dialog scenarios keep the strict production request boundary."""

    def setUp(self):
        self.mod = _load()

    def _parse(self, request):
        return self.mod.parse_request(json.dumps(request))

    def test_dialog_scenarios_are_supported_and_parse(self):
        for scenario in DIALOG_SCENARIOS:
            with self.subTest(scenario=scenario):
                parsed = self._parse(_dialog_request(scenario))
                self.assertEqual(parsed["production"], True)
                self.assertEqual(parsed["scenario"], scenario)
                self.assertEqual(parsed["search_text"], "")
                self.assertEqual(parsed["events"], [])
                self.assertEqual(parsed["state_overrides"], {})

    def test_dialog_scenarios_have_a_contract_with_stages(self):
        for scenario in DIALOG_SCENARIOS:
            with self.subTest(scenario=scenario):
                self.assertIn(scenario, self.mod.SUPPORTED_SCENARIOS)
                self.assertIn(scenario, self.mod.DIALOG_SCENARIOS)
                contract = self.mod.SCENARIO_CONTRACTS[scenario]
                self.assertTrue(contract["stages"])
        self.assertEqual(
            tuple(self.mod.SCENARIO_CONTRACTS["form-permission-v1"]["stages"]), PERMISSION_STAGES)
        self.assertEqual(
            tuple(self.mod.SCENARIO_CONTRACTS["form-multiselect-v1"]["stages"]), FORM_STAGES)
        self.assertEqual(
            self.mod.SUPPORTED_SCENARIOS,
            {"home-command-palette-v1", "form-permission-v1", "form-multiselect-v1"})

    def test_supported_and_contract_sets_are_exactly_consistent(self):
        # Every supported scenario must have a contract (there is no scenario
        # the parser accepts but the postcondition validator cannot name), and
        # the dialog set is exactly the supported scenarios minus the home
        # journey. This is the structural guarantee the dispatcher relies on.
        self.assertEqual(set(self.mod.SCENARIO_CONTRACTS), self.mod.SUPPORTED_SCENARIOS)
        self.assertEqual(
            self.mod.DIALOG_SCENARIOS,
            self.mod.SUPPORTED_SCENARIOS - {"home-command-palette-v1"})

    def test_permission_contract_declares_the_body_swap_and_option_stages(self):
        contract = self.mod.SCENARIO_CONTRACTS["form-permission-v1"]
        # FORM-001: the initial frame paints the option set...
        for label in ("Permission required", "Allow once", "Always allow", "Reject"):
            self.assertIn(label, contract["initial_visible"])
        # ... and selecting `always` swaps the body to the persisted rules.
        self.assertIn(ALWAYS_BODY_MARKER, contract["always_body_visible"])

    def test_permission_contract_uses_the_reply_wire_key_not_decision(self):
        contract = self.mod.SCENARIO_CONTRACTS["form-permission-v1"]
        self.assertIn('"reply":"always"', contract["reply_body_contains"])
        self.assertNotIn("decision", contract["reply_body_contains"])
        # No scenario contract may fabricate the non-existent `decision` key.
        self.assertNotIn("decision", json.dumps(self.mod.SCENARIO_CONTRACTS))

    def test_permission_contract_declares_save_rule_and_option_order(self):
        contract = self.mod.SCENARIO_CONTRACTS["form-permission-v1"]
        self.assertIn("- echo oracle", contract["always_body_visible"])
        self.assertEqual(contract["option_order"], ["Allow once", "Always allow", "Reject"])
        self.assertEqual(contract["initial_selected_visible"], ["→ Allow once"])
        self.assertEqual(contract["always_selected_visible"], ["→ Always allow"])

    def test_permission_contract_declares_prompt_dismissal_markers(self):
        contract = self.mod.SCENARIO_CONTRACTS["form-permission-v1"]
        self.assertEqual(contract["prompt_visible_after_reply"], [
            "Permission required", "Allow once", "Always allow", "Reject"])

    def test_permission_contract_dropped_the_unverifiable_status_marker(self):
        # The pre-repair contract asserted a painted "UnexpectedStatus" marker
        # that no in-workspace source anchors. The decisive post-reply checks
        # (prompt gone, reply wire shape) replaced it; the fabricated marker
        # must not return.
        contract = self.mod.SCENARIO_CONTRACTS["form-permission-v1"]
        self.assertNotIn("forbidden_visible", contract)
        self.assertNotIn("UnexpectedStatus", json.dumps(self.mod.SCENARIO_CONTRACTS))

    def test_unknown_dialog_scenario_denied(self):
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self._parse(_dialog_request("form-unknown-v1"))
        self.assertIn("unsupported scenario", str(caught.exception))

    def test_dialog_events_are_denied_at_request_boundary(self):
        request = _dialog_request("form-permission-v1", events=[{"type": "permission.asked"}])
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self._parse(request)
        self.assertIn("unsupported state/event", str(caught.exception))

    def test_dialog_state_overrides_are_denied_at_request_boundary(self):
        request = _dialog_request("form-multiselect-v1", state_overrides={"form": "x"})
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self._parse(request)
        self.assertIn("unsupported state/event", str(caught.exception))

    def test_dialog_search_text_contract_is_enforced_both_ways(self):
        # Empty is the dialog contract and must parse; a non-empty value must be
        # refused for the exact reason (not merely "some failure happened").
        for scenario in DIALOG_SCENARIOS:
            with self.subTest(scenario=scenario, value=""):
                self._parse(_dialog_request(scenario, search_text=""))
            with self.subTest(scenario=scenario, value="new"):
                with self.assertRaises(self.mod.RequestDenied) as caught:
                    self._parse(_dialog_request(scenario, search_text="new"))
                self.assertIn("search_text", str(caught.exception))

    def test_provenance_mismatch_still_denied_for_dialog(self):
        request = _dialog_request("form-multiselect-v1")
        request["provenance"]["expected"]["files"]["src/app.tsx"] = "0" * 64
        with self.assertRaises(self.mod.RequestDenied) as caught:
            self._parse(request)
        self.assertIn("provenance files", str(caught.exception))

    def test_dialog_production_request_rejects_masquerade_flags(self):
        parsed = self._parse(_dialog_request("form-permission-v1"))
        for extra in (["--component"], ["--synthetic"]):
            with self.subTest(extra=extra):
                with self.assertRaises(self.mod.RequestDenied) as caught:
                    self.mod.validate_forward(parsed, extra)
                self.assertIn("masquerade", str(caught.exception))

    def test_dialog_production_request_rejects_unknown_forwarded_flags(self):
        # ``validate_forward`` has two independent denial branches: the
        # production masquerade check above, and the allowlist that refuses any
        # flag outside {--component, --synthetic}. A production dialog request
        # must not smuggle an arbitrary flag to the adapter through either one.
        # These cases exercise the second branch specifically (none of them is a
        # synthetic flag), so the denial reason is the forwarded-flag allowlist,
        # not masquerade.
        parsed = self._parse(_dialog_request("form-multiselect-v1"))
        for extra in (["--oracle"], ["--timeout", "5"], ["--evil"]):
            with self.subTest(extra=extra):
                with self.assertRaises(self.mod.RequestDenied) as caught:
                    self.mod.validate_forward(parsed, extra)
                self.assertIn("unsupported forwarded flags", str(caught.exception))


class CaptureStdoutContract(unittest.TestCase):
    """The capture trust boundary: stdout must be exactly one JSON object."""

    def setUp(self):
        self.mod = _load()

    def _run(self, stdout):
        proc = unittest.mock.MagicMock(returncode=0, stdout=stdout, stderr="")
        run = unittest.mock.MagicMock(return_value=proc)
        with unittest.mock.patch.object(self.mod, "subprocess", unittest.mock.MagicMock(run=run)):
            return self.mod.run_capture("bun", Path("/unused"), "// capture", {}, [], 10.0)

    def test_exactly_one_json_object_is_accepted(self):
        self.assertEqual(self._run('{"schema":"probe"}\n'), {"schema": "probe"})

    def test_surrounding_bytes_are_refused_not_sliced(self):
        for stdout in ('noise {"schema":"probe"}', '{"schema":"probe"} trailing', '{"a":1}{"b":2}'):
            with self.subTest(stdout=stdout):
                with self.assertRaises(self.mod.WrapperError) as caught:
                    self._run(stdout)
                self.assertIn("did not emit JSON", str(caught.exception))

    def test_non_object_json_is_refused(self):
        with self.assertRaises(self.mod.WrapperError) as caught:
            self._run('[1, 2, 3]')
        self.assertIn("non-object", str(caught.exception))

    def test_nonzero_exit_is_refused_before_parsing(self):
        proc = unittest.mock.MagicMock(returncode=2, stdout='{"schema":"probe"}', stderr="boom")
        run = unittest.mock.MagicMock(return_value=proc)
        with unittest.mock.patch.object(self.mod, "subprocess", unittest.mock.MagicMock(run=run)):
            with self.assertRaises(self.mod.WrapperError) as caught:
                self.mod.run_capture("bun", Path("/unused"), "// capture", {}, [], 10.0)
        self.assertIn("capture exited 2", str(caught.exception))

    def test_failure_diagnostic_never_discards_partial_stdout(self):
        # A non-zero exit may still have emitted a partial measured payload (or
        # a Bun runtime diagnostic) on stdout. The wrapper includes it in the
        # failure so a partial production capture is never silently lost behind
        # a stderr-only message; this is the trust boundary that keeps a failed
        # dialog journey reportable for repair instead of opaque.
        proc = unittest.mock.MagicMock(
            returncode=1, stdout='{"partial":"dialog-flow"}', stderr="boom")
        run = unittest.mock.MagicMock(return_value=proc)
        with unittest.mock.patch.object(self.mod, "subprocess", unittest.mock.MagicMock(run=run)):
            with self.assertRaises(self.mod.WrapperError) as caught:
                self.mod.run_capture("bun", Path("/unused"), "// capture", {}, [], 10.0)
        message = str(caught.exception)
        self.assertIn("capture exited 1", message)
        self.assertIn("stdout=", message)
        self.assertIn("dialog-flow", message)


class DialogPostconditionContract(unittest.TestCase):
    """The measured dialog postconditions, driven by in-file payloads."""

    def setUp(self):
        self.mod = _load()

    def reject(self, payload, code):
        with self.assertRaises(self.mod.WrapperError) as caught:
            self.mod.validate_production_result(payload)
        self.assertIn(code, str(caught.exception))

    def test_form_flow_accepts(self):
        self.mod.validate_production_result(_form_flow())

    def test_permission_flow_accepts(self):
        self.mod.validate_production_result(_permission_flow())

    def test_form_unchecked_state_is_required(self):
        flow = _form_flow()
        next(f for f in flow["frames"] if f["stage"] == "form_initial")["screen"]["rows"] = _screen(
            ["Choose targets", "Targets  Optional  Submit", "1. [✓] Staging", "Deploy to staging"])["rows"]
        self.reject(flow, "form_required_check_not_painted")

    def test_permission_flow_accepts_every_genuine_reply_body_shape(self):
        # The local CLI posts {reply, scope}; the bare {reply} shape is genuine
        # too. Both must be accepted.
        for body in (PERMISSION_REPLY_BODY, '{"reply":"always"}'):
            with self.subTest(body=body):
                flow = _permission_flow()
                flow["replies"][0]["body"] = body
                self.mod.validate_production_result(flow)

    def test_duplicate_or_out_of_order_stages_are_rejected(self):
        duplicate = _form_flow()
        duplicate["frames"].append(dict(duplicate["frames"][-1]))
        self.reject(duplicate, "production_result_duplicate_stage: form_submit_error")

        out_of_order = _form_flow()
        out_of_order["frames"][0], out_of_order["frames"][1] = out_of_order["frames"][1], out_of_order["frames"][0]
        self.reject(out_of_order, "production_result_stage_order")

    def test_permission_duplicate_or_out_of_order_stages_are_rejected(self):
        # Mirror of the form-journey stage-timeline guard for the permission
        # journey: a duplicated submitted frame or a reordered body-swap frame
        # is an unmeasurable timeline, not a genuine capture.
        duplicate = _permission_flow()
        duplicate["frames"].append(dict(duplicate["frames"][-1]))
        self.reject(duplicate, "production_result_duplicate_stage: permission_submitted")

        out_of_order = _permission_flow()
        out_of_order["frames"][0], out_of_order["frames"][1] = out_of_order["frames"][1], out_of_order["frames"][0]
        self.reject(out_of_order, "production_result_stage_order")

    def test_permission_unmeasurable_rejected(self):
        # The permission journey must fail closed on a missing reply channel,
        # no frames, or any dropped stage — the mirror of the form journey's
        # unmeasurable guard. Without these, a reply-only or body-only payload
        # could pass on identity alone.
        flow = _permission_flow(); del flow["replies"]
        self.reject(flow, "production_result_unmeasurable_replies")
        flow = _permission_flow(); flow["frames"] = []
        self.reject(flow, "production_result_unmeasurable_frames")
        for stage in ("permission_initial", "permission_always_body", "permission_submitted"):
            flow = _permission_flow()
            flow["frames"] = [f for f in flow["frames"] if f["stage"] != stage]
            self.reject(flow, f"production_result_unmeasurable_stage: {stage}")

    def test_permission_reply_route_and_wire_shape_are_both_required(self):
        # The reply must travel on the real permission endpoint AND carry the
        # real `reply` key with the selected `always` decision. A correctly
        # keyed body on the wrong route (or vice versa) is not production
        # evidence and must be refused with the reply code.
        flow = _permission_flow()
        flow["replies"][0]["path"] = "/api/session/x/permission/y/reply"
        # A structurally valid permission route is accepted (the session/request
        # ids are opaque), proving the guard is the route shape, not a literal.
        self.mod.validate_production_result(flow)
        flow = _permission_flow()
        flow["replies"][0]["path"] = "/api/session/x/permission/y/reply/extra"
        self.reject(flow, "permission_reply_not_successful")

    def test_home_flow_keeps_existing_contract(self):
        # The home flow must not be forced through the dialog postconditions.
        self.mod.validate_production_result(_home_flow())

    def test_home_flow_does_not_route_into_the_dialog_postconditions(self):
        # Routing is part of the contract: only the two dialog scenarios may
        # reach validate_dialog_postconditions. A home flow that silently
        # entered it would be judged against the wrong journey.
        with unittest.mock.patch.object(
                self.mod, "validate_dialog_postconditions",
                wraps=self.mod.validate_dialog_postconditions) as spy:
            self.mod.validate_production_result(_home_flow())
        self.assertEqual(spy.call_count, 0)

    def test_both_dialog_scenarios_route_into_the_postcondition_validator(self):
        # The mirror of the home non-routing guard: each dialog scenario must be
        # dispatched to validate_dialog_postconditions exactly once. A scenario
        # that silently skipped the journey checks would be accepted on identity
        # alone — the exact failure mode this seam exists to prevent.
        for scenario, flow in (("form-permission-v1", _permission_flow()),
                               ("form-multiselect-v1", _form_flow())):
            with self.subTest(scenario=scenario):
                with unittest.mock.patch.object(
                        self.mod, "validate_dialog_postconditions",
                        wraps=self.mod.validate_dialog_postconditions) as spy:
                    self.mod.validate_production_result(flow)
                self.assertEqual(spy.call_count, 1)
                self.assertIs(spy.call_args[0][0], flow)

    def test_form_unexpected_reply_rejected(self):
        flow = _form_flow()
        flow["replies"].append({"path": "/form/reply", "body": "{}", "status": 204})
        self.reject(flow, "form_reply_unexpectedly_observed")

    def test_form_review_must_already_mark_the_required_field(self):
        # The pre-submit review surface marks required fields (audit-messages.md
        # §8). A flow whose review frame hides the marker is not the modelled
        # journey.
        flow = _form_flow()
        flow["frames"] = [
            frame for frame in flow["frames"] if frame["stage"] != "form_review"
        ] + [{"stage": "form_review", "screen": _screen(
            ["Choose targets", "Targets  Optional  Submit", "Optional: (not answered)",
             "⇆ tab enter submit esc dismiss"])}]
        self.reject(flow, "form_required_error_missing: form_review")

    def test_form_review_must_show_the_validation_text_not_a_placeholder(self):
        # A required field's review row carries the validation text
        # `validate(field, None) == "Answer required"` (util/form.ts:72-110 via
        # native_view_dialogs.rs:4903-4921, consumed by review_rows:6087-6089),
        # not the `(required)` placeholder used only for other states. A review
        # that shows the placeholder never reached the review validation surface
        # the blocked-submit differential depends on.
        flow = _form_flow()
        next(f for f in flow["frames"] if f["stage"] == "form_review")["screen"]["rows"] = _screen([
            "Choose targets", "Targets  Optional  Submit", "Targets: (required)",
            "Optional: (not answered)", "⇆ tab enter submit esc dismiss"])["rows"]
        self.reject(flow, "form_required_error_missing: form_review")

    def test_form_blocked_submit_is_decisive(self):
        # The submit-point validation error is a NEW occurrence painted only at
        # the matching submit point (FORM-006). A post-Enter frame with the same
        # marker count as the pre-submit review frame proves Enter changed
        # nothing — the submit attempt was not observed.
        flow = _form_flow()
        next(f for f in flow["frames"] if f["stage"] == "form_submit_error")["screen"] = _screen(
            list(FORM_REVIEW))
        self.reject(flow, "form_submit_attempt_not_observed")

        stripped = _form_flow()
        next(f for f in stripped["frames"] if f["stage"] == "form_submit_error")["screen"] = _screen(
            ["Choose targets", "Targets  Optional  Submit", "Optional: (not answered)",
             "⇆ tab enter submit esc dismiss"])
        self.reject(stripped, "form_submit_attempt_not_observed")

    def test_form_optional_control_required_on_review_and_submit(self):
        flow = _form_flow()
        next(f for f in flow["frames"] if f["stage"] == "form_review")["screen"]["rows"] = [
            row for row in flow["frames"][-2]["screen"]["rows"] if OPTIONAL_MARKER not in row["text"]
        ]
        self.reject(flow, "form_optional_control_missing")
        flow = _form_flow()
        next(f for f in flow["frames"] if f["stage"] == "form_submit_error")["screen"]["rows"] = [
            row for row in flow["frames"][-1]["screen"]["rows"] if OPTIONAL_MARKER not in row["text"]
        ]
        self.reject(flow, "form_optional_control_missing")

    def test_form_toggle_round_trip_is_required(self):
        # A flow whose "selected" frame equals the initial frame proves no
        # toggle was observed; a flow whose "cleared" frame differs proves the
        # toggle did not round-trip. Both are refused.
        no_toggle = _form_flow()
        next(f for f in no_toggle["frames"] if f["stage"] == "form_required_selected")["screen"]["rows"] = _screen(FORM_INITIAL)["rows"]
        self.reject(no_toggle, "form_required_toggle_not_observed")

        not_reversible = _form_flow()
        next(f for f in not_reversible["frames"] if f["stage"] == "form_required_cleared")["screen"]["rows"] = _screen(FORM_SELECTED)["rows"]
        self.reject(not_reversible, "form_required_toggle_not_reversible")

    def test_form_unmeasurable_rejected(self):
        flow = _form_flow(); del flow["replies"]
        self.reject(flow, "production_result_unmeasurable_replies")
        flow = _form_flow(); flow["frames"] = []
        self.reject(flow, "production_result_unmeasurable_frames")
        for stage in ("form_submit_error", "form_required_selected", "form_review"):
            flow = _form_flow()
            flow["frames"] = [f for f in flow["frames"] if f["stage"] != stage]
            self.reject(flow, f"production_result_unmeasurable_stage: {stage}")
        flow = _form_flow(); flow["frames"][0]["screen"]["rows"] = [{"text": 7}]
        self.reject(flow, "production_result_unmeasurable_rows")
        flow = _form_flow(); flow["frames"][0]["screen"]["rows"] = [{"text": "bad", "w": 3, "spans": []}]
        self.reject(flow, "production_result_unmeasurable_styles")
        flow = _form_flow(); flow["frames"][0]["screen"]["rows"][0]["spans"] = [
            {"c": 1, "n": 2, "fg": "#ffffff", "bg": "#000000", "attrs": []}]
        self.reject(flow, "production_result_unmeasurable_styles")

    def test_permission_reply_not_successful_rejected(self):
        flow = _permission_flow(); flow["replies"][0]["status"] = 200
        self.reject(flow, "permission_reply_not_successful")
        # The `decision` key is not a key any client sends: a payload that only
        # carries it must be refused rather than accepted as "always".
        flow = _permission_flow(); flow["replies"][0]["body"] = '{"decision":"always"}'
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["body"] = '{"reply":"once"}'
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["body"] = '{"reply":"reject"}'
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["path"] = ""
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["path"] = "/api/session/x/message"
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["path"] = "/api/session/x/permission/y"
        self.reject(flow, "permission_reply_not_successful")
        flow = _permission_flow(); flow["replies"][0]["path"] = "/api/session/x/permission/y/reply?extra=1"
        self.reject(flow, "permission_reply_not_successful")
        for path in ("/api/session/x/message/permission/y/reply", "/api/session/x/permission/y/reply/extra"):
            with self.subTest(path=path):
                flow = _permission_flow(); flow["replies"][0]["path"] = path
                self.reject(flow, "permission_reply_not_successful")
        for body in (
            # A foreign scope, the non-existent `decision` key, and arbitrary
            # extra keys are all outside the production reply wire shape: only
            # `reply` (plus the optional `scope`) may travel.
            '{"reply":"always","scope":"session"}',
            '{"reply":"always","decision":"always"}',
            '{"reply":"always","sessionID":"ses_x"}',
            "not-json",
        ):
            with self.subTest(body=body):
                flow = _permission_flow(); flow["replies"][0]["body"] = body
                self.reject(flow, "permission_reply_not_successful")

    def test_permission_body_swap_is_required_and_real(self):
        # FORM-001: the always frame must differ from the initial frame and must
        # show the rules the option will persist.
        identical = _permission_flow()
        next(f for f in identical["frames"] if f["stage"] == "permission_always_body")["screen"]["rows"] = _screen(PERMISSION_INITIAL)["rows"]
        self.reject(identical, "permission_body_swap_not_observed")

        no_rules = _permission_flow()
        next(f for f in no_rules["frames"] if f["stage"] == "permission_always_body")["screen"]["rows"] = _screen(
            ["Permission required", "→ Allow once", "  Always allow", "  Reject"]
        )["rows"]
        self.reject(no_rules, "permission_always_body_missing")

    def test_permission_body_bullet_format_is_required(self):
        # The persisted rules body lists each save pattern as a `- <pattern>`
        # bullet (permission_always_lines -> `format!("- {item}")`,
        # native_permissions.rs:182-188). A body that names the command without
        # the bullet is not "the rules it will persist", so the sentence alone
        # must not satisfy the swap.
        flow = _permission_flow()
        next(f for f in flow["frames"] if f["stage"] == "permission_always_body")["screen"]["rows"] = _screen(
            ["Permission required", ALWAYS_BODY_MARKER, "echo oracle",
             "  Allow once", "→ Always allow", "  Reject"])["rows"]
        self.reject(flow, "permission_always_body_missing")

    def test_permission_rules_body_must_not_be_pre_painted(self):
        # The swap must be a swap: if the initial frame already paints the
        # persisted-rules body, the journey never observed the option change.
        flow = _permission_flow()
        next(f for f in flow["frames"] if f["stage"] == "permission_initial")["screen"]["rows"] = _screen(
            [PERMISSION_INITIAL[0], ALWAYS_BODY_MARKER, *PERMISSION_INITIAL[1:]])["rows"]
        self.reject(flow, "permission_always_body_pre_painted")

    def test_permission_selected_option_is_measured(self):
        flow = _permission_flow()
        next(f for f in flow["frames"] if f["stage"] == "permission_initial")["screen"]["rows"] = _screen(
            ["Permission required", "  Allow once", "  Always allow", "  Reject"])["rows"]
        self.reject(flow, "permission_initial_selected_option")
        flow = _permission_flow()
        next(f for f in flow["frames"] if f["stage"] == "permission_always_body")["screen"]["rows"] = _screen(
            ["Permission required", ALWAYS_BODY_MARKER, "- echo oracle", "→ Allow once", "Always allow"])["rows"]
        self.reject(flow, "permission_always_option_not_selected")

    def test_permission_option_order_is_pinned(self):
        flow = _permission_flow()
        initial = next(f for f in flow["frames"] if f["stage"] == "permission_initial")
        initial["screen"]["rows"] = _screen([
            "Permission required", "→ Always allow", "  Allow once", "  Reject"])["rows"]
        self.reject(flow, "permission_initial_option_order")

    def test_permission_initial_option_set_is_required(self):
        # The validator walks initial_visible in order and raises at the FIRST
        # missing marker, so each dropped option must name itself.
        for dropped in ("Always allow", "Reject"):
            with self.subTest(dropped=dropped):
                rows = [row for row in PERMISSION_INITIAL if dropped not in row]
                flow = _permission_flow()
                next(f for f in flow["frames"] if f["stage"] == "permission_initial")["screen"]["rows"] = _screen(rows)["rows"]
                self.reject(flow, f"permission_initial_missing: {dropped!r}")

    def test_permission_prompt_must_close_after_the_reply(self):
        for marker in (ALWAYS_BODY_MARKER, "Permission required", "Allow once", "Always allow", "Reject"):
            with self.subTest(marker=marker):
                flow = _permission_flow()
                next(f for f in flow["frames"] if f["stage"] == "permission_submitted")["screen"]["rows"].append(
                    {"text": marker, "w": len(marker), "spans": [
                        {"c": 0, "n": len(marker), "fg": "#ffffff", "bg": "#000000", "attrs": []}
                    ]}
                )
                self.reject(flow, "permission_prompt_still_open")

    def test_permission_missing_stage_rejected(self):
        for stage in ("permission_always_body", "permission_submitted"):
            with self.subTest(stage=stage):
                flow = _permission_flow()
                flow["frames"] = [f for f in flow["frames"] if f["stage"] != stage]
                self.reject(flow, f"production_result_unmeasurable_stage: {stage}")

    def test_permission_unexpected_reply_rejected(self):
        flow = _permission_flow()
        flow["replies"].append({"path": PERMISSION_REPLY_PATH, "body": PERMISSION_REPLY_BODY, "status": 204})
        self.reject(flow, "form_reply_unexpectedly_observed")

    def test_permission_missing_reply_rejected(self):
        # A permission journey that posts nothing never observed the reply: the
        # reply-count postcondition must reject an empty list, not accept the
        # frames on identity alone. This is the mirror of the extra-reply case.
        flow = _permission_flow(); flow["replies"] = []
        self.reject(flow, "form_reply_unexpectedly_observed")

    def test_unknown_scenario_rejected_by_postconditions(self):
        # validate_production_result only routes the two dialog scenarios to the
        # dialog validator; the validator itself must still refuse anything else
        # rather than silently accepting an unmodelled payload.
        with self.assertRaises(self.mod.WrapperError) as caught:
            self.mod.validate_dialog_postconditions(_form_flow(scenario="form-unknown-v1"))
        self.assertIn("production_result_unknown_scenario", str(caught.exception))


class DialogMainPathContract(unittest.TestCase):
    """main() must reach the dialog validator and fail closed on its verdict."""

    def setUp(self):
        self.mod = _load()
        self.tmp = tempfile.mkdtemp(prefix="oc2-dialog-main-")
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.capture = Path(self.tmp) / "capture.ts"
        self.capture.write_text("// capture\n")

    def _run_main(self, payload, result):
        stdin = type("Reader", (), {"read": lambda self: payload})()
        stdout, stderr = io.StringIO(), io.StringIO()
        with unittest.mock.patch.object(self.mod, "run_capture", return_value=result), \
                unittest.mock.patch.object(self.mod, "resolve_bun", return_value="bun"), \
                unittest.mock.patch.object(sys, "argv",
                                           ["upstream_opentui.py", "--capture", str(self.capture)]), \
                unittest.mock.patch.object(sys, "stdin", stdin), \
                unittest.mock.patch.object(sys, "stdout", stdout), \
                unittest.mock.patch.object(sys, "stderr", stderr):
            code = self.mod.main()
        return code, stdout.getvalue(), stderr.getvalue()

    def test_main_accepts_valid_form_flow(self):
        code, out, err = self._run_main(json.dumps(_dialog_request("form-multiselect-v1")), _form_flow())
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["scenario"], "form-multiselect-v1")

    def test_main_accepts_valid_permission_flow(self):
        code, out, err = self._run_main(json.dumps(_dialog_request("form-permission-v1")), _permission_flow())
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["scenario"], "form-permission-v1")

    def test_main_reaches_dialog_validator_and_fails_closed(self):
        bad = _form_flow()
        bad["replies"].append({"path": "/form/reply", "body": "{}", "status": 204})
        with unittest.mock.patch.object(
                self.mod, "validate_dialog_postconditions",
                wraps=self.mod.validate_dialog_postconditions) as spy:
            code, out, err = self._run_main(json.dumps(_dialog_request("form-multiselect-v1")), bad)
        self.assertEqual(code, 3)
        self.assertIn("form_reply_unexpectedly_observed", err)
        self.assertEqual(out, "")
        self.assertTrue(spy.called)
        self.assertEqual(spy.call_count, 1)

    def test_main_fails_closed_on_a_decision_keyed_permission_reply(self):
        bad = _permission_flow()
        bad["replies"][0]["body"] = '{"decision":"always"}'
        code, out, err = self._run_main(json.dumps(_dialog_request("form-permission-v1")), bad)
        self.assertEqual(code, 3)
        self.assertIn("permission_reply_not_successful", err)
        self.assertEqual(out, "")

    def test_main_fails_closed_on_a_pre_painted_permission_body(self):
        # FORM-001: the always-body swap must be a swap. A flow whose initial
        # frame already paints the persisted-rules body never observed the
        # option change, so main() must refuse it with the exact code.
        bad = _permission_flow()
        next(f for f in bad["frames"] if f["stage"] == "permission_initial")["screen"]["rows"] = _screen(
            [PERMISSION_INITIAL[0], ALWAYS_BODY_MARKER, *PERMISSION_INITIAL[1:]])["rows"]
        code, out, err = self._run_main(json.dumps(_dialog_request("form-permission-v1")), bad)
        self.assertEqual(code, 3)
        self.assertIn("permission_always_body_pre_painted", err)
        self.assertEqual(out, "")

    def test_main_fails_closed_on_a_reordered_dialog_timeline(self):
        # A content-valid flow whose stage timeline is out of order must still
        # be refused at the main() boundary with the exact stage-order code.
        bad = _form_flow()
        bad["frames"][0], bad["frames"][1] = bad["frames"][1], bad["frames"][0]
        code, out, err = self._run_main(json.dumps(_dialog_request("form-multiselect-v1")), bad)
        self.assertEqual(code, 3)
        self.assertIn("production_result_stage_order", err)
        self.assertEqual(out, "")

    def test_main_fails_closed_on_a_non_reversible_form_toggle(self):
        # FORM-003 multiselect slice: the space toggle must round-trip. A flow
        # whose cleared frame differs from the initial frame never observed the
        # deselect, so main() must refuse it with the exact code.
        bad = _form_flow()
        next(f for f in bad["frames"] if f["stage"] == "form_required_cleared")["screen"]["rows"] = _screen(
            FORM_SELECTED)["rows"]
        code, out, err = self._run_main(json.dumps(_dialog_request("form-multiselect-v1")), bad)
        self.assertEqual(code, 3)
        self.assertIn("form_required_toggle_not_reversible", err)
        self.assertEqual(out, "")


class GenuineDialogCaptureContract(unittest.TestCase):
    """The core goal, executed: genuine pinned upstream dialog captures.

    Runs the real wrapper (``upstream_opentui.py``) as a subprocess. The wrapper
    launches the real Bun adapter inside the pinned oracle workspace, which
    mounts the pinned upstream app through its own ``createAppFixture``,
    injects the dialog event through the app's event stream, drives the
    production keymap, and emits the measured frames. The returned flow is then
    validated with the real postcondition validator AND with journey assertions
    on the genuine bytes — no hand-written frame is accepted here.

    Execution is attempted whenever the pinned runtime is available on this
    machine. When it is absent or fails, a retained real capture from a previous
    executed run is used (same policy as the sibling FlowOutputContract). Only
    when neither is obtainable the class fails closed, matching the sibling
    ``FlowOutputContract``. ``_authentic_flow`` is used only by the validator
    unit tests, which build their inputs explicitly.
    """

    CAPTURE_TIMEOUT_ARG = "40"
    SUBPROCESS_TIMEOUT_S = 50.0

    @classmethod
    def _runtime_available(cls) -> bool:
        if not shutil.which("bun"):
            return False
        oracle = Path(_load().ORACLE)
        return (oracle / "packages" / "tui").is_dir()

    @classmethod
    def _execute(cls, scenario: str):
        request = _dialog_request(scenario)
        command = [sys.executable, "-B", str(WRAPPER_PATH), "--timeout", cls.CAPTURE_TIMEOUT_ARG]
        try:
            proc = subprocess.run(
                command, input=json.dumps(request), text=True, capture_output=True,
                timeout=cls.SUBPROCESS_TIMEOUT_S, check=False,
            )
        except subprocess.TimeoutExpired:
            return None, f"wrapper timed out after {cls.SUBPROCESS_TIMEOUT_S}s"
        if proc.returncode != 0:
            return None, f"exit {proc.returncode}: {proc.stderr.strip()[-2000:]}"
        try:
            return json.loads(proc.stdout), ""
        except json.JSONDecodeError as error:
            return None, f"wrapper did not emit JSON: {error}; stdout={proc.stdout.strip()[-800:]}"

    @classmethod
    def _retain(cls, scenario: str, flow: dict) -> None:
        """Best-effort evidence export to the persistent artifacts root."""
        try:
            RETAINED_DIR.mkdir(parents=True, exist_ok=True)
            (RETAINED_DIR / f"{scenario}.flow.json").write_text(
                json.dumps(flow, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        except OSError:
            pass

    @classmethod
    def setUpClass(cls):
        cls.mod = _load()
        cls.executed = False
        cls.flows = {}
        cls.capture_diagnostics = {}
        runtime = cls._runtime_available()
        for scenario in DIALOG_SCENARIOS:
            flow, diagnostic = None, "pinned Bun runtime/oracle unavailable"
            if runtime:
                flow, diagnostic = cls._execute(scenario)
                if flow is not None:
                    cls.executed = True
                    cls._retain(scenario, flow)
            if flow is None:
                # A retained real capture is genuine production evidence from a
                # previously executed run — exactly the policy the sibling
                # ``FlowOutputContract`` applies to its real capture. It is used
                # only when this run could not produce a capture.
                retained = RETAINED_DIR / f"{scenario}.flow.json"
                if retained.exists():
                    cls.capture_diagnostics[scenario] = f"retained real capture: {diagnostic}"
                    flow = json.loads(retained.read_text())
                else:
                    # No live capture and no retained real capture. The sibling
                    # ``FlowOutputContract`` in tests/test_upstream_matched_capture.py
                    # treats exactly this state as a FAILURE ("a missing capture is a
                    # FAILURE, not a silent skip"). Falling back to
                    # ``_authentic_flow`` here did the opposite: it handed
                    # source-anchored, never-executed bytes to assertions that
                    # demand ``evidence_class == "production_flow"`` and a real
                    # ``provenance.bun_version``, so the class reported PASS with
                    # zero genuine upstream captures. Failing closed keeps the
                    # tier impossible to mistake for a measurement.
                    raise AssertionError(
                        "no genuine upstream dialog capture for %r: bun=%r, live run "
                        "diagnostic=%r, and no retained capture at %s. Run the bounded "
                        "Bun adapter to produce one; a source-anchored flow is not "
                        "production evidence." % (
                            scenario, cls._runtime_available(), diagnostic, retained))
            cls.flows[scenario] = flow

    def test_permission_capture_is_genuine_production_evidence(self):
        flow = self.flows["form-permission-v1"]
        # The real validator on the real bytes.
        self.mod.validate_production_result(flow)
        self.assertEqual(flow["schema"], "oc2.ui-parity.screen.flow/v1")
        self.assertEqual(flow["evidence_class"], "production_flow")
        self.assertEqual(flow["scenario"], "form-permission-v1")
        self.assertEqual(flow["viewport"], {"cols": 80, "rows": 24})
        self.assertEqual(flow["provenance"]["bun_version"], BUN_RUNTIME)
        self.assertEqual(set(flow["provenance"]["verified_files"]), set(EXPECTED_FILES))
        self.assertEqual([frame["stage"] for frame in flow["frames"]], list(PERMISSION_STAGES))
        self.assertTrue(all(frame["screen"]["rows"] for frame in flow["frames"]))

    def test_permission_capture_shows_the_form_001_body_swap(self):
        flow = self.flows["form-permission-v1"]
        stages = _stage_text(flow)
        for marker in ("Permission required", "Allow once", "Always allow", "Reject", "→ Allow once"):
            self.assertIn(marker, stages["permission_initial"])
        # The swap must be a swap: the rules body is absent before the cycle...
        self.assertNotIn(ALWAYS_BODY_MARKER, stages["permission_initial"])
        # ...present while the prompt is still open after `l`...
        self.assertNotEqual(stages["permission_always_body"], stages["permission_initial"])
        self.assertIn("→ Always allow", stages["permission_always_body"])
        self.assertIn(ALWAYS_BODY_MARKER, stages["permission_always_body"])
        self.assertIn("- echo oracle", stages["permission_always_body"])
        # ...and gone once the reply resolved the prompt.
        for marker in ("Permission required", "Allow once", "Always allow", "Reject", ALWAYS_BODY_MARKER):
            self.assertNotIn(marker, stages["permission_submitted"])

    def test_permission_capture_carries_the_real_reply_wire_shape(self):
        flow = self.flows["form-permission-v1"]
        self.assertEqual(len(flow["replies"]), 1)
        reply = flow["replies"][0]
        self.assertEqual(reply["status"], 204)
        self.assertIn("/permission/", reply["path"])
        self.assertIn("/reply", reply["path"])
        self.assertIn('"reply":"always"', reply["body"])
        self.assertNotIn('"decision"', reply["body"])

    def test_permission_capture_actions_are_measured_replay(self):
        flow = self.flows["form-permission-v1"]
        for frame in flow["frames"]:
            screen = frame["screen"]
            self.assertEqual(screen["actions_source"], "upstream_input_replay_measured")
            self.assertEqual(screen["actions_cursor_after_source"], "measured_per_dispatch")
            self.assertEqual(screen["input_trace_source"], "upstream_input_replay_trace")
            self.assertIsNone(screen["semantic_actions"])
        actions = flow["frames"][-1]["screen"]["actions"]
        self.assertEqual([action["key"] for action in actions], ["l", "enter"])
        for action in actions:
            self.assertIn("seq", action)
            self.assertEqual(action["type"], "key")
            self.assertTrue(action["applied"])
            self.assertIn("row", action["cursor_after"])
            self.assertIn("col", action["cursor_after"])
            self.assertIn("visible", action["cursor_after"])
        for entry in flow["frames"][-1]["screen"]["input_trace"]:
            self.assertIn("measured_cursor", entry)

    def test_form_capture_is_genuine_production_evidence(self):
        flow = self.flows["form-multiselect-v1"]
        self.mod.validate_production_result(flow)
        self.assertEqual(flow["schema"], "oc2.ui-parity.screen.flow/v1")
        self.assertEqual(flow["evidence_class"], "production_flow")
        self.assertEqual(flow["scenario"], "form-multiselect-v1")
        self.assertEqual(flow["viewport"], {"cols": 80, "rows": 24})
        self.assertEqual(flow["provenance"]["bun_version"], BUN_RUNTIME)
        self.assertEqual(set(flow["provenance"]["verified_files"]), set(EXPECTED_FILES))
        self.assertEqual([frame["stage"] for frame in flow["frames"]], list(FORM_STAGES))
        self.assertTrue(all(frame["screen"]["rows"] for frame in flow["frames"]))
        self.assertEqual(flow["replies"], [], "a blocked submit must post nothing")

    def test_form_capture_shows_the_toggle_round_trip_on_real_bytes(self):
        flow = self.flows["form-multiselect-v1"]
        stages = _stage_text(flow)
        self.assertNotEqual(stages["form_required_selected"], stages["form_initial"])
        self.assertIn("[ ] Staging", stages["form_initial"])
        self.assertIn("[✓] Staging", stages["form_required_selected"])
        self.assertEqual(stages["form_required_cleared"], stages["form_initial"])

    def test_form_capture_blocked_submit_is_decisive_on_real_bytes(self):
        flow = self.flows["form-multiselect-v1"]
        stages = _stage_text(flow)
        # The pre-submit review surface already marks the required field...
        self.assertIn(REQUIRED_MARKER, stages["form_review"])
        self.assertIn(OPTIONAL_MARKER, stages["form_review"])
        # ...and the post-Enter frame carries a NEW submit-point occurrence.
        self.assertGreater(
            stages["form_submit_error"].count(REQUIRED_MARKER),
            stages["form_review"].count(REQUIRED_MARKER),
            "the submit-point validation error must be painted only at the submit point",
        )
        self.assertIn(OPTIONAL_MARKER, stages["form_submit_error"])

    def test_form_capture_actions_are_measured_replay(self):
        flow = self.flows["form-multiselect-v1"]
        for frame in flow["frames"]:
            screen = frame["screen"]
            self.assertEqual(screen["actions_source"], "upstream_input_replay_measured")
            self.assertEqual(screen["actions_cursor_after_source"], "measured_per_dispatch")
        actions = flow["frames"][-1]["screen"]["actions"]
        self.assertEqual(
            [action["key"] for action in actions], ["space", "space", "tab", "tab", "enter"])
        for action in actions:
            self.assertTrue(action["applied"])
            self.assertIn("cursor_after", action)

    def test_executed_capture_retention_matches_the_executed_scenario(self):
        # Retention is best-effort evidence export; when a retained file exists
        # it must be a genuine flow for the same scenario.
        for scenario in DIALOG_SCENARIOS:
            retained = RETAINED_DIR / f"{scenario}.flow.json"
            if retained.exists():
                flow = json.loads(retained.read_text())
                self.assertEqual(flow.get("schema"), "oc2.ui-parity.screen.flow/v1")
                self.assertEqual(flow.get("scenario"), scenario)


class RequirementAnchorContract(unittest.TestCase):
    """The coverage anchors are executable, not prose-only.

    The task brief cited ``resume-20261114/dialogs/confirmation-3319802.json``,
    which is not present in this checkout; the assertions delivered here are
    anchored to the in-workspace sources that DO exist, and this class verifies
    each anchor actually carries the quoted obligation.
    """

    def setUp(self):
        self.checklist = json.loads(CHECKLIST_PATH.read_text())
        self.audit = AUDIT_PATH.read_text()

    def _obligation(self, obligation_id):
        matches = [item for item in self.checklist["items"] if item.get("id") == obligation_id]
        self.assertTrue(matches, f"{obligation_id} missing from the kit checklist")
        return matches[0]

    def test_checklist_declares_the_permission_obligation(self):
        item = self._obligation("FORM-001")
        self.assertEqual(
            item["requirement"],
            "Permission once/always/reject variants; missing save rules omit always; "
            "changing selected option changes the body; nested rejection reason is "
            "correctly scoped.",
        )
        self.assertIn("permission-layout-79", item["fixture_ids"])

    def test_checklist_declares_the_form_obligation(self):
        item = self._obligation("FORM-003")
        self.assertEqual(
            item["requirement"],
            "Text/number/integer/boolean/single/multi/external/conditional/default "
            "fields, validation, review, and draft restoration behave identically.",
        )
        self.assertIn("form-multi", item["fixture_ids"])

    def test_audit_documents_the_permission_keymap_and_body_swap(self):
        # audit-messages.md §8 — the exact sentences the adapter journey relies
        # on: the cycle keys, the confirm key, and the always-body swap.
        self.assertIn("Permission required", self.audit)
        self.assertIn("Arrow-left/h and arrow-right/l cycle options; Enter confirms", self.audit)
        self.assertIn("Selecting always shows the rules it will persist", self.audit)
        self.assertIn("always available only when save rules exist", self.audit)

    def test_audit_documents_the_review_markers_and_validation_timing(self):
        # The pre-submit review marker and the submit-point validation timing
        # are what make the blocked-submit postcondition decisive.
        self.assertIn("marks invalid/required/missing external acknowledgements using error color", self.audit)
        self.assertIn("validation waits until the matching submit point", self.audit)
        self.assertIn("Space toggles multiselect but not single choice", self.audit)

    def test_captured_scenarios_exactly_the_two_named_journeys(self):
        # Coverage honesty: the dialog seam claims exactly the permission
        # journey and the required-multiselect journey — no invented extra
        # coverage beyond the task goal.
        self.assertEqual(set(_load().DIALOG_SCENARIOS), set(DIALOG_SCENARIOS))


class DialogAdapterSourceContract(unittest.TestCase):
    """Slim structural guard for the production dialog flow.

    The flow itself is executed by ``GenuineDialogCaptureContract``; this class
    only pins the properties that execution cannot observe per run: capability
    probes that fail closed, no fabricated pins/hashes, and the real reply key.
    """

    def setUp(self):
        self.source = ADAPTER_PATH.read_text()

    def test_dialog_flow_exists_and_is_dispatched(self):
        self.assertIn("async function runDialogFlow", self.source)
        self.assertIn("await runDialogFlow(request, fixture, cols, rows)", self.source)
        self.assertIn("await runProductionFlow(request, fixture, cols, rows)", self.source)

    def test_dialog_flow_mounts_the_real_fixture_and_event_stream(self):
        self.assertIn('await import("./test/fixture/app.ts")', self.source)
        self.assertIn("createAppFixture", self.source)
        self.assertIn("setup.events.emit", self.source)
        self.assertIn('"permission.asked"', self.source)
        self.assertIn('"form.created"', self.source)

    def test_dialog_flow_probes_the_fixture_capabilities_fail_closed(self):
        # Every fixture-API member beyond the proven home-flow surface is
        # verified before use: a wrong assumption is a precise diagnostic,
        # never a silent wrong capture.
        for code in ("fixture_entry_missing_createAppFixture", "fixture_mount_failed",
                     "fixture_events_channel_missing", "fixture_mock_input_missing",
                     "fixture_renderer_helpers_missing"):
            with self.subTest(code=code):
                self.assertIn(f'"{code}"', self.source)

    def test_dialog_flow_fails_closed_on_state_event_replay(self):
        # The dialog flow mirrors the home flow's fail-closed refusal of
        # state/event replay: the journey is produced by the events THIS flow
        # injects, so a request carrying its own events/state must be refused
        # by a directly-run adapter too, not silently ignored.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        self.assertIn('"unsupported_state_events"', dialog)

    def test_dialog_flow_uses_the_fixture_fetch_option_with_normalized_shapes(self):
        self.assertIn("fetch: async (url", self.source)
        self.assertIn('"/reply"', self.source)
        # URL object or string, Request or plain init — both normalized.
        self.assertIn("typeof url === \"string\"", self.source)
        self.assertIn("typeof request.clone === \"function\"", self.source)

    def test_dialog_flow_matches_the_real_permission_reply_route(self):
        # The recorded reply path must be anchored to the real production
        # endpoint -- `POST /api/session/{id}/permission/{request_id}/reply`
        # (web/src/lib/api.ts; mounted at crates/server/src/lib.rs) -- not a
        # loose substring. This is what makes a captured reply proof that the
        # production transport was exercised.
        self.assertIn(
            r"/^\/api\/session\/[^/]+\/permission\/[^/]+\/reply$/",
            self.source,
        )

    def test_dialog_flow_intercepts_the_global_transport_and_restores_it(self):
        # The mounted app may reach the network through `globalThis.fetch`
        # instead of the injected option, so the same router is installed on
        # both surfaces: the reply/cancel mutation is recorded whichever route
        # the production code takes.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        self.assertIn("const originalFetch = globalThis.fetch", dialog)
        self.assertIn("globalThis.fetch = globalFetch", dialog)
        # Installed before the fixture module is imported/mounted, so no startup
        # request can slip past the interceptor.
        self.assertLess(
            dialog.index("globalThis.fetch = globalFetch"),
            dialog.index('await import("./test/fixture/app.ts")'),
            "the interceptor must be installed before the fixture is imported",
        )
        # Restored on the mount-failure paths and in the teardown finally.
        self.assertGreaterEqual(dialog.count("restoreFetch()"), 3)

    def test_dialog_flow_never_resolves_unknown_requests_to_undefined(self):
        # A fetch resolving to `undefined` becomes an opaque client failure that
        # would mask the dialog stage: the fixture transport answers unknown
        # requests with an explicit 404 (never the network), while the global
        # transport delegates to the original fetch so the capture never takes
        # over unrelated process-wide requests.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        self.assertNotIn("return undefined", dialog)
        self.assertIn("status: 404", dialog)
        # The fixture transport is hermetic (404, never the network)...
        self.assertIn(
            "const fixtureFetch = async (url: any, input: any) => (await route(url, input)) ?? notFound(url)",
            dialog,
        )
        # ...while the global transport delegates anything it does not own.
        self.assertIn("const globalFetch", dialog)
        self.assertIn("await originalFetch(url, input)", dialog)

    def test_multiselect_journey_toggles_through_the_real_keymap(self):
        # Two real space presses (select then deselect) and two real tabs into
        # the review tab, then a real Enter; no modifier-only key press.
        self.assertEqual(self.source.count('await press("space")'), 2)
        self.assertEqual(self.source.count('await press("tab")'), 2)
        self.assertIn("await pressEnter()", self.source)
        self.assertIn("setup.mockInput.pressKey", self.source)
        self.assertIn("setup.mockInput.pressEnter", self.source)
        self.assertNotIn('pressKey("shift"', self.source)

    def test_permission_journey_cycles_the_option_through_the_real_keymap(self):
        # audit-messages.md §8: "Arrow-left/h and arrow-right/l cycle options;
        # Enter confirms." The always selection is a real key dispatch, not a
        # fabricated state override.
        self.assertEqual(self.source.count('await press("l")'), 1)

    def test_dialog_flow_declares_the_measurable_stages(self):
        for stage in (*PERMISSION_STAGES, *FORM_STAGES):
            with self.subTest(stage=stage):
                self.assertIn(f'"{stage}"', self.source)

    def test_dialog_frames_declare_the_measured_cursor_source(self):
        # Parity with the home flow: a captured dialog frame states that its
        # cursor comes from the renderer's own captured frame, not an inferred
        # or synthesized position. The home-flow declaration sits before the
        # dialog function, so slicing the dialog body proves the dialog flow
        # carries its own.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        self.assertIn('screen.cursor_source = "renderer_captured_frame"', dialog)

    def test_permission_body_swap_is_captured_before_submit(self):
        # FORM-001: the always-body frame must be captured while the prompt is
        # still open. If it were captured after Enter it would only show the
        # post-submit surface and the swap would never be exercised.
        cycle = self.source.index('await press("l")')
        swap = self.source.index('capture("permission_always_body")')
        submit = self.source.index("await pressEnter()")
        self.assertLess(cycle, swap, "the option must be cycled before the swap frame")
        self.assertLess(swap, submit, "the swap frame must be captured before Enter")

    def test_form_review_is_captured_before_the_submit_attempt(self):
        # The decisive blocked-submit postcondition needs the PRE-submit review
        # frame: the submit-point error is a NEW occurrence only relative to it.
        review = self.source.index('capture("form_review")')
        form_enter = self.source.index("await pressEnter()", review)
        self.assertLess(review, form_enter, "the review frame must precede Enter")

    def test_form_submit_wait_uses_the_occurrence_differential(self):
        # The readiness gate itself is the differential, not a bare marker.
        self.assertIn('f.split("Answer required").length - 1 > reviewCount', self.source)

    def test_dialog_flow_fails_closed_on_its_postconditions(self):
        for code in ("waitForFrame", "permission_prompt_not_observed", "permission_initial_selected_option",
                     "permission_always_option_not_selected", "permission_initial_option_order",
                     "form_prompt_not_observed", "form_required_initial_check_missing",
                     "permission_reply_not_observed", "permission_reply_not_successful",
                     "permission_reply_path_invalid",
                     "permission_always_body_pre_painted", "permission_always_body_missing",
                     "permission_body_swap_not_observed", "permission_prompt_still_open",
                     "form_reply_unexpectedly_observed", "form_required_error_missing",
                     "form_optional_control_missing", "form_required_toggle_not_observed",
                     "form_required_toggle_not_reversible", "form_required_check_not_painted",
                     "form_submit_attempt_not_observed"):
            with self.subTest(code=code):
                self.assertIn(f'"{code}"', self.source)

    def test_dialog_flow_embeds_no_fabricated_pins_or_hashes(self):
        self.assertNotIn(UPSTREAM_COMMIT, self.source)
        self.assertNotIn(OPENTUI_COMMIT, self.source)
        self.assertNotIn("a2013c73", self.source)

    def test_dialog_flow_asserts_the_reply_wire_key(self):
        # The decision key is `reply`; the adapter must assert it and must never
        # fabricate the non-existent `decision` key.
        self.assertIn('"reply":"always"', self.source)
        self.assertNotIn('"decision"', self.source)

    def test_dialog_flow_records_measured_cursor_per_dispatch(self):
        self.assertIn("measured_cursor", self.source)
        self.assertIn("waitForVisualIdle", self.source)

    def test_dialog_flow_snapshots_replies_before_teardown(self):
        # The returned object is serialized AFTER the finally-block teardown, so
        # each journey result must carry its own reply snapshot. A disposal-time
        # cancel routed through the fixture transport would otherwise mutate the
        # same array before `main` writes the evidence, turning a genuine
        # single-reply permission journey into a false reply-count failure.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        self.assertEqual(dialog.count("replies: [...replies]"), 2)
        self.assertNotIn(", replies }", dialog)

    def test_dialog_flow_teardown_cannot_discard_measured_evidence(self):
        # The measured frames and the reply snapshot are produced before the
        # `finally` runs. A disposal failure therefore must not discard a
        # successful capture (it would turn genuine evidence into a protocol
        # failure that hides the measured bytes), and the disposal capability
        # itself must be probed so a missing member is a precise diagnostic.
        dialog = self.source[
            self.source.index("async function runDialogFlow"):self.source.index("async function main")
        ]
        dispose = dialog.index("await setup[Symbol.asyncDispose]()")
        self.assertIn("catch", dialog[dispose:dispose + 200])
        self.assertIn(
            '"asyncDispose": typeof (setup as any)?.[Symbol.asyncDispose]', dialog)


class PortedDialogSourceAnchorContract(unittest.TestCase):
    """The captured markers are anchored to the in-workspace ported sources.

    The executed journey assertions hardcode the permission option labels/order,
    the always-body bullet format, the reply wire keys, and the form
    review/checkbox markers. Those literals are genuine only while the ported
    product render (the Rust parity target the capture is compared against)
    still emits them, so each is pinned to its exact source here. A drift in the
    ported render is then a failure in this file rather than a silently wrong
    capture that quietly stops matching the product.
    """

    def setUp(self):
        self.permissions = (ROOT / "crates/cli/src/native_permissions.rs").read_text()
        self.dialogs = (ROOT / "crates/cli/src/native_view_dialogs.rs").read_text()

    def test_permission_option_labels_and_order_are_anchored(self):
        for label in ("Allow once", "Always allow", "Reject"):
            with self.subTest(label=label):
                self.assertIn(f'"{label}"', self.permissions)
        self.assertIn(
            "vec![PermissionOption::Once, PermissionOption::Always, PermissionOption::Reject]",
            self.permissions)

    def test_always_body_sentence_and_bullet_are_anchored(self):
        self.assertIn(ALWAYS_BODY_MARKER, self.permissions)
        self.assertIn('format!("- {item}")', self.permissions)

    def test_permission_reply_wire_keys_are_anchored(self):
        # The reply body the client POSTs is built with the `reply`/`scope`
        # keys (permission_reply_payload, native_permissions.rs:560-569). Anchor
        # the exact payload literal so a rename to the non-existent `decision`
        # key would break this contract instead of silently changing the wire.
        self.assertIn('serde_json::json!({ "reply": reply, "scope": scope })', self.permissions)

    def test_form_review_and_checkbox_markers_are_anchored(self):
        self.assertIn('field.required.then(|| "Answer required"', self.dialogs)
        self.assertIn('("(not answered)".to_string()', self.dialogs)
        self.assertIn('if picked { "✓" } else { " " }', self.dialogs)


if __name__ == "__main__":
    unittest.main()
