#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Native PTY Verification Harness for Phase 1 OpenCode-rk convergence.
# Reconstructs the lost Ruby permission-matrix / import harness from the
# tracked evidence logs under target/phase1-convergence-20261021/.
#
# Scope: ONLY creates verification artifacts under
#   target/phase1-convergence-20261021/native-usability/pty-verification/
#   (untracked). No product edits, no Cargo mutations, no existing user DB
#   or preferences, no daemon port 4096 touch.
#
# Usage:
#   ruby3 pty_verification_harness.rb <binary_path> <output_dir> <label>
#
# The binary must be the real native `oc2` build. The harness exercises:
#   1. Six permission choices via real mouse input (SGR protocol)
#   2. Import first-offer / decline / Settings reopen / Escape routing
#   3. All effects verified, not glyph-only.
#
# Dependencies: Ruby standard library only (PTY, JSON, Socket, Net::HTTP, FileUtils).

require 'pty'
require 'json'
require 'socket'
require 'net/http'
require 'fileutils'
require 'securerandom'
require 'tmpdir'
require 'digest'
require 'rbconfig'
require 'timeout'
require 'open3'
require 'shellwords'

# ---------------------------------------------------------------------------
# Configuration constants
# ---------------------------------------------------------------------------

TERMINAL_ROWS = (ENV['PTY_ROWS'] || ENV['PHASE1_ROWS'] || '32').to_i
TERMINAL_COLS = (ENV['PTY_COLS'] || ENV['PHASE1_COLS'] || '100').to_i
raise 'PTY_COLS must be at least 20' if TERMINAL_COLS < 20
raise 'PTY_ROWS must be positive' if TERMINAL_ROWS < 1

# The six documented permission decision choices:
# 0 Allow once, 1 Deny once, 2 Allow for this session,
# 3 Always allow, 4 Deny for this session, 5 Always deny
PERMISSION_CHOICES = [
  'Allow once',             # index 0
  'Deny once',              # index 1
  'Allow for this session', # index 2
  'Always allow',           # index 3
  'Deny for this session',  # index 4
  'Always deny'             # index 5
].freeze

# ---------------------------------------------------------------------------
# Helper: PTY drain with bounded timeout
# ---------------------------------------------------------------------------

def drain_pty(reader, seconds)
  result = +''
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (remaining = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    break unless IO.select([reader], nil, nil, remaining)
    begin
      result << reader.read_nonblock(65536)
    rescue IO::WaitReadable
      # retry
    rescue EOFError, Errno::EIO
      break
    end
  end
  result.force_encoding('UTF-8').scrub
end

# Drain only newly available PTY bytes into the accumulated evidence buffer.
# The block form is passed to HTTP waits so the TUI keeps painting while the
# daemon/provider work is in flight; a blocked PTY reader can otherwise stall
# the renderer and hide the permission dialog.
def pump_pty(reader, output, seconds = 0.1)
  chunk = drain_pty(reader, seconds)
  output << chunk unless chunk.empty?
  chunk
rescue StandardError
  +''
end

def wait_for_provider_request(provider, path, seconds = 8)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  until provider.requests.any? { |request| request[:path] == path }
    yield if block_given?
    break if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    sleep 0.03
  end
  unless provider.requests.any? { |request| request[:path] == path }
    raise "provider request #{path} not observed within #{seconds}s " \
          "(server_errors=#{provider.server_errors.inspect})"
  end
  true
end

# ---------------------------------------------------------------------------
# Helper: Send an SGR mouse event through the PTY
# ---------------------------------------------------------------------------

def send_sgr_mouse(writer, button, x, y, press: true)
  terminator = press ? 'M' : 'm'
  # The native decoder consumes SGR (1006) mouse packets, including the
  # mandatory '<' introducer.  Do not use the legacy X10 packet shape here:
  # it is silently treated as ordinary input by the native TUI.
  seq = "\e[<#{button};#{x + 1};#{y + 1}#{terminator}"
  writer.write(seq)
  writer.flush
end

# ---------------------------------------------------------------------------
# Fake HTTP provider server (deterministic, stdlib only)
# ---------------------------------------------------------------------------

class FakeProviderServer
  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @requests = []
    @stop = false
    @thread = Thread.new { serve }
    @tool_path = nil
    @tool_content = nil
    @followup_content = nil
    @responses = []
    @server_errors = []
    @response_phase = :initial
    @call_sequence = 0
    @restart_mode = false
  end

  attr_reader :port, :requests
  attr_reader :responses
  attr_reader :server_errors
  attr_reader :followup_content

  def tool_requests
    @requests.select { |request| request[:method] == 'POST' && request[:path] == '/v1/responses' }
  end

  def wait_for_response(seconds = 5)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
    until @responses.any? && @responses.last[:response_sse]
      yield if block_given?
      break if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      sleep 0.02
    end
    entry = @responses.last
    if entry.nil? || entry[:response_sse].nil? || entry[:response_sse].empty?
      raise "provider response SSE not observed within #{seconds}s " \
            "(server_errors=#{@server_errors.inspect})"
    end
    entry
  end

  def clear_requests
    @requests.clear
    @responses.clear
    @response_phase = :initial
    @call_sequence = 0
  end

  def function_calls
    @responses.flat_map do |entry|
      entry[:response_sse].to_s.lines.grep(/^data: /).map do |line|
        begin
          event = JSON.parse(line.delete_prefix('data: ').strip)
          item = event['item']
          item if event['type'] == 'response.output_item.done' && item.is_a?(Hash) && item['type'] == 'function_call'
        rescue JSON::ParserError
          nil
        end
      end.compact
    end
  end

  def function_call_outputs
    @requests.select { |request| request[:path] == '/v1/responses' }.flat_map do |request|
      input = request[:body] && request[:body]['input']
      input.is_a?(Array) ? input.select { |item| item.is_a?(Hash) && item['type'] == 'function_call_output' } : []
    end
  end

  def configure_write(path, content)
    @tool_path = path
    @tool_content = content
    @followup_content = "#{content.chomp}\nFOLLOWUP_#{SecureRandom.hex(8)}\n"
  end

  def prepare_restart
    @restart_mode = true
  end

  def write_arguments
    # Native write tool schema is additionalProperties:false with exactly
    # path/content.  An extra key risks parser/validator rejection before the
    # permission broker is armed.
    { 'path' => @tool_path, 'content' => @tool_content }
  end

  def write_arguments_for_current_call(sequence = @call_sequence)
    content = @restart_mode ? @tool_content : (sequence <= 1 ? @tool_content : @followup_content)
    { 'path' => @tool_path, 'content' => content }
  end

  def url
    "http://127.0.0.1:#{@port}"
  end

  def stop
    @stop = true
    begin
      @server.close
    rescue StandardError
      nil
    end
    @thread.join(5)
  rescue StandardError => error
    @server_errors << { class: error.class.to_s, message: error.message, backtrace: error.backtrace&.first(5) }
    nil
  end

  private

  def serve
    until @stop
      begin
        socket = @server.accept_nonblock
      rescue IO::WaitReadable
        IO.select([@server], nil, nil, 0.1)
        retry
      rescue IOError
        break
      end
      handle_request(socket)
    end
  rescue StandardError => error
    @server_errors << { class: error.class.to_s, message: error.message, backtrace: error.backtrace&.first(10), phase: 'serve' }
    nil
  end

  def handle_request(socket)
    request = +''
    while !request.include?("\r\n\r\n") && request.bytesize < 8192
      begin
        chunk = socket.readpartial(8192)
        request << chunk
      rescue IO::WaitReadable
        IO.select([socket], nil, nil, 5)
        retry
      rescue EOFError
        break
      end
    end

    header_part, body_part = request.split("\r\n\r\n", 2)
    lines = header_part.lines
    request_line = lines.first || ''
    method, path = request_line.split(' ').first(2).map { |s| s || '' }

    content_length = header_part[/Content-Length: (\d+)/i, 1].to_i
    if body_part && content_length > 0
      while body_part.bytesize < content_length
        begin
          body_part << socket.readpartial([content_length - body_part.bytesize, 8192].min)
        rescue IO::WaitReadable
          IO.select([socket], nil, nil, 5)
          retry
        rescue EOFError
          break
        end
      end
    end

    parsed = body_part.nil? || body_part.empty? ? nil : begin
      JSON.parse(body_part)
    rescue JSON::ParserError
      nil
    end

    @requests << { method: method, path: path, model: parsed && parsed['model'], body: parsed }
    response = build_response(method, path, parsed)
    # Capture the exact bytes before any socket write so an empty/failed client
    # read can never erase the fixture-side evidence.
    if path == '/v1/responses'
      @responses << {
        request: @requests.last,
        response_full: response,
        response_sse: response.split("\r\n\r\n", 2).last
      }
    end
    socket.write(response)
    socket.close
  rescue StandardError => error
    @server_errors << {
      class: error.class.to_s,
      message: error.message,
      backtrace: error.backtrace&.first(10),
      method: method,
      path: path,
      request_body: parsed
    }
    begin
      socket.close
    rescue StandardError => close_error
      @server_errors << { class: close_error.class.to_s, message: "socket close failed: #{close_error.message}" }
    end
  end

  def build_response(_method, path, body)
    if path == '/v1/models'
      payload = JSON.generate({ data: [{ id: 'fixture-model', object: 'model', owned_by: 'fixture' }] })
      "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: #{payload.bytesize}\r\nConnection: close\r\n\r\n#{payload}"
    elsif path == '/v1/responses'
      # ResponsesStreamParser only recognizes a function call at
      # response.output_item.done/item.  The former fixture emitted an
      # invented `tool_call` event, so it could never arm the real permission
      # broker even when /v1/models succeeded.
      # Keep every response buffer mutable: this file intentionally runs with
      # frozen_string_literal and the SSE stream is assembled incrementally.
      # After the tool output, return a terminal text response. The harness
      # submits a distinct second user turn below, so the initial write is
      # observable before any follow-up call can overwrite it.
      input = body && body['input'].is_a?(Array) ? body['input'] : []
      latest_output = input.rindex { |item| item.is_a?(Hash) && item['type'] == 'function_call_output' }
      latest_user = input.rindex do |item|
        item.is_a?(Hash) && (item['role'] == 'user' || item['type'] == 'message' && item['role'] == 'user')
      end
      has_output = !latest_output.nil?
      is_followup_user_turn = has_output && !latest_user.nil? && latest_user > latest_output
      if has_output && !is_followup_user_turn
        @response_phase = :ready
        payload = +"event: response.output_text.delta\ndata: #{JSON.generate({ type: 'response.output_text.delta', delta: ' TOOL_OK' })}\n\n"
        payload << "event: response.completed\ndata: #{JSON.generate({ type: 'response.completed', response: { status: 'completed' } })}\n\n"
      else
        @response_phase = :followup
        @call_sequence += 1
        call_id = "fixture-call-#{@call_sequence}"
        item_id = "fixture-item-#{@call_sequence}"
        payload = +"event: response.output_text.delta\ndata: #{JSON.generate({ type: 'response.output_text.delta', delta: 'ONBOARDING_OK' })}\n\n"
        payload << "event: response.output_item.done\ndata: #{JSON.generate({
          type: 'response.output_item.done',
          item: { type: 'function_call', id: item_id, call_id: call_id, name: 'write', arguments: JSON.generate(write_arguments_for_current_call) }
        })}\n\n"
        payload << "event: response.completed\ndata: #{JSON.generate({ type: 'response.completed', response: { status: 'completed' } })}\n\n"
      end
      "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: #{payload.bytesize}\r\nConnection: close\r\n\r\n#{payload}"
    elsif path == '/v1/tool/bash' || path == '/tools/bash'
      payload = JSON.generate({ stdout: 'hello', stderr: '', exit_code: 0 })
      "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: #{payload.bytesize}\r\nConnection: close\r\n\r\n#{payload}"
    else
      payload = JSON.generate({ error: 'not found' })
      "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: #{payload.bytesize}\r\nConnection: close\r\n\r\n#{payload}"
    end
  end
end

# ---------------------------------------------------------------------------
# Disposable fixture environment (outside project tree, in system temp)
# ---------------------------------------------------------------------------

class DisposableEnv
  def initialize(label)
    prefix = "rk-pty-verification-#{label.gsub(/[^A-Za-z0-9_.-]/, '_')}-"
    @root = Dir.mktmpdir(prefix, Dir.tmpdir)
    @root_owner = fixture_root_identity(@root, prefix)
    setup_dirs
  rescue StandardError
    cleanup_owned_fixture_dir(@root, @root_owner) if @root && @root_owner
    raise
  end

  attr_reader :root, :root_owner

  def data_dir
    File.join(@root, 'data')
  end

  def config_dir
    File.join(@root, 'config')
  end

  def home_dir
    File.join(@root, 'home')
  end

  def state_dir
    File.join(@root, 'state')
  end

  def setup_dirs
    FileUtils.mkdir_p(data_dir)
    FileUtils.mkdir_p(config_dir)
    FileUtils.mkdir_p(home_dir)
    FileUtils.mkdir_p(state_dir)
    FileUtils.mkdir_p(File.join(root, 'xdg', 'config'))
    FileUtils.mkdir_p(File.join(root, 'xdg', 'data'))
  end

  def env_vars(extra = {})
    {
      'HOME' => home_dir,
      'XDG_CONFIG_HOME' => File.join(root, 'xdg', 'config'),
      'XDG_DATA_HOME' => File.join(root, 'xdg', 'data'),
      'XDG_STATE_HOME' => state_dir,
      'OPENCODE_RK_CONFIG_DIR' => config_dir,
      'XDG_CACHE_HOME' => File.join(root, 'cache'),
      'PATH' => '/usr/bin:/bin',
      'TERM' => 'xterm-256color',
      'COLUMNS' => TERMINAL_COLS.to_s,
      'LINES' => TERMINAL_ROWS.to_s,
    }.merge(extra)
  end

  def cleanup
    cleanup_owned_fixture_dir(@root, @root_owner)
  end

  def write_config(name, content)
    path = File.join(config_dir, name)
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, content)
  end

  def config_written?(name)
    File.exist?(File.join(config_dir, name))
  end

  def config_content(name)
    path = File.join(config_dir, name)
    File.exist?(path) ? File.read(path) : nil
  end
end

FixtureRoot = Struct.new(:path, :realpath, :device, :inode, :prefix)

def fixture_root_identity(path, prefix)
  expanded = File.expand_path(path)
  stat = File.lstat(expanded)
  realpath = File.realpath(expanded)
  temp_root = File.realpath(Dir.tmpdir)
  unless stat.directory? && !stat.symlink? &&
         (realpath == expanded || File.realpath(expanded) == realpath) &&
         File.realpath(File.dirname(realpath)) == temp_root && File.basename(realpath).start_with?(prefix)
    raise "fixture root ownership check failed: #{path}"
  end
  FixtureRoot.new(expanded, realpath, stat.dev, stat.ino, prefix).freeze
end

MAX_FIXTURE_EVIDENCE_BYTES = 256 * 1024
MAX_TARGET_EVIDENCE_BYTES = 64 * 1024

def snapshot_fixture_file(path, owner, max_bytes: MAX_FIXTURE_EVIDENCE_BYTES, include_content: false)
  expanded = File.expand_path(path)
  root_prefix = owner.path.end_with?(File::SEPARATOR) ? owner.path : owner.path + File::SEPARATOR
  raise "fixture evidence path outside owned root: #{path}" unless expanded.start_with?(root_prefix)

  root_stat = File.lstat(owner.path)
  raise "fixture evidence root changed: #{owner.path}" unless root_stat.directory? && !root_stat.symlink? && root_stat.dev == owner.device && root_stat.ino == owner.inode && File.realpath(owner.path) == owner.realpath

  relative_parts = expanded.delete_prefix(root_prefix).split(File::SEPARATOR)
  raise "invalid fixture evidence path: #{path}" if relative_parts.empty? || relative_parts.any? { |part| part.empty? || part == '.' || part == '..' }
  cursor = owner.path
  relative_parts[0...-1].each do |part|
    cursor = File.join(cursor, part)
    stat = File.lstat(cursor)
    raise "unsafe fixture evidence parent: #{cursor}" unless stat.directory? && !stat.symlink?
    canonical = File.realpath(cursor)
    raise "fixture evidence parent escaped root: #{cursor}" unless canonical == owner.realpath || canonical.start_with?(owner.realpath + File::SEPARATOR)
  end

  leaf = File.lstat(expanded)
  raise "fixture evidence is not a regular file: #{path}" unless leaf.file? && !leaf.symlink?
  File.open(expanded, File::RDONLY | File::NOFOLLOW | File::NONBLOCK) do |file|
    stat = file.stat
    raise "fixture evidence is not a regular file: #{path}" unless stat.file?
    raise "fixture evidence changed before read: #{path}" unless stat.dev == leaf.dev && stat.ino == leaf.ino
    raise "fixture evidence exceeds #{max_bytes} bytes: #{path}" if stat.size > max_bytes
    content = file.read(max_bytes + 1)
    raise "fixture evidence exceeds #{max_bytes} bytes: #{path}" if content.bytesize > max_bytes
    current = File.lstat(expanded)
    raise "fixture evidence changed during read: #{path}" unless current.file? && !current.symlink? && current.dev == stat.dev && current.ino == stat.ino
    record = { exists: true, size_bytes: content.bytesize, sha256: Digest::SHA256.hexdigest(content) }
    record[:content] = content.force_encoding('UTF-8').scrub if include_content
    record
  end
rescue Errno::ENOENT
  { exists: false }
rescue StandardError => error
  { exists: false, error: "#{error.class}: #{error.message}" }
end

def fixture_target_snapshot(path, expected_content, owner)
  return { exists: false } unless path
  record = snapshot_fixture_file(path, owner, max_bytes: MAX_TARGET_EVIDENCE_BYTES, include_content: true)
  return record unless record[:exists]
  record.merge(content_matches: record[:content].b == expected_content.b)
end

def snapshot_trial_fixture_files(trial_env)
  owner = trial_env.root_owner
  paths = {
    'custom-providers.json' => [File.join(trial_env.config_dir, 'custom-providers.json'), true, true],
    'permission-rules.json' => [File.join(trial_env.data_dir, 'permission-rules.json'), true],
    'model.json' => [File.join(trial_env.state_dir, 'opencode', 'model.json'), true],
    'backend.json' => [File.join(trial_env.data_dir, 'runtime', 'backend.json'), false]
  }
  paths.each_with_object({}) do |(name, (path, include_content)), evidence|
    begin
      record = snapshot_fixture_file(path, owner, include_content: include_content)
      if record[:exists] && name == 'backend.json'
        backend_record = snapshot_fixture_file(path, owner, include_content: true)
        descriptor = JSON.parse(backend_record.fetch(:content))
        record[:descriptor_keys] = descriptor.keys.sort
        record[:pid] = descriptor['pid']
        record[:http_origin] = descriptor['http_origin']
        record[:auth_token_present] = descriptor['auth_token'].is_a?(String) && !descriptor['auth_token'].empty?
      end
      evidence[name] = record
    rescue StandardError => error
      evidence[name] = { exists: false, error: "#{error.class}: #{error.message}" }
    end
  end
end

def fixture_snapshot_errors(evidence, required: [])
  errors = evidence.each_with_object([]) do |(name, record), found|
    found << "#{name}: #{record[:error]}" if record[:error]
    found << "#{name}: required fixture missing" if required.include?(name) && record[:exists] != true && !record[:error]
  end
  errors
end

def cleanup_owned_fixture_dir(path, owner)
  raise 'fixture root ownership record missing' unless owner
  expanded = File.expand_path(path)
  stat = File.lstat(expanded)
  realpath = File.realpath(expanded)
  temp_root = File.realpath(Dir.tmpdir)
  unless expanded == owner.path && realpath == owner.realpath &&
         File.realpath(File.dirname(realpath)) == temp_root && File.basename(realpath).start_with?(owner.prefix) &&
         stat.directory? && !stat.symlink? && stat.dev == owner.device && stat.ino == owner.inode
    raise "fixture root ownership changed: #{path}"
  end

  FileUtils.remove_entry_secure(realpath)
  gone = begin
    File.lstat(expanded)
    false
  rescue Errno::ENOENT
    true
  end
  raise "fixture cleanup could not verify removal: #{path}" unless gone
  { verified_gone: true }
rescue StandardError => error
  { verified_gone: false, error: "#{error.class}: #{error.message}" }
end

def permission_cleanup_self_test!
  prefix = "permission-cleanup-self-test-#{SecureRandom.hex(8)}-"
  owned_path = Dir.mktmpdir(prefix, Dir.tmpdir)
  owned = fixture_root_identity(owned_path, prefix)
  target = File.join(owned.path, 'canary.txt')
  canary = "FIXTURE_CANARY_#{SecureRandom.hex(8)}\n"
  File.binwrite(target, canary)
  snapshot = fixture_target_snapshot(target, canary, owned)
  raise 'fixture evidence hash/content self-test failed' unless
    snapshot[:sha256] == Digest::SHA256.hexdigest(canary) && snapshot[:content_matches]
  receipt = cleanup_owned_fixture_dir(owned.path, owned)
  raise 'owned fixture cleanup self-test failed' unless receipt[:verified_gone]

  unrelated_prefix = "permission-cleanup-unrelated-#{SecureRandom.hex(8)}-"
  unrelated_path = Dir.mktmpdir(unrelated_prefix, Dir.tmpdir)
  unrelated = fixture_root_identity(unrelated_path, unrelated_prefix)
  sentinel = File.join(unrelated.path, 'must-survive.txt')
  File.write(sentinel, 'unowned')
  denied = cleanup_owned_fixture_dir(unrelated.path, owned)
  raise 'unowned fixture protection self-test failed' if denied[:verified_gone] || File.read(sentinel) != 'unowned'
  cleanup = cleanup_owned_fixture_dir(unrelated.path, unrelated)
  raise 'self-test fixture cleanup failed' unless cleanup[:verified_gone]
  unrelated = nil

  oversized_prefix = "permission-cleanup-oversized-#{SecureRandom.hex(8)}-"
  oversized_root = Dir.mktmpdir(oversized_prefix, Dir.tmpdir)
  oversized_owner = fixture_root_identity(oversized_root, oversized_prefix)
  oversized = File.join(oversized_root, 'oversized.json')
  File.open(oversized, 'wb') { |file| file.write('x' * (MAX_FIXTURE_EVIDENCE_BYTES + 1)) }
  oversized_record = snapshot_fixture_file(oversized, oversized_owner)
  raise 'oversized evidence rejection self-test failed' unless oversized_record[:error]
  cleanup_owned_fixture_dir(oversized_root, oversized_owner)

  owner = nil
  daemon_root = Dir.mktmpdir("permission-cleanup-daemon-#{SecureRandom.hex(8)}-", Dir.tmpdir)
  owner = fixture_root_identity(daemon_root, File.basename(daemon_root).sub(/\d+-.+\z/, ''))
  daemon_data = File.join(daemon_root, 'data')
  daemon_bin = RbConfig.ruby
  FileUtils.mkdir_p(File.join(daemon_data, 'runtime'))
  fake_command = Shellwords.join(['/bin/echo', daemon_bin, '--data-dir', daemon_data])
  raise 'fixture matcher accepted executable in arguments' if fixture_process_matches?(fake_command, daemon_bin, File.realpath(daemon_data))
  daemon_pid = Process.spawn(daemon_bin, '-e', 'sleep 30', '--', '--data-dir', daemon_data, out: File::NULL, err: File::NULL)
  Process.kill(0, daemon_pid)
  File.write(File.join(daemon_data, 'runtime', 'backend.json'), JSON.generate({ pid: daemon_pid }))
  daemon_receipt = cleanup_fixture_daemon(daemon_data, daemon_bin)
  Process.waitpid(daemon_pid) rescue nil
  raise "fixture daemon cleanup self-test failed: #{daemon_receipt.inspect}" unless daemon_receipt[:verified_gone] && daemon_receipt[:daemon] == 'stopped' && daemon_receipt[:pids].include?(daemon_pid)
  cleanup_owned_fixture_dir(daemon_root, fixture_root_identity(daemon_root, File.basename(daemon_root).sub(/\d+-.+\z/, '')))
  daemon_root = nil
  symlink_root = Dir.mktmpdir("permission-cleanup-symlink-#{SecureRandom.hex(8)}-", Dir.tmpdir)
  symlink_owner = fixture_root_identity(symlink_root, File.basename(symlink_root).sub(/\d+-.+\z/, ''))
  outside = File.join(symlink_root, 'outside')
  FileUtils.mkdir_p(outside)
  outside_file = File.join(outside, 'kept.json')
  File.write(outside_file, '{"kept":true}')
  File.symlink(outside, File.join(symlink_root, 'escape'))
  escaped = snapshot_fixture_file(File.join(symlink_root, 'escape', 'kept.json'), symlink_owner)
  raise 'symlink-parent evidence escape self-test failed' unless escaped[:error]
  fifo = File.join(symlink_root, 'must-not-block.fifo')
  File.mkfifo(fifo, 0o600)
  fifo_record = Timeout.timeout(0.25) { snapshot_fixture_file(fifo, symlink_owner) }
  raise 'FIFO evidence snapshot blocked or accepted a non-regular file' unless fifo_record[:error]&.include?('not a regular file')
  cleanup_owned_fixture_dir(symlink_root, symlink_owner)

  contaminated = { 'fixture.json' => { exists: false, error: 'unsafe fixture evidence file' } }
  raise 'snapshot entry error self-test failed' unless fixture_snapshot_errors(contaminated).any?

  puts JSON.generate({ owned_cleanup: true, evidence_hash_before_cleanup: true, unowned_root_protected: true, fixture_daemon_stopped: true, symlink_parent_rejected: true, oversized_file_rejected: true, snapshot_entry_error_fails: true, fifo_rejected_without_blocking: true })
ensure
  cleanup_owned_fixture_dir(owned.path, owned) if defined?(owned) && owned
  cleanup_owned_fixture_dir(unrelated.path, unrelated) if defined?(unrelated) && unrelated
  if defined?(daemon_root) && daemon_root && File.exist?(daemon_root)
    prefix = File.basename(daemon_root).sub(/\d+-.+\z/, '')
    cleanup_owned_fixture_dir(daemon_root, fixture_root_identity(daemon_root, prefix))
  end
end

# ---------------------------------------------------------------------------
# Terminal output parser for rendered content
# ---------------------------------------------------------------------------

class TerminalScreen
  def initialize(raw, rows: TERMINAL_ROWS, cols: TERMINAL_COLS)
    @cells = Array.new(rows) { Array.new(cols) { { text: ' ', fg: nil, bg: nil } } }
    parse(raw)
  end

  def lines
    @cells.map { |row| row.map { |cell| cell[:text] }.join }
  end

  def find_row(text)
    lines.each_with_index.find { |line, _| line.include?(text) }&.last
  end

  def row_at(y)
    return nil if y < 0 || y >= @cells.length
    @cells[y].map { |cell| cell[:text] }.join
  end

  def cell_index(y, phrase)
    line = row_at(y)
    return nil unless line
    line.index(phrase)
  end

  private

  def parse(raw)
    x = 0
    y = 0
    fg = nil
    bg = nil
    raw.scan(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]|./m) do |token|
      if token.start_with?("\e[") && token.end_with?('m')
        params = token[2...-1].split(';').map(&:to_i)
        params = [0] if params.empty?
        i = 0
        while i < params.length
          case params[i]
          when 0 then fg = bg = nil
          when 1
            # bold - keep fg
          when 38, 48
            if params[i + 1] == 2
              color = params[i + 2, 3]
              fg = color if params[i] == 38
              bg = color if params[i] == 48
              i += 4
            end
          end
          i += 1
        end
      elsif token.start_with?("\e[") && token.end_with?('H')
        match = token.match(/\A\e\[(\d*);?(\d*)H\z/)
        if match
          y = [match[1].to_i - 1, 0].max
          x = [match[2].to_i - 1, 0].max
        end
      elsif token.start_with?("\e[") && token.end_with?('f')
        match = token.match(/\A\e\[(\d*);?(\d*)f\z/)
        if match
          y = [match[1].to_i - 1, 0].max
          x = [match[2].to_i - 1, 0].max
        end
      elsif token == "\e[2J"
        @cells.each { |row| row.each { |cell| cell[:text] = ' ' } }
      elsif token == "\r"
        x = 0
      elsif token == "\n"
        y += 1
      elsif token.ord >= 32
        token.each_char do |char|
          break unless y < @cells.length && x < @cells[y].length
          # The permission labels and footer are ASCII, but keeping combining
          # marks zero-width avoids shifting later hit targets in real output.
          width = char.match?(/[\p{Mn}\p{Me}]/) ? 0 : (char.match?(/[\u1100-\u115F\u2329\u232A\u2E80-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE10-\uFE19\uFE30-\uFE6F\uFF00-\uFF60\uFFE0-\uFFE6]/) ? 2 : 1)
          if width == 0
            @cells[y][[x - 1, 0].max][:text] << char if x.positive?
          else
            @cells[y][x] = { text: char, fg: fg, bg: bg }
            x += width
          end
        end
      end
    end
  end
end

# ---------------------------------------------------------------------------
# Helper: Spawn oc2 in a PTY and set the window size
# ---------------------------------------------------------------------------

def spawn_oc2(binary, env_vars, data_dir)
  workdir = env_vars.fetch('PHASE1_WORKDIR', Dir.pwd)
  Dir.chdir(workdir) do
    PTY.spawn(env_vars, binary, '--data-dir', data_dir, { unsetenv_others: true }) do |reader, writer, pid|
    writer.sync = true
    packed = [TERMINAL_ROWS, TERMINAL_COLS, 0, 0].pack('S4')
    begin
      reader.ioctl(0x80087467, packed) # TIOCSWINSZ
    rescue Errno::ENOTTY
      # PTY may not support ioctl; continue without
    end
      yield reader, writer, pid
    end
  end
rescue PTY::ChildExited
  nil
end

# ---------------------------------------------------------------------------
# Helper: Get daemon descriptor from backend.json
# ---------------------------------------------------------------------------

def read_backend_json(data_dir)
  path = File.join(data_dir, 'runtime', 'backend.json')
  raise "backend.json not found at #{path}" unless File.exist?(path)
  JSON.parse(File.read(path))
rescue JSON::ParserError => e
  raise "Failed to parse backend.json: #{e}"
end

def fixture_process_matches?(command, expected_binary, expected_data)
  argv = Shellwords.shellsplit(command) rescue []
  return false unless argv.first && File.realpath(argv.first) == File.realpath(expected_binary)
  argv.each_cons(2).any? do |left, right|
    next false unless left == '--data-dir'
    begin
      File.realpath(right) == expected_data
    rescue Errno::ENOENT
      false
    end
  end
rescue Errno::ENOENT, Errno::ENOTDIR
  false
end

def fixture_processes(expected_binary, expected_data)
  stdout, _stderr, status = Open3.capture3('/bin/ps', '-axo', 'pid=,command=')
  raise 'ps process listing failed' unless status.success?
  stdout.lines.map do |line|
    pid_text, command = line.strip.split(/\s+/, 2)
    next unless pid_text && command && fixture_process_matches?(command, expected_binary, expected_data)
    pid_text.to_i
  end.compact
end

def cleanup_fixture_daemon(data_dir, binary)
  expected_binary = File.expand_path(binary)
  expected_data = File.realpath(data_dir)
  descriptor_path = File.join(expected_data, 'runtime', 'backend.json')
  descriptor_stat = File.lstat(descriptor_path) rescue nil
  unless descriptor_stat
    running = fixture_processes(expected_binary, expected_data)
    return { verified_gone: false, daemon: 'missing_descriptor_process_found', pids: running } unless running.empty?
    return { verified_gone: true, daemon: 'not_started' }
  end
  unless descriptor_stat.file? && !descriptor_stat.symlink?
    return { verified_gone: false, daemon: 'unsafe_descriptor_type' }
  end

  descriptor = JSON.parse(File.read(descriptor_path))
  pid = descriptor['pid']
  return { verified_gone: false, daemon: 'invalid_pid' } unless pid.is_a?(Integer) && pid > 1
  owned_pids = fixture_processes(expected_binary, expected_data)
  return { verified_gone: true, daemon: 'already_gone', pid: pid } if owned_pids.empty?
  unless owned_pids.include?(pid)
    return { verified_gone: false, daemon: 'identity_mismatch', pid: pid, owned_pids: owned_pids }
  end

  owned_pids.each { |owned_pid| Process.kill('TERM', owned_pid) }
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
    remaining = fixture_processes(expected_binary, expected_data)
    return { verified_gone: true, daemon: 'stopped', pids: owned_pids, signal: 'TERM' } if remaining.empty?
    sleep 0.05
  end
  { verified_gone: false, daemon: 'term_timeout', pids: owned_pids, remaining_pids: fixture_processes(expected_binary, expected_data) }
rescue StandardError => error
  { verified_gone: false, daemon: 'cleanup_error', error: "#{error.class}: #{error.message}" }
end

def read_session_id(descriptor)
  response = api_request(descriptor, 'GET', '/api/sessions')
  raise "session listing failed (HTTP #{response[:status]})" unless response[:status] == 200
  sessions = JSON.parse(response[:body])['sessions']
  id = sessions.is_a?(Array) && sessions.first && sessions.first['id']
  raise 'session listing contained no session id' unless id.is_a?(String) && !id.empty?
  id
end

def read_session_info(descriptor, fallback_root)
  response = api_request(descriptor, 'GET', '/api/sessions')
  raise "session listing failed (HTTP #{response[:status]})" unless response[:status] == 200
  session = JSON.parse(response[:body])['sessions']&.first
  id = session && session['id']
  raise 'session listing contained no session id' unless id.is_a?(String) && !id.empty?
  actual_root = session['directory'] || fallback_root
  [id, File.realpath(actual_root)]
end

def destroy_fixture_session(descriptor, session_id)
  # There is intentionally no destructive session-delete route in the native
  # API. The disposable data directory owns the session; leave durable
  # evidence until the trial cleanup removes that fixture.
  nil
end

# ---------------------------------------------------------------------------
# Helper: Make an authenticated API request to the daemon
# ---------------------------------------------------------------------------

def api_request(descriptor, method, path, body = nil)
  uri = URI(descriptor['http_origin'] + path)
  http = Net::HTTP.new(uri.host, uri.port)
  http.open_timeout = 5
  http.read_timeout = 5
  request = case method
            when 'GET' then Net::HTTP::Get.new(uri)
            when 'POST' then Net::HTTP::Post.new(uri)
            when 'PUT' then Net::HTTP::Put.new(uri)
            else raise "unsupported method #{method}"
            end
  request['Authorization'] = 'Bearer ' + descriptor.fetch('auth_token')
  request['Content-Type'] = 'application/json' if body
  request.body = JSON.generate(body) if body
  response = http.request(request)
  { status: response.code.to_i, body: response.body }
end

# ---------------------------------------------------------------------------
# Helper: Find permission choice row from terminal output
# -----------------------------------------------------------------------

def find_permission_choice_row(raw_output, choice_label)
  screen = TerminalScreen.new(raw_output)
  screen.find_row(choice_label)
end

# ---------------------------------------------------------------------------
# Helper: Get column for permission choice from terminal output
# -----------------------------------------------------------------------

def find_permission_choice_col(raw_output, y_row, choice_label)
  screen = TerminalScreen.new(raw_output)
  line = screen.row_at(y_row)
  return nil unless line && line.include?(choice_label)
  line.index(choice_label)
end

def find_permission_footer(raw_output)
  screen = TerminalScreen.new(raw_output)
  screen.lines.each_with_index do |line, row|
    col = line.index('click here confirm')
    return [row, col] if col
  end
  nil
end

# ---------------------------------------------------------------------------
# Helper: Kill daemon process
# -----------------------------------------------------------------------

def kill_daemon(descriptor)
  return unless descriptor && descriptor['pid']
  begin
    Process.kill('TERM', descriptor['pid'])
    Process.wait(descriptor['pid'], 5)
  rescue StandardError
    nil
  end
end

def stop_pty_process(reader, writer, pid)
  begin
    writer.write("\x04")
    writer.flush
  rescue StandardError
    nil
  end
  begin
    drain_pty(reader, 0.5)
  rescue StandardError
    nil
  end
  begin
    Process.waitpid(pid, Process::WNOHANG)
  rescue StandardError
    nil
  end
  begin
    Process.kill('TERM', pid)
  rescue StandardError
    nil
  end
  begin
    Process.wait(pid)
  rescue StandardError
    nil
  end
end

def sha256_file(path)
  File.file?(path) ? Digest::SHA256.file(path).hexdigest : nil
rescue StandardError
  nil
end

def durable_restart_probe(binary, env_vars, data_dir, provider, label, choice_index, target_path,
                          results, old_descriptor, old_pid, reader, writer)
  raise 'durable restart only supports Always allow / Always deny' unless [3, 5].include?(choice_index)

  restart_content = "PTY_RESTART_#{label}_#{choice_index}_#{SecureRandom.hex(8)}\n"
  provider.configure_write(target_path, restart_content)
  provider.prepare_restart
  results[:restart_expected_content] = restart_content
  results[:restart_old_daemon_pid] = old_descriptor && old_descriptor['pid']

  # Stop the client first, then the fixture-owned daemon. Never touch a
  # process outside this trial's authenticated descriptor.
  stop_pty_process(reader, writer, old_pid)
  kill_daemon(old_descriptor)
  sleep 0.2

  restart_error = nil
  restart_descriptor = nil
  restart_pid = nil
  spawn_oc2(binary, env_vars, data_dir) do |new_reader, new_writer, new_pid|
    restart_pid = new_pid
    new_writer.sync = true
    new_raw = +' '
    new_raw.clear
    begin
      new_raw << drain_pty(new_reader, 4)
      restart_descriptor = read_backend_json(data_dir)
      results[:restart_new_daemon_pid] = restart_descriptor['pid']
      results[:restart_daemon_pid_distinct] = restart_descriptor['pid'] != results[:restart_old_daemon_pid]
      raise 'restart daemon PID was not distinct' unless results[:restart_daemon_pid_distinct]

      new_writer.write("write fixture restart\r")
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
      while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
        pump_pty(new_reader, new_raw, 0.1)
        calls = provider.function_calls
        if calls.length >= 3
          results[:restart_function_call] = calls.last
          break
        end
        sleep 0.05
      end
      calls = provider.function_calls
      restart_call = calls.last
      results[:restart_function_call_id] = restart_call && restart_call['id']
      results[:restart_call_id] = restart_call && restart_call['call_id']
      results[:restart_function_call_distinct] = calls.length >= 3 &&
                                                  calls[-2]['call_id'] != calls[-1]['call_id']
      results[:restart_permission_api_used] = false
      new_raw << drain_pty(new_reader, 1.0)
      results[:restart_dialog_visible] = TerminalScreen.new(new_raw).lines.any? do |line|
        line.include?('Allow once') && line.include?('Always deny')
      end
      output = provider.function_call_outputs.find { |item| item['call_id'] == results[:restart_call_id] }
      results[:restart_call_output] = output
      result_text = output && [output['output'], output['error']].compact.join(' ')
      results[:restart_denied_call_result] = result_text && result_text.match?(/denied|rejected|permission rule|not allowed/i)
      results[:restart_marker_written] = File.file?(target_path) && File.read(target_path) == restart_content
      results[:restart_rule_behavior_correct] = if choice_index == 3
        results[:restart_function_call_distinct] == true &&
          results[:restart_marker_written] == true && results[:restart_dialog_visible] == false
      else
        results[:restart_function_call_distinct] == true &&
          results[:restart_denied_call_result] == true && results[:restart_marker_written] == false &&
          results[:restart_dialog_visible] == false
      end
    rescue StandardError => error
      restart_error = "restart probe: #{error.class}: #{error.message}"
      results[:restart_raw_output] = new_raw[-4000..]
    ensure
      stop_pty_process(new_reader, new_writer, new_pid)
      kill_daemon(restart_descriptor) if restart_descriptor
    end
  end
  results[:restart_error] = restart_error
  results[:restart_scope_verified] = restart_error.nil? && results[:restart_rule_behavior_correct] == true
  results[:restart_raw_output] ||= nil
  results
end

# ---------------------------------------------------------------------------
# Permission Matrix Verification (6 choices via real mouse)
# -----------------------------------------------------------------------

# Expected effect for each permission choice, based on the evidence logs:
# - Allow once (0): tool executes, subsequent ops prompt again
# - Deny once (1): tool denied, no side effects, subsequent ops may proceed
# - Allow for this session (2): tool executes, no further prompts this session
# - Always allow (3): tool executes, saved rule so no future prompts
# - Deny for this session (4): tool denied, no future prompts this session
# - Always deny (5): tool denied, saved rule for future denials

def permission_expects_tool_execution(choice_index)
  # Allow once, Allow for session, Always allow => tool executes
  [0, 2, 3].include?(choice_index)
end

def permission_expects_persisted_rule(choice_index)
  # Always allow (3) and Always deny (5) persist rules
  [3, 5].include?(choice_index)
end

DENY_CHOICES = [1, 4, 5].freeze

# Verify the broker-owned permission decision and its actual file effect. A
# provider POST is required for every choice because the LLM must request the
# tool; it is never evidence that the tool executed.
def verify_permission_effects(descriptor, choice_index, provider, target_path, target_content)
  expects_persist = permission_expects_persisted_rule(choice_index)
  tool_requests = provider.tool_requests
  request_seen = !tool_requests.empty?
  file_exists = File.file?(target_path)
  file_content = file_exists ? File.read(target_path) : nil
  # Allow time for async tool execution to complete after permission grant
  unless file_exists || !permission_expects_tool_execution(choice_index)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
    while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
      sleep 0.1
      file_exists = File.file?(target_path)
      file_content = file_exists ? File.read(target_path) : nil
      break if file_exists
    end
  end
  write_effect = file_exists && file_content == target_content
  denied_effect = !file_exists

  {
    choice: choice_index,
    label: PERMISSION_CHOICES[choice_index],
    provider_requests_observed: tool_requests.length,
    provider_request_required: true,
    provider_request_observed: request_seen,
    target_path: target_path,
    target_exists: file_exists,
    target_content_exact: write_effect,
    tool_effect_correct: permission_expects_tool_execution(choice_index) ? write_effect : denied_effect,
    expects_persisted_rule: expects_persist,
    expected_side_effect: permission_expects_tool_execution(choice_index) ? 'fixture_file_written' : 'fixture_file_absent',
    # scope_verified is NOT set here - it is set by the followup phase in run_single_permission_trial
  }
end

def tool_result_evidence(raw_output)
  cleaned = raw_output.gsub(/\e\][^\a]*(?:\a|\e\\)/, '')
  cleaned.scan(/(?:write denied|write rejected|write cancelled|tool.completed|permission denied|TOOL_OK|error: [^\r\n]*)/i).last(20)
end

def capture_session_observations(descriptor, session_id)
  observations = {}
  %W[/api/sessions/#{session_id}/messages?limit=200 /api/session/#{session_id}/permission].each do |path|
    response = api_request(descriptor, 'GET', path)
    observations[path] = { status: response[:status], body: response[:body] }
  end
  # /api/event and /events are long-lived SSE streams; a full-body HTTP
  # request would intentionally block until daemon shutdown. Record that the
  # supported route was identified, while messages/permission remain bounded
  # request-response observations.
  observations['event_routes'] = ['/api/event', '/events']
  observations
rescue StandardError => error
  { error: "observation failed: #{error.class}: #{error.message}" }
end

# ---------------------------------------------------------------------------
# Permission Matrix: spawn binary, trigger a tool request, click a choice, verify
# ---------------------------------------------------------------------------

def run_single_permission_trial(binary, env_vars, data_dir, choice_index, provider, label)
  results = { label: label, choice_index: choice_index, choice: PERMISSION_CHOICES[choice_index] }
  choice_y = nil
  choice_x = nil
  provider_configured = false
  # The daemon's project root is set explicitly through POST /api/sessions
  # (the supported session-directory API), rather than inferred from cwd.
  trial_root = File.dirname(data_dir)
  fixture_root = File.join(trial_root, 'project-root')
  outside_root = nil
  outside_owner = nil
  target_path = nil
  target_content = nil

  begin
  outside_prefix = "rk-pty-outside-#{label.gsub(/[^A-Za-z0-9_.-]/, '_')}-#{choice_index}-"
  outside_root = Dir.mktmpdir(outside_prefix, Dir.tmpdir)
  outside_owner = fixture_root_identity(outside_root, outside_prefix)
  FileUtils.mkdir_p(fixture_root)
  target_path = File.join(outside_root, "write-#{label}-#{choice_index}.txt")
  target_content = "PTY_PERMISSION_#{label}_#{choice_index}_#{SecureRandom.hex(8)}\n"
  provider.configure_write(target_path, target_content)
  results[:fixture_root] = fixture_root
  results[:outside_root] = outside_root
  results[:target_path] = target_path
  results[:expected_provider_write_arguments] = provider.write_arguments

  # Pre-write provider config directly (bypass fragile UI flow)
  config_dir = env_vars['OPENCODE_CONFIG_DIR']
  FileUtils.mkdir_p(config_dir)
  provider_config = {
    providers: {
      '9router' => {
        'base_url' => provider.url + '/v1',
        'api_key' => '9router',
        'type' => '9router'
      }
    }
  }
  config_path = File.join(config_dir, 'custom-providers.json')
  File.write(config_path, JSON.generate(provider_config))

  spawn_oc2(binary, env_vars, data_dir) do |reader, writer, pid|
    descriptor = nil
    raw_output = +''

    begin
      raw_output << drain_pty(reader, 4)

      # Read backend descriptor
      begin
        descriptor = read_backend_json(data_dir)
      rescue StandardError => e
        raw_output << drain_pty(reader, 1)
        File.write(File.join(data_dir, 'error.log'), "backend.json read failed: #{e}\n#{raw_output}")
        results[:error] = "backend.json read failed: #{e}"
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      # Verify not reusing foreign port 4096
      if descriptor['http_origin'] == 'http://127.0.0.1:4096'
        results[:error] = 'reused foreign port 4096'
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      results[:daemon_pid] = descriptor['pid']

      session_id, project_root = read_session_info(descriptor, env_vars.fetch('PHASE1_WORKDIR'))
      target_real = File.realpath(File.dirname(target_path))
      project_real = File.realpath(project_root)
      target_contained = target_real == project_real || target_real.start_with?(project_real + File::SEPARATOR)
      results[:session_id] = session_id
      results[:project_root] = project_real
      results[:target_parent_realpath] = target_real
      results[:target_parent_within_project_root] = target_contained
      results[:selected_session_id] = session_id
      listing_response = api_request(descriptor, 'GET', '/api/sessions')
      listed_ids = JSON.parse(listing_response[:body])['sessions'].to_a.map { |row| row['id'] }
      results[:selected_session_in_listing] = listed_ids.include?(session_id)
      results[:selected_session_id] = session_id
      raise 'fixture target unexpectedly contained by project root' if target_contained

      # Verify provider config was persisted
      if File.exist?(config_path)
        saved_config = JSON.parse(File.read(config_path))
        provider_configured = JSON.generate(saved_config).include?('9router')
      end
      results[:provider_saved] = provider_configured

      # PREREQUISITE: provider must be saved or we cannot verify tool execution
      unless provider_configured
        results[:error] = 'provider not saved - cannot verify tool execution'
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      # Verify model discovery against fake provider
      # Trigger model discovery by entering provider config in UI
      writer.write("/models \r")
      raw_output << drain_pty(reader, 2.0)

      # Verify provider was contacted for /v1/models
      model_discovery_ok = provider.requests.any? { |r| r[:path] == '/v1/models' }
      results[:model_discovery_ok] = model_discovery_ok

      # PREREQUISITE: provider must have been contacted
      unless model_discovery_ok
        results[:error] = 'provider not contacted for model discovery'
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      # Select the model to enable chat
      writer.write("fixture-model \r")
      raw_output << drain_pty(reader, 1.5)

      # Bind the explicitly created directory session. Native startup binds
      # the most recent session, so ensure the session-directory contract is
      # independently verified before issuing the turn.
      writer.write("/sessions \r")
      raw_output << drain_pty(reader, 0.5)
      # The command opens the native Sessions page; Escape returns focus to
      # the composer before the provider turn is submitted.
      writer.write("\e")
      raw_output << drain_pty(reader, 0.5)

      # Trigger a real write request. The target is deliberately outside the
      # project root used by the daemon, so bash auto-allow cannot mask the
      # file capability broker.
      # Submit a normal chat turn. The provider fixture itself requests the
      # absolute write operation; the composer text is intentionally harmless.
      writer.write("request fixture write\r")
      # Provider/daemon work is asynchronous.  Keep draining the PTY while
      # waiting for both the HTTP request and the response, otherwise the TUI
      # renderer blocks on a full PTY and never paints the permission dialog.
      pump = -> { pump_pty(reader, raw_output, 0.1) }
      wait_for_provider_request(provider, '/v1/responses', 10, &pump)
      response_entry = provider.wait_for_response(8, &pump)
      raise 'provider response SSE empty after tool request' if response_entry.nil?
      # Bounded post-response drain before the fresh evidence snapshot.
      raw_output << drain_pty(reader, 1.0)
      response_request = provider.tool_requests.last
      results[:provider_tool_arguments] = response_request && response_request[:body] &&
                                          response_request[:body]['tools']
      results[:provider_response_entry] = response_entry
      results[:provider_responses] = provider.responses.dup
      results[:provider_server_errors] = provider.server_errors.dup
      results[:provider_response_sse] = provider.responses.map { |entry| entry[:response_sse] }
      results[:provider_call_arguments] = provider.responses.map { |entry|
        entry[:request] && entry[:request][:body] && entry[:request][:body]['input']
      }
      results[:session_observations_after_response] = capture_session_observations(descriptor, session_id)

      # Poll the broker API as the authority; terminal glyphs alone are not
      # sufficient evidence that a pending request exists.
      permission = nil
      permission_poll_trace = []
      permission_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 8
      while Process.clock_gettime(Process::CLOCK_MONOTONIC) < permission_deadline
        response = api_request(descriptor, 'GET', "/api/session/#{session_id}/permission")
        permission_poll_trace << { status: response[:status], body: response[:body] }
        permission_poll_trace = permission_poll_trace.last(8)
        if response[:status] == 200
          parsed_permission = JSON.parse(response[:body])
          permission = parsed_permission['data']&.first
          break if permission
        end
        pump_pty(reader, raw_output, 0.1)
        sleep 0.03
      end
      results[:permission_poll] = permission
      results[:permission_poll_trace] = permission_poll_trace
      results[:tool_result_before_reply] = tool_result_evidence(raw_output)
      results[:permission_request_id] = permission && permission['id']
      results[:permission_action] = permission && permission['action']
      results[:permission_resource] = permission && permission['resources']
      raw_output << drain_pty(reader, 1.0)

      # PREREQUISITE: permission dialog must be visible and broker request
      # must be pending. Do not classify an absent screen before the write
      # request reaches the daemon.
      screen = TerminalScreen.new(raw_output)
      # The six-choice menu occupies separate rows; never require both labels
      # on the same rendered line.
      dialog_found = screen.lines.any? { |l| l.include?('Allow once') } &&
                     screen.lines.any? { |l| l.include?('Always deny') }
      results[:permission_dialog_visible] = dialog_found

      unless permission && dialog_found
        results[:error] = if permission.nil?
          'permission broker request not observed via GET /api/session/:id/permission'
        else
          'permission broker request observed but dialog not visible'
        end
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      # Find the permission choice row
      choice_row = find_permission_choice_row(raw_output, PERMISSION_CHOICES[choice_index])
      results[:choice_row_found] = !choice_row.nil?

      unless choice_row
        results[:error] = "could not find choice row for #{PERMISSION_CHOICES[choice_index]}"
        results[:raw_output] = raw_output[-2000..]
        return results
      end

      # Find the column for the choice label
      choice_col = find_permission_choice_col(raw_output, choice_row, PERMISSION_CHOICES[choice_index])
      unless choice_col
        results[:error] = "choice label #{PERMISSION_CHOICES[choice_index]} has no rendered cell column"
        return results
      end
      choice_x = choice_col
      choice_y = choice_row

      # Send mouse click on the chosen permission
      send_sgr_mouse(writer, 0, choice_x, choice_y, press: true)
      raw_output << drain_pty(reader, 0.2)
      send_sgr_mouse(writer, 0, choice_x, choice_y, press: false)
      raw_output << drain_pty(reader, 3.0)
      # Confirmation is deliberately mouse-only.  Locate the currently
      # rendered footer in the parsed screen rather than guessing a column or
      # using keyboard Enter (which would make this mixed-input evidence).
      footer = find_permission_footer(raw_output)
      results[:footer_found] = !footer.nil?
      unless footer
        results[:error] = 'rendered click here confirm footer not found after mouse selection'
        return results
      end
      footer_y, footer_x = footer
      send_sgr_mouse(writer, 0, footer_x, footer_y, press: true)
      raw_output << drain_pty(reader, 0.2)
      send_sgr_mouse(writer, 0, footer_x, footer_y, press: false)
      raw_output << drain_pty(reader, 1.0)

      permission_cleared = false
      clear_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
      while Process.clock_gettime(Process::CLOCK_MONOTONIC) < clear_deadline
        response = api_request(descriptor, 'GET', "/api/session/#{session_id}/permission")
        if response[:status] == 200
          pending = JSON.parse(response[:body])['data']
          # A once-scoped decision can immediately create the next broker
          # request.  The original request is settled when its id is absent;
          # requiring the whole queue to be empty would reject valid chained
          # tool calls.
          if pending.is_a?(Array) && !pending.any? { |entry| entry.is_a?(Hash) && entry['id'] == permission['id'] }
            permission_cleared = true
            break
          end
        end
        pump_pty(reader, raw_output, 0.1)
        sleep 0.03
      end
      results[:permission_cleared_after_reply] = permission_cleared
      raw_output << drain_pty(reader, 1.0)
      results[:tool_result_after_reply] = tool_result_evidence(raw_output)

      results[:raw_output] = raw_output[-3000..]
      results[:provider_requests] = provider.requests.dup

      # Record request count before effect verification
      tool_requests_before = provider.requests.select { |r| r[:path] == '/v1/responses' }.length
      results[:tool_calls_before_verify] = tool_requests_before

      # Verify effects - NO default scope_verified:true
      effects = verify_permission_effects(descriptor, choice_index, provider, target_path, target_content)
      results.merge!(effects)
      results[:tool_executed_correct] = results[:tool_effect_correct] == true &&
                                         results[:provider_request_observed] == true &&
                                         results[:permission_cleared_after_reply] == true

      # Follow-up verification uses a distinct user turn after the initial
      # provider turn is terminal. This prevents the fixture from overwriting
      # the initial marker before it is sampled and also covers engines where a
      # denied tool result terminates the turn without an automatic call.
      writer.write("write fixture followup\r")
      raw_output << drain_pty(reader, 1.0)
      followup_permission = nil
      followup_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 10
      while Process.clock_gettime(Process::CLOCK_MONOTONIC) < followup_deadline
        pump_pty(reader, raw_output, 0.1)
        response = api_request(descriptor, 'GET', "/api/session/#{session_id}/permission")
        if response[:status] == 200
          data = JSON.parse(response[:body])['data']
          followup_permission = data.first if data.is_a?(Array) && !data.empty?
          break if followup_permission || provider.function_calls.length >= 2
        end
        sleep 0.05
      end
      raw_output << drain_pty(reader, 1.0)
      results[:tool_result_followup] = tool_result_evidence(raw_output)

      followup_screen = TerminalScreen.new(raw_output)
      followup_pending = followup_screen.lines.any? { |l| l.include?('Allow once') } &&
                         followup_screen.lines.any? { |l| l.include?('Always deny') }
      results[:followup_prompts_again] = followup_pending
      results[:followup_permission_id] = followup_permission && followup_permission['id']
      results[:followup_permission_observed_via_api] = !followup_permission.nil?
      results[:initial_permission_id] = permission && permission['id']
      results[:followup_permission_distinct] = followup_permission && permission && followup_permission['id'] != permission['id']
      results[:followup_target_exists] = File.file?(target_path)

      # Follow-up scope is based on actual tool-call protocol and file effect,
      # never merely on another provider POST.  A once-scoped allow/deny must
      # produce a new function_call with a distinct id/call_id.
      calls = provider.function_calls
      results[:function_call_ids] = calls.map { |call| call['id'] }
      results[:function_call_call_ids] = calls.map { |call| call['call_id'] }
      results[:function_call_outputs] = provider.function_call_outputs
      followup_call_id = calls[1] && calls[1]['call_id']
      followup_output = provider.function_call_outputs.find { |output| output['call_id'] == followup_call_id }
      results[:followup_output_denied] = if followup_output
        [followup_output['output'], followup_output['error']].compact.join(' ').match?(/denied|rejected|permission rule|not allowed/i)
      else
        false
      end
      results[:distinct_followup_function_call] = calls.map { |call| call['id'] }.uniq.length >= 2 &&
                                                  calls.map { |call| call['call_id'] }.uniq.length >= 2
      second_call = results[:distinct_followup_function_call] == true
      # Compute actual followup tool outcome BEFORE scope verification so the
      # scope predicate gates on real effect, not just call structure.
      if [0, 2, 3].include?(choice_index)
        followup_file = File.file?(target_path) && File.read(target_path) == provider.followup_content
        results[:tool_executed_followup] = followup_file
      else
        followup_calls = calls.length >= 2
        followup_call_id = calls[1] && calls[1]['call_id']
        denied_outputs = provider.function_call_outputs.select { |output| output['call_id'] == followup_call_id }
        denied_result = denied_outputs.any? do |output|
          text = [output['output'], output['error']].compact.join(' ')
          text.match?(/denied|rejected|permission rule|not allowed/i)
        end
        results[:tool_executed_followup] = followup_calls && denied_result &&
                                           (!File.file?(target_path) || File.read(target_path) != provider.followup_content)
      end
      # Verify followup file outcome with EXACT filesystem predicates per scope.
      # Choice 0 (Allow once): file MUST exist with EXACT initial content
      # (not overwritten with FOLLOWUP); follows from pending re-prompt.
      # Choices 1,4,5 (Deny once/session/always): file MUST NOT exist.
      # Choices 2,3 (Allow session/always): file MUST exist with EXACT followup content.
      initial_file_correct = File.file?(target_path) && File.read(target_path) == target_content
      followup_file_has_marker = File.file?(target_path) && File.read(target_path) == provider.followup_content
      file_absent = !File.exist?(target_path)
      results[:initial_file_correct] = initial_file_correct
      results[:followup_file_has_marker] = followup_file_has_marker
      results[:followup_marker_absent] = !followup_file_has_marker
      results[:scope_verified] = case choice_index
                                  when 0
                                    # Allow once: second call pending new prompt + file retains EXACT initial content
                                    second_call && followup_pending && results[:followup_permission_observed_via_api] == true &&
                                      results[:followup_permission_distinct] == true &&
                                      initial_file_correct == true && results[:followup_marker_absent] == true
                                  when 1
                                    # Deny once: second call pending new prompt + file MUST NOT exist
                                    second_call && followup_pending && results[:followup_permission_observed_via_api] == true &&
                                      results[:followup_permission_distinct] == true &&
                                      file_absent == true
                                  when 2, 3
                                    # Allow session/always: second call, no prompt, file has EXACT FOLLOWUP content
                                    second_call && !followup_pending &&
                                      followup_file_has_marker == true
                                  when 4, 5
                                    # Deny session/always: second call denied, no prompt, file MUST NOT exist
                                    second_call && !followup_pending && results[:followup_output_denied] == true &&
                                      file_absent == true
                                  else
                                    false
                                  end

      # Check for persisted security rules
      # Look in standard OpenCode config location for permission rules
      rules_checked = false
      rules_found = false
      rules_path = env_vars['OPENCODE_RK_PERMISSION_RULES'] ||
                   File.join(env_vars.fetch('OPENCODE_RK_HOME', File.join(env_vars['HOME'], '.local', 'share', 'opencode-rk')), 'permission-rules.json')
      if File.exist?(rules_path)
        begin
          rules = JSON.parse(File.read(rules_path))
          rules_checked = true
          rules_found = rules.is_a?(Array) ? rules.any? : rules['rules'] && !rules['rules'].empty?
          results[:persisted_rule_count] = rules.is_a?(Array) ? rules.length : (rules['rules'] || []).length
        rescue StandardError
          rules_checked = true
          results[:persisted_rule_count] = 0
        end
      else
        results[:persisted_rule_count] = 0
      end
      results[:rules_path_checked] = rules_path
      results[:durable_rule_verified] = if [3, 5].include?(choice_index)
        rules_checked && rules_found
      else
        true
      end
      # A durable-file observation is not a restart test. A fresh-daemon
      # probe must independently establish this field later.
      results[:restart_scope_verified] = false
      results[:scope_verified] = false unless results[:durable_rule_verified]

      if [3, 5].include?(choice_index) && results[:durable_rule_verified]
        durable_restart_probe(binary, env_vars, data_dir, provider, label, choice_index,
                              target_path, results, descriptor, pid, reader, writer)
      else
        results[:restart_scope_verified] = nil
      end

    rescue => e
      results[:error] = "trial exception: #{e.class}: #{e.message}"
      results[:raw_output] = raw_output[-2000..]
      results[:provider_requests] = provider.requests.dup
    ensure
      # Preserve exact upstream interaction evidence even for early returns
      # (discovery/tool/permission failures).  Drain first, then snapshot so the
      # final record includes the last provider response/error and messages,
      # not a stale pre-turn view.  A missing dialog is not enough to diagnose
      # whether the provider, parser, or broker stalled.
      begin
        raw_output << drain_pty(reader, 0.5)
      rescue StandardError
        nil
      end
      results[:raw_output] = raw_output[-4000..] if raw_output && !raw_output.empty?
      results[:provider_requests] = provider.requests.dup
      results[:provider_responses] = provider.responses.dup
      results[:provider_response_sse] = provider.responses.map { |entry| entry[:response_sse] }
      results[:provider_server_errors] = provider.server_errors.dup
      begin
        if descriptor && session_id
          results[:session_observations_final] = capture_session_observations(descriptor, session_id)
        end
      rescue StandardError => capture_error
        results[:session_observations_final] = {
          error: "#{capture_error.class}: #{capture_error.message}"
        }
      end
      # Cleanup
      begin
        writer.write("\x04")
        drain_pty(reader, 0.3)
      rescue StandardError
        nil
      end
      Process.waitpid(pid, Process::WNOHANG) rescue nil
      Process.kill('TERM', pid) unless Process.waitpid(pid, Process::WNOHANG) rescue nil
      Process.wait(pid) rescue nil
    end
  end
  rescue StandardError => error
    results[:error] ||= "trial setup/cleanup exception: #{error.class}: #{error.message}"
  ensure
    begin
      results[:target_fixture_before_cleanup] = fixture_target_snapshot(target_path, target_content, outside_owner) if outside_owner && target_path
    rescue StandardError => snapshot_error
      results[:target_fixture_before_cleanup_error] = "#{snapshot_error.class}: #{snapshot_error.message}"
      results[:error] ||= 'target fixture evidence snapshot failed'
    ensure
      results[:target_fixture_cleanup] = cleanup_owned_fixture_dir(outside_root, outside_owner) if outside_owner
      results[:target_fixture_cleanup] ||= { verified_gone: false, error: 'outside fixture root was not created' }
      unless results.dig(:target_fixture_cleanup, :verified_gone) == true
        results[:error] ||= 'outside fixture cleanup failed'
      end
    end
  end

  results
end

# ---------------------------------------------------------------------------
# Main entry point: run all 6 permission choices
# ---------------------------------------------------------------------------

def main
  if ARGV == ['--self-test-fixture-cleanup']
    permission_cleanup_self_test!
    return
  end
  if ARGV.length < 3
    raise "Usage: ruby3 pty_verification_harness.rb <binary_path> <output_dir> <label>\n" \
          "  binary_path: path to target/debug/oc2\n" \
          "  output_dir: directory for evidence output\n" \
          "  label: identifier for this run (e.g. 'candidate')"
  end

  binary = ARGV[0]
  output_dir = ARGV[1]
  label = ARGV[2]
  default_lib_path = File.expand_path('../ui-native/lib', File.dirname(binary))
  lib_path = ENV['DYLD_LIBRARY_PATH']
  lib_path = default_lib_path unless lib_path && File.directory?(lib_path)

  FileUtils.mkdir_p(output_dir)

  all_results = []
  provider_errors = []

  # Start fake provider server
  provider = FakeProviderServer.new

  begin
  # Run each permission choice in a fresh disposable environment
  PERMISSION_CHOICES.each_with_index do |choice_label, choice_index|
    provider.clear_requests
    trial_env = nil
    trial_data_dir = nil
    result = nil
    begin
    trial_env = DisposableEnv.new("#{label}-#{choice_index}")
    trial_data_dir = trial_env.data_dir
    env_vars = trial_env.env_vars(
      'DYLD_LIBRARY_PATH' => lib_path,
      'OPENCODE_CONFIG_DIR' => trial_env.config_dir,
      'OPENCODE_RK_CONFIG_DIR' => trial_env.config_dir,
      # The server advertises/dispatches turn tools only from this explicit
      # deny-by-default allowlist. Without write here the provider call can
      # never reach the permission broker.
      'OPENCODE_RK_TURN_TOOLS' => 'write',
      'OPENCODE_RK_TUI_SUBMIT_KEY' => 'enter',
      'OPENCODE_DATA_DIR' => trial_data_dir,
      # The daemon's serve path sets OPENCODE_RK_HOME to its data directory;
      # pin the verifier to that same disposable durable-rule file explicitly.
      'OPENCODE_RK_PERMISSION_RULES' => File.join(trial_data_dir, 'permission-rules.json'),
      'PHASE1_WORKDIR' => File.join(trial_env.root, 'project-root')
    )
    FileUtils.mkdir_p(File.join(trial_env.root, 'project-root'))
    # The outside target is intentionally a sibling outside the session root;
    # it is created by run_single_permission_trial in the system temp area.

    # Seed the native consumer's recent selection. Discovery is still
    # required below; this only avoids fragile picker typing.
    state_path = File.join(trial_env.state_dir, 'opencode', 'model.json')
    FileUtils.mkdir_p(File.dirname(state_path))
    File.write(state_path, JSON.generate({
      'recent' => [{ 'providerID' => '9router', 'modelID' => 'fixture-model' }],
      'favorite' => [], 'variant' => {}
    }))

    puts "[harness] Running permission choice #{choice_index}: #{choice_label}..."
    result = run_single_permission_trial(binary, env_vars, trial_data_dir, choice_index, provider, label)
    result[:trial_data_dir] = trial_data_dir

    result_file = File.join(output_dir, "permission-footer-fix-20261022-mouse-#{choice_index}.json")
    File.write(result_file, JSON.pretty_generate(result))
    puts "[harness] Result: #{JSON.pretty_generate(result)}"

    all_results << result
    rescue StandardError => error
      result ||= { label: label, choice_index: choice_index, choice: choice_label }
      result[:error] ||= "trial orchestration exception: #{error.class}: #{error.message}"
      all_results << result unless all_results.include?(result)
    ensure
      result[:trial_data_dir] = trial_data_dir if result && trial_data_dir
      if trial_env
        begin
          result[:trial_fixture_evidence] = snapshot_trial_fixture_files(trial_env)
          required_fixtures = ['custom-providers.json', 'model.json']
          errors = fixture_snapshot_errors(result[:trial_fixture_evidence], required: required_fixtures)
          result[:trial_fixture_evidence_errors] = errors unless errors.empty?
          result[:error] ||= 'trial fixture evidence snapshot failed' unless errors.empty?
        rescue StandardError => evidence_error
          result[:trial_fixture_evidence_error] = "#{evidence_error.class}: #{evidence_error.message}"
          result[:error] ||= 'trial fixture evidence snapshot failed'
        end
        daemon_cleanup = cleanup_fixture_daemon(trial_data_dir, binary)
        result[:daemon_cleanup] = daemon_cleanup
        result[:error] ||= 'fixture daemon cleanup failed' unless daemon_cleanup[:verified_gone] == true
        if daemon_cleanup[:verified_gone] == true
          cleanup = trial_env.cleanup
          result[:trial_fixture_cleanup] = cleanup
          result[:error] ||= 'trial fixture cleanup failed' unless cleanup[:verified_gone] == true
        else
          result[:trial_fixture_cleanup] = { verified_gone: false, retained: true, reason: 'fixture daemon identity/exit was not verified' }
        end
      end
      result_file = File.join(output_dir, "permission-footer-fix-20261022-mouse-#{choice_index}.json")
      File.write(result_file, JSON.pretty_generate(result)) if result
      puts "[harness] Result: #{JSON.pretty_generate(result)}" if result
    end

    break if ENV['PHASE1_ONLY'] == '1'

    if result && result[:error]
      provider_errors << result[:error]
    end
  end
  rescue StandardError => error
    provider_errors << "orchestration: #{error.class}: #{error.message}"
  ensure
    provider.stop if provider
  end

  # Summary
  # All checks must pass: tool execution correct, no errors, provider saved, dialog seen
  passed = all_results.count do |r|
    r[:tool_executed_correct] == true &&
    r[:error].nil? &&
    r[:provider_saved] == true &&
    r[:permission_dialog_visible] == true &&
    r[:scope_verified] == true &&
    r[:tool_effect_correct] == true
  end
  total = all_results.length
  summary = {
    binary: binary,
    binary_sha256: sha256_file(binary),
    native_library: File.join(lib_path, 'libopentui.dylib'),
    native_library_sha256: sha256_file(File.join(lib_path, 'libopentui.dylib')),
    label: label,
    terminal_cols: TERMINAL_COLS,
    terminal_rows: TERMINAL_ROWS,
    total_trials: total,
    tool_execution_verified: passed,
    selected_trials_passed: total.positive? && passed == total,
    all_passed: total == PERMISSION_CHOICES.length && passed == total,
    durable_restart_all_passed: [3, 5].all? { |choice| all_results.any? { |r| r[:choice_index] == choice && r[:restart_scope_verified] == true } },
    results: all_results.map { |r| { choice: r[:choice], tool_executed: r[:tool_executed_correct], tool_effect: r[:tool_effect_correct], scope_verified: r[:scope_verified] == true, restart_scope_verified: r[:restart_scope_verified], error: r[:error], provider_saved: r[:provider_saved], dialog_visible: r[:permission_dialog_visible], permission_poll: r[:permission_poll], provider_requests: r[:provider_requests], followup_function_call: r[:distinct_followup_function_call], followup_outcome_verified: r[:scope_verified] == true, initial_file_correct: r[:initial_file_correct], followup_file_has_marker: r[:followup_file_has_marker], file_absent: r[:file_absent], denied_call_result: r[:followup_output_denied] == true } }
  }

  File.write(File.join(output_dir, 'summary.json'), JSON.pretty_generate(summary))
  puts JSON.pretty_generate(summary)

  all_passed = summary[:all_passed]
  puts "[harness] ALL PASS" if all_passed
  puts "[harness] SOME FAILED" unless all_passed
  exit(all_passed ? 0 : 1)
end

# Run main if called directly, not required
main if __FILE__ == $0
