#!/usr/bin/env ruby
# frozen_string_literal: true
# Focused real-native imported-default-model journey. Stdlib only.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'socket'
require 'timeout'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/model-import-pty-verification', __dir__)
RUN_DIR = File.join(ARTIFACT_DIR, 'run-' + Time.now.strftime('%Y%m%d-%H%M%S') + '-' + Process.pid.to_s)
BINARY = ARGV[0] ? File.expand_path(ARGV[0]) : File.expand_path('../../../target/debug/oc2', __dir__)
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
FileUtils.mkdir_p(RUN_DIR)
%w[home xdg-config xdg-data xdg-state xdg-cache own-config legacy-config project data].each do |name|
  FileUtils.mkdir_p(File.join(RUN_DIR, name))
end
HOME_DIR = File.join(RUN_DIR, 'home')
CONFIG_DIR = File.join(RUN_DIR, 'own-config')
DATA_DIR = File.join(RUN_DIR, 'data')
PROJECT_DIR = File.join(RUN_DIR, 'project')
STATE_FILE = File.join(RUN_DIR, 'xdg-state', 'opencode', 'model.json')

raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && BINARY_SHA != EXPECTED_SHA

class FakeProvider
  attr_reader :port

  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @requests = []
    @errors = []
    @lock = Mutex.new
    @closed = false
    @thread = Thread.new { accept_loop }
  end

  def requests
    @lock.synchronize { @requests.map(&:dup) }
  end

  def errors
    @lock.synchronize { @errors.dup }
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
        ready = IO.select([@server], nil, nil, 0.15)
        next unless ready
        client = @server.accept
        Thread.new(client) { |socket| serve_one(socket) }
      rescue IOError, Errno::EBADF
        break if @closed
      end
    end
  rescue StandardError => error
    @lock.synchronize { @errors << "accept #{error.class}: #{error.message}" }
  end

  def serve_one(socket)
    socket.close_on_exec = true
    request = read_request(socket)
    return unless request
    method, path, headers, body = request
    parsed_body = begin JSON.parse(body) unless body.empty? rescue nil end
    @lock.synchronize do
      @requests << { 'method' => method, 'path' => path, 'host' => headers['host'], 'json' => parsed_body }
    end
    if method == 'GET' && path == '/v1/models'
      respond(socket, 200, 'application/json', JSON.generate({ 'data' => [{ 'id' => 'fixture-model', 'name' => 'Fixture Model' }] }))
    elsif method == 'POST' && path == '/v1/responses'
      sse = [
        ['response.created', { 'type' => 'response.created' }],
        ['response.output_text.delta', { 'type' => 'response.output_text.delta', 'delta' => 'fixture reply' }],
        ['response.completed', { 'type' => 'response.completed', 'response' => { 'usage' => { 'input_tokens' => 1, 'output_tokens' => 2 } } }]
      ].map { |event, value| "event: #{event}\ndata: #{JSON.generate(value)}\n\n" }.join
      respond(socket, 200, 'text/event-stream', sse)
    else
      respond(socket, 404, 'application/json', '{"error":"not found"}')
    end
  rescue StandardError => error
    @lock.synchronize { @errors << "request #{error.class}: #{error.message}" }
  ensure
    socket.close rescue nil
  end

  def read_request(socket)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
    bytes = ''.b
    header_end = nil
    until header_end
      part = read_part(socket, deadline)
      return nil unless part
      bytes << part
      raise 'request headers exceed 16 KiB' if bytes.bytesize > 16 * 1024
      header_end = bytes.index("\r\n\r\n")
    end
    head = bytes.byteslice(0, header_end).force_encoding('UTF-8').scrub
    lines = head.split("\r\n")
    method, path, = lines.shift.to_s.split(' ', 3)
    headers = {}
    lines.each do |line|
      key, value = line.split(':', 2)
      headers[key.to_s.downcase] = value.to_s.strip
    end
    length = Integer(headers.fetch('content-length', '0'), 10)
    raise 'request body exceeds 1 MiB' if length > 1024 * 1024
    body_start = header_end + 4
    until bytes.bytesize >= body_start + length
      part = read_part(socket, deadline)
      return nil unless part
      bytes << part
    end
    [method, path, headers, bytes.byteslice(body_start, length).to_s.force_encoding('UTF-8').scrub]
  end

  def read_part(socket, deadline)
    left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)
    return nil if left <= 0 || !IO.select([socket], nil, nil, left)
    socket.readpartial(4096)
  rescue EOFError
    nil
  end

  def respond(socket, status, content_type, body)
    reason = status == 200 ? 'OK' : 'Not Found'
    bytes = body.b
    headers = "HTTP/1.1 #{status} #{reason}\r\nContent-Type: #{content_type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n"
    socket.write(headers)
    socket.write(bytes)
    socket.flush
  end
end

class Screen
  attr_reader :rows
  def initialize(rows = 32, cols = 100)
    @rows = Array.new(rows) { ' ' * cols }
    @x = 0
    @y = 0
    @esc = ''
  end

  def feed(raw)
    raw.each_byte do |byte|
      char = byte.chr
      if @esc.empty?
        if byte == 27
          @esc = char
        elsif byte == 13
          @x = 0
        elsif byte == 10
          @y = [@y + 1, @rows.length - 1].min
        elsif byte >= 32 && byte < 127
          @rows[@y][@x] = char if @x < @rows[@y].length
          @x = [@x + 1, @rows[@y].length - 1].min
        end
      elsif @esc == "\e" && char == '['
        @esc = 'CSI:'
      elsif @esc.start_with?('CSI:')
        @esc = @esc.dup << char
        if char.ord.between?(0x40, 0x7e)
          body = @esc[4...-1].to_s
          nums = body.split(';').map { |number| number.empty? ? 1 : number.to_i }
          case char
          when 'H', 'f'
            @y = [(nums[0] || 1) - 1, 0].max.clamp(0, @rows.length - 1)
            @x = [(nums[1] || 1) - 1, 0].max.clamp(0, @rows.first.length - 1)
          when 'J'
            @rows.map! { ' ' * @rows.first.length }
            @x = @y = 0
          when 'K'
            @rows[@y] = @rows[@y][0...@x] + ' ' * (@rows[@y].length - @x)
          end
          @esc = ''
        end
      else
        @esc = ''
      end
    end
  end

  def text
    @rows.join("\n")
  end
end

def read_for(io, writer, seconds)
  output = +''
  pending_query = +''
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([io], nil, nil, [left, 0.15].min)
    begin
      chunk = io.read_nonblock(65_536)
      pending_query = (pending_query + chunk)[-32..-1].to_s
      if pending_query.include?("\e[6n")
        writer.write("\e[1;1R")
        writer.flush
        pending_query = +''
      end
      output << chunk
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO
      break
    end
  end
  output.force_encoding('UTF-8').scrub
end

def stop_client(pid, io, writer)
  begin
    writer.write("\x03")
    writer.flush
    read_for(io, writer, 0.5)
  rescue StandardError
  end
  Process.kill('TERM', pid) rescue nil
  begin
    Timeout.timeout(2) { Process.wait(pid) }
  rescue StandardError
    Process.kill('KILL', pid) rescue nil
    Process.wait(pid) rescue nil
  end
end

def stop_own_daemon
  descriptor = File.join(DATA_DIR, 'runtime', 'backend.json')
  return unless File.file?(descriptor)
  value = JSON.parse(File.read(descriptor)) rescue nil
  pid = value && value['pid']
  return unless pid.is_a?(Integer) && pid > 1 && pid != Process.pid
  # DATA_DIR was created uniquely for this probe and had no descriptor before launch.
  begin
    Process.kill('TERM', pid)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 2
    while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
      begin
        Process.kill(0, pid)
      rescue Errno::ESRCH
        return
      end
      sleep 0.05
    end
    Process.kill('KILL', pid) rescue nil
  rescue Errno::ESRCH
  end
end

def write_evidence(name, raw)
  File.binwrite(File.join(RUN_DIR, name + '.ansi'), raw)
  clean = raw.gsub(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]/, '')
  File.write(File.join(RUN_DIR, name + '.txt'), clean)
end

results = {
  'binary' => BINARY,
  'binary_sha256' => BINARY_SHA,
  'own_config_file' => File.join(CONFIG_DIR, 'opencode.json'),
  'recent_state_absent_before' => !File.exist?(STATE_FILE),
  'manual_provider_selection' => false,
  'manual_recent_selection' => false
}
provider = nil
client_pid = nil
client_io = nil
client_writer = nil
config_path = File.join(CONFIG_DIR, 'opencode.json')
config_body = nil
source_hash = nil
pty_raw = +''
begin
  provider = FakeProvider.new
  base_url = "http://127.0.0.1:#{provider.port}/v1"
  config_body = JSON.pretty_generate({
    'model' => 'acme/fixture-model',
    'providers' => { 'acme' => { 'name' => 'Acme fixture', 'base_url' => base_url } }
  }) + "\n"
  File.binwrite(config_path, config_body)
  source_hash = Digest::SHA256.file(config_path).hexdigest
  results['source_hash_before'] = source_hash
  results['provider_base_url'] = base_url
  results['expected_discovery_endpoint'] = "#{base_url}/models"
  results['expected_turn_endpoint'] = "#{base_url}/responses"
  results['initial_config_model'] = 'acme/fixture-model'

  env = {
    'HOME' => HOME_DIR,
    'XDG_CONFIG_HOME' => File.join(RUN_DIR, 'xdg-config'),
    'XDG_DATA_HOME' => File.join(RUN_DIR, 'xdg-data'),
    'XDG_STATE_HOME' => File.join(RUN_DIR, 'xdg-state'),
    'XDG_CACHE_HOME' => File.join(RUN_DIR, 'xdg-cache'),
    'OPENCODE_CONFIG_DIR' => File.join(RUN_DIR, 'legacy-config'),
    'OPENCODE_RK_CONFIG_DIR' => CONFIG_DIR,
    'OPENCODE_DATA_DIR' => DATA_DIR,
    'OPENCODE_RK_HOME' => DATA_DIR,
    'TERM' => 'xterm-256color',
    'COLUMNS' => '100',
    'LINES' => '32',
    'DYLD_LIBRARY_PATH' => ENV.fetch('DYLD_LIBRARY_PATH') { File.expand_path('../../../target/ui-native/lib', __dir__) },
    'PATH' => '/usr/bin:/bin'
  }
  FileUtils.mkdir_p(env['OPENCODE_CONFIG_DIR'])
  native_lib = env['DYLD_LIBRARY_PATH']
  raise "native library directory missing: #{native_lib}" unless File.directory?(native_lib)

  screen = Screen.new
  PTY.spawn(env, BINARY, '--native', '--data-dir', DATA_DIR,
            { unsetenv_others: true, chdir: PROJECT_DIR }) do |reader, writer, pid|
    client_pid = pid
    client_io = reader
    client_writer = writer
    begin
      initial = read_for(reader, writer, 2)
      pty_raw << initial
      screen.feed(initial)
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 20
      selected_frame = nil
      while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
        requests = provider.requests
        discovered = requests.any? { |item| item['method'] == 'GET' && item['path'] == '/v1/models' }
        # The composer displays the catalogued name and provider, not raw model ID.
        # Exact model identity is independently required in the later /responses body.
        displayed = screen.rows.any? { |row| row.match?(/Fixture Model\s+acme/) }
        if discovered && displayed
          selected_frame = screen.text
          break
        end
        chunk = read_for(reader, writer, 0.25)
        pty_raw << chunk
        screen.feed(chunk)
      end

      results['provider_discovery_requested'] = provider.requests.any? do |item|
        item['method'] == 'GET' && item['path'] == '/v1/models'
      end
      results['selected_model_displayed'] = !selected_frame.nil?
      results['selected_frame'] = selected_frame
      File.write(File.join(RUN_DIR, 'startup-screen.txt'), screen.text)
      if selected_frame
        File.write(File.join(RUN_DIR, 'selected-model-frame.txt'), selected_frame)
        writer.write("ping\r")
        writer.flush
        prompt_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 8
        until provider.requests.any? { |item| item['method'] == 'POST' && item['path'] == '/v1/responses' } ||
              Process.clock_gettime(Process::CLOCK_MONOTONIC) >= prompt_deadline
          chunk = read_for(reader, writer, 0.25)
          pty_raw << chunk
          screen.feed(chunk)
        end
        turn = provider.requests.select { |item| item['method'] == 'POST' && item['path'] == '/v1/responses' }
        results['turn_request_count'] = turn.length
        results['turn_model'] = turn.first && turn.first.dig('json', 'model')
        results['turn_prompt'] = turn.first && turn.first.dig('json', 'input')
        reply_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
        until pty_raw.include?('fixture reply') || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= reply_deadline
          chunk = read_for(reader, writer, 0.25)
          pty_raw << chunk
          screen.feed(chunk)
        end
        results['fixture_reply_displayed'] = pty_raw.include?('fixture reply') || screen.text.include?('fixture reply')
      else
        requests = provider.requests
        results['stage_gap'] = if results['provider_discovery_requested']
          'Provider model catalogue fetched, but fixture-model never appeared in the native frame; not submitting to avoid selecting a fixture manually.'
        else
          'Native startup never requested acme /v1/models from canonical root model; provider/model selection did not reach catalogue discovery. No manual provider/model selection or recent-state injection used.'
        end
      end
      write_evidence('native-session', pty_raw)
      stop_client(pid, reader, writer)
    ensure
      client_pid = nil
      client_io = nil
      client_writer = nil
    end
  end
rescue StandardError => error
  results['error'] = "#{error.class}: #{error.message}"
  write_evidence('failure-session', pty_raw) unless pty_raw.empty?
ensure
  if client_pid
    stop_client(client_pid, client_io, client_writer) if client_io && client_writer
  end
  stop_own_daemon
  provider.close if provider
end

results['provider_requests'] = provider ? provider.requests : []
results['provider_server_errors'] = provider ? provider.errors : []
results['no_manual_provider_selection'] = results['manual_provider_selection'] == false
results['no_manual_recent_selection'] = results['manual_recent_selection'] == false
results['fixture_reply_displayed'] = false unless results.key?('fixture_reply_displayed')
results['config_hash_after'] = File.file?(config_path) ? Digest::SHA256.file(config_path).hexdigest : nil
results['source_config_unchanged'] = results['source_hash_before'] == results['config_hash_after']
results['recent_state_absent_after'] = !File.exist?(STATE_FILE)
results['discovery_endpoint_observed'] = results['provider_requests'].any? do |item|
  item['method'] == 'GET' && item['path'] == '/v1/models'
end
turn_requests = results['provider_requests'].select { |item| item['method'] == 'POST' && item['path'] == '/v1/responses' }
results['turn_endpoint_observed_once'] = turn_requests.length == 1
results['turn_endpoint_observed'] = !turn_requests.empty?
results['turn_model_is_fixture_model'] = turn_requests.length == 1 && turn_requests[0].dig('json', 'model') == 'fixture-model'
results['required_checks'] = %w[
  recent_state_absent_before no_manual_provider_selection no_manual_recent_selection
  discovery_endpoint_observed selected_model_displayed turn_endpoint_observed_once
  turn_model_is_fixture_model fixture_reply_displayed source_config_unchanged recent_state_absent_after
]
results['all_required_checks_passed'] = results['error'].nil? && results['required_checks'].all? { |key| results[key] == true }
results['artifact_dir'] = RUN_DIR
File.write(File.join(RUN_DIR, 'summary.json'), JSON.pretty_generate(results) + "\n")
puts JSON.pretty_generate(results)
exit(results['all_required_checks_passed'] ? 0 : 1)
