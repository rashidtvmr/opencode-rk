#!/usr/bin/env ruby
# frozen_string_literal: true
# Two native clients, one authenticated daemon/session. Stdlib only.
# Run: DYLD_LIBRARY_PATH=/absolute/path/to/ui-native/lib ruby tests/e2e/native_usability/second_client_pty_journey.rb /absolute/path/to/oc2
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'socket'
require 'net/http'
require 'uri'
require 'io/console'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/second-client-pty-verification', __dir__)
RUN_DIR = File.join(ARTIFACT_DIR, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY = File.expand_path(ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__)))
ROWS = Integer(ENV.fetch('PTY_ROWS', '32'))
COLS = Integer(ENV.fetch('PTY_COLS', '100'))
MAX_CAPTURE = 4 * 1024 * 1024
MAX_HTTP_BODY = 256 * 1024
ACTIVE_CLIENTS = []
FIRST_PROMPT = 'native second-client first turn'
FIRST_REPLY = 'native second-client first reply'
SECOND_PROMPT = 'native second-client distinct second turn'
SECOND_REPLY = 'native second-client second reply'

raise 'PTY_COLS must be at least 40' if COLS < 40
raise 'PTY_ROWS must be at least 12' unless ROWS.between?(12, 120)
FileUtils.mkdir_p(RUN_DIR)

class Screen
  attr_reader :rows

  def initialize(rows, cols)
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
          apply(char, @csi)
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

  def apply(command, body)
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
    when 'A'
      @y = [@y - (values[0] || 1), 0].max
    when 'B'
      @y = [@y + (values[0] || 1), @rows.length - 1].min
    when 'C'
      @x = [@x + (values[0] || 1), @rows.first.length - 1].min
    when 'D'
      @x = [@x - (values[0] || 1), 0].max
    end
  end
end

class Provider
  MODEL_ID = 'fixture-model'
  API_KEY = 'second-client-fixture-key'

  attr_reader :port

  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @lock = Mutex.new
    @requests = []
    @errors = []
    @threads = []
    @closed = false
    @acceptor = Thread.new { accept_loop }
  end

  def requests
    @lock.synchronize { @requests.map(&:dup) }
  end

  def posts
    requests.select { |request| request['method'] == 'POST' && request['path'] == '/v1/responses' }
  end

  def errors
    @lock.synchronize { @errors.dup }
  end

  def close
    @closed = true
    @server.close rescue nil
    @acceptor.join(2)
    threads = @lock.synchronize { @threads.dup }
    threads.each { |thread| thread.join(2) }
    threads.each { |thread| thread.kill if thread.alive? }
  end

  private

  def record_error(error)
    @lock.synchronize { @errors << "#{error.class}: #{error.message}" }
  end

  def accept_loop
    until @closed
      begin
        next unless IO.select([@server], nil, nil, 0.1)
        socket = @server.accept
        socket.close_on_exec = true
        thread = Thread.new(socket) { |client| serve(client) }
        @lock.synchronize { @threads << thread }
      rescue IOError, Errno::EBADF
        break if @closed
        raise
      rescue StandardError => error
        record_error(error)
      end
    end
  rescue StandardError => error
    record_error(error) unless @closed
  end

  def read_request(socket)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
    bytes = +''.b
    header_end = nil
    until header_end
      raise 'provider request header timeout' unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096)
      raise 'provider request headers exceed 16 KiB' if bytes.bytesize > 16 * 1024
      header_end = bytes.index("\r\n\r\n")
    end
    lines = bytes.byteslice(0, header_end).force_encoding('UTF-8').scrub.split("\r\n")
    method, path = lines.shift.to_s.split(' ', 3)
    headers = {}
    lines.each do |line|
      key, value = line.split(':', 2)
      headers[key.to_s.downcase] = value.to_s.strip
    end
    length = Integer(headers.fetch('content-length', '0'), 10)
    raise 'provider request body exceeds 128 KiB' if length.negative? || length > 128 * 1024
    body_start = header_end + 4
    while bytes.bytesize < body_start + length
      raise 'provider request body timeout' unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096)
    end
    [method, path, headers, bytes.byteslice(body_start, length).to_s]
  rescue EOFError
    nil
  end

  def serve(socket)
    request = read_request(socket)
    return unless request
    method, path, headers, raw = request
    if path == '/v1/responses' && headers['authorization'] != "Bearer #{API_KEY}"
      raise 'provider fixture received wrong authorization'
    end
    value = raw.empty? ? nil : JSON.parse(raw)
    entry = { 'method' => method, 'path' => path, 'json' => value }
    @lock.synchronize { @requests << entry }
    if method == 'GET' && path == '/v1/models'
      respond(socket, 200, 'application/json', JSON.generate('data' => [{ 'id' => MODEL_ID, 'name' => 'Two-client fixture' }]))
    elsif method == 'POST' && path == '/v1/responses'
      respond_to_turn(socket, value)
    else
      respond(socket, 404, 'application/json', '{"error":"not found"}')
    end
  rescue StandardError => error
    record_error(error)
    begin
      respond(socket, 400, 'application/json', JSON.generate('error' => 'fixture rejected request'))
    rescue StandardError
      nil
    end
  ensure
    socket.close rescue nil
  end

  def respond_to_turn(socket, body)
    raise 'provider body must be a JSON object' unless body.is_a?(Hash)
    raise "unexpected provider model #{body['model'].inspect}" unless body['model'] == MODEL_ID
    raise 'provider turn must stream' unless body['stream'] == true
    input = body['input']
    raise 'provider input must be an array' unless input.is_a?(Array)
    users = input.select { |item| item.is_a?(Hash) && item['role'] == 'user' }.map { |item| item['content'] }
    assistant = input.select { |item| item.is_a?(Hash) && item['role'] == 'assistant' }.map { |item| item['content'] }
    if users == [FIRST_PROMPT] && assistant.empty?
      reply = FIRST_REPLY
    elsif users == [FIRST_PROMPT, SECOND_PROMPT] && assistant == [FIRST_REPLY]
      reply = SECOND_REPLY
    else
      raise "unexpected provider history users=#{users.inspect} assistants=#{assistant.inspect}"
    end
    frames = [
      { 'type' => 'response.output_text.delta', 'delta' => reply },
      { 'type' => 'response.completed', 'response' => { 'status' => 'completed' } }
    ].map { |event| "data: #{JSON.generate(event)}\n\n" }.join
    respond(socket, 200, 'text/event-stream', frames)
  end

  def respond(socket, status, content_type, body)
    bytes = body.b
    reason = status == 200 ? 'OK' : 'Bad Request'
    socket.write("HTTP/1.1 #{status} #{reason}\r\nContent-Type: #{content_type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n")
    socket.write(bytes)
    socket.flush
  end
end

def now
  Process.clock_gettime(Process::CLOCK_MONOTONIC)
end

def sha256(path)
  Digest::SHA256.file(path).hexdigest
end

def library_dir(binary)
  explicit = ENV.fetch('DYLD_LIBRARY_PATH', '').split(File::PATH_SEPARATOR).find { |path| File.directory?(path) }
  explicit ||= ENV.fetch('LD_LIBRARY_PATH', '').split(File::PATH_SEPARATOR).find { |path| File.directory?(path) }
  explicit || File.expand_path('../ui-native/lib', File.dirname(binary))
end

def library_hashes(directory)
  files = Dir[File.join(directory, '*')].select { |path| File.file?(path) }.sort
  raise "native library directory has no files: #{directory}" if files.empty?
  files.to_h { |path| [File.basename(path), sha256(path)] }
end

def alive?(pid)
  Process.kill(0, Integer(pid))
  true
rescue Errno::ESRCH
  false
rescue Errno::EPERM
  true
end

def reap_status(client)
  return client[:status] if client[:status]
  _pid, status = Process.waitpid2(client[:pid], Process::WNOHANG)
  client[:status] = status if status
  client[:status]
rescue Errno::ECHILD
  client[:status] ||= :already_reaped
end

def set_winsize(client)
  client[:reader].ioctl(0x80087467, [ROWS, COLS, 0, 0].pack('S4'))
end

def pump(client, seconds = 0.1)
  reader = client[:reader]
  writer = client[:writer]
  deadline = now + seconds
  while (left = deadline - now) > 0
    ready = IO.select([reader], nil, nil, [left, 0.1].min)
    next unless ready
    begin
      chunk = reader.read_nonblock(65_536)
      raise 'PTY output exceeded 4 MiB' if client[:captured].bytesize + chunk.bytesize > MAX_CAPTURE
      client[:captured] << chunk
      client[:screen].feed(chunk)
      client[:query] = (client[:query] + chunk).byteslice(-64, 64).to_s
      while (query_index = client[:query].index("\e[6n"))
        writer.write("\e[1;1R")
        writer.flush
        client[:query] = client[:query].byteslice(query_index + 4, client[:query].bytesize).to_s
        client[:cursor_queries] += 1
        client[:cursor_query_answers] << client[:cursor_queries]
        client[:cursor_queries_outstanding] = 0
      end
    rescue IO::WaitReadable
      next
    rescue EOFError, Errno::EIO
      client[:eof] = true
      break
    end
  end
  reap_status(client)
end

def pump_active(seconds = 0.05)
  ACTIVE_CLIENTS.dup.each do |client|
    pump(client, seconds) unless reap_status(client)
  end
end

def wait_for(client, timeout, label)
  deadline = now + timeout
  until yield
    return false if now >= deadline || reap_status(client)
    pump_active(0.05)
  end
  true
end

def launch(env, project, session = nil)
  args = [BINARY, '--native', '--data-dir', File.join(RUN_DIR, 'data')]
  args.concat(['--session', session]) if session
  reader, writer, pid = PTY.spawn(env, *args, { unsetenv_others: true, chdir: project })
  reader.binmode
  writer.binmode
  client = { reader: reader, writer: writer, pid: pid, captured: +''.b,
             screen: Screen.new(ROWS, COLS), query: +''.b, cursor_queries: 0,
             cursor_query_answers: [], cursor_queries_outstanding: 0, eof: false, status: nil }
  set_winsize(client)
  client[:pty_geometry] = client[:reader].winsize.first(2)
  raise "native PTY geometry mismatch: #{client[:pty_geometry].inspect}" unless client[:pty_geometry] == [ROWS, COLS]
  client
end

def stop_client(client)
  return unless client
  unless reap_status(client)
    begin
      client[:writer].write("\x03")
      client[:writer].flush
    rescue IOError, Errno::EIO
      nil
    end
    deadline = now + 5
    until reap_status(client) || now >= deadline
      pump(client, 0.1)
    end
    unless reap_status(client)
      Process.kill('TERM', client[:pid]) rescue nil
      deadline = now + 2
      until reap_status(client) || now >= deadline
        pump(client, 0.1)
      end
    end
    unless reap_status(client)
      Process.kill('KILL', client[:pid]) rescue nil
      begin
        _pid, status = Process.waitpid2(client[:pid])
        client[:status] = status
      rescue Errno::ECHILD
        client[:status] ||= :already_reaped
      end
    end
  end
  client[:reader].close rescue nil
  client[:writer].close rescue nil
end

def checked_descriptor(data_dir)
  path = File.join(data_dir, 'runtime', 'backend.json')
  deadline = now + 20
  until File.file?(path) || now >= deadline
    sleep 0.05
  end
  raise 'daemon backend descriptor missing' unless File.file?(path)
  value = JSON.parse(File.read(path))
  origin = value['http_origin'].to_s
  token = value['auth_token'].to_s
  pid = value['pid']
  match = /\Ahttp:\/\/127\.0\.0\.1:([1-9][0-9]{0,4})\z/.match(origin)
  raise 'daemon descriptor origin is not exact loopback' unless match && match[1].to_i <= 65_535
  raise 'daemon descriptor token is invalid' unless /\A[0-9a-fA-F]{64}\z/.match?(token)
  raise 'daemon descriptor PID is invalid' unless pid.is_a?(Integer) && pid > 1 && pid != Process.pid
  raise 'daemon PID is not alive' unless alive?(pid)
  value
end

def api(descriptor, method, path)
  uri = URI(descriptor.fetch('http_origin') + path)
  http = Net::HTTP.new(uri.host, uri.port, nil)
  request = Net::HTTP.const_get(method.capitalize).new(uri)
  request['Authorization'] = "Bearer #{descriptor.fetch('auth_token')}"
  request['Accept'] = 'application/json'
  response = nil
  worker = Thread.new do
    http = Net::HTTP.new(uri.host, uri.port, nil)
    http.open_timeout = 3
    http.read_timeout = 5
    http.start { |connection| connection.request(request) }
  end
  deadline = now + 10
  until worker.join(0.02)
    pump_active(0.02)
    raise "authenticated #{method} #{path} timed out" if now >= deadline
  end
  response = worker.value
  raw = response.body.to_s
  raise "daemon response exceeds #{MAX_HTTP_BODY} bytes" if raw.bytesize > MAX_HTTP_BODY
  raise "authenticated #{method} #{path} HTTP #{response.code}" unless response.code.to_i == 200
  JSON.parse(raw)
ensure
  worker&.kill if worker&.alive?
end

def session_id(descriptor)
  sessions = api(descriptor, 'get', '/api/sessions').fetch('sessions')
  raise "expected one session, found #{sessions.length}" unless sessions.is_a?(Array) && sessions.length == 1
  id = sessions.first.fetch('id')
  raise 'session ID is not a UUID' unless /\A[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}\z/.match?(id)
  id
end

def history(descriptor, id)
  value = api(descriptor, 'get', "/api/sessions/#{URI.encode_www_form_component(id)}/messages?limit=100")
  rows = value.fetch('messages')
  raise 'session history is not an array' unless rows.is_a?(Array)
  rows.each do |row|
    raise 'history row belongs to another session' unless row['session_id'] == id
    raise 'history row lacks typed role/body' unless row['role'].is_a?(String) && row['body'].is_a?(Hash)
  end
  rows
end

def role_texts(rows, role)
  rows.map do |row|
    body = row['body']
    body['text'] if row['role'] == role && body['storage'] == 'inline' && body['text'].is_a?(String)
  end.compact
end

def expected_history!(rows, expected)
  actual = rows.map do |row|
    body = row['body']
    [row['role'], body['text']] if body['storage'] == 'inline' && body['text'].is_a?(String)
  end.compact
  raise "typed persisted history mismatch: #{actual.inspect}" unless actual == expected
  ids = rows.map { |row| row['id'] }
  raise 'history message IDs are absent or duplicated' unless ids.all? { |id| id.is_a?(String) && !id.empty? } && ids.uniq == ids
end

def descriptor_identity(value)
  [value.fetch('pid'), value.fetch('http_origin'), value.fetch('auth_token')]
end

def wait_ready(client)
  wait_for(client, 25, 'native composer/model') do
    text = client[:screen].text
    composer = text.include?('Ask anything') || text.include?('>')
    composer && text.include?('Two-client fixture') && text.include?('acme')
  end
end

def wait_screen_text(client, text, timeout = 15)
  wait_for(client, timeout, "visible #{text.inspect}") { client[:screen].text.include?(text) }
end

def wait_post_count(provider, count, timeout = 15)
  deadline = now + timeout
  while provider.posts.length < count && now < deadline
    pump_active(0.05)
  end
  raise "provider POST count expected #{count}, got #{provider.posts.length}" unless provider.posts.length == count
  raise "provider fixture errors: #{provider.errors.inspect}" unless provider.errors.empty?
end

def settle_composer(client)
  raise 'native composer/model readiness lost before submit' unless wait_ready(client)
  pump_active(0.2)
end

def submit(client, text)
  settle_composer(client)
  client[:writer].write(text)
  client[:writer].flush
  pump_active(0.2)
  client[:writer].write("\r")
  client[:writer].flush
end

def kill_owned_daemon(descriptor, initial_identity)
  return true unless descriptor
  current = checked_descriptor(File.join(RUN_DIR, 'data'))
  raise 'refusing daemon cleanup after descriptor identity changed' unless descriptor_identity(current) == initial_identity
  pid = current.fetch('pid')
  Process.kill('TERM', pid)
  deadline = now + 3
  sleep 0.05 while alive?(pid) && now < deadline
  Process.kill('KILL', pid) if alive?(pid)
  deadline = now + 2
  sleep 0.05 while alive?(pid) && now < deadline
  !alive?(pid)
rescue Errno::ESRCH
  true
end

def self_check
  provider = Provider.new
  uri = URI("http://127.0.0.1:#{provider.port}/v1/responses")
  request = Net::HTTP::Post.new(uri)
  request['Content-Type'] = 'application/json'
  request['Authorization'] = "Bearer #{Provider::API_KEY}"
  request.body = JSON.generate(
    'model' => Provider::MODEL_ID,
    'stream' => true,
    'input' => [{ 'role' => 'user', 'content' => FIRST_PROMPT }]
  )
  response = Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 5, proxy_from_env: false) { |http| http.request(request) }
  raise "fixture response HTTP #{response.code}: #{provider.errors.inspect}" unless response.code.to_i == 200
  raise 'fixture did not return the expected stream reply' unless response.body.include?(FIRST_REPLY)
  raise 'fixture request capture mismatch' unless provider.posts.length == 1
  puts 'self-check: loopback provider stream and exact request count passed'
ensure
  provider&.close
end

if ARGV == ['--self-check']
  self_check
  exit 0
end

library = library_dir(BINARY)
provider = nil
clients = []
daemon = nil
daemon_identity = nil
result = {
  'phase' => 'running', 'binary' => BINARY,
  'binary_sha256' => File.file?(BINARY) ? sha256(BINARY) : nil,
  'expected_binary_sha256' => ENV['EXPECTED_BINARY_SHA256'],
  'binary_identity_verified' => false, 'native_library_hashed' => false,
  'native_pty_geometry_verified' => false, 'client_a_native_ready' => false,
  'daemon_authenticated' => false, 'session_created' => false,
  'first_turn_exact_post' => false, 'first_reply_rendered' => false,
  'first_turn_typed_history' => false, 'client_b_native_ready' => false,
  'same_session_resumed' => false, 'same_daemon_identity' => false,
  'both_clients_alive_on_attach' => false, 'no_extra_post_on_attach' => false,
  'persisted_history_unchanged_on_attach' => false, 'client_b_second_turn_exact_post' => false,
  'second_reply_rendered' => false, 'distinct_second_turn_persisted' => false,
  'client_a_live_refresh' => false, 'no_extra_post_during_live_refresh' => false,
  'both_clients_clean_exit' => false, 'reopened_same_session' => false,
  'reopened_typed_history' => false, 'no_post_on_restart' => false,
  'cursor_queries_balanced' => false, 'owned_cleanup' => false,
  'native_library_dir' => library,
  'native_library_sha256' => File.directory?(library) ? library_hashes(library) : {},
  'test_sha256' => sha256(__FILE__), 'terminal' => { 'rows' => ROWS, 'cols' => COLS }
}

begin
  raise "binary missing or not executable: #{BINARY}" unless File.file?(BINARY) && File.executable?(BINARY)
  if ENV['EXPECTED_BINARY_SHA256'] && result['binary_sha256'] != ENV['EXPECTED_BINARY_SHA256']
    raise "binary SHA mismatch expected=#{ENV['EXPECTED_BINARY_SHA256']} actual=#{result['binary_sha256']}"
  end
  result['binary_identity_verified'] = ENV['EXPECTED_BINARY_SHA256'] && result['binary_sha256'] == ENV['EXPECTED_BINARY_SHA256']
  raise 'EXPECTED_BINARY_SHA256 is required for installed acceptance' unless result['binary_identity_verified']
  raise "native library directory missing: #{library}" unless File.directory?(library)
  raise 'native library fixture has no hashed files' if result['native_library_sha256'].empty?
  result['native_library_hashed'] = true

  %w[home xdg-config xdg-data xdg-state xdg-cache config legacy-config project data].each do |name|
    FileUtils.mkdir_p(File.join(RUN_DIR, name))
  end
  data_dir = File.join(RUN_DIR, 'data')
  project = File.join(RUN_DIR, 'project')
  own_config = File.join(RUN_DIR, 'config')
  provider = Provider.new
  config_path = File.join(own_config, 'opencode.json')
  config = {
    'model' => "acme/#{Provider::MODEL_ID}",
    'providers' => {
      'acme' => {
        'name' => 'Two-client fixture',
        'base_url' => "http://127.0.0.1:#{provider.port}/v1",
        'api_key_env' => 'SECOND_CLIENT_PROVIDER_KEY'
      }
    }
  }
  File.write(config_path, JSON.pretty_generate(config) + "\n")
  result['provider_config_sha256'] = sha256(config_path)
  env = {
    'HOME' => File.join(RUN_DIR, 'home'),
    'XDG_CONFIG_HOME' => File.join(RUN_DIR, 'xdg-config'),
    'XDG_DATA_HOME' => File.join(RUN_DIR, 'xdg-data'),
    'XDG_STATE_HOME' => File.join(RUN_DIR, 'xdg-state'),
    'XDG_CACHE_HOME' => File.join(RUN_DIR, 'xdg-cache'),
    'OPENCODE_CONFIG_DIR' => File.join(RUN_DIR, 'legacy-config'),
    'OPENCODE_RK_CONFIG_DIR' => own_config,
    'OPENCODE_DATA_DIR' => data_dir,
    'OPENCODE_RK_HOME' => data_dir,
    'OPENCODE_RK_TUI_SUBMIT_KEY' => 'enter',
    'SECOND_CLIENT_PROVIDER_KEY' => Provider::API_KEY,
    'TERM' => 'xterm-256color', 'COLUMNS' => COLS.to_s, 'LINES' => ROWS.to_s,
    'PATH' => '/usr/bin:/bin', 'LANG' => 'C', 'LC_ALL' => 'C',
    'DYLD_LIBRARY_PATH' => library, 'LD_LIBRARY_PATH' => library
  }
  result['fixture_root'] = RUN_DIR

  client_a = launch(env, project)
  clients << client_a
  ACTIVE_CLIENTS << client_a
  raise 'client A did not reach native composer/model' unless wait_ready(client_a)
  result['client_a_native_ready'] = true
  raise 'PTY A geometry mismatch' unless [client_a[:reader].winsize[0], client_a[:reader].winsize[1]] == [ROWS, COLS]
  result['native_pty_geometry_verified'] = true
  daemon = checked_descriptor(data_dir)
  result['daemon_authenticated'] = true
  daemon_identity = descriptor_identity(daemon)
  id = session_id(daemon)
  result['session_created'] = true
  result['session_id'] = id
  result['client_a_pid'] = client_a[:pid]
  result['daemon_pid'] = daemon['pid']
  raise 'client A and daemon PID unexpectedly match' if client_a[:pid] == daemon['pid']

  submit(client_a, FIRST_PROMPT)
  wait_post_count(provider, 1)
  first_post = provider.posts.fetch(0)
  raise 'first provider POST history mismatch' unless first_post.dig('json', 'input') == [{'role' => 'user', 'content' => FIRST_PROMPT}]
  result['first_turn_exact_post'] = true
  raise 'client A did not render first provider reply' unless wait_screen_text(client_a, FIRST_REPLY)
  result['first_reply_rendered'] = true
  first_rows = history(daemon, id)
  expected_history!(first_rows, [['user', FIRST_PROMPT], ['assistant', FIRST_REPLY]])
  result['first_turn_typed_history'] = true
  result['first_turn_history_count'] = first_rows.length
  result['client_a_first_reply_visible'] = true
  result['provider_posts_after_first_turn'] = provider.posts.length

  client_b = launch(env, project, id)
  clients << client_b
  ACTIVE_CLIENTS << client_b
  raise 'client B did not render resumed session history' unless wait_ready(client_b) && wait_screen_text(client_b, FIRST_REPLY)
  result['client_b_native_ready'] = true
  after_attach = checked_descriptor(data_dir)
  raise 'client B changed authenticated daemon identity' unless descriptor_identity(after_attach) == daemon_identity
  result['same_daemon_identity'] = true
  result['same_session_resumed'] = client_b[:screen].text.include?(FIRST_REPLY) && history(after_attach, id) == first_rows
  raise 'client A exited while client B attached' unless reap_status(client_a).nil?
  raise 'client B exited while attached' unless reap_status(client_b).nil?
  result['both_clients_alive_on_attach'] = true
  raise 'client B attachment duplicated provider work' unless provider.posts.length == 1
  result['no_extra_post_on_attach'] = true
  attached_rows = history(after_attach, id)
  raise 'client B attachment changed persisted message records' unless attached_rows == first_rows
  result['persisted_history_unchanged_on_attach'] = true
  raise 'client B attachment created another session' unless session_id(after_attach) == id
  result['client_b_attached_same_session'] = true
  result['client_a_alive_during_attach'] = true
  result['client_b_alive_during_attach'] = true
  result['daemon_identity_unchanged'] = true
  result['no_provider_post_on_attach'] = true
  result['persisted_records_unchanged_on_attach'] = true
  result['client_b_first_reply_visible'] = true
  result['client_b_cursor_queries'] = client_b[:cursor_queries]

  submit(client_b, SECOND_PROMPT)
  wait_post_count(provider, 2)
  second_post = provider.posts.fetch(1)
  raise 'second provider POST history mismatch' unless second_post.dig('json', 'input') == [
    {'role' => 'user', 'content' => FIRST_PROMPT},
    {'role' => 'assistant', 'content' => FIRST_REPLY},
    {'role' => 'user', 'content' => SECOND_PROMPT}
  ]
  result['client_b_second_turn_exact_post'] = true
  raise 'client B did not render second provider reply' unless wait_screen_text(client_b, SECOND_REPLY)
  result['second_reply_rendered'] = true
  second_rows = history(daemon, id)
  expected_history!(second_rows, [
    ['user', FIRST_PROMPT], ['assistant', FIRST_REPLY],
    ['user', SECOND_PROMPT], ['assistant', SECOND_REPLY]
  ])
  result['provider_posts_after_second_turn'] = provider.posts.length
  result['second_turn_history_count'] = second_rows.length
  result['client_b_second_reply_visible'] = true
  result['distinct_second_turn_persisted'] = true

  result['client_a_observed_second_reply_live'] = wait_screen_text(client_a, SECOND_REPLY, 8)
  result['client_a_live_refresh'] = result['client_a_observed_second_reply_live']
  unless result['client_a_observed_second_reply_live']
    result['failures'] ||= []
    result['failures'] << 'client A did not live-refresh client B second-turn reply'
  end
  result['provider_posts_after_live_refresh_window'] = provider.posts.length
  raise 'client A or B exited before restart checks' unless reap_status(client_a).nil? && reap_status(client_b).nil?
  raise 'provider work duplicated while checking live refresh' unless provider.posts.length == 2
  result['no_extra_post_during_live_refresh'] = true

  stop_client(client_b)
  stop_client(client_a)
  ACTIVE_CLIENTS.clear
  client_b = client_a = nil
  result['both_clients_clean_exit'] = clients[0][:status]&.success? && clients[1][:status]&.success?
  raise 'native clients did not exit successfully' unless result['both_clients_clean_exit']
  result['client_a_exit_status'] = clients[0][:status].to_i
  result['client_b_exit_status'] = clients[1][:status].to_i

  client_a_reopened = launch(env, project, id)
  clients << client_a_reopened
  ACTIVE_CLIENTS << client_a_reopened
  raise 'reopened client A did not render both durable turns' unless wait_ready(client_a_reopened) &&
    wait_screen_text(client_a_reopened, FIRST_REPLY) && wait_screen_text(client_a_reopened, SECOND_REPLY)
  result['reopened_same_session'] = true
  restarted_descriptor = checked_descriptor(data_dir)
  raise 'restart changed authenticated daemon identity' unless descriptor_identity(restarted_descriptor) == daemon_identity
  reopened_rows = history(restarted_descriptor, id)
  expected_history!(reopened_rows, [
    ['user', FIRST_PROMPT], ['assistant', FIRST_REPLY],
    ['user', SECOND_PROMPT], ['assistant', SECOND_REPLY]
  ])
  result['reopened_typed_history'] = true
  raise 'restart replayed provider work' unless provider.posts.length == 2
  result['no_post_on_restart'] = true
  raise 'restart changed session identity' unless session_id(restarted_descriptor) == id
  result['reopened_client_same_session'] = true
  result['reopened_history_matches'] = true
  result['no_provider_post_on_restart'] = true
  result['final_provider_posts'] = provider.posts.length
  result['client_a_reopened_pid'] = client_a_reopened[:pid]
  result['cursor_position_queries'] = clients.map { |client| client[:cursor_queries] }
  result['cursor_query_answers_sent'] = clients.map { |client| client[:cursor_query_answers].length }
  result['cursor_queries_balanced'] = clients.each_with_index.all? do |client, index|
    client[:cursor_queries] == client[:cursor_query_answers].length && client[:cursor_queries_outstanding].zero?
  end
  raise 'native PTY cursor-position queries remain unanswered' unless result['cursor_queries_balanced']
  result['pty_geometry_verified'] = clients.all? { |client| client[:pty_geometry] == [ROWS, COLS] }
  raise 'native PTY geometry mismatch' unless result['pty_geometry_verified']
rescue StandardError => error
  result['error'] = "#{error.class}: #{error.message}"
  result['backtrace'] = error.backtrace&.first(8)
ensure
  clients.reverse_each do |client|
    begin
      stop_client(client)
    rescue StandardError => error
      result['client_cleanup_error'] ||= "#{error.class}: #{error.message}"
    end
  end
  if daemon
    begin
      result['owned_daemon_cleanup'] = kill_owned_daemon(daemon, daemon_identity)
      result['owned_daemon_alive_after_cleanup'] = alive?(daemon.fetch('pid'))
      result['owned_cleanup'] = result['owned_daemon_cleanup'] && !result['owned_daemon_alive_after_cleanup']
      result['cleanup_error'] = 'owned daemon remains alive' unless result['owned_cleanup']
    rescue StandardError => error
      result['owned_daemon_cleanup'] = false
      result['owned_cleanup'] = false
      result['cleanup_error'] = "#{error.class}: #{error.message}"
    end
  else
    result['owned_daemon_cleanup'] = true
    result['owned_daemon_alive_after_cleanup'] = false
    result['owned_cleanup'] = true
  end
  if provider
    result['provider_requests'] = provider.requests.map do |request|
      body = request['json']
      input = body.is_a?(Hash) && body['input'].is_a?(Array) ? body['input'] : []
      {
        'method' => request['method'], 'path' => request['path'],
        'model' => body.is_a?(Hash) ? body['model'] : nil,
        'stream' => body.is_a?(Hash) ? body['stream'] : nil,
        'user_messages' => input.select { |item| item.is_a?(Hash) && item['role'] == 'user' }.map { |item| item['content'] },
        'assistant_messages' => input.select { |item| item.is_a?(Hash) && item['role'] == 'assistant' }.map { |item| item['content'] }
      }
    end
    result['provider_errors'] = provider.errors
    result['provider_post_count'] = provider.posts.length
    provider.close
  end
  clients.each_with_index do |client, index|
    File.binwrite(File.join(RUN_DIR, "client-#{index + 1}.ansi"), client[:captured])
    result["client_#{index + 1}_pid_reaped"] = !reap_status(client).nil?
    client[:reader].close rescue nil
    client[:writer].close rescue nil
  end
  result['required_checks'] = %w[
    binary_identity_verified native_library_hashed native_pty_geometry_verified
    client_a_native_ready daemon_authenticated session_created first_turn_exact_post
    first_reply_rendered first_turn_typed_history client_b_native_ready same_session_resumed
    same_daemon_identity both_clients_alive_on_attach no_extra_post_on_attach
    persisted_history_unchanged_on_attach client_b_second_turn_exact_post
    second_reply_rendered distinct_second_turn_persisted client_a_live_refresh
    no_extra_post_during_live_refresh both_clients_clean_exit reopened_same_session
    reopened_typed_history no_post_on_restart cursor_queries_balanced owned_cleanup
  ]
  result['required_checks'].each { |key| result[key] = false unless result.key?(key) }
  result['all_required_checks_passed'] = result['required_checks'].all? { |key| result[key] == true } &&
    result['error'].nil? && result['cleanup_error'].nil? && result['client_cleanup_error'].nil? &&
    result['provider_errors'].to_a.empty? && result['provider_post_count'] == 2
  result['phase'] = result['all_required_checks_passed'] ? 'success' : 'failed'
  result['artifact_dir'] = RUN_DIR
  File.write(File.join(RUN_DIR, 'summary.json'), JSON.pretty_generate(result) + "\n")
end

puts JSON.pretty_generate(result)
exit(result['phase'] == 'success' ? 0 : 1)
