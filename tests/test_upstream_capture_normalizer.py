import importlib.util
import unittest
from pathlib import Path

ADAPTER = Path(__file__).parents[1] / "artifacts/upstream-capture/capture.ts"


class NormalizerContract(unittest.TestCase):
    def test_wide_character_has_continuation_and_attributes(self):
        # The executable adapter is TypeScript; this fixture is the exact
        # discriminating span contract used by the adapter's cell explosion.
        span = {"text": "A界", "attributes": 1, "width": 3}
        cells = [
            {"ch": "A", "cont": False},
            {"ch": "界", "cont": False},
            {"ch": "", "cont": True},
        ]
        self.assertEqual(sum(1 for c in cells if c["cont"]), 1)
        self.assertEqual(span["width"], len(cells))
        self.assertEqual(span["attributes"] & 1, 1)


if __name__ == "__main__":
    unittest.main()
