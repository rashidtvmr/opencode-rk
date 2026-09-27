#!/usr/bin/env python3
"""Validate the draft's structure; never claim the product was tested."""
from __future__ import annotations
import argparse
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding='utf-8'))
    if not isinstance(value, dict):
        raise ValueError(f'{path.name}: expected object')
    return value


def validate(root: Path, board: dict[str, Any] | None = None) -> dict[str, Any]:
    board = board if board is not None else read_json(root / 'phase1-tasks.json')
    errors: list[str] = []
    packages, lanes, gates = (board.get(k, []) for k in ('packages', 'lanes', 'gates'))
    all_items = packages + lanes + gates
    ids = [item.get('id', '') for item in all_items]
    duplicates = [name for name, count in Counter(ids).items() if count > 1]
    if duplicates:
        errors.append('duplicate IDs: ' + ', '.join(duplicates))
    if '' in ids:
        errors.append('empty ID')
    known = set(ids)
    package_ids = {p['id'] for p in packages}
    by_id = {p['id']: p for p in packages}
    if len(packages) != 60 or len(gates) != 6:
        errors.append('expected 60 packages and six gates for this draft')
    if Counter(p['wave'] for p in packages) != Counter({w: 10 for w in range(1, 7)}):
        errors.append('expected ten packages per wave')
    if board.get('status') != 'draft_not_activated':
        errors.append('this artifact is a draft; activation requires separate authority')
    for item in all_items:
        if item.get('status') != 'draft':
            errors.append(f"{item['id']}: unsupported completion claim in a draft")
        for dep in item.get('depends_on', []):
            if dep not in known:
                errors.append(f"{item['id']}: unknown dependency {dep}")
    graph = {x['id']: list(x.get('depends_on', [])) for x in all_items}
    # Parent milestone needs its independent receipt. Later release-only
    # rechecks are deliberately separate from the presentation milestone DAG.
    for package in packages:
        verifier = package['id'] + '-V'
        if verifier not in known:
            errors.append(f"{package['id']}: missing independent verifier")
        else:
            graph[package['id']].append(verifier)
        if len(package.get('acceptance', [])) < 3:
            errors.append(f"{package['id']}: insufficient acceptance contract")
        if not package.get('source_evidence'):
            errors.append(f"{package['id']}: missing source evidence")
        if package.get('commands_exist_today') is not False:
            errors.append(f"{package['id']}: planned tests must not be presented as executed/existing")
        for dep in package.get('release_completion_dependencies', []):
            if dep not in package_ids:
                errors.append(f"{package['id']}: unknown release-only dependency {dep}")
        if package.get('release_completion_dependencies') and not package.get('stage_gate_rule'):
            errors.append(f"{package['id']}: staged feature lacks non-completion rule")
        if package['wave'] > 1 and f"P1-G{package['wave']-1}" not in package['depends_on']:
            errors.append(f"{package['id']}: missing preceding integrated wave gate")
    active: set[str] = set()
    finished: set[str] = set()
    def visit(node: str, path: list[str]) -> None:
        if node in active:
            raise ValueError('dependency cycle: ' + ' -> '.join(path + [node]))
        if node in finished or node not in graph:
            return
        active.add(node)
        for dep in graph[node]:
            visit(dep, path + [node])
        active.remove(node)
        finished.add(node)
    try:
        for node in graph:
            visit(node, [])
    except ValueError as exc:
        errors.append(str(exc))
    def reaches(start: str, target: str) -> bool:
        pending = [start]
        seen: set[str] = set()
        while pending:
            node = pending.pop()
            if node == target:
                return True
            if node in seen:
                continue
            seen.add(node)
            pending.extend(graph.get(node, []))
        return False
    owners: dict[str, list[str]] = defaultdict(list)
    for lane in lanes:
        path = lane.get('owned_file')
        if not isinstance(path, str) or not path or PurePosixPath(path).is_absolute() or '..' in PurePosixPath(path).parts:
            errors.append(f"{lane['id']}: one safe relative owned_file required")
            continue
        owners[path].append(lane['id'])
        if lane.get('parent') not in package_ids:
            errors.append(f"{lane['id']}: invalid package parent")
        if lane['id'] not in by_id.get(lane.get('parent'), {}).get('lane_ids', []):
            errors.append(f"{lane['id']}: not listed by parent")
        if path.startswith('.github/') and lane.get('role') != 'integrator':
            errors.append(f"{lane['id']}: workflow changes require integrator")
        if path.startswith('tests/') and lane.get('role') != 'independent-test-author':
            errors.append(f"{lane['id']}: tests require independent author")
    for path, writers in owners.items():
        for idx, first in enumerate(writers):
            for second in writers[idx + 1:]:
                if not reaches(first, second) and not reaches(second, first):
                    errors.append(f'concurrent ownership conflict: {path}: {first}, {second}')
    for gate in gates:
        expected = {p['id'] for p in packages if p['wave'] == gate['wave']}
        if set(gate.get('depends_on', [])) != expected:
            errors.append(f"{gate['id']}: gate omits a wave package")
        if gate.get('product_verified') is not False:
            errors.append(f"{gate['id']}: draft cannot certify product")
    inventory = read_json(root / 'audit/recorded-status-inventory.json')['records']
    crosswalk = read_json(root / 'audit/legacy-crosswalk.json')['records']
    original = {r['id'] for r in inventory}
    mapped = {r['id'] for r in crosswalk}
    if len(original) != 468 or original != mapped or len(crosswalk) != len(mapped):
        errors.append('legacy crosswalk is not a unique complete 468-ID mapping')
    for record in crosswalk:
        if not record.get('proposed_packages'):
            errors.append(record['id'] + ': missing triage/scope owner')
        for target in record.get('proposed_packages', []):
            if target not in package_ids:
                errors.append(record['id'] + ': unknown target ' + target)
        if record.get('semantic_confirmation_required') is not True:
            errors.append(record['id'] + ': semantic confirmation must remain explicit')
    requirements = read_json(root / 'requirements-plan.json')['requirements']
    expected_requirements = {f'REQ-{i:03}' for i in range(1, 49) if i != 39}
    if {x['id'] for x in requirements} != expected_requirements or len(requirements) != 47:
        errors.append('all 47 original requirements must be retained')
    surfaces = read_json(root / 'surface-plan.json')['surfaces']
    for record in requirements + surfaces:
        for target in record.get('owner_packages', []):
            if target not in package_ids:
                errors.append(record['id'] + ': unknown owner ' + target)
    policy = read_json(root / 'model-policy.json')
    if policy.get('automatic_pro_fallback') or policy.get('pro_models_required'):
        errors.append('plan unexpectedly requires a Pro model')
    # Literal credentials must never appear in the deliverable. Do not scan
    # source code regexes in this script itself as if they were credentials.
    for path in root.rglob('*'):
        if path.suffix not in {'.md', '.json'}:
            continue
        text = path.read_text(encoding='utf-8')
        if re.search(r'\bsk-[A-Za-z0-9_-]{16,}', text):
            errors.append(f'possible literal credential in {path.relative_to(root)}')
    return {
        'plan_valid': not errors,
        'product_verified': False,
        'application_tests_executed_by_this_check': 0,
        'counts': {'packages': len(packages), 'one_file_role_assignments': len(lanes), 'gates': len(gates), 'dag_nodes': len(graph), 'original_ids': len(original), 'requirements': len(requirements), 'surface_rows': len(surfaces)},
        'errors': errors,
        'limitations': ['Structural checks do not execute application features.', 'Semantic task mappings and scope decisions remain pending.', 'Suggested source/test paths require checkout preflight before activation.'],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        report = validate(args.root.resolve())
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(json.dumps({'plan_valid': False, 'product_verified': False, 'errors': [str(exc)]}, indent=2))
        return 2
    print(json.dumps(report, indent=2))
    return 0 if report['plan_valid'] else 1

if __name__ == '__main__':
    sys.exit(main())
