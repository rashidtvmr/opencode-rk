#!/usr/bin/env python3
"""Read repository task metadata into a separate audit file; never alter ledgers."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any

MAX_SOURCE_BYTES = 4 * 1024 * 1024

def collect(repo: Path) -> dict[str, Any]:
    repo = repo.resolve(strict=True)
    sources: dict[str, str] = {}
    records: dict[str, dict[str, Any]] = {}
    warnings: list[str] = []
    def source(path: str) -> str:
        full = (repo / path).resolve(strict=True)
        if not full.is_relative_to(repo) or not full.is_file():
            raise ValueError(f'Unapproved source path: {path}')
        if full.stat().st_size > MAX_SOURCE_BYTES:
            raise ValueError(f'Source exceeds metadata cap: {path}')
        raw = full.read_bytes()
        sources[path] = hashlib.sha256(raw).hexdigest()
        return raw.decode('utf-8')
    def load(path: str) -> Any:
        return json.loads(source(path))
    def row(task_id: str) -> dict[str, Any]:
        if not isinstance(task_id, str) or not task_id:
            raise ValueError('Task ID must be a nonempty string')
        return records.setdefault(task_id, {'id': task_id, 'recorded_statuses': {}, 'source_records': {}, 'product_verification': 'not_reexecuted_by_metadata_inventory'})
    legacy = load('ralph.json')['userStories']
    exported = load('prd.json')['tasks']
    spec = load('ralph.completion.json')
    claims = load('tasks/completion/claims.json')['claims']
    for item in legacy:
        r = row(item['id']); r['recorded_statuses']['ralph'] = item.get('status', 'unspecified'); r['source_records']['ralph.json'] = item
    for item in exported:
        r = row(item['id']); r['recorded_statuses']['prd'] = item.get('status', 'unspecified'); r['source_records']['prd.json'] = item
    completion: list[dict[str, Any]] = []
    for path in spec['includes']:
        document = load(path)
        if 'stories' not in document:
            raise ValueError(f'{path}: expected stories array; refusing to silently skip it')
        for item in document['stories']:
            completion.append(item);row(item['id'])['source_records'][path] = item
    for item in spec.get('auditShards', []):
        row(item['id'])['source_records']['ralph.completion.json:auditShards'] = item
    for task_id, claim in claims.items():
        r = row(task_id);r['recorded_statuses']['claim'] = claim.get('status', 'unspecified')
        # Deliberately omit worker session identifiers and retain task evidence notes.
        r['source_records']['tasks/completion/claims.json'] = {k:v for k,v in claim.items() if k != 'session'}
    card_ids: set[str] = set()
    for path in sorted((repo / 'tasks').glob('*.md')):
        if not re.fullmatch(r'[A-Z][A-Z0-9-]*-\d+', path.stem):
            continue
        relative = path.relative_to(repo).as_posix();text = source(relative)
        card_ids.add(path.stem)
        row(path.stem)['source_records'][relative] = {
            'heading': next((line for line in text.splitlines() if line.startswith('# ')), ''),
            'recorded_status_lines': [line for line in text.splitlines() if re.match(r'^\*?\*?Status\b', line, re.IGNORECASE)],
            'references': sorted(set(re.findall(r'(?:crates|web|tools|scripts|tests|native)/[\w./*_-]+', text))),
        }
    requirements = load('requirements/user-requirements.json')['requirements']
    for item in requirements:
        for task_id in item.get('tasks', []):
            if task_id not in records:
                warnings.append(f"{item['id']} references undefined task {task_id}")
    git = subprocess.run(['git', '-C', str(repo), 'rev-parse', 'HEAD'], capture_output=True, text=True, check=True, timeout=15)
    revision = git.stdout.strip()
    dirt = subprocess.run(['git', '-C', str(repo), 'status', '--porcelain'], capture_output=True, text=True, check=True, timeout=15)
    legacy_ids = {x['id'] for x in legacy}
    formal_ids = {x['id'] for x in completion + spec.get('auditShards', [])}
    export_ids = {x['id'] for x in exported}
    old, exp = ({x['id']: x for x in group} for group in (legacy, exported))
    result = {
        'schema_version': 1,'revision': revision,'method': 'Read-only task/source inventory, not product verification or semantic acceptance.',
        'working_tree_dirty': bool(dirt.stdout.strip()),'source_sha256': sources,'pins': spec.get('pins', {}),
        'summary': {
            'unique_ids': len(records),'legacy': len(legacy),'legacy_statuses': dict(Counter(x.get('status') for x in legacy)),
            'export': len(exported),'export_statuses': dict(Counter(x.get('status') for x in exported)),
            'claim_count': len(claims),'claim_statuses': dict(Counter(x.get('status') for x in claims.values())),
            'completion_stories': len(completion),'audit_shards': len(spec.get('auditShards', [])),
            'status_disagreements': sum(old[k].get('status') != exp[k].get('status') for k in legacy_ids & export_ids),
            'export_missing_ids': sorted(legacy_ids - export_ids),'formal_unclaimed_ids': sorted(formal_ids - claims.keys()),
            'claim_only_ids': sorted(claims.keys() - formal_ids - legacy_ids),'card_only_ids': sorted(card_ids - formal_ids - legacy_ids - claims.keys()),
            'empty_legacy_dependencies': sum(not x.get('dependencyIds') for x in legacy),
            'tbd_legacy_titles': sum('TBD' in x.get('userStory', '') for x in legacy),
        },'requirements': requirements,'records':[records[k] for k in sorted(records)],'warnings': sorted(set(warnings)),
    }
    return result

def write_output(path: Path, result: dict[str, Any], overwrite: bool) -> None:
    path = path.resolve()
    if path.exists() and not overwrite:
        raise ValueError('Output already exists; select a new path or explicitly pass --overwrite')
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temporary = tempfile.mkstemp(prefix='.phase1-audit-', dir=path.parent)
    try:
        with os.fdopen(handle, 'w', encoding='utf-8') as file:
            json.dump(result, file, indent=2);file.write('\n')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--overwrite', action='store_true')
    args=parser.parse_args()
    try:
        output=args.output.resolve();repo=args.repo.resolve(strict=True)
        # An audit file is never a replacement for a canonical input or controller file.
        if output.is_relative_to(repo):
            relative=output.relative_to(repo)
            if relative.as_posix() in {'ralph.json','prd.json','ralph.completion.json','AGENTS.md','PLAN.md'} or relative.parts[0] in {'tasks','requirements','sources','tools','.github'}:
                raise ValueError('Refusing to write an audit over canonical repository/authority paths')
        result=collect(repo)
        write_output(output,result,args.overwrite)
        print(json.dumps({'output':str(output),'revision':result['revision'],'summary':result['summary'],'product_verified':False},indent=2))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(str(exc),file=sys.stderr);return 2
    return 0

if __name__ == '__main__':
    sys.exit(main())
