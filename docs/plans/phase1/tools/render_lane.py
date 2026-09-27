#!/usr/bin/env python3
"""Print one bounded task packet. Never dispatch or mutate task status."""
from __future__ import annotations
import argparse
import json
import sys
from pathlib import Path

def render(root: Path, lane_id: str) -> str:
    board = json.loads((root / 'phase1-tasks.json').read_text())
    lane = next((x for x in board['lanes'] if x['id'] == lane_id), None)
    if lane is None:
        raise ValueError(f'Unknown lane ID: {lane_id}')
    package = next(x for x in board['packages'] if x['id'] == lane['parent'])
    role = lane['role']
    prompt = 'TEST_AUTHOR.md' if role == 'independent-test-author' else 'VERIFIER.md' if 'verifier' in role else 'INTEGRATOR.md' if role == 'integrator' else 'WORKER.md'
    parts = [
        f"# {lane_id}: {package['title']}",
        '**DRAFT PACKET. Dispatch only after scope approval, current checkout preflight, claim registration and dependency evidence.**',
        f"Role: {role}\nModel class: {lane.get('model_class', 'reasoning_non_pro')} (resolve an actually available ID)\nOwned file: `{lane['owned_file']}`\nBaseline audited: `{board['source_revision']}`",
        '## Task\n' + lane['objective'],
        '## Observable parent contract\n' + package['contract'],
        '## Dependencies\n' + (', '.join(lane['depends_on']) or 'Scope/contract approval and current-baseline preflight.'),
        '## Focused source packet\n' + '\n'.join('- '+s for s in package['read_packet']),
        '## Acceptance\n' + '\n'.join(f'{i}. {a}' for i,a in enumerate(package['acceptance'],1)),
        '## Test contract\nPlanned test file: `' + package['test_file'] + '`. The test author must verify its valid baseline outcome and freeze it before implementation. This drafting pass did not create or run this application test.\nPlanned command:\n```sh\n' + package['planned_test_command'] + '\n```',
        '## External prerequisites\n' + ('\n'.join('- '+s for s in package['external_prerequisites']) or 'No additional declared prerequisites beyond the parent scope and gates.'),
        '## Reuse and ownership\n' + package['reuse_policy'],
        '## Role rules\n' + (root / 'prompts' / prompt).read_text().strip(),
        '## Evidence\nReturn `templates/LANE_HANDOFF.md` fields. No parent/release acceptance from a worker. Never include credentials or original user data.',
    ]
    if package.get('stage_gate_rule'):
        parts.insert(-1, '## Mandatory later service check\n' + package['stage_gate_rule'] + '\n' + ', '.join(package['release_completion_dependencies']))
    return '\n\n'.join(parts) + '\n'

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--id', required=True)
    args = parser.parse_args()
    try:
        print(render(args.root.resolve(), args.id), end='')
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(str(exc), file=sys.stderr)
        return 2
    return 0

if __name__ == '__main__':
    sys.exit(main())
