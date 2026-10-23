#!/usr/bin/env ruby
# frozen_string_literal: true
# Real native TUI selection-copy journey. OSC 52 is captured by this PTY harness,
# never forwarded to the user's terminal or host clipboard.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'socket'
require 'timeout'
require 'base64'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/selection-pty-verification', __dir__)
RUN_DIR = File.join(ARTIFACT_DIR, 'run-' + Time.now.strftime('%Y%m%d-%H%M%S') + '-' + Process.pid.to_s)
BINARY = ARGV[0] ? File.expand_path(ARGV[0]) : File.expand_path('../../../target/debug/oc2', __dir__)
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
EXPECT_COPY_RED = ENV['EXPECT_COPY_RED'] == '1'
MARKER = 'SEL_COPY_MARKER_7A9'
FileUtils.mkdir_p(RUN_DIR)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && BINARY_SHA != EXPECTED_SHA
DEFAULT_NATIVE_LIBRARY_DIR = File.expand_path('../../../target/ui-native/lib', __dir__)
NATIVE_LIBRARY_PATH = ENV.fetch('DYLD_LIBRARY_PATH', DEFAULT_NATIVE_LIBRARY_DIR)
NATIVE_LIBRARY_FILE = NATIVE_LIBRARY_PATH.split(File::PATH_SEPARATOR).reject(&:empty?).lazy.flat_map do |library_dir|
  %w[libopentui.dylib libopentui.so].map { |name| File.join(library_dir, name) }
end.find { |path| File.file?(path) }
raise "native library not found in DYLD_LIBRARY_PATH=#{NATIVE_LIBRARY_PATH}" unless NATIVE_LIBRARY_FILE
NATIVE_LIBRARY_SHA256 = Digest::SHA256.file(NATIVE_LIBRARY_FILE).hexdigest

class FixtureProvider
  attr_reader :port

  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @requests = []
    @lock = Mutex.new
    @closed = false
    @workers = []
    @acceptor = Thread.new do
      until @closed
        begin
          socket = @server.accept
          @workers << Thread.new(socket) { |client| serve(client) }
        rescue IOError, Errno::EBADF
          break if @closed
        end
      end
    end
  end

  def requests
    @lock.synchronize { @requests.map(&:dup) }
  end

  def close
    @closed = true
    @server.close rescue nil
    @acceptor.join(1)
    @workers.each { |worker| worker.join(1) }
    @workers.each { |worker| worker.kill if worker.alive? }
  end

  private

  def serve(socket)
    socket.close_on_exec = true
    request = read_request(socket)
    return unless request
    method, path, body = request
    model = JSON.parse(body).fetch('model', nil) rescue nil
    @lock.synchronize { @requests << { 'method' => method, 'path' => path, 'model' => model } }
    if method == 'GET' && path == '/v1/models'
      respond(socket, 200, 'application/json', JSON.generate({ 'data' => [{ 'id' => 'fixture-model', 'name' => 'Fixture Model' }] }))
    elsif method == 'POST' && path == '/v1/responses'
      events = [
        ['response.created', { 'type' => 'response.created' }],
        ['response.output_text.delta', { 'type' => 'response.output_text.delta', 'delta' => MARKER }],
        ['response.completed', { 'type' => 'response.completed', 'response' => { 'usage' => { 'input_tokens' => 1, 'output_tokens' => 1 } } }]
      ]
      respond(socket, 200, 'text/event-stream', events.map { |name, data| "event: #{name}\ndata: #{JSON.generate(data)}\n\n" }.join)
    else
      respond(socket, 404, 'application/json', '{"error":"not found"}')
    end
  rescue StandardError
    nil
  ensure
    socket.close rescue nil
  end

  def read_request(socket)
    bytes = +''.b
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
    header_end = nil
    until header_end
      left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)
      return nil if left <= 0 || !IO.select([socket], nil, nil, left)
      bytes << socket.readpartial(4096)
      raise 'request header exceeds 16 KiB' if bytes.bytesize > 16 * 1024
      header_end = bytes.index("\r\n\r\n")
    end
    head = bytes.byteslice(0, header_end).force_encoding('UTF-8').scrub.split("\r\n")
    method, path, = head.shift.to_s.split(' ', 3)
    headers = {}
    head.each do |line|
      key, value = line.split(':', 2)
      headers[key.to_s.downcase] = value.to_s.strip
    end
    length = Integer(headers.fetch('content-length', '0'), 10)
    raise 'request body exceeds 1 MiB' if length > 1024 * 1024
    start = header_end + 4
    while bytes.bytesize < start + length
      left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)
      return nil if left <= 0 || !IO.select([socket], nil, nil, left)
      bytes << socket.readpartial(4096)
    end
    [method, path, bytes.byteslice(start, length).to_s.force_encoding('UTF-8').scrub]
  rescue EOFError
    nil
  end

  def respond(socket, status, content_type, body)
    reason = status == 200 ? 'OK' : 'Not Found'
    bytes = body.b
    socket.write("HTTP/1.1 #{status} #{reason}\r\nContent-Type: #{content_type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n")
    socket.write(bytes)
    socket.flush
  end
end

class Screen
  attr_reader :rows

  def initialize(rows = 32, cols = 100)
    @rows = Array.new(rows) { ' ' * cols }
    @x = @y = 0
    @state = nil
    @csi = +''
  end

  def feed(raw)
    raw.each_byte do |byte|
      char = byte.chr
      case @state
      when :esc
        @state = char == '[' ? :csi : char == ']' ? :osc : nil
        @csi = +'' if @state == :csi
      when :csi
        if byte.between?(0x40, 0x7e)
          apply_csi(char, @csi)
          @state = nil
        else
          @csi << char
        end
      when :osc
        @state = nil if byte == 7
        @state = :osc_esc if byte == 27
      when :osc_esc
        @state = byte == 92 ? nil : byte == 27 ? :osc_esc : :osc
      else
        if byte == 27
          @state = :esc
        elsif byte == 13
          @x = 0
        elsif byte == 10
          @y = [@y + 1, @rows.length - 1].min
        elsif byte >= 32 && byte < 127
          @rows[@y][@x] = char if @x < @rows[@y].length
          @x = [@x + 1, @rows[@y].length - 1].min
        end
      end
    end
  end

  def text
    @rows.join("\n")
  end

  private

  def apply_csi(command, body)
    values = body.sub(/\A[?>=]/, '').split(';').map { |value| value.empty? ? 1 : value.to_i }
    case command
    when 'H', 'f'
      @y = ((values[0] || 1) - 1).clamp(0, @rows.length - 1)
      @x = ((values[1] || 1) - 1).clamp(0, @rows.first.length - 1)
    when 'J'
      @rows.map! { ' ' * @rows.first.length }
      @x = @y = 0
    when 'K'
      @rows[@y] = @rows[@y][0...@x] + ' ' * (@rows[@y].length - @x)
    end
  end
end

def read_for(reader, writer, seconds)
  output = +''.b
  query_tail = +''.b
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([reader], nil, nil, [left, 0.1].min)
    begin
      chunk = reader.read_nonblock(65_536)
      query_tail = (query_tail + chunk).byteslice(-32, 32).to_s
      if query_tail.include?("\e[6n")
        writer.write("\e[1;1R")
        writer.flush
        query_tail.clear
      end
      output << chunk
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO
      break
    end
  end
  output
end

def stop_client(pid, reader, writer)
  writer.write("\x03") rescue nil
  read_for(reader, writer, 0.3) rescue nil
  Process.kill('TERM', pid) rescue nil
  begin
    Timeout.timeout(2) { Process.wait(pid) }
  rescue StandardError
    Process.kill('KILL', pid) rescue nil
    Process.wait(pid) rescue nil
  end
end

def stop_own_daemon(data_dir)
  descriptor = File.join(data_dir, 'runtime', 'backend.json')
  return unless File.file?(descriptor)
  value = JSON.parse(File.read(descriptor)) rescue nil
  pid = value && value['pid']
  return unless pid.is_a?(Integer) && pid > 1 && pid != Process.pid
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
  nil
end

def osc52_texts(raw)
  raw.scan(/\e\]52;c;([A-Za-z0-9+\/=]*)(?:\a|\e\\)/n).map do |(encoded)|
    Base64.strict_decode64(encoded).force_encoding('UTF-8').scrub
  end
end

def save_capture(dir, raw)
  File.binwrite(File.join(dir, 'pty-output.ansi'), raw)
  clean = raw.gsub(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]/, '')
  File.write(File.join(dir, 'pty-output.txt'), clean.force_encoding('UTF-8').scrub)
end

def run_case(name, flag:, gestures:, gesture_assertions:)
  dir = File.join(RUN_DIR, name)
  %w[home xdg-config xdg-data xdg-state xdg-cache own-config legacy-config project data].each do |part|
    FileUtils.mkdir_p(File.join(dir, part))
  end
  provider = FixtureProvider.new
  raw = +''.b
  pid = reader = writer = nil
  result = { 'case' => name, 'expected_text' => MARKER, 'gesture_assertions' => gesture_assertions }
  config_path = File.join(dir, 'own-config', 'opencode.json')
  File.write(config_path, JSON.generate({
    'model' => 'acme/fixture-model',
    'providers' => { 'acme' => { 'name' => 'Selection fixture', 'base_url' => "http://127.0.0.1:#{provider.port}/v1" } }
  }) + "\n")
  config_sha = Digest::SHA256.file(config_path).hexdigest
  recent_path = File.join(dir, 'xdg-state', 'opencode', 'model.json')
  result['recent_state_absent_before'] = !File.exist?(recent_path)
  result['native_library_path'] = NATIVE_LIBRARY_FILE
  result['native_library_sha256'] = NATIVE_LIBRARY_SHA256
  env = {
    'HOME' => File.join(dir, 'home'),
    'XDG_CONFIG_HOME' => File.join(dir, 'xdg-config'),
    'XDG_DATA_HOME' => File.join(dir, 'xdg-data'),
    'XDG_STATE_HOME' => File.join(dir, 'xdg-state'),
    'XDG_CACHE_HOME' => File.join(dir, 'xdg-cache'),
    'OPENCODE_CONFIG_DIR' => File.join(dir, 'legacy-config'),
    'OPENCODE_RK_CONFIG_DIR' => File.join(dir, 'own-config'),
    'OPENCODE_DATA_DIR' => File.join(dir, 'data'),
    'OPENCODE_RK_HOME' => File.join(dir, 'data'),
    'TERM' => 'xterm-256color', 'COLUMNS' => '100', 'LINES' => '32',
    'DYLD_LIBRARY_PATH' => NATIVE_LIBRARY_PATH,
    'LD_LIBRARY_PATH' => ENV.fetch('LD_LIBRARY_PATH', NATIVE_LIBRARY_PATH),
    'PATH' => '/usr/bin:/bin', 'LANG' => 'C', 'LC_ALL' => 'C'
  }
  if flag == :disabled
    env['OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT'] = 'true'
  elsif flag == :enabled
    env['OPENCODE_EXPERIMENTAL_DISABLE_COPY_ON_SELECT'] = 'false'
  end
  screen = Screen.new

  PTY.spawn(env, BINARY, '--native', '--data-dir', env.fetch('OPENCODE_DATA_DIR'), { unsetenv_others: true, chdir: File.join(dir, 'project') }) do |pty_reader, pty_writer, child|
    pid, reader, writer = child, pty_reader, pty_writer
    begin
      raw << read_for(reader, writer, 2)
      screen.feed(raw)
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 20
      until provider.requests.any? { |request| request['method'] == 'GET' && request['path'] == '/v1/models' } || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        chunk = read_for(reader, writer, 0.2)
        raw << chunk
        screen.feed(chunk)
      end
      result['catalogue_requested'] = provider.requests.any? { |request| request['method'] == 'GET' && request['path'] == '/v1/models' }
      writer.write("select copy marker\r")
      writer.flush
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
      marker_row = nil
      until marker_row || Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        chunk = read_for(reader, writer, 0.2)
        raw << chunk
        screen.feed(chunk)
        marker_row = screen.rows.index { |row| row.include?(MARKER) }
      end
      result['fixture_reply_rendered'] = !marker_row.nil?
      if marker_row
        marker_col = screen.rows.fetch(marker_row).index(MARKER)
        result['marker_screen_cell'] = { 'x' => marker_col, 'y' => marker_row }
        File.write(File.join(dir, 'marker-frame.txt'), screen.text)
        events = gestures.call(marker_col, marker_row, MARKER.length)
        event_copy_texts = []
        observed_total = 0
        events.each do |event|
          writer.write(event)
          writer.flush
          chunk = read_for(reader, writer, 0.35)
          raw << chunk
          screen.feed(chunk)
          copies = osc52_texts(raw)
          event_copy_texts << copies.drop(observed_total)
          observed_total = copies.length
        end
        observed_gesture_copies = event_copy_texts.map do |texts|
          texts.empty? ? false : texts == [MARKER] ? true : :unexpected_copy
        end
        result['gesture_copy_texts'] = event_copy_texts
        result['observed_gesture_copies'] = observed_gesture_copies
        result['osc52_texts'] = osc52_texts(raw)
        result['gesture_assertions_observed'] = observed_gesture_copies == gesture_assertions &&
          gesture_assertions.all? { |expected| expected == true || expected == false } &&
          result['osc52_texts'] == Array.new(gesture_assertions.count(true), MARKER)
        result['copy_count'] = result['osc52_texts'].length
        if gesture_assertions.count(true) == 1 && result['gesture_assertions_observed']
          duplicate = gestures.call(marker_col, marker_row, MARKER.length).last
          writer.write(duplicate)
          writer.flush
          chunk = read_for(reader, writer, 0.35)
          raw << chunk
          screen.feed(chunk)
          result['duplicate_release_inert'] = osc52_texts(raw) == [MARKER]
        else
          result['duplicate_release_inert'] = gesture_assertions.none?
        end
      else
        result['gesture_assertions_observed'] = false
        result['osc52_texts'] = osc52_texts(raw)
        result['observed_gesture_copies'] = []
        result['duplicate_release_inert'] = false
      end
      requests = provider.requests
      result['provider_requests'] = requests
      result['fixture_model_requested'] = requests.any? do |request|
        request['method'] == 'POST' && request['path'] == '/v1/responses' && request['model'] == 'fixture-model'
      end
      result['provider_requests'] = requests.map { |request| request.reject { |key, _| key == 'body' } }
      result['source_config_unchanged'] = Digest::SHA256.file(config_path).hexdigest == config_sha
      result['recent_state_absent_after'] = !File.exist?(recent_path)
      save_capture(dir, raw)
      stop_client(pid, reader, writer)
      pid = nil
    ensure
      stop_client(pid, reader, writer) if pid && reader && writer
    end
  end
  result['artifact_dir'] = dir
  result
ensure
  provider.close if provider
  stop_own_daemon(File.join(dir, 'data')) if dir
end

# SGR coordinates come from the marker's actual location in the rendered frame.
def drag_events(x, y, length, release)
  ["\e[<0;#{x + 1};#{y + 1}M", "\e[<32;#{x + length};#{y + 1}M", release.call(x, y)]
end

results = {
  'binary' => BINARY,
  'binary_sha256' => BINARY_SHA,
  'run_dir' => RUN_DIR,
  'pty_osc52_is_captured_not_forwarded' => true,
  'verification_mode' => EXPECT_COPY_RED ? 'expected_red' : 'acceptance'
}
begin
  if EXPECT_COPY_RED
    results['pre_wiring_default_release'] = run_case(
      'expected-pre-wiring-red', flag: :default,
      gestures: lambda do |x, y, length|
        release = ->(px, py) { "\e[<0;#{px + 1};#{py + 1}m" }
        drag_events(x, y, length, release) + [release.call(x, y), "\e[<0;#{x + 1};#{y + 1}m"]
      end,
      gesture_assertions: [false, false, false, false, false]
    )
    results['expected_red_observed'] = results.dig('pre_wiring_default_release', 'fixture_reply_rendered') == true &&
      results.dig('pre_wiring_default_release', 'fixture_model_requested') == true &&
      results.dig('pre_wiring_default_release', 'gesture_assertions_observed') == true &&
      results.dig('pre_wiring_default_release', 'osc52_texts') == []
    results['required_checks'] = ['expected_red_observed']
    results['verification_result'] = results['expected_red_observed'] ? 'expected_red_reproduced' : 'expected_red_not_reproduced'
  else
    results['default_left_release'] = run_case(
      'default-left-release', flag: :default,
      gestures: ->(x, y, length) { drag_events(x, y, length, ->(px, py) { "\e[<0;#{px + 1};#{py + 1}m" }) },
      gesture_assertions: [false, false, true]
    )
    results['disabled_copy_right_press'] = run_case(
      'disabled-right-press', flag: :disabled,
      gestures: lambda do |x, y, length|
        ["\e[<0;#{x + 1};#{y + 1}M", "\e[<32;#{x + length};#{y + 1}M",
         "\e[<0;#{x + 1};#{y + 1}m", "\e[<2;#{x + 1};#{y + 1}M", "\e[<2;#{x + 1};#{y + 1}m"]
      end,
      gesture_assertions: [false, false, false, true, false]
    )
    results['enabled_right_release'] = run_case(
      'enabled-right-release', flag: :enabled,
      gestures: lambda do |x, y, length|
        ["\e[<0;#{x + 1};#{y + 1}M", "\e[<32;#{x + length};#{y + 1}M",
         "\e[<2;#{x + 1};#{y + 1}M", "\e[<2;#{x + 1};#{y + 1}m"]
      end,
      gesture_assertions: [false, false, false, true]
    )
    checks = %w[default_left_release disabled_copy_right_press enabled_right_release].flat_map do |key|
      [results.dig(key, 'catalogue_requested') == true,
       results.dig(key, 'provider_requests')&.count { |request| request['method'] == 'POST' && request['path'] == '/v1/responses' } == 1,
       results.dig(key, 'fixture_reply_rendered') == true,
       results.dig(key, 'fixture_model_requested') == true,
       results.dig(key, 'source_config_unchanged') == true,
       results.dig(key, 'recent_state_absent_before') == true,
       results.dig(key, 'recent_state_absent_after') == true,
       results.dig(key, 'gesture_assertions_observed') == true,
       results.dig(key, 'duplicate_release_inert') == true,
       results.dig(key, 'osc52_texts') == [MARKER]]
    end
    results['required_check_count'] = checks.length
    results['all_required_checks_passed'] = checks.all? { |value| value == true } && results['verification_mode'] == 'acceptance'
    results['verification_result'] = results['all_required_checks_passed'] ? 'product_acceptance_checks_passed' : 'product_acceptance_checks_failed'
    results['required_checks'] = %w[default_left_release disabled_copy_right_press enabled_right_release]
  end
rescue StandardError => error
  results['error'] = "#{error.class}: #{error.message}"
  results['all_required_checks_passed'] = false
  results['expected_red_observed'] = false if EXPECT_COPY_RED
end
results['artifact_dir'] = RUN_DIR
File.write(File.join(RUN_DIR, 'summary.json'), JSON.pretty_generate(results) + "\n")
puts JSON.pretty_generate(results)
exit((EXPECT_COPY_RED ? results['expected_red_observed'] == true : results['all_required_checks_passed'] == true) ? 0 : 1)
