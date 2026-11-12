import json
import unittest
from pathlib import Path


class Par03CaptureContract(unittest.TestCase):
    def test_driver_uses_shared_fixture_and_portable_screen_schema(self):
        source = (Path(__file__).parents[1] / "tools/ui_parity/par03_capture.py").read_text()
        self.assertIn('"oc2.ui-parity.request/v1"', source)
        self.assertIn('"oc2.ui-parity.screen/v1"', source)
        self.assertIn('"upstream"', source)
        self.assertIn('"rust"', source)

    def test_fixture_is_deterministic(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/ui-parity/cases/par-static-home-80x24-dark.json").read_text())
        self.assertEqual(fixture["events"], [])
        self.assertEqual(fixture["clock_ms"], 0)
        self.assertEqual(fixture["viewport"], {"columns": 80, "rows": 24})

    def test_commands_surface_requires_styled_cursor_focus_and_trace_capture(self):
        """Keep one interactive surface tied to the full capture contract.

        This validates the fixture specification only; it deliberately does not
        manufacture or refresh a golden. The real PTY adapter and comparator
        remain the authority for the rendered result.
        """
        path = Path(__file__).parent / "fixtures/ui-parity/cases/par-static-commands-120x40-dark.json"
        fixture = json.loads(path.read_text())
        self.assertEqual(fixture["id"], "par-static-commands-120x40-dark")
        self.assertEqual(fixture["events"], [
            {"type": "wait", "ms": 300},
            {"type": "key", "key": "ctrl-k"},
            {"type": "wait", "ms": 500},
        ])
        self.assertEqual(
            fixture["compare"],
            ["styled_cells", "cursor", "layout_and_clipping", "focus", "semantic_actions", "hit_targets"],
        )
        self.assertIn("DIALOG-11", fixture["checklist_ids"])
        self.assertEqual(fixture["viewport"], {"columns": 120, "rows": 40})


if __name__ == "__main__":
    unittest.main()
