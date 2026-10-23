#!/usr/bin/env ruby
# frozen_string_literal: true

# Cross-product configuration migration journey.  This deliberately uses the
# real native discovery/preview/apply path; the only fake component is the
# loopback OpenAI Responses-compatible provider used to prove that a Codex
# model survives Apply and is consumed by the runtime.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'socket'
require 'timeout'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/foreign-import-pty-verification', __dir__)
RUN_DIR = File.join(ARTIFACT_DIR, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY = ARGV[0] ? File.expand_path(ARGV[0]) : File.expand_path('../../../target/debug/oc2', __dir__)
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
FileUtils.mkdir_p(RUN_DIR)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && EXPECTED_SHA != BINARY_SHA
NATIVE_LIBRARY_DIR = ENV.fetch('DYLD_LIBRARY_PATH', File.expand_path('../../../target/ui-native/lib', __dir__))
NATIVE_LIBRARY_FILE = Dir[File.join(NATIVE_LIBRARY_DIR, 'libopentui*.dylib')].sort.first
raise "native library missing in #{NATIVE_LIBRARY_DIR}" unless NATIVE_LIBRARY_FILE && File.file?(NATIVE_LIBRARY_FILE)
NATIVE_LIBRARY_SHA = Digest::SHA256.file(NATIVE_LIBRARY_FILE).hexdigest

def set_pty_winsize(reader, rows = 32, cols = 100)
  # TIOCSWINSZ on macOS.  The environment is retained as a fallback, but the
  # PTY kernel window is the authoritative geometry for native rendering.
  reader.ioctl(0x80087467, [rows, cols, 0, 0].pack('S4'))
rescue SystemCallError => error
  raise "unable to set PTY winsize: #{error.message}"
end

class FakeResponsesProvider
  attr_reader :port

  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @requests = []
    @errors = []
    @mutex = Mutex.new
    @closed = false
    @thread = Thread.new { accept_loop }
  end

  def requests
    @mutex.synchronize { @requests.map(&:dup) }
  end

  def errors
    @mutex.synchronize { @errors.dup }
  end

  def close
    @closed = true
    @server.close rescue nil
    @thread.join(2)
    @thread.kill if @thread.alive?
  end

  private

  def accept_loop
    until @closed
      begin
        next unless IO.select([@server], nil, nil, 0.15)
        Thread.new(@server.accept) { |socket| serve(socket) }
      rescue IOError, Errno::EBADF
        break if @closed
      end
    end
  rescue StandardError => e
    @mutex.synchronize { @errors << "accept #{e.class}: #{e.message}" }
  end

  def serve(socket)
    request = read_request(socket)
    return unless request
    method, path, headers, body = request
    parsed = JSON.parse(body) unless body.empty?
    @mutex.synchronize { @requests << { 'method' => method, 'path' => path, 'headers' => headers, 'json' => parsed } }
    if method == 'GET' && path == '/v1/models'
      respond(socket, 200, 'application/json', JSON.generate('data' => [{ 'id' => 'fixture-model', 'name' => 'Codex fixture model' }]))
    elsif method == 'POST' && path == '/v1/responses'
      events = [
        ['response.created', { 'type' => 'response.created' }],
        ['response.output_text.delta', { 'type' => 'response.output_text.delta', 'delta' => 'codex import runtime reply' }],
        ['response.completed', { 'type' => 'response.completed', 'response' => { 'usage' => { 'input_tokens' => 1, 'output_tokens' => 4 } } }]
      ]
      body = events.map { |event, value| "event: #{event}\ndata: #{JSON.generate(value)}\n\n" }.join
      respond(socket, 200, 'text/event-stream', body)
    else
      respond(socket, 404, 'application/json', '{"error":"not found"}')
    end
  rescue StandardError => e
    @mutex.synchronize { @errors << "request #{e.class}: #{e.message}" }
  ensure
    socket.close rescue nil
  end

  def read_request(socket)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
    bytes = ''.b
    header_end = nil
    until header_end
      return nil unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096)
      raise 'request headers exceed 16 KiB' if bytes.bytesize > 16 * 1024
      header_end = bytes.index("\r\n\r\n")
    end
    head = bytes.byteslice(0, header_end).force_encoding('UTF-8').scrub
    lines = head.split("\r\n")
    method, path = lines.shift.to_s.split(' ', 3)
    headers = {}
    lines.each { |line| key, value = line.split(':', 2); headers[key.to_s.downcase] = value.to_s.strip }
    length = Integer(headers.fetch('content-length', '0'), 10)
    raise 'request body exceeds 1 MiB' if length > 1_048_576
    start = header_end + 4
    while bytes.bytesize < start + length
      return nil unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096)
    end
    [method, path, headers, bytes.byteslice(start, length).to_s.force_encoding('UTF-8').scrub]
  rescue EOFError
    nil
  end

  def respond(socket, status, content_type, body)
    reason = status == 200 ? 'OK' : 'Not Found'
    bytes = body.b
    socket.write("HTTP/1.1 #{status} #{reason}\r\nContent-Type: #{content_type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n")
    socket.write(bytes)
  end
end

class Screen
  attr_reader :rows
  def initialize(rows = 32, cols = 100); @rows = Array.new(rows) { ' ' * cols }; @x = 0; @y = 0; @esc = +''; end
  def feed(raw)
    raw.each_byte do |byte|
      char = byte.chr
      if @esc.empty?
        if byte == 27 then @esc = char
        elsif byte == 13 then @x = 0
        elsif byte == 10 then @y = [@y + 1, @rows.length - 1].min
        elsif byte >= 32 && byte < 127 then @rows[@y][@x] = char if @x < @rows[@y].length; @x = [@x + 1, @rows[@y].length - 1].min end
        elsif @esc == "\e" && char == '[' then @esc = +'CSI:'
      elsif @esc.start_with?('CSI:')
        @esc << char
        if char.ord.between?(0x40, 0x7e)
          body = @esc[4...-1].to_s; nums = body.split(';').map { |n| n.empty? ? 1 : n.to_i }
          case char
          when 'H', 'f' then @y = [[(nums[0] || 1) - 1, 0].max, @rows.length - 1].min; @x = [[(nums[1] || 1) - 1, 0].max, @rows.first.length - 1].min
          when 'J' then @rows.map! { ' ' * @rows.first.length }; @x = @y = 0
          when 'K' then @rows[@y] = @rows[@y][0...@x] + ' ' * (@rows[@y].length - @x)
          end
          @esc = ''
        end
      else @esc = '' end
    end
  end
  def text
    @rows.join("\n")
  end
end

def read_for(io, writer, seconds)
  $foreign_import_cursor_pending ||= {}
  pending = ($foreign_import_cursor_pending[io.object_id] ||= +'')
  out = +''; deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([io], nil, nil, [left, 0.15].min)
    begin
      chunk = io.read_nonblock(65_536); pending << chunk; pending = pending.byteslice(-32, 32).to_s.dup; $foreign_import_cursor_pending[io.object_id] = pending
      if pending.include?("\e[6n"); writer.write("\e[1;1R"); writer.flush; pending = +'' end
      out << chunk
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO then break end
  end
  out.force_encoding('UTF-8').scrub
end

def wait_for(io, writer, screen, needle, raw, timeout = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
  until screen.text.include?(needle)
    raise "timeout waiting for #{needle.inspect}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    chunk = read_for(io, writer, 0.25); raw << chunk; screen.feed(chunk)
  end
end

def stop_client(pid, io, writer)
  writer.write("\x03"); writer.flush; read_for(io, writer, 0.5)
  Process.kill('TERM', pid) rescue nil
  Timeout.timeout(2) { Process.wait(pid) }
rescue StandardError
  Process.kill('KILL', pid) rescue nil; Process.wait(pid) rescue nil
end

def stop_daemon(data_dir)
  descriptor = File.join(data_dir, 'runtime', 'backend.json')
  return unless File.file?(descriptor)
  pid = JSON.parse(File.read(descriptor))['pid'] rescue nil
  return unless pid.is_a?(Integer) && pid > 1
  Process.kill('TERM', pid) rescue nil
  sleep 0.1
  Process.kill('KILL', pid) rescue nil
end

def clean_ansi(value)
  value.gsub(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]/, '')
end

def run_apply(binary, env, data_dir, label)
  raw = +''; screen = Screen.new; result = { 'offer_visible' => false, 'preview_visible' => false, 'apply_visible' => false }
  PTY.spawn(env, binary, '--native', '--data-dir', data_dir, { unsetenv_others: true, chdir: env.fetch('PROJECT_DIR') }) do |reader, writer, pid|
    begin
      set_pty_winsize(reader)
      raw << read_for(reader, writer, 2); screen.feed(raw)
      wait_for(reader, writer, screen, 'Import settings from OpenCode?', raw)
      result['offer_visible'] = true; writer.write("\r"); writer.flush
      wait_for(reader, writer, screen, 'Import preview', raw); result['preview_visible'] = true
      writer.write("\r"); writer.flush; wait_for(reader, writer, screen, 'Import applied', raw)
      result['apply_visible'] = true
    ensure
      File.binwrite(File.join(RUN_DIR, "#{label}-apply.ansi"), raw)
      File.write(File.join(RUN_DIR, "#{label}-apply.txt"), clean_ansi(raw))
      stop_client(pid, reader, writer)
    end
  end
  result
rescue StandardError => e
  result['error'] = "#{e.class}: #{e.message}"
  result['backtrace'] = e.backtrace&.first(8)
  result['offer_visible'] = false unless result.key?('offer_visible')
  File.write(File.join(RUN_DIR, "#{label}-failure.txt"), clean_ansi(raw))
  result
end

results = {
  'binary' => BINARY, 'binary_sha256' => BINARY_SHA,
  'native_library' => NATIVE_LIBRARY_FILE, 'native_library_sha256' => NATIVE_LIBRARY_SHA,
  'raw_credentials_in_fixture' => false
}
provider = FakeResponsesProvider.new
at_exit do
  begin
    if $!
      results['uncaught_exception'] ||= "#{$!.class}: #{$!.message}"
      File.write(File.join(RUN_DIR, 'uncaught-failure.txt'), "#{$!.class}: #{$!.message}\n#{$!.backtrace&.join("\n")}\n")
    end
    results['provider_requests'] = provider.requests
    results['provider_errors'] = provider.errors
    results['artifact_dir'] = RUN_DIR
    results['required_checks'] ||= []
    results['all_required_checks_passed'] = results['error'].nil? && results['required_checks'].all? { |key| results[key] == true }
    File.write(File.join(RUN_DIR, 'summary.json'), JSON.pretty_generate(results) + "\n")
  rescue StandardError => error
    File.write(File.join(RUN_DIR, 'summary-failure.txt'), "#{error.class}: #{error.message}\n") rescue nil
  ensure
    provider.close rescue nil
  end
end
begin
  # --- Codex: source bytes -> discovery/preview/Apply -> real provider turn.
  codex_root = File.join(RUN_DIR, 'codex'); project = File.join(RUN_DIR, 'project'); codex_data = File.join(RUN_DIR, 'codex-data'); own = File.join(RUN_DIR, 'own-config')
  [codex_root, project, codex_data, own, File.join(project, '.claude', 'agents')].each { |path| FileUtils.mkdir_p(path) }
  base_url = "http://127.0.0.1:#{provider.port}/v1"
  codex_body = <<~TOML
    model = "fixture-model"
    model_provider = "codex_fixture"

    [model_providers.codex_fixture]
    name = "Codex fixture gateway"
    base_url = "#{base_url}"
    env_key = "CODEX_FIXTURE_API_KEY"
    wire_api = "responses"

    [agents.codex-reviewer]
    description = "Reviews imported Codex changes"
    config_file = "reviewer.toml"
  TOML
  codex_path = File.join(codex_root, 'config.toml'); File.binwrite(codex_path, codex_body)
  codex_hash = Digest::SHA256.file(codex_path).hexdigest
  data_dir = File.join(RUN_DIR, 'codex-runtime'); FileUtils.mkdir_p(data_dir)
  codex_env = {
    'HOME' => File.join(RUN_DIR, 'home-codex'), 'CODEX_HOME' => codex_root,
    'CLAUDE_CONFIG_DIR' => File.join(RUN_DIR, 'claude-unused'), 'XDG_CONFIG_HOME' => File.join(RUN_DIR, 'xdg-codex'),
    'XDG_DATA_HOME' => File.join(RUN_DIR, 'xdg-data-codex'), 'XDG_STATE_HOME' => File.join(RUN_DIR, 'xdg-state-codex'),
    'OPENCODE_CONFIG_DIR' => File.join(RUN_DIR, 'legacy-codex'), 'OPENCODE_RK_CONFIG_DIR' => own,
    'OPENCODE_DATA_DIR' => data_dir, 'OPENCODE_RK_HOME' => data_dir, 'PROJECT_DIR' => project,
    'TERM' => 'xterm-256color', 'COLUMNS' => '100', 'LINES' => '32',
    'OPENCODE_RK_TUI_SUBMIT_KEY' => 'enter',
    'DYLD_LIBRARY_PATH' => NATIVE_LIBRARY_DIR, 'PATH' => '/usr/bin:/bin'
  }
  # Keep an unrelated existing value and one nested agent leaf. Apply must
  # preserve both while still adding the imported normalized model/provider.
  File.write(File.join(own, 'opencode.json'), JSON.pretty_generate(
    'preexisting' => true,
    'agents' => { 'codex-reviewer' => { 'description' => 'existing description' } }
  ) + "\n")
  codex_apply = run_apply(BINARY, codex_env, data_dir, 'codex')
  results['codex_apply_error_nil'] = codex_apply['error'].nil?
  results['codex_apply_ui_complete'] = %w[offer_visible preview_visible apply_visible].all? { |key| codex_apply[key] == true }
  applied = File.join(own, 'opencode.json')
  applied_value = JSON.parse(File.read(applied))
  results['codex_apply'] = codex_apply
  results['codex_model_normalized'] = applied_value['model'] == 'codex_fixture/fixture-model'
  results['codex_provider_persisted'] = applied_value.dig('providers', 'codex_fixture', 'base_url') == base_url && applied_value.dig('providers', 'codex_fixture', 'api_key_env') == 'CODEX_FIXTURE_API_KEY'
  results['codex_agent_persisted'] = applied_value.dig('agents', 'codex-reviewer', 'mode') == 'subagent'
  results['codex_unsupported_diagnostic_source'] = File.read(File.join(RUN_DIR, 'codex-apply.txt')).include?('config_file')
  results['codex_source_unchanged'] = Digest::SHA256.file(codex_path).hexdigest == codex_hash
  results['existing_destination_preserved'] = applied_value['preexisting'] == true && applied_value.dig('agents', 'codex-reviewer', 'description') == 'existing description'
  # The next launch consumes the newly written provider/model; no manual model
  # selection or recent-state injection is used.
  codex_env['OPENCODE_RK_CONFIG_DIR'] = own
  raw = +''; screen = Screen.new
  PTY.spawn(codex_env, BINARY, '--native', '--data-dir', data_dir, { unsetenv_others: true, chdir: project }) do |reader, writer, pid|
    begin
      set_pty_winsize(reader)
      raw << read_for(reader, writer, 2); screen.feed(raw); deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 20
      until provider.requests.any? { |r| r['method'] == 'GET' && r['path'] == '/v1/models' } || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        chunk = read_for(reader, writer, 0.25); raw << chunk; screen.feed(chunk)
      end
      results['codex_catalogue_requested'] = provider.requests.any? { |r| r['method'] == 'GET' && r['path'] == '/v1/models' }
      # Submit only after the settled home frame; the native submit contract is
      # explicit so this cannot depend on an incidental key binding.
      sleep 0.2
      writer.write("ping"); writer.flush; sleep 0.1; writer.write("\r"); writer.flush
      post_started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      deadline = post_started + 10
      until provider.requests.any? { |r| r['method'] == 'POST' && r['path'] == '/v1/responses' } || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        chunk = read_for(reader, writer, 0.25); raw << chunk; screen.feed(chunk)
      end
      turns = provider.requests.select { |r| r['method'] == 'POST' && r['path'] == '/v1/responses' }
      results['codex_turn_model'] = turns.first && turns.first.dig('json', 'model')
      results['codex_actual_provider_request'] = turns.length == 1 && turns.first.dig('json', 'model') == 'fixture-model'
      results['codex_post_observed_before_sse_check'] = turns.length == 1
      reply_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 6
      until raw.include?('codex import runtime reply') || screen.text.include?('codex import runtime reply') || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= reply_deadline
        chunk = read_for(reader, writer, 0.2); raw << chunk; screen.feed(chunk)
      end
      results['codex_reply_rendered'] = raw.include?('codex import runtime reply') || screen.text.include?('codex import runtime reply')
    ensure
      stop_client(pid, reader, writer)
    end
  end
  File.write(File.join(RUN_DIR, 'codex-runtime.txt'), clean_ansi(raw))
rescue StandardError => error
  results['error'] = "codex: #{error.class}: #{error.message}"
  File.binwrite(File.join(RUN_DIR, 'codex-runtime.ansi'), raw)
  File.write(File.join(RUN_DIR, 'codex-runtime-failure.txt'), clean_ansi(raw))
ensure
  stop_daemon(data_dir) if defined?(data_dir) && data_dir
end

begin
  # --- Claude Code: strict settings + markdown agent, then Apply persistence.
  claude_root = File.join(RUN_DIR, 'claude'); claude_project = File.join(RUN_DIR, 'claude-project'); claude_data = File.join(RUN_DIR, 'claude-data'); claude_own = File.join(RUN_DIR, 'claude-own')
  [claude_root, claude_project, claude_data, claude_own, File.join(claude_project, '.claude', 'agents')].each { |path| FileUtils.mkdir_p(path) }
  settings = { 'model' => 'claude-import-model', 'env' => { 'CLAUDE_FIXTURE_OPTION' => 'not-a-secret' }, 'hooks' => { 'PreToolUse' => [{ 'command' => 'never-execute' }] } }
  settings_path = File.join(claude_root, 'settings.json'); settings_bytes = JSON.pretty_generate(settings) + "\n"; File.binwrite(settings_path, settings_bytes)
  agent_path = File.join(claude_project, '.claude', 'agents', 'migration-reviewer.md')
  agent_bytes = "---\nname: migration-reviewer\ndescription: Reviews migrated settings\nmodel: claude-import-model\ntools: read, grep\npermissionMode: plan\n---\nReview the migrated configuration without executing commands.\n"
  File.binwrite(agent_path, agent_bytes)
  settings_hash = Digest::SHA256.file(settings_path).hexdigest; agent_hash = Digest::SHA256.file(agent_path).hexdigest
  File.write(File.join(claude_own, 'opencode.json'), JSON.pretty_generate('preexisting' => true) + "\n")
  claude_env = {
    'HOME' => File.join(RUN_DIR, 'home-claude'), 'CLAUDE_CONFIG_DIR' => claude_root, 'XDG_CONFIG_HOME' => File.join(RUN_DIR, 'xdg-claude'),
    'XDG_DATA_HOME' => File.join(RUN_DIR, 'xdg-data-claude'), 'XDG_STATE_HOME' => File.join(RUN_DIR, 'xdg-state-claude'),
    'OPENCODE_CONFIG_DIR' => File.join(RUN_DIR, 'legacy-claude'), 'OPENCODE_RK_CONFIG_DIR' => claude_own,
    'OPENCODE_DATA_DIR' => claude_data, 'OPENCODE_RK_HOME' => claude_data, 'PROJECT_DIR' => claude_project,
    'TERM' => 'xterm-256color', 'COLUMNS' => '100', 'LINES' => '32',
    'OPENCODE_RK_TUI_SUBMIT_KEY' => 'enter',
    'DYLD_LIBRARY_PATH' => NATIVE_LIBRARY_DIR, 'PATH' => '/usr/bin:/bin'
  }
  results['claude_apply'] = run_apply(BINARY, claude_env, claude_data, 'claude')
  results['claude_apply_error_nil'] = results['claude_apply']['error'].nil?
  results['claude_apply_ui_complete'] = %w[offer_visible preview_visible apply_visible].all? { |key| results['claude_apply'][key] == true }
  value = JSON.parse(File.read(File.join(claude_own, 'opencode.json')))
  results['claude_model_persisted'] = value['model'] == 'claude-import-model'
  results['claude_agent_persisted'] = value.dig('agents', 'migration-reviewer', 'model') == 'claude-import-model' && value.dig('agents', 'migration-reviewer', 'prompt').include?('without executing')
  results['claude_unsupported_diagnostics'] = File.read(File.join(RUN_DIR, 'claude-apply.txt')).include?('hooks') && File.read(File.join(RUN_DIR, 'claude-apply.txt')).include?('permissionMode')
  results['claude_settings_unchanged'] = Digest::SHA256.file(settings_path).hexdigest == settings_hash
  results['claude_agent_source_unchanged'] = Digest::SHA256.file(agent_path).hexdigest == agent_hash
  results['no_raw_credential_literals'] = !File.read(File.join(RUN_DIR, 'claude-apply.txt')).include?('not-a-secret')
rescue StandardError => error
  results['error'] = "claude: #{error.class}: #{error.message}"
  File.write(File.join(RUN_DIR, 'claude-failure.txt'), "#{error.class}: #{error.message}\n")
ensure
  stop_daemon(claude_data) if defined?(claude_data) && claude_data
end

results['required_checks'] = %w[
  codex_apply_error_nil codex_apply_ui_complete codex_model_normalized
  codex_provider_persisted codex_agent_persisted codex_unsupported_diagnostic_source
  codex_source_unchanged codex_catalogue_requested codex_actual_provider_request
  codex_post_observed_before_sse_check codex_reply_rendered claude_apply_error_nil
  claude_apply_ui_complete claude_model_persisted claude_agent_persisted
  claude_unsupported_diagnostics claude_settings_unchanged claude_agent_source_unchanged
  existing_destination_preserved no_raw_credential_literals
]
puts JSON.pretty_generate(results)
exit(results['error'].nil? && results['required_checks'].all? { |key| results[key] == true } ? 0 : 1)
