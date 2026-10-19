"""Regression tests for cleanup-safe second-client overlap callbacks.

These tests exercise only the fixture's callback seam. They do not launch the
installed client, a provider, a PTY, or a Cargo build.
"""

import pathlib
import sys
import tempfile
import unittest
from unittest import mock


E2E_DIR = pathlib.Path(__file__).resolve().parents[1] / "e2e"
if str(E2E_DIR) not in sys.path:
    sys.path.insert(0, str(E2E_DIR))

import native_second_client as probe  # noqa: E402


class CallbackEventCleanupTests(unittest.TestCase):
    def test_legacy_callback_without_readiness_is_still_one_shot(self):
        calls = []
        event = probe.CallbackEvent(lambda: calls.append("called"))

        event.set()
        event.set()

        self.assertEqual(calls, ["called"])
        self.assertTrue(event.is_set())

    def test_cleanup_signal_sets_event_without_consuming_callback(self):
        calls = []
        event = probe.CallbackEvent(lambda: calls.append("called"), ready=lambda: False)

        event.set()

        self.assertTrue(event.is_set())
        self.assertEqual(calls, [])
        self.assertFalse(event.callback_used)

    def test_ready_signal_invokes_callback_once(self):
        calls = []
        event = probe.CallbackEvent(lambda: calls.append("called"), ready=lambda: True)

        event.set()
        event.set()

        self.assertEqual(calls, ["called"])
        self.assertTrue(event.callback_used)
        self.assertTrue(event.is_set())

    def test_cleanup_then_valid_overlap_still_invokes_callback_once(self):
        calls = []
        state = {"valid_overlap": False}
        event = probe.CallbackEvent(
            lambda: calls.append("called"), ready=lambda: state["valid_overlap"]
        )

        event.set()
        self.assertFalse(event.callback_used)
        event.clear()
        state["valid_overlap"] = True
        event.set()
        event.set()

        self.assertEqual(calls, ["called"])
        self.assertTrue(event.callback_used)

    def test_callback_error_is_recorded_and_does_not_escape_set(self):
        expected = RuntimeError("overlap callback failed")

        def fail():
            raise expected

        event = probe.CallbackEvent(fail, ready=lambda: True)
        event.set()

        self.assertTrue(event.is_set())
        self.assertIs(event.callback_error, expected)

    def test_run_reports_callback_error_only_after_golden_cleanup(self):
        expected = RuntimeError("overlap callback failed")
        state_box = {}
        run_finished = []

        def fake_golden_run(*_args):
            state = probe.golden.ProviderState(pathlib.Path("/disposable/project"))
            state_box["state"] = state
            state.phase = "waiting_partial_observation"
            state.requests.append({"fixture": "first request"})
            try:
                state.partial_visible.set()
            except RuntimeError:
                # The underlying golden journey records its own failure and
                # continues into finally; callback failures should be deferred
                # to the wrapper instead of aborting this cleanup path.
                pass
            finally:
                state.phase = "idle"
                state.release_partial.set()
                run_finished.append("cleanup completed")

        with tempfile.TemporaryDirectory() as temp:
            artifacts = pathlib.Path(temp)
            with (
                mock.patch.object(
                    probe,
                    "checked_artifact",
                    return_value=(pathlib.Path("/bin/oc2"), pathlib.Path("/lib/native"), "c" * 40, "a" * 64, "b" * 64),
                ),
                mock.patch.object(probe, "run_second_client", side_effect=expected),
                mock.patch.object(probe.golden, "run", side_effect=fake_golden_run),
            ):
                with self.assertRaises(RuntimeError) as raised:
                    probe.run("binary", "library", "manifest", artifacts)

        self.assertEqual(run_finished, ["cleanup completed"])
        self.assertTrue(state_box["state"].partial_visible.is_set())
        self.assertIs(raised.exception, expected)

    def test_run_fails_when_golden_journey_never_observes_overlap(self):
        def fake_golden_run(*_args):
            probe.golden.ProviderState(pathlib.Path("/disposable/project"))

        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(
                    probe,
                    "checked_artifact",
                    return_value=(pathlib.Path("/bin/oc2"), pathlib.Path("/lib/native"), "c" * 40, "a" * 64, "b" * 64),
                ),
                mock.patch.object(probe.golden, "run", side_effect=fake_golden_run),
            ):
                with self.assertRaisesRegex(
                    RuntimeError, "second-client overlap probe did not run exactly once"
                ):
                    probe.run("binary", "library", "manifest", pathlib.Path(temp))


if __name__ == "__main__":
    unittest.main()
