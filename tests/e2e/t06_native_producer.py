#!/usr/bin/env python3
"""Build and stage real T06 diagnostic inputs; never attest release acceptance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(65536), b''):
            h.update(chunk)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-root', required=True)
    parser.add_argument('--native-library', required=True)
    parser.add_argument('--output-dir', required=True)
    parser.add_argument('--cargo', default='cargo')
    args = parser.parse_args()
    root = Path(args.source_root).resolve(strict=True)
    library = Path(args.native_library).resolve(strict=True)
    out = Path(args.output_dir).resolve()
    host = subprocess.check_output(['rustc', '-vV'], text=True, timeout=10)
    target = next(line.removeprefix('host: ').strip() for line in host.splitlines() if line.startswith('host: '))
    suffix = '.dylib' if 'apple-darwin' in target else '.so'
    expected = root / 'crates/opentui-bridge/native/lib' / target / ('libopentui' + suffix)
    if library != expected.resolve(strict=True) or not library.is_file():
        raise SystemExit('native library must be the candidate host-target library')
    if not (root / 'Cargo.toml').is_file() or out.exists():
        raise SystemExit('source manifest required; output directory must be new')
    source = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True, timeout=10).strip()
    dirty = subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True, timeout=10)
    inputs = sorted(p for p in root.rglob('*.rs') if 'target' not in p.relative_to(root).parts and '.git' not in p.relative_to(root).parts)
    inputs += sorted(p for p in root.rglob('Cargo.toml')
                     if 'target' not in p.relative_to(root).parts and '.git' not in p.relative_to(root).parts)
    inputs += [root / 'Cargo.lock']
    binding = {str(p.relative_to(root)): digest(p) for p in inputs}
    out.mkdir(parents=True)
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR=str(out / 'target'), TUI015_NATIVE_LIB_DIR=str(library.parent), CARGO_BUILD_JOBS='2')
    argv = [args.cargo, 'build', '--offline', '--locked', '--release', '--manifest-path', str(root / 'Cargo.toml'), '-p', 'opencode-rk-cli', '--bin', 'oc2', '--features', 'native']
    subprocess.run(argv, cwd=root, env=env, check=True, timeout=1200)
    if any(digest(root / rel) != value for rel, value in binding.items()):
        raise SystemExit('source changed during build; diagnostic binding invalid')
    binary = out / 'install/bin/oc2'
    native = out / 'install/lib' / library.name
    binary.parent.mkdir(parents=True)
    native.parent.mkdir(parents=True)
    shutil.copy2(out / 'target/release/oc2', binary)
    shutil.copy2(library, native)
    binary.chmod(0o755)
    native.chmod(0o644)
    record = dict(source_sha=source, source_dirty=bool(dirty), source_files_sha256=binding,
                  binary_sha256=digest(binary), native_library_sha256=digest(native),
                  target=target, profile='release', command=argv, diagnostic_producer=True,
                  release_accepted=False)
    manifest = out / 'build.json'
    manifest.write_text(json.dumps(record, indent=2) + '\n')
    for key, value in [('OC2_TEST_BINARY', binary), ('OC2_TEST_NATIVE_LIBRARY', native), ('OC2_TEST_BUILD_JSON', manifest)]:
        print('export ' + key + '=' + shlex.quote(str(value)))


if __name__ == '__main__':
    main()
