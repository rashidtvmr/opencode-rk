#!/usr/bin/env ruby
# frozen_string_literal: true

# Focused Phase-1 PTY lifecycle contract. Stdlib only: exact kernel termios
# snapshots, real child statuses, and cleanup restricted to this run's daemon.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/terminal-restore-pty-verification', __dir__)
RUN_DIR = File.join(ARTIFACT_DIR, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY = File.expand_path(ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__)))
ROWS = Integer(ENV.fetch('PHASE1_ROWS', '32'))
COLS = Integer(ENV.fetch('PHASE1_COLS', '100'))
NATIVE_LIBRARY = File.expand_path('../ui-native/lib', File.dirname(BINARY))
NATIVE_LIBRARY_FILE = File.join(NATIVE_LIBRARY, 'libopentui.dylib')
FileUtils.mkdir_p(RUN_DIR)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)

def now
  Process.clock_gettime(Process::CLOCK_MONOTONIC)
end

def termios(slave)
  reader, writer = IO.pipe
  child = fork do
    reader.close
    STDIN.reopen(slave)
    STDOUT.reopen(writer)
    STDERR.reopen('/dev/null')
    exec('/bin/stty', '-g')
  end
  writer.close
  value = reader.read
  reader.close
  _pid, status = Process.waitpid2(child)
  raise "stty failed status=#{status.inspect}" unless status.success?
  value.strip
ensure
  reader&.close rescue nil
  writer&.close rescue nil
end

def set_winsize(master)
  master.ioctl(0x80087467, [ROWS, COLS, 0, 0].pack('S4'))
rescue StandardError
  nil
end

# Keep a fragment buffer because OpenTUI can split the cursor-position query
# across reads. The response is sent to the PTY master, as in the established
# native harnesses, rather than treating ANSI output as readiness evidence.
def drain(master, seconds, query_state)
  output = +''.b
  deadline = now + seconds
  while (left = deadline - now) > 0
    next unless IO.select([master], nil, nil, [left, 0.1].min)
    begin
      chunk = master.read_nonblock(65_536)
      output << chunk
      query_state << chunk
      if query_state.include?("\e[6n")
        master.write("\e[1;1R")
        master.flush
        query_state.clear
      else
        query_state.replace(query_state.byteslice(-32, 32).to_s)
      end
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO
      break
    end
  end
  output
end

def backend(data)
  path = File.join(data, 'runtime', 'backend.json')
  return nil unless File.file?(path)
  JSON.parse(File.read(path))
rescue JSON::ParserError, Errno::ENOENT, Errno::ENOTDIR
  nil
end

def process_alive?(pid)
  Process.kill(0, Integer(pid))
  true
rescue Errno::ESRCH, Errno::EPERM, Errno::EINVAL, TypeError, ArgumentError
  false
end

def stop_process(pid, timeout = 3)
  return unless pid && process_alive?(pid)
  Process.kill('TERM', pid) rescue nil
  deadline = now + timeout
  until !process_alive?(pid) || now >= deadline
    sleep 0.05
  end
  Process.kill('KILL', pid) rescue nil if process_alive?(pid)
end

def cleanup_owned_daemon(descriptor)
  pid = descriptor && descriptor['pid']
  return true unless pid && Integer(pid).positive?
  stop_process(Integer(pid))
  !process_alive?(pid)
rescue ArgumentError, TypeError
  false
end

def isolated_env(root, data)
  xdg = File.join(root, 'xdg')
  native_lib = File.expand_path('../ui-native/lib', File.dirname(BINARY))
  {
    'HOME' => File.join(root, 'home'),
    'XDG_CONFIG_HOME' => File.join(xdg, 'config'),
    'XDG_DATA_HOME' => File.join(xdg, 'data'),
    'XDG_STATE_HOME' => File.join(xdg, 'state'),
    'XDG_CACHE_HOME' => File.join(xdg, 'cache'),
    'OPENCODE_CONFIG_DIR' => File.join(root, 'config'),
    'OPENCODE_RK_CONFIG_DIR' => File.join(root, 'rk-config'),
    'OPENCODE_DATA_DIR' => data, 'OPENCODE_RK_HOME' => data,
    'TERM' => 'xterm-256color', 'COLUMNS' => COLS.to_s, 'LINES' => ROWS.to_s,
    'PATH' => '/usr/bin:/bin', 'DYLD_LIBRARY_PATH' => NATIVE_LIBRARY,
    'LD_LIBRARY_PATH' => NATIVE_LIBRARY
  }
end

def wait_ready(data, master, query_state, output)
  deadline = now + 15
  loop do
    descriptor = backend(data)
    begin
      if descriptor && Integer(descriptor['pid']) > 0 && process_alive?(descriptor['pid'])
        return [descriptor, output]
      end
    rescue ArgumentError, TypeError
      # A partially-written descriptor is not readiness.
    end
    raise 'native client never reached authenticated daemon readiness' if now >= deadline
    output << drain(master, 0.15, query_state)
  end
end

def write_case_artifacts(root, result, output)
  File.binwrite(File.join(root, 'pty-output.ansi'), output.to_s.b)
  File.write(File.join(root, 'result.json'), JSON.pretty_generate(result))
rescue StandardError => error
  warn "unable to persist #{root}: #{error.class}: #{error.message}"
end

def run_case(label, signal)
  root = File.join(RUN_DIR, label)
  data = File.join(root, 'data')
  project = File.join(root, 'project')
  FileUtils.mkdir_p([data, project, File.join(root, 'home'), File.join(root, 'xdg'),
                     File.join(root, 'config'), File.join(root, 'rk-config')])
  master = slave = pid = nil
  descriptor = nil
  status = nil
  before = after = nil
  query_state = +''.b
  output = +''.b
  begin
    master, slave = PTY.open
    set_winsize(master)
    before = termios(slave)
    pid = Process.spawn(isolated_env(root, data), BINARY, '--native', '--data-dir', data,
                        in: slave, out: slave, err: slave, chdir: project,
                        unsetenv_others: true)
    output << drain(master, 1, query_state)
    descriptor, output = wait_ready(data, master, query_state, output)
    master.write(signal)
    master.flush
    deadline = now + 12
    until status
      waited, candidate = Process.waitpid2(pid, Process::WNOHANG)
      status = candidate if waited
      break if status
      raise "#{label}: client did not exit" if now >= deadline
      output << drain(master, 0.1, query_state)
    end
    output << drain(master, 0.5, query_state)
    after = termios(slave) # same slave FD remains open for both snapshots
    cleanup = cleanup_owned_daemon(descriptor)
    result = {
      'label' => label, 'client_pid' => pid, 'client_exited' => true,
      'client_status' => status.to_i, 'daemon_pid' => descriptor['pid'],
      'daemon_ready' => true, 'termios_before' => before, 'termios_after' => after,
      'termios_restored' => before == after,
      'owned_daemon_cleanup' => cleanup,
      'owned_daemon_alive_after_cleanup' => process_alive?(descriptor['pid']),
      'output_sha256' => Digest::SHA256.hexdigest(output)
    }
    write_case_artifacts(root, result, output)
    result
  rescue StandardError => error
    result = {
      'label' => label, 'client_pid' => pid, 'client_exited' => !status.nil?,
      'client_status' => status&.to_i, 'daemon_pid' => descriptor && descriptor['pid'],
      'daemon_ready' => !descriptor.nil?, 'termios_before' => before,
      'termios_after' => after, 'termios_restored' => !before.nil? && before == after,
      'owned_daemon_cleanup' => false, 'owned_daemon_alive_after_cleanup' => true,
      'error' => "#{error.class}: #{error.message}", 'error_backtrace' => error.backtrace&.first(8)
    }
    write_case_artifacts(root, result, output)
    result
  ensure
    stop_process(pid) if pid && !status
    cleanup_owned_daemon(descriptor) if descriptor
    master&.close rescue nil
    slave&.close rescue nil
  end
end

def startup_failure
  root = File.join(RUN_DIR, 'startup-failure')
  project = File.join(root, 'project')
  bad_data = File.join(root, 'not-a-directory')
  FileUtils.mkdir_p(project)
  File.write(bad_data, "disposable startup failure fixture\n")
  master = slave = pid = nil
  query_state = +''.b
  output = +''.b
  status = nil
  before = after = nil
  begin
    master, slave = PTY.open
    set_winsize(master)
    before = termios(slave)
    pid = Process.spawn(isolated_env(root, bad_data), BINARY, '--native', '--data-dir', bad_data,
                        in: slave, out: slave, err: slave, chdir: project,
                        unsetenv_others: true)
    deadline = now + 4
    until status
      waited, candidate = Process.waitpid2(pid, Process::WNOHANG)
      status = candidate if waited
      break if status
      break if now >= deadline
      output << drain(master, 0.1, query_state)
    end
    # Invalid data may intentionally enter an offline/error TUI. Preserve that
    # behavior as evidence and use the ordinary client quit action, rather than
    # incorrectly calling the still-running TUI a startup hang.
    unless status
      master.write("\x04")
      master.flush
      deadline = now + 4
      until status
        waited, candidate = Process.waitpid2(pid, Process::WNOHANG)
        status = candidate if waited
        break if status
        break if now >= deadline
        output << drain(master, 0.1, query_state)
      end
    end
    output << drain(master, 0.2, query_state)
    after = termios(slave)
    descriptor = backend(bad_data)
    result = {
      'label' => 'startup_failure', 'client_pid' => pid,
      'client_exited' => !status.nil?, 'nonzero_status' => status && !status.success?,
      'offline_probe_quit_sent' => true, 'daemon_absent' => descriptor.nil?,
      'termios_before' => before, 'termios_after' => after,
      'termios_restored' => before == after,
      'output_sha256' => Digest::SHA256.hexdigest(output)
    }
    write_case_artifacts(root, result, output)
    result
  rescue StandardError => error
    result = {
      'label' => 'startup_failure', 'client_pid' => pid,
      'client_exited' => !status.nil?, 'nonzero_status' => status && !status.success?,
      'daemon_absent' => backend(bad_data).nil?, 'termios_before' => before,
      'termios_after' => after, 'termios_restored' => !before.nil? && before == after,
      'error' => "#{error.class}: #{error.message}", 'error_backtrace' => error.backtrace&.first(8)
    }
    write_case_artifacts(root, result, output)
    result
  ensure
    stop_process(pid) if pid && !status
    master&.close rescue nil
    slave&.close rescue nil
  end
end

def run_invalid_cli_case(root)
  project = File.join(root, 'project')
  FileUtils.mkdir_p(project)
  master = slave = pid = nil
  status = nil
  before = after = nil
  query_state = +''.b
  output = +''.b
  begin
    master, slave = PTY.open
    set_winsize(master)
    before = termios(slave)
    pid = Process.spawn(isolated_env(root, File.join(root, 'data')), BINARY,
                        '--phase1-invalid-cli-argument', in: slave, out: slave, err: slave,
                        chdir: project, unsetenv_others: true)
    deadline = now + 8
    until status
      waited, candidate = Process.waitpid2(pid, Process::WNOHANG)
      status = candidate if waited
      break if status
      break if now >= deadline
      output << drain(master, 0.1, query_state)
    end
    after = termios(slave)
    result = {
      'label' => 'invalid_cli_argument', 'client_pid' => pid,
      'client_exited' => !status.nil?, 'nonzero_status' => status && !status.success?,
      'daemon_absent' => backend(File.join(root, 'data')).nil?,
      'termios_before' => before, 'termios_after' => after,
      'termios_restored' => !before.nil? && before == after,
      'output_sha256' => Digest::SHA256.hexdigest(output)
    }
    write_case_artifacts(root, result, output)
    result
  rescue StandardError => error
    result = {
      'label' => 'invalid_cli_argument', 'client_pid' => pid,
      'client_exited' => !status.nil?, 'nonzero_status' => status && !status.success?,
      'daemon_absent' => backend(File.join(root, 'data')).nil?,
      'termios_before' => before, 'termios_after' => after,
      'termios_restored' => !before.nil? && before == after,
      'error' => "#{error.class}: #{error.message}", 'error_backtrace' => error.backtrace&.first(8)
    }
    write_case_artifacts(root, result, output)
    result
  ensure
    stop_process(pid) if pid && !status
    master&.close rescue nil
    slave&.close rescue nil
  end
end

results = {}
begin
  results['normal_exit'] = run_case('normal-exit', "\x04")
  results['interrupt'] = run_case('interrupt', "\x03")

  # Preserve the originally observed invalid-data behavior as evidence, but do
  # not demand that a product which enters an offline/error TUI auto-exit.
  # The deterministic startup-failure contract is an invalid CLI argument.
  results['invalid_data_offline_probe'] = startup_failure

  bad_root = File.join(RUN_DIR, 'invalid-cli-argument')
  FileUtils.mkdir_p(bad_root)
  results['invalid_cli_argument'] = run_invalid_cli_case(bad_root)
rescue StandardError => error
  results['harness_error'] = "#{error.class}: #{error.message}"
  results['harness_backtrace'] = error.backtrace&.first(8)
ensure
  results['binary'] = BINARY
  results['binary_sha256'] = Digest::SHA256.file(BINARY).hexdigest
  results['native_library'] = NATIVE_LIBRARY
  library = results['native_library']
  results['native_library_directory_digest'] = if File.file?(library)
  # Kept for compatibility with an accidentally file-valued library path;
  # the child still receives the same resolved directory below.
  Digest::SHA256.file(library).hexdigest
  elsif File.directory?(library)
  entries = Dir[File.join(library, '**', '*')].select { |path| File.file?(path) }.sort
  Digest::SHA256.hexdigest(entries.map { |path| "#{path.delete_prefix(library)}\0#{Digest::SHA256.file(path).hexdigest}\n" }.join)
  else
  nil
  end
  results['native_library_file'] = NATIVE_LIBRARY_FILE
  results['native_library_file_resolved'] = if File.file?(NATIVE_LIBRARY_FILE)
    File.realpath(NATIVE_LIBRARY_FILE)
  else
    NATIVE_LIBRARY_FILE
  end
  results['native_library_file_sha256'] = if File.file?(NATIVE_LIBRARY_FILE)
    Digest::SHA256.file(NATIVE_LIBRARY_FILE).hexdigest
  end
  begin
    results['predicates'] = {
      'normal_exit' => %w[client_exited daemon_ready termios_restored owned_daemon_cleanup owned_daemon_alive_after_cleanup],
      'interrupt' => %w[client_exited daemon_ready termios_restored owned_daemon_cleanup owned_daemon_alive_after_cleanup],
      'invalid_cli_argument' => %w[client_exited nonzero_status daemon_absent termios_restored]
    }
    results['all_required_checks_passed'] = %w[normal_exit interrupt invalid_cli_argument].all? do |label|
      result = results[label]
      result && results['predicates'][label].all? do |key|
        key == 'owned_daemon_alive_after_cleanup' ? result[key] == false : result[key] == true
      end
    end
  rescue StandardError => error
    results['all_required_checks_passed'] = false
    results['harness_error'] ||= "#{error.class}: #{error.message}"
  end
end
# This final write is deliberately unconditional: timeout/error evidence from
# every completed case and the aggregate outcome survive a later failure.
File.write(File.join(RUN_DIR, 'summary.json'), JSON.pretty_generate(results))
puts JSON.pretty_generate(results)
exit(results['all_required_checks_passed'] ? 0 : 1)
