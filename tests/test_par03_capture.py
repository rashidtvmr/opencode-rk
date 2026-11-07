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


if __name__ == "__main__":
    unittest.main()
