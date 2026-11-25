"""A live check must survive removal of the worktree the controller loaded from.

The guardian was executed via `pathlib.Path(__file__).with_name(...)`, so a job
running out of a worktree that the controller reclaimed mid-flight lost the
guardian and every command failed with FileNotFoundError. That failure set
`stop_new_publications`, which gates every `implement` and `publish` fleet-wide,
so one cleanup pass idled the entire fleet. The source is now materialised at
import, which this pins.
"""
import importlib.util
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[2] / "tools" / "delivery"


class GuardSurvivesSourceRemoval(unittest.TestCase):
    def test_run_still_executes_after_its_directory_is_deleted(self):
        with tempfile.TemporaryDirectory() as tmp:
            staged = Path(tmp) / "delivery"
            staged.mkdir()
            for name in ("runtime.py", "process_guard.py"):
                shutil.copy2(SOURCE / name, staged / name)
            spec = importlib.util.spec_from_file_location("guard_survival_runtime", staged / "runtime.py")
            runtime = importlib.util.module_from_spec(spec)
            sys.modules["guard_survival_runtime"] = runtime
            spec.loader.exec_module(runtime)
            shutil.rmtree(staged)
            self.assertFalse(staged.exists())
            result = runtime.run([sys.executable, "-c", "print('alive')"], tmp, timeout=60)
            self.assertEqual(result["returncode"], 0)
            self.assertEqual(result["output"].strip(), "alive")


if __name__ == "__main__":
    unittest.main()
