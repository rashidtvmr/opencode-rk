"""Static RED contract for source-bound final-release platform journeys.

This test deliberately performs no platform work.  It reads the tracked release
template and gate driver only; the platform journeys themselves remain the
frozen, separately owned contracts.  The assertions describe the invocation
boundary that a final exact-SHA release must provide to those journeys.
"""

import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "release" / "release-gates.manifest.template.json"
DRIVER = ROOT / "release" / "run_release_gates.py"


def _template():
    return json.loads(TEMPLATE.read_text(encoding="utf-8"))


def _gate(manifest, platform, label):
    return next(g for g in manifest["platforms"][platform]["gates"] if g["label"] == label)


G6_FROZEN = {
    "tests/e2e/web_browser_gate.py": "0bfcaab9dbb77a876a7f3242abc681be31328687e9e4e41c39a36cdadab8a87f",
    "tests/e2e/web_tool_journey_fixture.py": "d950994a50e9edd9b251c310c0c3a2f176774a509271158cc1343ffb8926a73e",
    "tests/e2e/web_browser_journey.mjs": "b8e4efa6c00cd269c8a95d802eddad1ba88b4091670ff1327e65057d93eef000",
}
G7_PATH = "tests/e2e/ubuntu_installed_golden.py"
G7_SHA = "f40ac734511173a0e1c0bbec47ac7d730b7ce3e9af7bd6ce49ade3e41b35b5af"


class ReleaseRuntimeBindingTest(unittest.TestCase):
    def test_web_final_gate_is_actual_browser_journey_with_exact_sha_bindings(self):
        manifest = _template()
        gate = _gate(manifest, "web", "web-journey-real-router")
        command = gate["command"]

        self.assertIn("{root}/tests/e2e/web_browser_gate.py", command)
        self.assertNotIn("tests/e2e/web_tool_journey_fixture.py", command)
        for token in ("--source-sha", "{sha}", "--binary", "{artifact}/install/bin/oc2",
                      "--build-json", "{artifact}/build.json", "--artifact-dir",
                      "{evidence}/journey"):
            self.assertIn(token, command)
        self.assertEqual(manifest["artifact_contract"]["frozen_tests"],
                         {**manifest["artifact_contract"]["frozen_tests"], **G6_FROZEN})
        for path, digest in G6_FROZEN.items():
            self.assertEqual(manifest["artifact_contract"]["frozen_tests"].get(path), digest)

    def test_macos_loader_gate_binds_attested_installed_payload(self):
        manifest = _template()
        gate = _gate(manifest, "mac", "macos-loader")
        command = gate["command"]
        env = gate.get("env", {})

        self.assertIn("--test", command)
        self.assertIn("macos_loader", command)
        self.assertIn("--test-threads=1", command)
        self.assertEqual(env.get("OC2_NATIVE_BINARY"), "{artifact}/install/bin/oc2")
        self.assertEqual(
            env.get("MAC_OPENTUI_FIXTURE"),
            "{artifact}/install/lib/libopentui.dylib",
        )

    def test_ubuntu_final_gate_runs_installed_golden_with_all_sha_bindings(self):
        manifest = _template()
        commands = [g["command"] for g in manifest["platforms"]["ubuntu"]["gates"]]
        command = [token for row in commands for token in row]

        self.assertIn("{root}/tests/e2e/ubuntu_installed_golden.py", command)
        for token in (
            "--archive", "{artifact}/oc2-linux-arm64.tar.gz", "--checksum",
            "{archive_sha256}", "--build-json", "{artifact}/build.json",
            "--artifact-dir", "{evidence}/ubuntu-installed-golden",
            "--expected-source-sha", "{sha}",
        ):
            self.assertIn(token, command)
        self.assertEqual(manifest["artifact_contract"]["frozen_tests"].get(G7_PATH), G7_SHA)


if __name__ == "__main__":
    unittest.main()
