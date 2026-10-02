"""Serial exact-SHA Ubuntu installed release verification."""
import argparse
import hashlib
import json
import os
import pathlib
import re
import select
import shlex
import signal
import subprocess
import tarfile
import time

parser = argparse.ArgumentParser()
parser.add_argument('--sha', required=True)
parser.add_argument('--source-name', required=True)
parser.add_argument('--output-name', required=True)
parser.add_argument('--tool-gates', action='store_true')
args = parser.parse_args()
assert re.fullmatch(r'[0-9a-f]{40}', args.sha)
assert re.fullmatch(r'source-[a-z0-9-]+', args.source_name)
assert re.fullmatch(r'ubuntu-native-[a-z0-9-]+', args.output_name)
ROOT = pathlib.Path('/work')
SOURCE = ROOT/args.source_name
OUTPUT = ROOT/args.output_name
assert not SOURCE.exists() and not OUTPUT.exists()
source = json.loads((ROOT/(args.source_name+'.json')).read_text())
assert source['source_sha'] == args.sha and source['source_path'] == str(SOURCE)

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(65536), b''):
            h.update(block)
    return h.hexdigest()

archive_source = ROOT/(args.source_name+'.tar')
assert digest(archive_source) == source['source_archive_sha256']
SOURCE.mkdir()
with tarfile.open(archive_source) as archive:
    for member in archive.getmembers():
        path = pathlib.PurePosixPath(member.name)
        assert not path.is_absolute() and '..' not in path.parts
        if member.issym() or member.islnk():
            target = pathlib.PurePosixPath(member.linkname)
            assert not target.is_absolute() and '..' not in target.parts
    archive.extractall(SOURCE)
OUTPUT.mkdir()
home = OUTPUT/'home'
home.mkdir()
records = []
env = {'HOME': str(home), 'PATH': '/usr/bin:/bin', 'TMPDIR': '/work/verify-tmp', 'LANG': 'C', 'PYTHONDONTWRITEBYTECODE': '1'}
build_env = {**env, 'CARGO_HOME': '/work/cargo', 'RUSTUP_HOME': '/work/rustup', 'CARGO_TARGET_DIR': '/work/target', 'CARGO_BUILD_JOBS': '2', 'RUST_TEST_THREADS': '1', 'RUSTFLAGS': '-C link-arg=-Wl,-rpath,$ORIGIN/../lib', 'PATH': '/work/cargo/bin:/usr/bin:/bin', 'XDG_CONFIG_HOME': str(home/'config'), 'XDG_DATA_HOME': str(home/'data'), 'XDG_STATE_HOME': str(home/'state'), 'XDG_CACHE_HOME': str(home/'cache')}

def run(label, command, environ=env, timeout=90):
    log = OUTPUT/(label+'.log')
    start = time.monotonic()
    failure = None
    total = 0
    with log.open('wb') as out:
        child = subprocess.Popen(command, env=environ, cwd=SOURCE, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            while True:
                left = timeout - (time.monotonic()-start)
                if left <= 0:
                    failure = 'timeout'
                    break
                ready, _, _ = select.select([child.stdout], [], [], min(left, 1))
                if not ready:
                    continue
                block = os.read(child.stdout.fileno(), 65536)
                if not block:
                    break
                total += len(block)
                if total > 32*1024*1024:
                    failure = 'output budget'
                    break
                out.write(block)
            if failure:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    code = child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    code = child.wait(timeout=5)
            else:
                code = child.wait(timeout=5)
        finally:
            child.stdout.close()
    record = {'source_sha': args.sha, 'label': label, 'command': shlex.join(command), 'cwd': str(SOURCE), 'environment': environ, 'exit': code, 'limit_failure': failure, 'seconds': round(time.monotonic()-start, 2), 'log_sha256': digest(log)}
    records.append(record)
    (OUTPUT/'commands.json').write_text(json.dumps(records, indent=2)+'\n')
    print(label, code, record['seconds'], flush=True)
    if code or failure:
        print(log.read_text(errors='replace')[-2500:], flush=True)
        raise RuntimeError('Required Ubuntu gate failed: '+label)
    return log

frozen = {
    'tests/e2e/native_utf8_input.py': 'f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619',
    'tests/e2e/native_stream_tool.py': 'c6d8d561a96136061c4d6e87e183bad144e762df17847fcfca4a347d0d30633e',
    'tests/e2e/native_provider_setup.py': '32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc',
    'tests/bootstrap/test_native_provider_auth_control.py': 'f5b7dd3372b1d0aac4e285d16fb4295833854c8ab9b6ec836bacdb9205d8809d',
    'tests/e2e/native_interactive_pty.py': '51c37cfe066987ab94b3bba8e8496c7632a37902702e4caf571f8b1dc4ebc9b5',
}
for path, expected in frozen.items():
    assert digest(SOURCE/path) == expected
if args.tool_gates:
    owned_test = 'crates/tools/tests/shell_owned_lifetime.rs'
    frozen[owned_test] = '8b39c6eafcbb065e42752c8c1d19bac53be6928612fc59b1da654465353097fd'
    assert digest(SOURCE/owned_test) == frozen[owned_test]
run('build', ['/work/cargo/bin/cargo', 'build', '--offline', '--locked', '--release', '-p', 'opencode-rk-cli', '--bin', 'oc2', '--features', 'native'], build_env, 240)
native_source = SOURCE/'crates/opentui-bridge/native/lib/aarch64-unknown-linux-gnu/libopentui.so'
assert digest(native_source) == 'e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89'
archive = OUTPUT/'oc2-linux-arm64.tar.gz'
with tarfile.open(archive, 'w:gz', compresslevel=1) as output:
    output.add(ROOT/'target/release/oc2', arcname='oc2', recursive=False)
    output.add(native_source, arcname='native/lib/linux-arm64/libopentui.so', recursive=False)
run('install', ['/bin/sh', str(SOURCE/'scripts/install-oc2.sh'), '--archive', str(archive), '--checksum', digest(archive), '--install-dir', str(OUTPUT/'install/bin')], timeout=30)
binary = OUTPUT/'install/bin/oc2'
library = OUTPUT/'install/lib/libopentui.so'
build = {**source, 'profile': 'release', 'native': True, 'binary_sha256': digest(binary), 'native_library_sha256': digest(library), 'archive_sha256': digest(archive), 'build_command': records[0], 'installer_command': records[1], 'frozen_tests': frozen, 'runner_sha256': digest(pathlib.Path(__file__))}
(OUTPUT/'build.json').write_text(json.dumps(build, indent=2)+'\n')
for name, file in [('utf8', 'native_utf8_input.py'), ('stream', 'native_stream_tool.py'), ('provider', 'native_provider_setup.py')]:
    run(name, ['/usr/bin/python3', str(SOURCE/'tests/e2e'/file), '--binary', str(binary), '--native-library', str(library), '--build-json', str(OUTPUT/'build.json'), '--artifact-dir', str(OUTPUT/name)], timeout=45)
controls = {**env, 'OC2_TEST_BINARY': str(binary), 'OC2_TEST_NATIVE_LIBRARY': str(library), 'OC2_TEST_BUILD_JSON': str(OUTPUT/'build.json'), 'OC2_AUTH_CONTROL_ARTIFACT_ROOT': str(OUTPUT/'auth-cases')}
run('auth', ['/usr/bin/python3', '-m', 'unittest', 'tests.bootstrap.test_native_provider_auth_control', '-v'], controls, 45)
run('pty', ['/usr/bin/python3', str(SOURCE/'tests/e2e/native_interactive_pty.py'), '-v'], {**env, 'OC2_NATIVE_BINARY': str(binary), 'OC2_NATIVE_LIBRARY': str(library)}, 40)
run('packaging', ['/usr/bin/python3', '-m', 'unittest', 'tests.bootstrap.test_app010_packaged_cli_identity', 'tests.bootstrap.test_tui011_installer_native_closure', 'tests.bootstrap.test_tui011_installer_rollback', 'tests.bootstrap.test_tui011_installer_signal_layout', '-v'], controls, 45)
dynamic = run('dynamic', ['readelf', '-d', str(binary)]).read_text()
assert 'libopentui.so' in dynamic and '$ORIGIN/../lib' in dynamic
linked = run('loader', ['ldd', str(binary)]).read_text()
assert 'not found' not in linked
native_line = next(line for line in linked.splitlines() if line.strip().startswith('libopentui.so => '))
assert pathlib.Path(native_line.split('=>', 1)[1].strip().split()[0]).resolve() == library.resolve()
symbols = run('abi', ['nm', '-D', '--defined-only', str(library)]).read_text()
assert len(symbols.encode()) <= 4*1024*1024
names = {line.split()[-1] for line in symbols.splitlines() if line.split()}
required = {'createRenderer', 'destroyRenderer', 'getCurrentBuffer', 'getNextBuffer', 'bufferDrawText', 'bufferWriteResolvedChars', 'setupTerminal', 'restoreTerminalModes', 'suspendRenderer', 'resumeRenderer'}
assert required <= names
(OUTPUT/'loader-abi-verification.json').write_text(json.dumps({'source_sha': args.sha, 'relative_rpath': '$ORIGIN/../lib', 'native_path': str(library.resolve()), 'required_exports': sorted(required), 'loader_pass': True, 'abi_pass': True}, indent=2)+'\n')
utf8 = json.loads((OUTPUT/'utf8/native-utf8-result.json').read_text())
assert utf8['phase'] == 'success' and len(utf8['requests']) == 2 and not utf8['semantic_errors']
assert utf8['termios_restored'] and utf8['cli_reaped_zero'] and utf8['validated_daemons_gone'] and utf8['provider_thread_joined']
stream = json.loads((OUTPUT/'stream/native-stream-tool-result.json').read_text())
assert stream['phase'] == 'success' and len(stream['requests']) == 4 and stream['owned_process_cleanup']
if args.tool_gates:
    cargo = ['/work/cargo/bin/cargo']
    run('tools-clippy', cargo + ['clippy', '--offline', '--locked', '-p', 'opencode-rk-tools', '--lib', '--', '-D', 'warnings'], build_env, 90)
    owned_log = run('shell-lifetime', cargo + ['test', '--offline', '--locked', '-p', 'opencode-rk-tools', '--test', 'shell_owned_lifetime', '--', '--test-threads=1', '--nocapture'], build_env, 90).read_text()
    assert 'test result: ok. 5 passed; 0 failed; 0 ignored;' in owned_log
    assert 'forced cleanup' not in owned_log
    tools_log = run('tools-tests', cargo + ['test', '--offline', '--locked', '-p', 'opencode-rk-tools', '--all-targets', '--all-features', '--', '--test-threads=1'], build_env, 180).read_text()
    counts = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', tools_log)
    assert counts and all(failed == '0' and ignored == '0' for _, failed, ignored in counts)
    assert 'forced cleanup' not in tools_log
    (OUTPUT/'tools-verification.json').write_text(json.dumps({'source_sha': args.sha, 'shell_test_sha256': frozen[owned_test], 'shell_passed': 5, 'tools_passed': sum(int(passed) for passed, _, _ in counts), 'failed': 0, 'ignored': 0, 'forced_cleanup': False}, indent=2)+'\n')
print('Exact integrated Ubuntu release passed:', args.sha, OUTPUT, flush=True)
