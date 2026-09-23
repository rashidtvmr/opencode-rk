"""Offline semantic RED contract for PROV-024 provider fixtures.

The validator is deliberately test-local.  It specifies the fixture boundary
without adding a product parser, network client, or live credential path.
"""

from __future__ import annotations

import copy
import json
import os
import pathlib
import re
import socket
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURE_ROOT = pathlib.Path(
    os.environ.get("PROV024_FIXTURE_ROOT", ROOT / "fixtures" / "provider_contracts")
)
CATALOG = ROOT / "docs" / "provider-compatibility.json"
PROVIDERS = ("openai", "anthropic", "google")
FILES = tuple(
    f"{provider}/{name}.json"
    for provider in PROVIDERS
    for name in ("request", "auth_state")
)
FIXTURE_IMPLEMENTATION_ORDER = (
    "manifest.json",
    "openai/request.json",
    "openai/auth_state.json",
    "anthropic/request.json",
    "anthropic/auth_state.json",
    "google/request.json",
    "google/auth_state.json",
)
MAX_FILE_BYTES = 16 * 1024
MAX_DIRECTORY_BYTES = 128 * 1024
MAX_HEADERS = 16


class FixtureValidationError(ValueError):
    def __init__(self, code: str, detail: str) -> None:
        self.code = code
        super().__init__(f"{code}: {detail}")


class UnknownProvider(FixtureValidationError):
    def __init__(self, provider: str) -> None:
        super().__init__("UnknownProvider", provider)


def _fail(code: str, detail: str) -> None:
    raise FixtureValidationError(code, detail)


def _reject_constant(value: str) -> None:
    raise ValueError(f"non-finite JSON constant: {value}")


def _unique_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _load_json(path: pathlib.Path) -> object:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        _fail("bad-fixture", f"cannot read {path.name}: {exc}")
    if len(raw) > MAX_FILE_BYTES:
        _fail("over-cap", path.name)
    try:
        text = raw.decode("utf-8")
        return json.loads(
            text,
            object_pairs_hook=_unique_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as exc:
        _fail("bad-fixture", f"invalid JSON {path.name}: {exc}")


def _assert_sorted_keys(value: object, path: str = "$") -> None:
    if isinstance(value, dict):
        keys = list(value)
        if keys != sorted(keys):
            _fail("bad-fixture", f"non-canonical key order at {path}")
        for key, child in value.items():
            _assert_sorted_keys(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _assert_sorted_keys(child, f"{path}[{index}]")


def _canonical_json(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


SECRET_PATTERNS = (
    re.compile(r"sk-(?:ant-)?[A-Za-z0-9][A-Za-z0-9._~+/=-]*"),
    re.compile(r"(?i)\bbearer\s+\S+"),
    re.compile(r'"(?:access_token|refresh_token)"\s*:', re.IGNORECASE),
    re.compile(r"-----BEGIN [^-]*PRIVATE KEY-----"),
)


def _scan_secret_bytes(raw: bytes, relative: str) -> None:
    text = raw.decode("utf-8", errors="replace")
    if any(pattern.search(text) for pattern in SECRET_PATTERNS):
        _fail("secret-leak", relative)


def _reject_secret_fields(value: object, path: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if re.search(
                r"authorization|(?:access|refresh)?[_-]?token|(?:api[_-]?key)|secret|credential",
                key,
                re.IGNORECASE,
            ):
                if child != "REDACTED":
                    _fail("secret-leak", f"{path}.{key}")
            _reject_secret_fields(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_secret_fields(child, f"{path}[{index}]")


def _catalog_provider_ids() -> set[str]:
    catalog = _load_json(CATALOG)
    if not isinstance(catalog, dict) or not isinstance(catalog.get("providers"), list):
        _fail("bad-fixture", "provider compatibility catalog")
    return {
        item["id"]
        for item in catalog["providers"]
        if isinstance(item, dict) and isinstance(item.get("id"), str)
    }


def _require_https(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.startswith("https://"):
        _fail("bad-endpoint", label)
    if any(character in value for character in "\r\n"):
        _fail("bad-endpoint", label)
    return value


def _validate_request(provider: str, value: object) -> dict[str, object]:
    if not isinstance(value, dict) or value.get("provider") != provider:
        _fail("bad-fixture", f"{provider}/request.json provider")
    if set(value) != {"body_schema_ref", "headers", "method", "provider", "url_template"}:
        _fail("bad-fixture", f"{provider}/request.json shape")
    if value.get("method") != "POST":
        _fail("bad-fixture", f"{provider}/request.json method")
    _require_https(value.get("url_template"), f"{provider}/request.json url_template")
    schema = value.get("body_schema_ref")
    if not isinstance(schema, str) or not schema.startswith(f"{provider}."):
        _fail("bad-fixture", f"{provider}/request.json body schema")
    headers = value.get("headers")
    if not isinstance(headers, dict) or not headers or len(headers) > MAX_HEADERS:
        _fail("over-cap", f"{provider}/request.json headers")
    if headers.get("Content-Type") != "application/json":
        _fail("bad-fixture", f"{provider}/request.json content type")
    secret_headers = {
        "openai": {"Authorization"},
        "anthropic": {"x-api-key"},
        "google": {"x-goog-api-key"},
    }[provider]
    for name in secret_headers:
        if headers.get(name) != "REDACTED":
            _fail("secret-leak", f"{provider}/request.json header {name}")
    catalog_ids = _catalog_provider_ids()
    if provider not in catalog_ids:
        _fail("bad-fixture", f"undocumented provider {provider}")
    for name, header_value in headers.items():
        if not isinstance(name, str) or not name or "\r" in name or "\n" in name:
            _fail("bad-fixture", f"{provider}/request.json header name")
        if not isinstance(header_value, str):
            _fail("bad-fixture", f"{provider}/request.json header value")
        if re.search(r"(?:authorization|api[-_]?key|secret|token)", name, re.IGNORECASE):
            if header_value != "REDACTED":
                _fail("secret-leak", f"{provider}/request.json header {name}")
    _reject_secret_fields(value, f"{provider}/request.json")
    return value


def _validate_auth_state(provider: str, value: object) -> dict[str, object]:
    if not isinstance(value, dict) or value.get("provider") != provider:
        _fail("bad-fixture", f"{provider}/auth_state.json provider")
    states = value.get("states")
    if not isinstance(states, list) or len(states) != 4:
        _fail("bad-fixture", f"{provider}/auth_state.json lifecycle")
    by_state: dict[str, dict[str, object]] = {}
    for state in states:
        if not isinstance(state, dict) or not isinstance(state.get("state"), str):
            _fail("bad-fixture", f"{provider}/auth_state.json state")
        name = state["state"]
        if name in by_state:
            _fail("bad-fixture", f"duplicate auth state {name}")
        by_state[name] = state
    if set(by_state) != {"logged-out", "pending-consent", "ready", "expired"}:
        _fail("bad-fixture", f"{provider}/auth_state.json lifecycle")
    if set(by_state["logged-out"]) != {"state"}:
        _fail("bad-fixture", f"{provider}/auth_state.json logged-out shape")
    pending = by_state["pending-consent"]
    if set(pending) != {"consent_url", "device_code", "state"}:
        _fail("bad-fixture", f"{provider}/auth_state.json pending shape")
    _require_https(pending["consent_url"], f"{provider}/auth_state.json consent_url")
    if pending["device_code"] != "REDACTED":
        _fail("secret-leak", f"{provider}/auth_state.json device_code")
    for name in ("ready", "expired"):
        state = by_state[name]
        if set(state) != {"expires_at", "state"}:
            _fail("bad-fixture", f"{provider}/auth_state.json {name} shape")
        if not isinstance(state["expires_at"], str) or not re.fullmatch(
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", state["expires_at"]
        ):
            _fail("bad-fixture", f"{provider}/auth_state.json {name} expiry")
    _reject_secret_fields(value, f"{provider}/auth_state.json")
    return value


def validate_fixture_dir(root: pathlib.Path) -> dict[str, dict[str, object]]:
    """Validate one bounded, static fixture bundle without I/O beyond reads."""
    if not root.is_dir():
        _fail("bad-fixture", "missing provider_contracts directory")
    files = sorted(path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file())
    expected = sorted(("manifest.json",) + FILES)
    if files != expected:
        _fail("bad-fixture", "exact file inventory")
    total = 0
    for relative in expected:
        path = root / relative
        if path.is_symlink():
            _fail("bad-fixture", f"symlink {relative}")
        size = path.stat().st_size
        total += size
        if size > MAX_FILE_BYTES:
            _fail("over-cap", relative)
        _scan_secret_bytes(path.read_bytes(), relative)
    if total > MAX_DIRECTORY_BYTES:
        _fail("over-cap", "provider_contracts directory")
    manifest = _load_json(root / "manifest.json")
    if not isinstance(manifest, dict) or manifest.get("version") != "1":
        _fail("bad-fixture", "manifest version")
    if manifest.get("providers") != sorted(PROVIDERS):
        _fail("bad-fixture", "manifest providers")
    if manifest.get("files") != list(sorted(FILES)):
        _fail("bad-fixture", "manifest file inventory")
    _assert_sorted_keys(manifest)
    result: dict[str, dict[str, object]] = {}
    for provider in PROVIDERS:
        request = _load_json(root / provider / "request.json")
        auth_state = _load_json(root / provider / "auth_state.json")
        _assert_sorted_keys(request)
        _assert_sorted_keys(auth_state)
        result[provider] = {
            "request": _validate_request(provider, request),
            "auth_state": _validate_auth_state(provider, auth_state),
        }
    return result


def diff_shapes(root: pathlib.Path, left: str, right: str) -> dict[str, object]:
    if left not in PROVIDERS:
        raise UnknownProvider(left)
    if right not in PROVIDERS:
        raise UnknownProvider(right)
    fixtures = validate_fixture_dir(root)
    left_request = fixtures[left]["request"]
    right_request = fixtures[right]["request"]
    assert isinstance(left_request, dict)
    assert isinstance(right_request, dict)
    left_headers = set(left_request["headers"])
    right_headers = set(right_request["headers"])
    return {
        "same_endpoint_shape": (
            left_request["method"] == right_request["method"]
            and left_request["url_template"] == right_request["url_template"]
        ),
        "header_name_diff": sorted(left_headers ^ right_headers),
        "notes": [f"compared:{left},{right}"],
    }


def _write_generated_bundle(root: pathlib.Path) -> None:
    requests = {
        "openai": {
            "body_schema_ref": "openai.chat-completions.request.v1",
            "headers": {"Authorization": "REDACTED", "Content-Type": "application/json"},
            "method": "POST",
            "provider": "openai",
            "url_template": "https://api.openai.com/v1/chat/completions",
        },
        "anthropic": {
            "body_schema_ref": "anthropic.messages.request.v1",
            "headers": {
                "Content-Type": "application/json",
                "anthropic-version": "2023-06-01",
                "x-api-key": "REDACTED",
            },
            "method": "POST",
            "provider": "anthropic",
            "url_template": "https://api.anthropic.com/v1/messages",
        },
        "google": {
            "body_schema_ref": "google.generate-content.request.v1",
            "headers": {"Content-Type": "application/json", "x-goog-api-key": "REDACTED"},
            "method": "POST",
            "provider": "google",
            "url_template": "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
        },
    }
    auth = {
        provider: {
            "provider": provider,
            "states": [
                {"state": "logged-out"},
                {
                    "consent_url": "https://example.invalid/authorize",
                    "device_code": "REDACTED",
                    "state": "pending-consent",
                },
                {"expires_at": "2030-01-01T00:00:00Z", "state": "ready"},
                {"expires_at": "2020-01-01T00:00:00Z", "state": "expired"},
            ],
        }
        for provider in PROVIDERS
    }
    values: dict[str, object] = {
        "manifest.json": {"files": list(sorted(FILES)), "providers": list(sorted(PROVIDERS)), "version": "1"}
    }
    values.update({f"{provider}/request.json": request for provider, request in requests.items()})
    values.update({f"{provider}/auth_state.json": state for provider, state in auth.items()})
    for relative, value in values.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


class ProviderContractFixtureTests(unittest.TestCase):
    def test_repository_manifest_and_exact_inventory(self) -> None:
        fixtures = validate_fixture_dir(FIXTURE_ROOT)
        self.assertEqual(set(fixtures), set(PROVIDERS))
        self.assertIn("google", _catalog_provider_ids())

    def test_documented_requests_and_auth_lifecycle_are_redacted(self) -> None:
        fixtures = validate_fixture_dir(FIXTURE_ROOT)
        for provider in PROVIDERS:
            request = fixtures[provider]["request"]
            auth_state = fixtures[provider]["auth_state"]
            self.assertEqual(request["provider"], provider)
            self.assertEqual(auth_state["provider"], provider)
            self.assertNotIn("refresh_token", json.dumps(auth_state))
            self.assertNotIn("access_token", json.dumps(auth_state))

    def test_differential_is_deterministic_and_shape_only(self) -> None:
        first = diff_shapes(FIXTURE_ROOT, "openai", "anthropic")
        second = diff_shapes(FIXTURE_ROOT, "openai", "anthropic")
        self.assertFalse(first["same_endpoint_shape"])
        self.assertTrue(first["header_name_diff"])
        self.assertEqual(_canonical_json(first), _canonical_json(second))
        self.assertNotIn("REDACTED", json.dumps(first))

    def test_unknown_provider_is_explicit_and_offline(self) -> None:
        with self.assertRaises(UnknownProvider) as raised:
            diff_shapes(FIXTURE_ROOT, "openai", "not-supported")
        self.assertEqual(raised.exception.code, "UnknownProvider")
        with mock.patch.object(socket, "socket", side_effect=AssertionError("network used")):
            validate_fixture_dir(FIXTURE_ROOT)
            diff_shapes(FIXTURE_ROOT, "openai", "anthropic")

    def test_bounds_and_canonical_key_order(self) -> None:
        fixtures = validate_fixture_dir(FIXTURE_ROOT)
        self.assertLessEqual(
            sum(path.stat().st_size for path in FIXTURE_ROOT.rglob("*") if path.is_file()),
            MAX_DIRECTORY_BYTES,
        )
        self.assertEqual(_canonical_json(fixtures), _canonical_json(copy.deepcopy(fixtures)))

    def test_generated_negative_fixtures_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory) / "provider_contracts"
            _write_generated_bundle(root)
            validate_fixture_dir(root)

            manifest = root / "manifest.json"
            manifest_value = json.loads(manifest.read_text())
            manifest_value["version"] = "2"
            manifest.write_text(json.dumps(manifest_value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(FixtureValidationError, r"^bad-fixture:"):
                validate_fixture_dir(root)
            manifest_value["version"] = "1"
            manifest.write_text(json.dumps(manifest_value, indent=2, sort_keys=True) + "\n")

            bad_endpoint = root / "openai" / "request.json"
            request = json.loads(bad_endpoint.read_text())
            request["url_template"] = "http://api.openai.com/v1/chat/completions"
            bad_endpoint.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(FixtureValidationError, r"^bad-endpoint:"):
                validate_fixture_dir(root)
            request["url_template"] = "https://api.openai.com/v1/chat/completions"
            bad_endpoint.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")

            request["headers"]["Authorization"] = "sk-test-secret"
            bad_endpoint.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(FixtureValidationError, r"^secret-leak:"):
                validate_fixture_dir(root)
            request["headers"]["Authorization"] = "REDACTED"
            bad_endpoint.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")

            bad_endpoint.write_text("{" + '"padding":"' + ("x" * MAX_FILE_BYTES) + '"}\n')
            with self.assertRaisesRegex(FixtureValidationError, r"^over-cap:"):
                validate_fixture_dir(root)

    def test_missing_repository_directory_is_the_single_red_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = pathlib.Path(directory) / "provider_contracts"
            with self.assertRaisesRegex(
                FixtureValidationError,
                r"^bad-fixture: missing provider_contracts directory$",
            ):
                validate_fixture_dir(missing)


if __name__ == "__main__":
    unittest.main()
