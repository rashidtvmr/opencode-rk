#!/usr/bin/env python3
"""Deterministic JSON renderer used by the harness self-test and protocol docs.

It does not launch OC2.  Instead it materializes the `static_screen` object
declared by the fixture, optionally applying a negative-control mutation so the
self-test can prove the comparator detects one-cell color, cursor, focus,
action-trace and width-truncation regressions.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import sys


def pad_rows(rows, cols, rows_count, default_style):
    result = []
    for index in range(rows_count):
        if index < len(rows):
            row = dict(rows[index])
            row["spans"] = [dict(span) for span in row.get("spans", [])]
        else:
            row = {"text": "", "w": cols, "spans": []}
        text = row.get("text", "")
        width = row.get("w", cols)
        if width > cols:
            text = text[:cols]
            width = cols
            row["spans"] = [
                span for span in row.get("spans", []) if span.get("c", 0) < cols
            ]
            for span in row["spans"]:
                span["n"] = min(span.get("n", 0), cols - span.get("c", 0))
        while len(text) < width:
            text += " "
        row["text"] = text
        row["w"] = width or cols
        row.setdefault("spans", [])
        result.append(row)
    return result


def apply_mutation(screen, mutation):
    if not mutation:
        return screen
    default = screen["default_style"]
    rows = screen["rows"]
    if mutation == "one_cell_color":
        rows[0].setdefault("spans", []).append(
            {"c": 0, "n": 1, "fg": "#ff00ff", "bg": default["bg"]}
        )
    elif mutation == "cursor_move":
        cursor = dict(screen["cursor"])
        cursor["visible"] = True
        cursor["col"] = int(cursor.get("col") or 0) + 1
        screen["cursor"] = cursor
    elif mutation == "focus_change":
        screen["focus"] = {"value": "sidebar", "source": "mutation"}
    elif mutation == "action_extra":
        screen["actions"] = list(screen.get("actions") or []) + [
            {"seq": 99, "type": "key", "key": "ctrl-p", "applied": True}
        ]
    elif mutation == "hit_target_extra":
        screen["hit_targets"] = list(screen.get("hit_targets") or []) + [
            {"x": 0, "y": 0, "action": "mutation.extra"}
        ]
    elif mutation == "width_truncate":
        old_cols = screen["viewport"]["cols"]
        new_cols = old_cols - 1
        screen["viewport"] = {"cols": new_cols, "rows": screen["viewport"]["rows"]}
        for row in rows:
            row["text"] = row["text"][:new_cols]
            row["w"] = new_cols
            for span in row.get("spans", []):
                if span.get("c", 0) >= new_cols:
                    span["n"] = 0
                else:
                    span["n"] = min(span.get("n", 0), new_cols - span["c"])
            row["spans"] = [span for span in row.get("spans", []) if span.get("n", 0) > 0]
    else:
        raise ValueError(f"unknown mutation: {mutation}")
    return screen


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mutation",
        default="",
        choices=[
            "",
            "one_cell_color",
            "cursor_move",
            "focus_change",
            "action_extra",
            "hit_target_extra",
            "width_truncate",
        ],
    )
    args = parser.parse_args()
    request = json.load(sys.stdin)
    fixture = request["fixture"]
    cols = int(request["viewport"]["cols"])
    rows_count = int(request["viewport"]["rows"])
    template = fixture.get("static_screen")
    if not template:
        template = {}
        base_rows = []
        for index in range(rows_count):
            text = f"fixture {fixture['id']} row {index + 1}"
            base_rows.append({"text": text[:cols], "w": cols, "spans": []})
        template = {
            "default_style": {"fg": "#ffffff", "bg": "#000000"},
            "rows": base_rows,
            "cursor": {"row": 0, "col": 0, "visible": True},
            "focus": {"value": "composer", "source": "static"},
            "actions": [],
        }
    default_style = template.get("default_style") or {"fg": "#ffffff", "bg": "#000000"}
    screen = {
        "schema": "oc2.ui-parity.screen/v1",
        "fixture_id": fixture["id"],
        "viewport": {"cols": cols, "rows": rows_count},
        "default_style": default_style,
        "rows": pad_rows(template.get("rows") or [], cols, rows_count, default_style),
        "cursor": template.get("cursor") or {"row": 0, "col": 0, "visible": True},
        "focus": template.get("focus"),
        "hit_targets": list(template.get("hit_targets") or []),
        "actions": list(template.get("actions") or []),
        "theme": template.get("theme") or request.get("theme"),
    }
    screen = apply_mutation(screen, args.mutation)
    canonical = {
        "id": fixture.get("id"),
        "viewport": fixture.get("viewport"),
        "theme": fixture.get("theme"),
        "static_screen": template,
    }
    payload = json.dumps(canonical, sort_keys=True).encode("utf-8")
    screen["environment"] = {
        "adapter": "static",
        "styled": True,
        "product": {
            "name": "static-json-renderer",
            "version": "1",
            "sha256": hashlib.sha256(payload).hexdigest(),
            "executable": "static_json.py",
        },
        "terminal": {
            "term": "none",
            "cols": cols,
            "rows": rows_count,
            "platform": sys.platform,
            "machine": platform.machine(),
        },
        "captured_with": ["static_json.py", args.mutation or "(none)"],
    }
    json.dump(screen, sys.stdout, ensure_ascii=False, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
