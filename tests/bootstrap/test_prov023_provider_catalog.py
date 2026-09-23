"""PROV-023 RED contract for the offline provider compatibility catalog.

The catalog is documentation, not a provider discovery mechanism.  These tests
read one bounded local file, resolve only exact documented entries, and never
contact a provider or read credentials.
"""

from __future__ import annotations

import copy
import datetime
import json
import pathlib
import re
import unittest
from urllib.parse import urlsplit


ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOG_PATH = ROOT / "docs" / "provider-compatibility.json"
MAX_BYTES = 256 * 1024
MAX_PROVIDERS = 32
MAX_ENDPOINTS = 16
MAX_ALIASES = 32

TOP_LEVEL_KEYS = {"catalog_version", "providers", "updated"}
PROVIDER_KEYS = {
    "display_name",
    "doc_date",
    "doc_source",
    "endpoints",
    "id",
    "limitations",
    "model_aliases",
    "refresh",
}
ENDPOINT_KEYS = {"auth_headers", "kind", "url_template"}
REFRESH_KEYS = {"method", "supported"}
HEADER_NAME = re.compile(r"^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$")
PROVIDER_ID = re.compile(r"^[a-z][a-z0-9_-]*$")
SECRET_LIKE = (
    re.compile(r"sk-[A-Za-z0-9_-]{8,}"),
    re.compile(r"Bearer\s+[A-Za-z0-9._~+/=-]{8,}"),
)
SPOOF_HEADER_PREFIXES = ("codex-", "claude-code-", "x-stainless", "x-openai-client")


class CatalogError(AssertionError):
    """Stable failure category used by the disposable negative fixtures."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(f"{code}{': ' + detail if detail else ''}")
        self.code = code


def _canonical(value: object) -> object:
    if isinstance(value, dict):
        return {key: _canonical(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        normalized = [_canonical(item) for item in value]
        return sorted(
            normalized,
            key=lambda item: json.dumps(
                item, ensure_ascii=False, sort_keys=True, separators=(",", ":")
            ),
        )
    return value


def _reject(code: str, detail: str = "") -> None:
    raise CatalogError(code, detail)


def _validate_catalog(document: object, raw: bytes) -> None:
    """Validate the complete local contract without network or secret access."""
    if len(raw) > MAX_BYTES:
        _reject("over-cap", "catalog bytes")
    if not isinstance(document, dict) or set(document) != TOP_LEVEL_KEYS:
        _reject("bad-schema", "top-level keys")
    if document["catalog_version"] != "1":
        _reject("bad-version")

    updated = document["updated"]
    if not isinstance(updated, str):
        _reject("bad-date")
    try:
        parsed_date = datetime.date.fromisoformat(updated)
    except ValueError:
        _reject("bad-date")
    if parsed_date.isoformat() != updated:
        _reject("bad-date")

    providers = document["providers"]
    if not isinstance(providers, list) or len(providers) > MAX_PROVIDERS:
        _reject("over-cap", "providers")
    provider_ids: list[object] = []
    for provider in providers:
        if not isinstance(provider, dict):
            _reject("bad-schema", "provider object")
        provider_id = provider.get("id")
        if provider_id in provider_ids:
            _reject("bad-schema", "duplicate provider id")
        provider_ids.append(provider_id)

    for provider in providers:
        if set(provider) != PROVIDER_KEYS:
            _reject("bad-schema", "provider keys")
        provider_id = provider["id"]
        if not isinstance(provider_id, str) or not PROVIDER_ID.fullmatch(provider_id):
            _reject("bad-schema", "provider id")
        for field in ("display_name", "doc_source", "doc_date"):
            if not isinstance(provider[field], str) or not provider[field]:
                _reject("bad-schema", field)
        _validate_date(provider["doc_date"])
        _validate_https(provider["doc_source"], "bad-source")

        endpoints = provider["endpoints"]
        if not isinstance(endpoints, list) or not endpoints or len(endpoints) > MAX_ENDPOINTS:
            _reject("over-cap", f"{provider_id} endpoints")
        endpoint_kinds: set[str] = set()
        for endpoint in endpoints:
            if not isinstance(endpoint, dict) or set(endpoint) != ENDPOINT_KEYS:
                _reject("bad-schema", "endpoint keys")
            kind = endpoint["kind"]
            if not isinstance(kind, str) or not kind or kind in endpoint_kinds:
                _reject("bad-schema", "endpoint kind")
            endpoint_kinds.add(kind)
            template = endpoint["url_template"]
            if not isinstance(template, str) or not template:
                _reject("bad-endpoint")
            _validate_https(template, "bad-endpoint")
            headers = endpoint["auth_headers"]
            if not isinstance(headers, list) or not headers:
                _reject("bad-schema", "auth headers")
            for header in headers:
                if (
                    not isinstance(header, str)
                    or not HEADER_NAME.fullmatch(header)
                    or header.lower().startswith(SPOOF_HEADER_PREFIXES)
                ):
                    _reject("bad-schema", "auth header name")
            if len(set(headers)) != len(headers):
                _reject("bad-schema", "duplicate auth header")

        aliases = provider["model_aliases"]
        if not isinstance(aliases, list) or not aliases or len(aliases) > MAX_ALIASES:
            _reject("over-cap", f"{provider_id} aliases")
        if any(not isinstance(alias, str) or not alias for alias in aliases):
            _reject("bad-schema", "model alias")
        if len(set(aliases)) != len(aliases):
            _reject("bad-schema", "duplicate model alias")

        limitations = provider["limitations"]
        if not isinstance(limitations, list) or any(
            not isinstance(limitation, str) or not limitation for limitation in limitations
        ):
            _reject("bad-schema", "limitations")
        refresh = provider["refresh"]
        if (
            not isinstance(refresh, dict)
            or set(refresh) != REFRESH_KEYS
            or not isinstance(refresh["supported"], bool)
            or not isinstance(refresh["method"], str)
            or not refresh["method"]
        ):
            _reject("bad-schema", "refresh")

    if {p["id"] for p in providers} < {"openai", "anthropic"}:
        _reject("bad-schema", "required providers")
    if len(providers) < 3:
        _reject("bad-schema", "third documented provider")

    text = raw.decode("utf-8")
    if "undocumented" in text.lower():
        _reject("bad-schema", "undocumented claim")
    for pattern in SECRET_LIKE:
        if pattern.search(text):
            _reject("secret")

    if _canonical(document) != document:
        _reject("bad-order", "canonical keys or arrays")
    expected = json.dumps(document, ensure_ascii=False, indent=2).encode("utf-8") + b"\n"
    if raw != expected:
        _reject("bad-order", "non-deterministic JSON serialization")


def _validate_date(value: str) -> None:
    try:
        parsed_date = datetime.date.fromisoformat(value)
    except ValueError:
        _reject("bad-date")
    if parsed_date.isoformat() != value:
        _reject("bad-date")


def _validate_https(value: str, code: str) -> None:
    parts = urlsplit(value)
    if parts.scheme != "https" or not parts.netloc or parts.username or parts.password:
        _reject(code)


def _lookup(document: dict, provider_id: str, endpoint_kind: str) -> dict | None:
    """Exact lookup only.  No provider aliases, URL synthesis, or fallback."""
    for provider in document["providers"]:
        if provider["id"] == provider_id:
            for endpoint in provider["endpoints"]:
                if endpoint["kind"] == endpoint_kind:
                    return endpoint
            return None
    return None


def _lookup_alias(document: dict, provider_id: str, alias: str) -> str | None:
    """Resolve only an exact documented alias for an exact provider id."""
    for provider in document["providers"]:
        if provider["id"] == provider_id:
            return alias if alias in provider["model_aliases"] else None
    return None


def _load_catalog() -> tuple[dict, bytes]:
    if not CATALOG_PATH.is_file():
        raise CatalogError("missing-catalog", str(CATALOG_PATH.relative_to(ROOT)))
    raw = CATALOG_PATH.read_bytes()
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise CatalogError("bad-json", str(exc)) from exc
    _validate_catalog(document, raw)
    return document, raw


def _negative_fixture_catalog() -> dict:
    """Small offline baseline used only to exercise rejection paths."""
    providers = []
    for provider_id, display_name, kind in (
        ("anthropic", "Fixture Anthropic", "messages"),
        ("google", "Fixture Google", "generate-content"),
        ("openai", "Fixture OpenAI", "chat-completions"),
    ):
        providers.append(
            {
                "display_name": display_name,
                "doc_date": "2026-01-01",
                "doc_source": "https://fixture.invalid/docs",
                "endpoints": [
                    {
                        "auth_headers": ["Authorization"],
                        "kind": kind,
                        "url_template": f"https://fixture.invalid/v1/{kind}",
                    }
                ],
                "id": provider_id,
                "limitations": ["Synthetic fixture only; no provider support asserted."],
                "model_aliases": [f"{provider_id}-fixture-model"],
                "refresh": {"method": "fixture", "supported": False},
            }
        )
    return {
        "catalog_version": "1",
        "providers": providers,
        "updated": "2026-01-01",
    }


class ProviderCatalogTests(unittest.TestCase):
    def test_t01_strict_schema_version_date_and_required_providers(self) -> None:
        document, raw = _load_catalog()
        _validate_catalog(document, raw)
        self.assertEqual(document["catalog_version"], "1")
        self.assertIn("openai", {provider["id"] for provider in document["providers"]})
        self.assertIn("anthropic", {provider["id"] for provider in document["providers"]})
        self.assertGreaterEqual(len(document["providers"]), 3)

    def test_t02_documented_https_endpoints_named_auth_and_exact_alias_lookup(self) -> None:
        document, _ = _load_catalog()
        for provider in document["providers"]:
            self.assertGreaterEqual(len(provider["model_aliases"]), 1)
            for endpoint in provider["endpoints"]:
                self.assertTrue(endpoint["url_template"].startswith("https://"))
                self.assertTrue(endpoint["auth_headers"])
                self.assertTrue(all(HEADER_NAME.fullmatch(header) for header in endpoint["auth_headers"]))

        openai = _lookup(document, "openai", "chat-completions")
        anthropic = _lookup(document, "anthropic", "messages")
        self.assertIsNotNone(openai)
        self.assertIsNotNone(anthropic)
        self.assertTrue(openai["url_template"].startswith("https://"))
        self.assertTrue(anthropic["url_template"].startswith("https://"))
        for provider in document["providers"]:
            alias = provider["model_aliases"][0]
            self.assertEqual(_lookup_alias(document, provider["id"], alias), alias)
            self.assertIsNone(_lookup_alias(document, provider["id"], alias + "-not-documented"))

    def test_t03_unknown_provider_or_endpoint_is_null_without_guessing(self) -> None:
        document, raw = _load_catalog()
        self.assertIsNone(_lookup(document, "unknown-provider", "chat-completions"))
        self.assertIsNone(_lookup(document, "openai", "unknown-endpoint"))
        self.assertNotIn("unknown-provider", raw.decode("utf-8"))
        self.assertNotIn("example.invalid", raw.decode("utf-8"))

    def test_t04_bounds_headers_and_secret_scan_negative_fixtures(self) -> None:
        document, raw = _load_catalog()
        self.assertLessEqual(len(raw), MAX_BYTES)
        self.assertLessEqual(len(document["providers"]), MAX_PROVIDERS)
        for provider in document["providers"]:
            self.assertLessEqual(len(provider["endpoints"]), MAX_ENDPOINTS)
            self.assertLessEqual(len(provider["model_aliases"]), MAX_ALIASES)
            for endpoint in provider["endpoints"]:
                self.assertTrue(all("=" not in header and ":" not in header for header in endpoint["auth_headers"]))
        self.assertNotIn("undocumented", raw.decode("utf-8").lower())
        self.assertFalse(any(pattern.search(raw.decode("utf-8")) for pattern in SECRET_LIKE))

    def test_t05_canonical_order_and_byte_identical_reserialization(self) -> None:
        document, raw = _load_catalog()
        self.assertEqual(_canonical(document), document)
        self.assertEqual(json.dumps(document, ensure_ascii=False, indent=2).encode() + b"\n", raw)

    def test_negative_disposable_fixtures_reject_version_endpoint_cap_and_secret(self) -> None:
        document = _negative_fixture_catalog()

        bad_version = copy.deepcopy(document)
        bad_version["catalog_version"] = "2"
        with self.assertRaisesRegex(CatalogError, "bad-version"):
            _validate_catalog(bad_version, json.dumps(bad_version).encode())

        bad_endpoint = copy.deepcopy(document)
        bad_endpoint["providers"][0]["endpoints"][0]["url_template"] = "http://fixture.invalid"
        with self.assertRaisesRegex(CatalogError, "bad-endpoint"):
            _validate_catalog(bad_endpoint, json.dumps(bad_endpoint).encode())

        over_cap = copy.deepcopy(document)
        template = copy.deepcopy(over_cap["providers"][0])
        for index in range(MAX_PROVIDERS - len(over_cap["providers"])):
            extra = copy.deepcopy(template)
            extra["id"] = f"fixture-provider-{index}"
            over_cap["providers"].append(extra)
        extra = copy.deepcopy(template)
        extra["id"] = "fixture-provider-over-cap"
        over_cap["providers"].append(extra)
        with self.assertRaisesRegex(CatalogError, "over-cap"):
            _validate_catalog(over_cap, json.dumps(over_cap).encode())

        secret = copy.deepcopy(document)
        secret["providers"][0]["limitations"].append("fixture sk-fake-token-123")
        secret_raw = json.dumps(secret).encode()
        with self.assertRaisesRegex(CatalogError, "secret"):
            _validate_catalog(secret, secret_raw)


if __name__ == "__main__":
    unittest.main()
