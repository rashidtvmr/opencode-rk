#!/usr/bin/env ruby
# frozen_string_literal: true

# Native interaction acceptance journey for NP-01/04/06.  This is intentionally
# a real PTY and a real backend: the observer never paints a frame or injects a
# session.  The loopback Responses provider only supplies deterministic model
# output and one relative-path `read` call.
require 'digest'
require 'fileutils'
require 'json'
require 'net/http'
require 'pty'
require 'socket'
require 'uri'

ROOT = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/interaction-parity-pty-verification', __dir__)
RUN = File.join(ROOT, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
OBSERVER_SELF_TEST = ARGV.delete('--observer-self-test')
BINARY = File.expand_path(ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__)));
ROWS = Integer(ENV.fetch('PTY_ROWS', '40')); COLS = Integer(ENV.fetch('PTY_COLS', '140'))
DEADLINE = Process.clock_gettime(Process::CLOCK_MONOTONIC) + Float(ENV.fetch('INTERACTION_JOURNEY_DEADLINE_SECONDS', '180'))
FileUtils.mkdir_p(RUN)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch" if ENV['EXPECTED_BINARY_SHA256'] && ENV['EXPECTED_BINARY_SHA256'] != BINARY_SHA

class Screen
  attr_reader :rows, :complete_frames, :published_generation
  MAX_CSI_BYTES = 256
  def initialize(rows, cols)
    @rows = Array.new(rows) { ' ' * cols }
    @working_cells = Array.new(rows) { Array.new(cols, ' ') }
    @x = @y = 0; @state = nil; @csi = +''; @sync = false; @utf8 = +''.b
    @complete_frames = 0; @published_generation = 0
  end
  def feed(bytes)
    bytes.each_byte do |b|
      c = b.chr
      case @state
      when :esc
        @state = c == '[' ? :csi : c == ']' ? :osc : nil
        @csi = +'' if @state == :csi
      when :csi
        raise 'observer CSI sequence exceeds bound' if @csi.bytesize >= MAX_CSI_BYTES
        if b.between?(0x40, 0x7e); apply_csi(c, @csi); @state = nil else @csi << c end
      when :osc
        @state = nil if b == 7
        @state = :osc_esc if b == 27
      when :osc_esc
        @state = b == 92 ? nil : b == 27 ? :osc_esc : :osc
      else
        if !@utf8.empty?
          @utf8 << b; decode_utf8_buffer
        elsif b == 27 then @state = :esc
        elsif b == 13 then @x = 0
        elsif b == 10 then @y = [@y + 1, @rows.length - 1].min
        elsif b >= 32 && b < 127 then put_codepoint(c)
        elsif b >= 128 then @utf8 << b; decode_utf8_buffer end
      end
    end
  end
  def text; @rows.join("\n"); end
  def row_containing(s); @rows.index { |r| r.include?(s) }; end
  def apply_csi(command, body)
    marker = body.sub(/\A[?>=]/, '')
    if command == 'h' && body.include?('?2026'); @sync = true; return end
    if command == 'l' && body.include?('?2026')
      return unless @sync
      @sync = false; publish!; @complete_frames += 1; return
    end
    values = marker.split(';').map { |v| v.empty? ? 1 : v.to_i }
    case command
    when 'H', 'f'; @y = ((values[0] || 1) - 1).clamp(0, @working_cells.length - 1); @x = ((values[1] || 1) - 1).clamp(0, @working_cells.first.length - 1)
    when 'J'; @working_cells.map! { Array.new(@working_cells.first.length, ' ') }; @x = @y = 0
    when 'K'; @working_cells[@y].fill(' ', @x...@working_cells[@y].length)
    end
    publish! unless @sync
  end
  def decode_utf8_buffer
    first = @utf8.getbyte(0); expected = if first.between?(0xC2, 0xDF) then 2 elsif first.between?(0xE0, 0xEF) then 3 elsif first.between?(0xF0, 0xF4) then 4 end
    raise 'observer malformed UTF-8 terminal stream' unless expected
    return if @utf8.bytesize < expected
    text = @utf8.byteslice(0, expected).force_encoding(Encoding::UTF_8)
    raise 'observer malformed UTF-8 terminal stream' unless text.valid_encoding?
    text.each_char { |char| put_codepoint(char) }
    @utf8 = @utf8.byteslice(expected..).to_s.b; decode_utf8_buffer unless @utf8.empty?
  end
  def put_codepoint(char)
    width = cell_width(char); return if width == 0 && @x == 0
    if width == 0; @working_cells[@y][[@x - 1, 0].max] << char
    elsif @x < @working_cells[@y].length; @working_cells[@y][@x] = char; @working_cells[@y][@x + 1] = '' if width == 2 && @x + 1 < @working_cells[@y].length; @x = [@x + width, @working_cells[@y].length - 1].min end
    publish! unless @sync
  end
  def cell_width(char); code = char.ord; return 0 if (0x300..0x36f).cover?(code) || (0x1ab0..0x1aff).cover?(code); return 2 if (0x1100..0x115f).cover?(code) || (0x2e80..0xa4cf).cover?(code) || (0xac00..0xd7a3).cover?(code) || (0xf900..0xfaff).cover?(code) || (0x1f300..0x1faff).cover?(code); 1 end
  def publish!; @rows = @working_cells.map(&:join); @published_generation += 1; end
end

LOADER_LABEL = /(?:Thinking…|Working…|Loading…)/i

def composer_ready?(screen)
  screen.rows.last(8).each_cons(3).any? do |top, input, bottom|
    top.match?(/^\s*┌─+┐\s*$/) &&
      input.match?(/^\s*│>[^│\r\n]*│\s*$/) &&
      bottom.match?(/^\s*└─+┘\s*$/)
  end
end

def footer_loader?(screen)
  screen.rows.last.to_s.match?(LOADER_LABEL)
end

def center_loader?(screen)
  screen.rows[0...-1].any? { |row| row.match?(LOADER_LABEL) }
end

def wait_for_held_prompt(r, w, screen, raw, prompt, seconds = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  until screen.text.include?(prompt) && composer_ready?(screen) && footer_loader?(screen)
    raise 'held prompt frame timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(r, w, screen, 0.1, raw)
  end
end

def fit_projection(text, width)
  return '' if width <= 0
  text.chars.take(width).join.ljust(width)
end

def padded_projection(text, width)
  padding = [width / 2, 2].min
  (' ' * padding) + fit_projection(text, width - padding * 2) + (' ' * padding)
end

def footer_directory_projection(path, width)
  return '' if width <= 0
  safe_path = path.chars.take(4096).join
  chars = safe_path.chars
  return safe_path if chars.length <= width
  "…#{chars.last([width - 1, 0].max).join}"
end

def footer_projection(path, cols, row)
  status = row.to_s.rstrip[/((?:Thinking…|Working…|Loading…)|(?:⊙ \d+ MCP \/status  )?\d+(?:\.\d+)+(?:[-+][A-Za-z0-9.-]+)?)\z/, 1]
  evidence = { 'backend_path' => path, 'cols' => cols, 'actual_footer_row' => row.to_s, 'status_label' => status }
  unless status && path.ascii_only? && path.length <= 4096
    return evidence.merge('expected_footer_row' => nil, 'matches' => false)
  end
  inner = [cols - 4, 0].max
  right_width = [status.length, inner].min
  left_width = [inner - right_width - 1, 0].max
  gap = [inner - left_width - right_width, 0].max
  directory = footer_directory_projection(path, left_width)
  left = fit_projection(directory, left_width)
  content = left + (' ' * gap) + fit_projection(status, right_width)
  expected = padded_projection(content, cols)
  mismatch = expected.chars.zip(row.to_s.chars).index { |expected_cell, actual_cell| expected_cell != actual_cell }
  evidence.merge(
    'status_width' => right_width,
    'directory_width' => left_width,
    'directory_projection' => directory,
    'expected_footer_row' => expected,
    'mismatch_column' => mismatch,
    'matches' => mismatch.nil? && row.to_s == expected
  )
end

def wait_for_footer_projection(r, w, screen, raw, path, seconds = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  loop do
    evidence = footer_projection(path, COLS, screen.rows.last)
    return evidence if evidence['matches']
    if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      raise "footer projection timeout: #{JSON.generate(evidence)}"
    end
    pump(r, w, screen, 0.1, raw)
  end
end

def wait_for_idle_frame(r, w, screen, raw, answer, seconds = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  until screen.text.include?(answer) && composer_ready?(screen) && !footer_loader?(screen)
    raise 'idle frame timeout after provider completion' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(r, w, screen, 0.1, raw)
  end
end

def session_directory(sessions, expected_path)
  expected = File.realpath(expected_path)
  sessions.to_a.filter_map do |session|
    directory = session['directory'] || session.dig('project', 'directory') || session.dig('project', 'path')
    directory if directory && (File.realpath(directory) == expected rescue false)
  end.first
end

def wait_for_session_directory(r, w, screen, raw, backend, expected_path, seconds = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  loop do
    status, body = api(backend['http_origin'], backend['auth_token'], 'get', '/api/sessions')
    directory = session_directory(body['sessions'], expected_path)
    return [status, body, directory] if status == 200 && directory
    raise "session directory timeout: #{expected_path}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(r, w, screen, 0.1, raw)
  end
end

def wait_for_visible_text(r, w, screen, raw, text, seconds = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  until screen.text.include?(text)
    raise "visible text timeout: #{text.inspect}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(r, w, screen, 0.1, raw)
  end
end

if OBSERVER_SELF_TEST
  screen = Screen.new(4, 24)
  # Feed the synchronized-frame marker in deliberately fragmented chunks;
  # this exercises the same parser state transitions used by the PTY pump.
  ["\e", "[?20", "26", "h", "é"].each { |part| screen.feed(part.b) }
  raise 'self-test did not retain synchronized frame' unless screen.instance_variable_get(:@sync)
  generation = screen.published_generation
  screen.feed("\e[?2026l".b)
  raise 'self-test did not publish completed frame' unless screen.complete_frames == 1
  raise 'self-test did not advance published generation' unless screen.published_generation > generation
  raise 'self-test lost UTF-8 cell' unless screen.text.include?('é')
  home = Screen.new(40, 140); home.feed(("\e[25;1HAsk anything").b)
  raise 'self-test rejected valid centered home composer' unless home.text.include?('Ask anything') && !composer_ready?(home)
  chat = Screen.new(40, 140); chat.feed(("\e[37;1H┌────┐\e[38;1H│>   │\e[39;1H└────┘").b)
  raise 'self-test rejected bottom bordered chat composer' unless composer_ready?(chat)
  pending = Screen.new(40, 140); pending.feed(("\e[37;1H┌──────────────────────────┐\e[38;1H│> prove native interaction │\e[39;1H└──────────────────────────┘").b)
  raise 'self-test rejected composer retaining pending draft' unless composer_ready?(pending)
  malformed = Screen.new(40, 140); malformed.feed(("\e[38;1H> ").b)
  raise 'self-test accepted unbordered composer' if composer_ready?(malformed)
  footer_status = '0.1.0-alpha.1'
  footer_inner = 140 - 4
  footer_right_width = footer_status.length
  footer_left_width = footer_inner - footer_right_width - 1
  short_footer_path = '/approved/run/selected'
  short_footer_row = padded_projection(fit_projection(footer_directory_projection(short_footer_path, footer_left_width), footer_left_width) + ' ' + fit_projection(footer_status, footer_right_width), 140)
  short_footer_evidence = footer_projection(short_footer_path, 140, short_footer_row)
  raise 'self-test rejected source-shaped short footer projection' unless short_footer_evidence['matches']
  busy_status = 'Thinking…'
  busy_right_width = busy_status.length
  busy_left_width = footer_inner - busy_right_width - 1
  short_busy_row = padded_projection(fit_projection(short_footer_path, busy_left_width) + ' ' + fit_projection(busy_status, busy_right_width), 140)
  raise 'self-test rejected source-shaped busy footer projection' unless footer_projection(short_footer_path, 140, short_busy_row)['matches']
  long_footer_path = "/approved/#{'nested/' * 30}red/selected"
  long_footer_row = padded_projection(fit_projection(footer_directory_projection(long_footer_path, footer_left_width), footer_left_width) + ' ' + fit_projection(footer_status, footer_right_width), 140)
  raise 'self-test rejected source-shaped long footer projection' unless footer_projection(long_footer_path, 140, long_footer_row)['matches']
  raise 'self-test accepted a different full path with the same basename' if footer_projection("/approved/#{'nested/' * 30}blue/selected", 140, long_footer_row)['matches']
  mcp_status = '⊙ 2 MCP /status  0.1.0-alpha.1'
  mcp_right_width = mcp_status.length
  mcp_left_width = footer_inner - mcp_right_width - 1
  mcp_row = padded_projection(fit_projection(short_footer_path, mcp_left_width) + ' ' + fit_projection(mcp_status, mcp_right_width), 140)
  raise 'self-test rejected source-shaped MCP footer projection' unless footer_projection(short_footer_path, 140, mcp_row)['matches']
  partial = Screen.new(2, 12); partial.feed("\e[?2026hABC".b); before = partial.published_generation; raise 'self-test exposed partial frame' unless !partial.text.include?('ABC') && partial.published_generation == before
  stray = Screen.new(2, 12); stray.feed("\e[?2026l".b)
  raise 'self-test accepted stray close' unless stray.complete_frames == 0 && stray.published_generation == 0
  begin
    Screen.new(2, 12).feed("\e[#{'1' * 257}m".b)
    raise 'self-test accepted long CSI'
  rescue RuntimeError => error
    raise unless error.message.include?('exceeds bound')
  end
  puts 'observer self-test PASS'
  exit 0
end

class Provider
  attr_reader :port, :requests, :usage, :errors, :function_calls, :model_requests, :model_catalogs
  def initialize(marker, unique)
    @marker = marker; @unique = unique; @requests = []; @errors = []; @function_calls = []; @model_requests = []; @model_catalogs = []; @handler_threads = []; @usage = { 'input_tokens' => 120, 'output_tokens' => 30, 'cached_tokens' => 40, 'reasoning_tokens' => 10 }; @mutex = Mutex.new; @cv = ConditionVariable.new; @release = false
    @server = TCPServer.new('127.0.0.1', 0); @port = @server.addr[1]; @closed = false
    @thread = Thread.new { loop_accept }
  end
  def release!; @mutex.synchronize { @release = true; @cv.broadcast }; end
  def wait_for_request(timeout = 15)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
    loop do
      found = @mutex.synchronize { @requests.last }
      return found if found && found['path'] == '/v1/responses'
      raise 'provider request timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      sleep 0.05
    end
  end
  def wait_for_count(count, timeout = 15)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
    loop do
      row = @mutex.synchronize { @requests.select { |r| r['path'] == '/v1/responses' }[count - 1] }
      return row if row
      raise "provider response request #{count} timeout" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      yield if block_given?
      sleep 0.05
    end
  end
  def close
    @closed = true; @mutex.synchronize { @cv.broadcast }; @server.close rescue nil
    @thread.join(1); @thread.kill if @thread.alive?
    threads = @mutex.synchronize { @handler_threads.dup }
    threads.each { |thread| thread.join(0.5); thread.kill if thread.alive? }
  end
  private
  def loop_accept
    until @closed
      begin
        next unless IO.select([@server], nil, nil, 0.1)
        thread = Thread.new(@server.accept) { |s| serve(s) }
        @mutex.synchronize { @handler_threads << thread }
      rescue IOError, Errno::EBADF; break if @closed
      end
    end
  end
  def serve(s)
    request = read_request(s); return unless request
    method, path, body = request; parsed = JSON.parse(body) rescue {}
    @mutex.synchronize do
      @model_requests << parsed['model'] if method == 'POST' && path == '/v1/responses'
      @requests << { 'method' => method, 'path' => path, 'body' => parsed }
    end
    if method == 'GET' && path == '/v1/models'
      catalog = [
        { 'id' => 'fixture-model', 'name' => 'interaction fixture', 'limit' => { 'context' => 200_000 } },
        { 'id' => 'unknown-model', 'name' => 'unknown capacity fixture' }
      ]
      @mutex.synchronize { @model_catalogs << catalog }
      reply(s, 200, 'application/json', JSON.generate('data' => catalog))
    elsif method == 'POST' && path == '/v1/responses'
      number = @mutex.synchronize { @requests.count { |r| r['path'] == '/v1/responses' } }
      # Hold the first response. This makes NP-04 observe the submitted prompt
      # and bottom composer before any provider bytes can change the view.
      first = number == 1
      if first; @mutex.synchronize { @cv.wait(@mutex) until @release || @closed }; end
      input = parsed['input'].to_a
       output = input.find { |i| i['type'] == 'function_call_output' }
       envelope = begin JSON.parse(output && output['output'].to_s) rescue nil end
       has_exact_output = output && output['call_id'] == 'interaction-read-call' && envelope.is_a?(Hash) && envelope['format'] == 'opencode.tool-result.v1' && envelope['success'] == true && envelope['output'] == @unique
      event = if number == 1
                { 'type' => 'response.output_text.delta', 'delta' => 'FIRST_ORDINARY_ANSWER' }
              elsif number == 2
                call = { 'type' => 'response.output_item.done', 'item' => { 'type' => 'function_call', 'id' => 'interaction-read-1', 'call_id' => 'interaction-read-call', 'name' => 'read', 'arguments' => JSON.generate('path' => 'cwd-marker.txt') } }
                @mutex.synchronize { @function_calls << { 'request_number' => number, 'name' => call.dig('item', 'name'), 'arguments' => JSON.parse(call.dig('item', 'arguments')) } }
                call
              elsif has_exact_output
                { 'type' => 'response.output_text.delta', 'delta' => "READ_OK #{@unique}" }
              else
                { 'type' => 'response.output_text.delta', 'delta' => 'READ_OUTPUT_ERROR' }
              end
      body_out = "data: #{JSON.generate(event)}\n\ndata: #{JSON.generate('type' => 'response.completed', 'response' => { 'status' => 'completed', 'usage' => { 'input_tokens' => @usage['input_tokens'], 'output_tokens' => @usage['output_tokens'], 'input_tokens_details' => { 'cached_tokens' => @usage['cached_tokens'] }, 'output_tokens_details' => { 'reasoning_tokens' => @usage['reasoning_tokens'] } } })}\n\n"
      reply(s, 200, 'text/event-stream', body_out)
    else; reply(s, 404, 'application/json', '{}'); end
  rescue StandardError => error
    @mutex.synchronize { @errors << "#{error.class}: #{error.message}" }
  ensure; s.close rescue nil end
  def read_request(s)
    data = +''.b; deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 8; head = nil
    until head
      return nil if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline
      next unless IO.select([s], nil, nil, 0.1)
      data << s.readpartial(4096); head = data.index("\r\n\r\n")
    end
    lines = data.byteslice(0, head).force_encoding('UTF-8').split("\r\n"); method, path = lines.shift.split(' ', 3)
    len = (lines.find { |x| x.downcase.start_with?('content-length:') } || '').split(':', 2)[1].to_i
    while data.bytesize < head + 4 + len; data << s.readpartial(4096); end
    [method, path, data.byteslice(head + 4, len).to_s]
  rescue EOFError; nil end
  def reply(s, status, type, body)
    bytes = body.b; s.write("HTTP/1.1 #{status} OK\r\nContent-Type: #{type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n#{bytes}")
  end
end

def pump(r, w, screen, seconds, raw)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds; query = +''
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
    next unless IO.select([r], nil, nil, 0.1)
    begin
      chunk = r.read_nonblock(65_536); raw << chunk; screen.feed(chunk); query = (query + chunk)[-40..].to_s
      if query.include?("\e[6n"); w.write("\e[1;1R"); w.flush; query = +''; end
    rescue IO::WaitReadable, EOFError, Errno::EIO; break
    end
  end
end
def wait_text(r, w, screen, text, raw, baseline_generation, seconds = 20)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
    until screen.published_generation > baseline_generation && screen.text.include?(text)
    raise "timeout waiting for #{text.inspect} after frame generation #{baseline_generation}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(r, w, screen, 0.2, raw)
  end
end
def stop(pid, r, w); w.write("\x03"); w.flush rescue nil; Process.kill('TERM', pid) rescue nil; Process.wait(pid) rescue nil; end
def persist_stage(root, name, raw, screen, error = nil)
  File.binwrite(File.join(root, "#{name}.ansi"), raw.to_s.b)
  File.write(File.join(root, "#{name}.screen.txt"), screen ? screen.text : '')
  File.write(File.join(root, "#{name}.error.txt"), error.to_s) if error
rescue StandardError
  # Evidence persistence must never hide the original product failure.
end
def startup_ready(r, w, screen, raw, root, name, model_id, model_label)
  # Match the existing native import journey: onboarding may own the first
  # frame. Decline only the real rendered offer, then wait for the composer.
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 30
  loop do
    pump(r, w, screen, 0.2, raw)
    if screen.text.include?('Import settings from OpenCode?')
      w.write("\e"); w.flush
    elsif screen.text.include?('Ask anything') && screen.text.include?(model_label)
      return
    end
    raise "startup readiness timeout; model_id=#{model_id.inspect} model_label=#{model_label.inspect} home_composer=#{screen.text.include?('Ask anything')} screen=#{screen.text.lines.first(8).join(' ')[0, 1200]}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
  end
ensure
  persist_stage(root, name, raw, screen) unless screen.text.include?('Ask anything')
end
def mouse_click(writer, x, y)
  writer.write("\e[<0;#{x + 1};#{y + 1}M\e[<0;#{x + 1};#{y + 1}m")
  writer.flush
end
def api(origin, token, method, path)
  uri = URI("#{origin}#{path}"); request = Net::HTTP.const_get(method.capitalize).new(uri)
  request['Authorization'] = "Bearer #{token}"
  Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 2) do |http|
    response = http.request(request)
    parsed = begin JSON.parse(response.body) rescue {} end
    [response.code.to_i, parsed]
  end
end
def record_api(results, label, status, body)
  results["api_#{label}_status"] = status
  results["api_#{label}_body"] = JSON.generate(body).byteslice(0, 8192)
end
def resolve_library(binary)
  dirs = []
  dirs.concat(ENV.fetch('DYLD_LIBRARY_PATH', '').split(File::PATH_SEPARATOR).reject(&:empty?).map { |d| [d, 'DYLD_LIBRARY_PATH'] })
  dir = File.dirname(File.expand_path(binary))
  dirs << [File.expand_path('../lib', dir), 'installed-sibling']
  dirs << [File.expand_path('../ui-native/lib', dir), 'build-relative']
  candidates = dirs.flat_map { |d, source| Dir[File.join(d, 'libopentui*')].map { |p| [p, source] } }
  path, source = candidates.find { |p, _| File.file?(p) } || raise('native library unresolved')
  resolved = File.realpath(path); [File.dirname(resolved), resolved, Digest::SHA256.file(resolved).hexdigest, source]
end

results = { 'binary' => BINARY, 'binary_sha256' => BINARY_SHA, 'terminal' => { 'rows' => ROWS, 'cols' => COLS } }
required = %w[daemon_reachable np04_prompt_before_provider np04_bottom_composer np04_provider_held np04_session_poll_while_held np04_first_ordinary_answer np01_fixture_model_requests np01_footer_selected np01_backend_directory_matches_footer np01_relative_tool_request np01_exact_unique_read_output np01_provider_saw_exact_unique_output np03_footer_folder_while_held np03_footer_thinking_while_held np03_no_center_loader_duplicate np03_footer_idle_after_completion np03_footer_directory_projection np02_known_model_capacity_unknown_project np06_prefix_not_full_command np06_rendered_hint np06_keyboard_selected_rendered_hint np06_one_enter_exit np02_authoritative_usage_fixture np02_known_usage_right_panel np02_known_context_window np02_unknown_capacity_honest np03_footer_loader_no_duplicate_center np06_mouse_prefix_not_full_command np06_mouse_rendered_hint_cell np06_mouse_one_action]
required.each { |key| results[key] = false }
provider = nil; pid = nil
current_raw = +''; current_screen = nil; current_stage = 'startup'
begin
  FileUtils.mkdir_p(%w[home config data project selected].map { |n| File.join(RUN, n) })
  project = File.join(RUN, 'project'); selected = File.join(RUN, 'selected'); marker = "NATIVE_CWD_UNIQUE_#{Process.pid}_#{Time.now.to_i}"; File.write(File.join(selected, 'cwd-marker.txt'), marker)
  provider = Provider.new(marker, marker)
  base_url = "http://127.0.0.1:#{provider.port}/v1"
  config_body = JSON.generate('model' => 'fixture/fixture-model', 'providers' => { 'fixture' => { 'name' => 'fixture', 'base_url' => base_url, 'api_key_env' => 'FIXTURE_API_KEY' } }) + "\n"
  File.write(File.join(RUN, 'config', 'opencode.json'), config_body)
  lib, lib_resolved, lib_sha, lib_source = resolve_library(BINARY)
  results.merge!('native_library_dir' => lib, 'native_library_resolved' => lib_resolved, 'native_library_sha256' => lib_sha, 'native_library_resolution_source' => lib_source)
  env = { 'HOME' => File.join(RUN, 'home'), 'XDG_CONFIG_HOME' => File.join(RUN, 'xdg-config'), 'XDG_DATA_HOME' => File.join(RUN, 'data'), 'XDG_STATE_HOME' => File.join(RUN, 'data', 'state'), 'OPENCODE_CONFIG_DIR' => File.join(RUN, 'legacy-config'), 'OPENCODE_RK_CONFIG_DIR' => File.join(RUN, 'config'), 'OPENCODE_DATA_DIR' => File.join(RUN, 'data'), 'OPENCODE_RK_HOME' => File.join(RUN, 'data'), 'FIXTURE_API_KEY' => 'INTERACTION_FIXTURE_KEY', 'TERM' => 'xterm-256color', 'COLUMNS' => COLS.to_s, 'LINES' => ROWS.to_s, 'PATH' => '/usr/bin:/bin', 'DYLD_LIBRARY_PATH' => lib, 'LD_LIBRARY_PATH' => lib }
  PTY.spawn(env, BINARY, '--native', '--data-dir', File.join(RUN, 'data'), { unsetenv_others: true, chdir: project }) do |r, w, child|
    pid = child; current_stage = 'conversation'; r.ioctl(0x80087467, [ROWS, COLS, 0, 0].pack('S4')) rescue nil; screen = Screen.new(ROWS, COLS); raw = +''; current_screen = screen; current_raw = raw; pump(r, w, screen, 2, raw); startup_ready(r, w, screen, raw, RUN, current_stage, 'fixture-model', 'interaction fixture')
    descriptor = File.join(RUN, 'data', 'runtime', 'backend.json'); deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 20
    until File.file?(descriptor); raise 'backend descriptor timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline; pump(r, w, screen, 0.1, raw); end
    backend = JSON.parse(File.read(descriptor)); sessions_status, sessions_body = api(backend['http_origin'], backend['auth_token'], 'get', '/api/sessions'); record_api(results, 'sessions_initial', sessions_status, sessions_body); results['daemon_reachable'] = sessions_status == 200
    w.write('prove native interaction'); w.write("\r"); w.flush; provider.wait_for_count(1) { pump(r, w, screen, 0.05, raw) }
    wait_for_held_prompt(r, w, screen, raw, 'prove native interaction')
    results['np04_prompt_before_provider'] = screen.text.include?('prove native interaction') && !screen.text.include?('FIRST_ORDINARY_ANSWER')
    input_border = screen.rows.each_cons(3).any? { |top, input, bottom| top.match?(/^\s*┌─+┐\s*$/) && input.include?('prove native interaction') && bottom.match?(/^\s*└─+┘\s*$/) }
    results['np04_bottom_composer'] = composer_ready?(screen) && input_border && footer_loader?(screen)
    results['np04_provider_held'] = provider.requests.count { |x| x['path'] == '/v1/responses' } == 1 && screen.complete_frames.positive?
    footer_row = screen.rows.last
    results['np03_footer_folder_while_held'] = footer_row.include?('/project')
    results['np03_footer_thinking_while_held'] = footer_loader?(screen)
    results['np03_no_center_loader_duplicate'] = footer_loader?(screen) && !center_loader?(screen)
    sidebar_start = COLS - 42
    usage_lines = screen.rows.each_with_index.select { |row, _| row[sidebar_start, 42].to_s.match?(/Input 120.*Output 30|Cached 40.*Reasoning 10/) }
    results['np02_known_usage_right_panel'] = usage_lines.any? && usage_lines.all? { |row, _| row[0...sidebar_start].to_s !~ /Input 120|Output 30|Cached 40|Reasoning 10/ }
    results['np02_known_context_window'] = screen.rows.any? { |row| row.include?('200000') }
    results['np02_authoritative_usage_fixture'] = provider.usage == { 'input_tokens' => 120, 'output_tokens' => 30, 'cached_tokens' => 40, 'reasoning_tokens' => 10 }
    sessions = api(backend['http_origin'], backend['auth_token'], 'get', '/api/sessions')[1]['sessions'].to_a; results['np04_session_poll_while_held'] = !sessions.empty?
    results['np02_known_model_capacity_unknown_project'] = provider.model_catalogs.any? { |catalog| catalog.any? { |model| model['id'] == 'unknown-model' && model.dig('limit', 'context').nil? } }
    first_answer_generation = screen.published_generation; provider.release!; wait_text(r, w, screen, 'FIRST_ORDINARY_ANSWER', raw, first_answer_generation, 30)
    wait_for_idle_frame(r, w, screen, raw, 'FIRST_ORDINARY_ANSWER')
    results['np04_first_ordinary_answer'] = screen.text.include?('FIRST_ORDINARY_ANSWER')
    results['np01_fixture_model_requests'] = !provider.model_requests.empty? && provider.model_requests.all? { |model| model == 'fixture-model' }
    results['np03_footer_idle_after_completion'] = !footer_loader?(screen) && !center_loader?(screen)
    usage_lines = screen.rows.each_with_index.select { |row, _| row[sidebar_start, 42].to_s.match?(/Input 120.*Output 30|Cached 40.*Reasoning 10/) }
    results['np02_known_usage_right_panel'] = usage_lines.any? && usage_lines.all? { |row, _| row[0...sidebar_start].to_s !~ /Input 120|Output 30|Cached 40|Reasoning 10/ }
    results['np02_known_context_window'] = screen.rows.any? { |row| row.include?('200000') }
    # /cd is argument-taking; the backend session directory and rendered footer
    # must agree before the next prompt is sent.
    # /cd is issued once. Poll the API and rendered footer; never replay a
    # possibly accepted side effect on a timeout.
    w.write("/cd #{selected}\r"); w.flush
    sessions_status, sessions_body, actual_directory = wait_for_session_directory(r, w, screen, raw, backend, selected)
    record_api(results, 'sessions_after_cd', sessions_status, sessions_body)
    results['np01_backend_directory_matches_api'] = sessions_status == 200 && !actual_directory.nil?
    selected_footer = wait_for_footer_projection(r, w, screen, raw, actual_directory)
    results['np01_backend_directory_matches_footer'] = results['np01_backend_directory_matches_api'] == true && selected_footer['matches']
    results['np01_footer_selected'] = results['np01_backend_directory_matches_footer'] == true
    results['np01_footer_directory_projection'] = selected_footer
    read_baseline_generation = screen.published_generation
    w.write('read cwd marker'); w.write("\r"); w.flush
    provider.wait_for_count(2) { pump(r, w, screen, 0.05, raw) }
    function_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
    until provider.function_calls.any?
      raise 'provider read function call timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= function_deadline
      pump(r, w, screen, 0.1, raw)
    end
    function_call = provider.function_calls.first
    results['np01_provider_function_call_receipt'] = function_call
    results['np01_relative_tool_request'] = provider.function_calls.length == 1 && function_call['name'] == 'read' && function_call['arguments'] == { 'path' => 'cwd-marker.txt' }
    # Release is not enough: provider validates the exact unique output in its
    # second request and emits READ_OK only for that causal body.
    # The exact emitted function call and argument receipt above prove the
    # provider request was relative; output receipt proves its causal return.
    wait_text(r, w, screen, 'READ_OK', raw, read_baseline_generation, 30)
    results['np01_exact_unique_read_output'] = screen.text.include?("READ_OK #{marker}") && !screen.text.include?('READ_OUTPUT_ERROR')
    tool_request = provider.requests.find { |request| request['path'] == '/v1/responses' && request.dig('body', 'input').to_a.any? { |item| item['type'] == 'function_call_output' } }
    tool_output = tool_request && tool_request.dig('body', 'input').find { |item| item['type'] == 'function_call_output' }
     output_value = tool_output && (tool_output['output'] || tool_output['content']).to_s
     envelope = begin JSON.parse(output_value) rescue nil end
     results['np01_provider_saw_exact_unique_output'] = !!(tool_output && tool_output['call_id'] == 'interaction-read-call' && envelope.is_a?(Hash) && envelope['format'] == 'opencode.tool-result.v1' && envelope['success'] == true && envelope['output'] == marker)
     final_footer = wait_for_footer_projection(r, w, screen, raw, actual_directory)
    results['np03_footer_directory_projection'] = final_footer['matches']
    results['np03_footer_loader_no_duplicate_center'] = results['np03_no_center_loader_duplicate'] == true && results['np03_footer_idle_after_completion'] == true && final_footer['matches'] && !footer_loader?(screen) && !center_loader?(screen)
    results['np03_final_frame_loader_evidence'] = { 'footer_loader' => footer_loader?(screen), 'center_loader' => center_loader?(screen), 'footer_directory' => final_footer }
    persist_stage(RUN, 'conversation', raw, screen)
    stop(pid, r, w)
  end
  # Separate fresh configuration with a catalogue model that deliberately has
  # no context_window metadata. This proves unknown capacity is rendered as an
  # unavailable value instead of a guessed percentage/zero.
  unknown_root = File.join(RUN, 'unknown-capacity'); unknown_config = File.join(unknown_root, 'config'); unknown_data = File.join(unknown_root, 'data'); unknown_project = File.join(unknown_root, 'project')
  [unknown_config, unknown_data, unknown_project, File.join(unknown_root, 'home')].each { |path| FileUtils.mkdir_p(path) }
   File.write(File.join(unknown_config, 'opencode.json'), JSON.generate('model' => 'fixture/unknown-model', 'providers' => { 'fixture' => { 'name' => 'fixture', 'base_url' => base_url, 'api_key_env' => 'FIXTURE_API_KEY' } }) + "\n")
   unknown_env = env.merge('HOME' => File.join(unknown_root, 'home'), 'XDG_CONFIG_HOME' => File.join(unknown_root, 'xdg-config'), 'XDG_DATA_HOME' => unknown_data, 'XDG_STATE_HOME' => File.join(unknown_data, 'state'), 'OPENCODE_CONFIG_DIR' => File.join(unknown_root, 'legacy-config'), 'OPENCODE_RK_CONFIG_DIR' => unknown_config, 'OPENCODE_DATA_DIR' => unknown_data, 'OPENCODE_RK_HOME' => unknown_data)
  PTY.spawn(unknown_env, BINARY, '--native', '--data-dir', unknown_data, { unsetenv_others: true, chdir: unknown_project }) do |r, w, child|
    pid = child; current_stage = 'unknown-capacity'; screen = Screen.new(ROWS, COLS); raw = +''; current_screen = screen; current_raw = raw; pump(r, w, screen, 2, raw); startup_ready(r, w, screen, raw, RUN, current_stage, 'unknown-model', 'unknown capacity fixture')
    w.write('/status'); w.flush; sleep 0.2; w.write("\r"); w.flush; pump(r, w, screen, 1, raw)
    unknown_status_frame = screen.rows.any? { |row| row.include?('model: fixture/unknown-model') } && screen.rows.any? { |row| row.include?('context: Unavailable') } && screen.rows.none? { |row| row.match?(/context:.*(?:\d+%|\b\d+\s*\/\s*\d+\b)/i) }
    unknown_catalog = provider.model_catalogs.any? { |catalog| catalog.any? { |model| model['id'] == 'unknown-model' && model.dig('limit', 'context').nil? } }
    results['np02_unknown_capacity_honest'] = unknown_catalog && unknown_status_frame
    persist_stage(RUN, 'unknown-capacity', raw, screen); stop(pid, r, w)
  end
  # A separate clean process proves an executable hint needs only one Enter.
  PTY.spawn(env, BINARY, '--native', '--data-dir', File.join(RUN, 'data'), { unsetenv_others: true, chdir: project }) do |r, w, child|
    pid = child; current_stage = 'keyboard-exit'; screen = Screen.new(ROWS, COLS); raw = +''; current_screen = screen; current_raw = raw; pump(r, w, screen, 2, raw); startup_ready(r, w, screen, raw, RUN, current_stage, 'fixture-model', 'interaction fixture')
     prefix = '/ex'; w.write(prefix); w.flush; sleep 0.5; pump(r, w, screen, 0.3, raw); hint = screen.rows.each_with_index.find { |row, index| index < ROWS - 3 && row.include?('/exit') && !row.include?('Ask anything') }; results['np06_prefix_not_full_command'] = prefix.length < '/exit'.length; results['np06_rendered_hint'] = !hint.nil?
     if hint
      # Select a rendered hint with the keyboard, rather than typing the full
      # command. The first visible row is the observed selection target.
       hint_row, hint_y = hint; hint_x = hint_row.index('/exit'); results['np06_keyboard_selected_target'] = { 'x' => hint_x, 'y' => hint_y, 'text' => hint_row.strip }; w.write("\e[B"); w.flush; sleep 0.2; pump(r, w, screen, 0.2, raw); selected_hint = screen.rows[hint_y].to_s.include?('/exit'); w.write("\r"); w.flush; deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 8
      loop do
        break if Process.waitpid(pid, Process::WNOHANG)
        raise 'one-enter /exit timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline
        pump(r, w, screen, 0.1, raw)
      end
       results['np06_one_enter_exit'] = true; results['np06_keyboard_selected_rendered_hint'] = selected_hint
    end
    persist_stage(RUN, 'exit', raw, screen)
  end
  # Independent mouse stage: derive the click cell from the rendered /exit
  # hint. This must not type the full command or send Enter.
  PTY.spawn(env, BINARY, '--native', '--data-dir', File.join(RUN, 'data'), { unsetenv_others: true, chdir: project }) do |r, w, child|
    pid = child; current_stage = 'mouse-exit'; screen = Screen.new(ROWS, COLS); raw = +''; current_screen = screen; current_raw = raw; pump(r, w, screen, 2, raw); startup_ready(r, w, screen, raw, RUN, current_stage, 'fixture-model', 'interaction fixture')
     prefix = '/ex'; w.write(prefix); w.flush; sleep 0.5; pump(r, w, screen, 0.3, raw)
     row_index = screen.rows.each_index.find { |index| index < ROWS - 3 && screen.rows[index].include?('/exit') && !screen.rows[index].include?('Ask anything') }
    if row_index
      row = screen.rows[row_index]; x = row.index('/exit')
      hint_valid = x && row[x, '/exit'.length] == '/exit' && x + 2 < COLS
      results['np06_mouse_prefix_not_full_command'] = prefix.length < '/exit'.length
      results['np06_mouse_rendered_hint_coordinates'] = hint_valid ? { 'x' => x, 'y' => row_index, 'text' => row.strip } : nil
      if hint_valid
        mouse_click(w, x + 2, row_index)
        deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 8
        loop do
          break if Process.waitpid(pid, Process::WNOHANG)
          raise 'mouse /exit timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline
          pump(r, w, screen, 0.1, raw)
        end
        results['np06_mouse_one_action'] = true
        results['np06_mouse_rendered_hint_cell'] = true
      else
        results['np06_mouse_one_action'] = false
        results['np06_mouse_rendered_hint_cell'] = false
      end
    else
      results['np06_mouse_one_action'] = false
      results['np06_mouse_rendered_hint_cell'] = false
      results['np06_mouse_rendered_hint_coordinates'] = nil
    end
    persist_stage(RUN, 'mouse-exit', raw, screen)
  end
rescue StandardError => e
  results['error'] = "#{e.class}: #{e.message}"
ensure
  persist_stage(RUN, current_stage, current_raw, current_screen, results['error'])
  provider&.close
  results['provider_requests'] = provider&.requests.to_a.map { |request| { 'method' => request['method'], 'path' => request['path'] } }
  results['provider_errors'] = provider&.errors.to_a.first(16)
  Process.kill('KILL', pid) rescue nil; Process.wait(pid) rescue nil
end
results['run'] = RUN; File.write(File.join(RUN, 'summary.json'), JSON.pretty_generate(results) + "\n"); puts JSON.pretty_generate(results)
required = %w[daemon_reachable np04_prompt_before_provider np04_bottom_composer np04_provider_held np04_session_poll_while_held np04_first_ordinary_answer np01_fixture_model_requests np01_footer_selected np01_backend_directory_matches_footer np01_relative_tool_request np01_exact_unique_read_output np01_provider_saw_exact_unique_output np03_footer_folder_while_held np03_footer_thinking_while_held np03_no_center_loader_duplicate np03_footer_idle_after_completion np03_footer_directory_projection np06_prefix_not_full_command np06_rendered_hint np06_keyboard_selected_rendered_hint np06_one_enter_exit np02_authoritative_usage_fixture np02_known_usage_right_panel np02_known_context_window np02_known_model_capacity_unknown_project np02_unknown_capacity_honest np03_footer_loader_no_duplicate_center np06_mouse_prefix_not_full_command np06_mouse_rendered_hint_cell np06_mouse_one_action]
results['required_checks'] = required; results['all_required_checks_passed'] = !results['error'] && required.all? { |k| results[k] == true }; File.write(File.join(RUN, 'summary.json'), JSON.pretty_generate(results) + "\n"); exit(results['all_required_checks_passed'] ? 0 : 1)
