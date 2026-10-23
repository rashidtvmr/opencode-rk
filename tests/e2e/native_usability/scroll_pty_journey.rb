#!/usr/bin/env ruby
# frozen_string_literal: true

# Native scroll journey.  This is deliberately a real daemon/API fixture: the
# transcript is persisted through POST /messages and is never painted by the
# observer.  The observer only parses bytes which the native child emitted.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'net/http'
require 'uri'
require 'timeout'
require 'stringio'

GLOBAL_DEADLINE_SECONDS = Float(ENV.fetch('SCROLL_JOURNEY_DEADLINE_SECONDS', '600'))
SCROLL_INPUT_MODE = ENV.fetch('SCROLL_INPUT_MODE', 'mouse')
raise "SCROLL_INPUT_MODE must be mouse or keyboard" unless %w[mouse keyboard].include?(SCROLL_INPUT_MODE)
INPUT_ROWS = SCROLL_INPUT_MODE == 'keyboard' ? 8 : 3

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/scroll-pty-verification', __dir__)
ROOT = File.join(ARTIFACT_DIR, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY_ARG = ARGV.reject { |arg| arg == '--observer-self-test' }.first
BINARY = File.expand_path(BINARY_ARG || File.expand_path('../../../target/debug/oc2', __dir__))
COLS = Integer(ENV.fetch('PTY_COLS', '100'))
ROWS = Integer(ENV.fetch('PTY_ROWS', '32'))
MESSAGE_COUNT = Integer(ENV.fetch('SCROLL_MESSAGE_COUNT', '501'))
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
FileUtils.mkdir_p(ROOT)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && EXPECTED_SHA != BINARY_SHA

class Screen
  attr_reader :rows, :complete_frame_counter
  def initialize(rows, cols)
    @rows = Array.new(rows) { ' ' * cols }
    @working_cells = Array.new(rows) { Array.new(cols, ' ') }
    @x = @y = 0; @state = nil; @csi = +''; @sync = false
    @utf8 = +''.b
    @complete_frame_counter = 0
    @completed_frames = []
  end
  def feed(raw)
    raw.each_byte do |b|
      c = b.chr
      case @state
      when :esc
        @state = c == '[' ? :csi : c == ']' ? :osc : nil
        @csi = +'' if @state == :csi
      when :csi
        if b.between?(0x40, 0x7e); apply(c, @csi); @state = nil else @csi << c end
      when :osc
        @state = nil if b == 7
        @state = :osc_esc if b == 27
      when :osc_esc
        @state = b == 92 ? nil : b == 27 ? :osc_esc : :osc
      else
        if !@utf8.empty?
          @utf8 << b
          decode_utf8_buffer
        elsif b == 27; @state = :esc
        elsif b == 13; @x = 0
        elsif b == 10; @y = [@y + 1, @rows.length - 1].min
        elsif b >= 32 && b < 127
          put_codepoint(c)
        elsif b >= 128
          @utf8 << b
          decode_utf8_buffer
        end
      end
    end
  end
  def text; @rows.join("\n"); end
  def resize(rows, cols)
    @rows = Array.new(rows) { ' ' * cols }
    @working_cells = Array.new(rows) { Array.new(cols, ' ') }
    @x = @y = 0
    @state = nil
    @csi = +''
    @utf8 = +''.b
    @sync = false
  end
  def apply(command, body)
    if command == 'h' && body.include?('?2026')
      @sync = true
      return
    elsif command == 'l' && body.include?('?2026')
      @sync = false
      publish!
      @complete_frame_counter += 1
      raise 'observer completed-frame queue overflow' if @completed_frames.length >= 128
      @completed_frames << @rows.map(&:dup)
      return
    end
    values = body.sub(/\A[?>=]/, '').split(';').map { |v| v.empty? ? 1 : v.to_i }
    case command
    when 'H', 'f'; @y = ((values[0] || 1) - 1).clamp(0, @working_cells.length - 1); @x = ((values[1] || 1) - 1).clamp(0, @working_cells.first.length - 1)
    when 'J'; @working_cells.map! { Array.new(@working_cells.first.length, ' ') }; @x = @y = 0
    when 'K'; @working_cells[@y].fill(' ', @x...@working_cells[@y].length)
    end
    publish! unless @sync
  end
  def decode_utf8_buffer
    return if @utf8.empty?
    first = @utf8.getbyte(0)
    expected = if first.between?(0xC2, 0xDF)
                 2
               elsif first.between?(0xE0, 0xEF)
                 3
               elsif first.between?(0xF0, 0xF4)
                 4
               end
    raise 'observer malformed UTF-8 terminal stream' unless expected
    return if @utf8.bytesize < expected
    text = @utf8.byteslice(0, expected).force_encoding(Encoding::UTF_8)
    if text.valid_encoding?
      text.each_char { |char| put_codepoint(char) }
      @utf8 = @utf8.byteslice(expected..).to_s.b
      decode_utf8_buffer unless @utf8.empty?
    else
      raise 'observer malformed UTF-8 terminal stream'
    end
  end
  def put_codepoint(char)
    width = cell_width(char)
    return if width == 0 && @x == 0
    if width == 0
      @working_cells[@y][[@x - 1, 0].max] << char
    else
      return if @x >= @working_cells[@y].length
      @working_cells[@y][@x] = char
      @working_cells[@y][@x + 1] = '' if width == 2 && @x + 1 < @working_cells[@y].length
      @x = [@x + width, @working_cells[@y].length - 1].min
    end
    publish! unless @sync
  end
  def cell_width(char)
    code = char.ord
    return 0 if (0x300..0x36f).cover?(code) || (0x1ab0..0x1aff).cover?(code)
    return 2 if (0x1100..0x115f).cover?(code) || (0x2e80..0xa4cf).cover?(code) || (0xac00..0xd7a3).cover?(code) || (0xf900..0xfaff).cover?(code) || (0x1f300..0x1faff).cover?(code)
    1
  end
  def publish!
    @rows = @working_cells.map(&:join)
  end
  def drain_completed_frames
    frames = @completed_frames
    @completed_frames = []
    frames
  end
  private
  def publish!
    @rows = @working_cells.map(&:join)
  end
end

class Trace
  attr_reader :started_at
  def initialize(root)
    @started_at = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    @raw_path = File.join(root, 'pty-output.ansi')
    @frames_path = File.join(root, 'frames.jsonl')
    @progress_path = File.join(root, 'progress.json')
    @raw = File.open(@raw_path, 'wb')
    @frames = File.open(@frames_path, 'ab')
    @tail = +''.b
  end
  def <<(chunk)
    @raw.write(chunk)
    @raw.flush
    @tail << chunk
    @tail = @tail.byteslice(-262_144, 262_144).to_s.b
    self
  end
  def to_s; @tail; end
  def elapsed; Process.clock_gettime(Process::CLOCK_MONOTONIC) - @started_at; end
  def frame(entry)
    payload = entry.merge('elapsed_seconds' => elapsed)
    @frames.write(JSON.generate(payload) + "\n")
    @frames.flush
    File.write(@progress_path, JSON.pretty_generate(payload) + "\n")
  end
  def close
    @raw.close unless @raw.closed?
    @frames.close unless @frames.closed?
  end
end

class TraceNull
  def frame(_entry); end
end

def winsize(io, rows, cols)
  io.ioctl(0x80087467, [rows, cols, 0, 0].pack('S4'))
rescue StandardError
  # PTY implementations without TIOCSWINSZ still get COLUMNS/LINES.
end

def pump(reader, writer, seconds, screen, raw, counters = nil)
  raise 'global journey deadline exceeded' if $journey_deadline && Process.clock_gettime(Process::CLOCK_MONOTONIC) >= $journey_deadline
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  query = +''
  fresh = false
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([reader], nil, nil, [left, 0.10].min)
    begin
      chunk = reader.read_nonblock(65_536)
      query = (query + chunk).byteslice(-64, 64).to_s
      if query.include?("\e[6n")
        writer.write("\e[1;1R"); writer.flush; query.clear
        counters['cursor_queries'] += 1 if counters
      end
      raw << chunk; screen.feed(chunk); fresh ||= !chunk.empty?
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO
      break
    end
  end
  fresh
end

def api(descriptor, method, path, body = nil)
  uri = URI(descriptor.fetch('http_origin') + path)
  request = Net::HTTP.const_get(method.capitalize).new(uri)
  request['Authorization'] = "Bearer #{descriptor.fetch('auth_token')}"
  request['Content-Type'] = 'application/json'
  request.body = JSON.generate(body) if body
  Net::HTTP.start(uri.host, uri.port, open_timeout: 3, read_timeout: 8) { |http| response = http.request(request); [response.code.to_i, JSON.parse(response.body)] }
end

def all_fixture_messages(descriptor, session, expected)
  # The ordinary messages endpoint intentionally caps at 500.  Walk the real
  # cursor endpoint so this check proves every persisted fixture record,
  # rather than silently accepting the server's retention/page cap.
  rows = []
  before = nil
  loop do
    path = "/api/sessions/#{session}/history?limit=100"
    path += "&before=#{URI.encode_www_form_component(before)}" if before
    status, value = api(descriptor, 'get', path)
    raise "history HTTP #{status}" unless status == 200
    page = value.fetch('messages')
    rows = page + rows
    before = value['next_before']
    break if before.nil? || page.empty? || rows.length >= expected
  end
  rows
end

def wait_for(reader, writer, screen, text, raw, timeout = 20, counters = nil)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
  until screen.text.include?(text)
    raise 'global journey deadline exceeded' if $journey_deadline && Process.clock_gettime(Process::CLOCK_MONOTONIC) >= $journey_deadline
    raise "timeout waiting for #{text.inspect}" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    pump(reader, writer, 0.25, screen, raw, counters)
  end
end

def paced_wheel(reader, writer, screen, raw, button, x, y, counters, frame_log: nil, phase: nil, trace: nil, baseline: nil, validator: nil, pump_fn: nil, clock: nil, input_strategy: nil, max_rows: 3)
  pump_fn ||= ->(r, w, seconds, s, output, counts) { pump(r, w, seconds, s, output, counts) }
  clock ||= -> { Process.clock_gettime(Process::CLOCK_MONOTONIC) }
  before = screen.text
  before_frame = screen.complete_frame_counter
  screen.drain_completed_frames
  baseline ||= record_frame(frame_log, screen, phase, trace) if frame_log
  changed_seen = false
  writer.write(input_strategy ? input_strategy.call : "\e[<#{button};#{x};#{y}M"); writer.flush
  deadline = clock.call + 5
  fresh = false
  until fresh && screen.complete_frame_counter > before_frame && changed_seen
    return false if $journey_deadline && clock.call >= $journey_deadline
    return false if clock.call >= deadline
    fresh = pump_fn.call(reader, writer, 0.20, screen, raw, counters)
    screen.drain_completed_frames.each do |rows|
      entry = record_rows(frame_log, rows, phase, trace)
      transcript_changed = entry['marker_rows'] != baseline['marker_rows'] ||
        entry['transcript_crop_sha256'] != baseline['transcript_crop_sha256']
      next unless transcript_changed
      if validator && !validator.call(baseline, entry)
        entry['continuity_failure'] = true
        trace.frame(entry.merge('event' => 'continuity_failure')) if trace
        return false
      end
      shared = baseline['marker_rows'].keys & entry['marker_rows'].keys
      deltas = shared.map { |index| entry['marker_rows'][index] - baseline['marker_rows'][index] }
      directional_motion = !shared.empty? && deltas.uniq.one? && deltas.first != 0 && deltas.first.abs <= max_rows
      baseline = entry
      next unless directional_motion
      changed_seen = true
    end
  end
  true
end

def observer_self_test!
  rail = Screen.new(6, 40)
  rail_bytes = "\e[?2026h\e[3;1H┃     oldest494\e[?2026l\e[3;15H2"
  rail_bytes.each_byte { |byte| rail.feed(byte.chr) }
  raise 'self-test Unicode cursor replay produced wrong marker' unless rail.text.include?('oldest492') && !rail.text.include?('oldest4942')
  ascii = Screen.new(2, 12)
  ascii.feed("abc\e[2;1Hdef")
  raise 'self-test ASCII cursor behavior changed' unless ascii.rows[0].start_with?('abc') && ascii.rows[1].start_with?('def')
  combining = Screen.new(1, 8)
  combining.feed("a\u0301b")
  raise 'self-test combining width advanced cursor' unless combining.rows[0].start_with?("a\u0301b")
  wide = Screen.new(1, 8)
  wide.feed("界x")
  raise 'self-test wide glyph width policy failed' unless wide.rows[0].start_with?('界x')
  malformed = false
  begin
    Screen.new(1, 8).feed("\xC3\x28")
  rescue RuntimeError => error
    malformed = error.message.include?('malformed UTF-8')
  end
  raise 'self-test malformed UTF-8 did not fail closed' unless malformed
  screen = Screen.new(8, 40)
  screen.feed("\e[?2026h\e[1;1Holdest000\e[2")
  raise 'self-test published partial synchronized frame' unless screen.complete_frame_counter == 0 && !screen.text.include?('oldest000')
  screen.feed(";1Holdest001\e[?2026")
  raise 'self-test published fragmented marker' unless screen.complete_frame_counter == 0
  screen.feed("l")
  raise 'self-test did not publish completed frame' unless screen.complete_frame_counter == 1 && screen.text.include?('oldest000') && screen.text.include?('oldest001')
  motion = Screen.new(8, 40)
  motion.feed("\e[?2026h\e[1;1H\e[3;1Holdest000\e[4;1Holdest001\e[?2026l")
  rows_before = marker_positions(motion)
  motion.feed("\e[?2026h\e[1;1H\e[2;1Holdest000\e[3;1Holdest001\e[?2026l")
  rows_after = marker_positions(motion)
  before = { 'visible_indices' => rows_before.keys.sort, 'marker_rows' => rows_before, 'transcript_crop_sha256' => 'a' }
  after = { 'visible_indices' => rows_after.keys.sort, 'marker_rows' => rows_after, 'transcript_crop_sha256' => 'b' }
  raise 'self-test validator rejected valid completed movement' unless down_frame_continuity(before, after, require_row_motion: true)
  up_before = { 'visible_indices' => [0, 1], 'marker_rows' => { 0 => 2, 1 => 6 }, 'transcript_crop_sha256' => 'c' }
  up_after = { 'visible_indices' => [0, 1], 'marker_rows' => { 0 => 3, 1 => 7 }, 'transcript_crop_sha256' => 'd' }
  raise 'self-test validator rejected valid up movement' unless up_frame_continuity(up_before, up_after, require_row_motion: true)
  teleport = up_before.merge('visible_indices' => [100, 101], 'marker_rows' => { 100 => 3, 101 => 7 }, 'transcript_crop_sha256' => 'e')
  raise 'self-test accepted up teleport' if up_frame_continuity(up_before, teleport)
  inconsistent = up_before.merge('marker_rows' => { 0 => 3, 1 => 8 }, 'transcript_crop_sha256' => 'f')
  raise 'self-test accepted inconsistent up rows' if up_frame_continuity(up_before, inconsistent)
  chrome_only = up_before.merge('marker_rows' => up_before['marker_rows'].dup, 'transcript_crop_sha256' => 'c')
  raise 'self-test accepted chrome-only up redraw' if up_frame_continuity(up_before, chrome_only, require_row_motion: true)
  raise 'self-test accepted six-row up jump' if up_frame_continuity(up_before, up_before.merge('marker_rows' => { 0 => 8, 1 => 12 }, 'transcript_crop_sha256' => 'g'))
  raise 'self-test accepted six-row down jump' if down_frame_continuity(before, before.merge('marker_rows' => { 0 => 8, 1 => 12 }, 'transcript_crop_sha256' => 'h'))
  frame = lambda do |rows|
    body = +"\e[?2026h\e[2J"
    rows.each_with_index { |text, index| body << "\e[#{index + 1};1H#{text}" }
    body << "\e[?2026l"
  end
  keyboard_after = up_before.merge('marker_rows' => { 0 => 10, 1 => 14 }, 'transcript_crop_sha256' => 'keyboard')
  raise 'self-test rejected valid eight-row keyboard movement' unless up_frame_continuity(up_before, keyboard_after, require_row_motion: true, max_rows: 8)
  raise 'self-test accepted nine-row keyboard movement' if up_frame_continuity(up_before, up_before.merge('marker_rows' => { 0 => 11, 1 => 15 }, 'transcript_crop_sha256' => 'keyboard9'), max_rows: 8)
  keyboard_ack_screen = Screen.new(12, 40)
  keyboard_ack_screen.feed(frame.call(['oldest000', 'oldest001']))
  keyboard_ack_writer = StringIO.new
  keyboard_ack_queue = [frame.call(['', '', '', '', '', '', '', 'oldest000', 'oldest001'])]
  keyboard_ack_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    value.feed(keyboard_ack_queue.shift.to_s) unless keyboard_ack_queue.empty?
    true
  end
  keyboard_ack_log = []
  keyboard_ack_time = 0.0
  keyboard_ack_before = record_frame(keyboard_ack_log, keyboard_ack_screen, 'unit-keyboard-ack')
  raise 'self-test rejected eight-row keyboard paced acknowledgement' unless paced_wheel(StringIO.new, keyboard_ack_writer, keyboard_ack_screen, TraceNull.new, 64, 1, 1, {}, frame_log: keyboard_ack_log, phase: 'unit-keyboard-ack', baseline: keyboard_ack_before, validator: lambda { |before_entry, after_entry| up_frame_continuity(before_entry, after_entry, require_row_motion: true, max_rows: 8) }, input_strategy: -> { "\x02" }, max_rows: 8, pump_fn: keyboard_ack_pump, clock: -> { keyboard_ack_time += 0.1 })
  raise 'self-test keyboard acknowledgement did not emit one Ctrl-B' unless keyboard_ack_writer.string == "\x02"
  mouse_ack_screen = Screen.new(12, 40)
  mouse_ack_screen.feed(frame.call(['oldest000', 'oldest001']))
  mouse_ack_writer = StringIO.new
  mouse_ack_queue = [frame.call(['', '', '', '', '', '', '', 'oldest000', 'oldest001'])]
  mouse_ack_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    value.feed(mouse_ack_queue.shift.to_s) unless mouse_ack_queue.empty?
    true
  end
  mouse_ack_log = []
  mouse_ack_time = 0.0
  mouse_ack_before = record_frame(mouse_ack_log, mouse_ack_screen, 'unit-mouse-ack')
  raise 'self-test accepted eight-row mouse acknowledgement' if paced_wheel(StringIO.new, mouse_ack_writer, mouse_ack_screen, TraceNull.new, 64, 1, 1, {}, frame_log: mouse_ack_log, phase: 'unit-mouse-ack', baseline: mouse_ack_before, validator: lambda { |before_entry, after_entry| up_frame_continuity(before_entry, after_entry, require_row_motion: true, max_rows: 3) }, max_rows: 3, pump_fn: mouse_ack_pump, clock: -> { mouse_ack_time += 0.1 })

  # Exercise the complete paced_wheel acknowledgement chain with a deterministic
  # pump.  These cases specifically protect against acknowledging a chrome-only
  # frame, skipping an invalid first motion frame, or dropping multiple complete
  # frames delivered by one read.
  frame = lambda do |rows|
    body = +"\e[?2026h\e[2J"
    rows.each_with_index { |text, index| body << "\e[#{index + 1};1H#{text}" }
    body << "\e[?2026l"
  end
  make_screen = lambda do
    value = Screen.new(8, 40)
    value.feed(frame.call(['oldest000', 'oldest001']))
    value
  end
  valid_up = ->(before_entry, after_entry) { up_frame_continuity(before_entry, after_entry, require_row_motion: true, max_rows: 3) }
  valid_up_continuity = ->(before_entry, after_entry) { up_frame_continuity(before_entry, after_entry) }
  chrome_then_motion = make_screen.call
  chrome_writer = StringIO.new
  chrome_queue = [frame.call(['oldest000', 'oldest001']), frame.call(['oldest000', 'oldest001']), frame.call(['oldest000', 'oldest001'])]
  # The first queued frame is a synchronized repaint with identical transcript;
  # the second moves both markers down three rows (wheel-up).
  chrome_queue[1] = frame.call(['', '', '', 'oldest000', 'oldest001'])
  chrome_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    value.feed(chrome_queue.shift.to_s) unless chrome_queue.empty?
    true
  end
  chrome_log = []
  chrome_time = 0.0
  chrome_clock = -> { chrome_time += 0.1 }
  raise 'self-test did not acknowledge after valid motion following chrome' unless paced_wheel(StringIO.new, chrome_writer, chrome_then_motion, TraceNull.new, 64, 1, 1, {}, frame_log: chrome_log, phase: 'unit-up', baseline: record_frame(chrome_log, chrome_then_motion, 'unit-up'), validator: valid_up, pump_fn: chrome_pump, clock: chrome_clock)
  raise 'self-test did not retain multiple completed frame records' unless chrome_log.length >= 3

  zero_then_motion = make_screen.call
  zero_writer = StringIO.new
  zero_queue = [frame.call(['oldest000 alpha', 'oldest001 beta']), frame.call(['oldest000 gamma', 'oldest001 delta']), frame.call(['', 'oldest000', 'oldest001'])]
  zero_fed = 0
  zero_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    unless zero_queue.empty?
      value.feed(zero_queue.shift.to_s)
      zero_fed += 1
    end
    true
  end
  zero_time = 0.0
  zero_log = []
  zero_before = record_frame(zero_log, zero_then_motion, 'unit-zero')
  raise 'self-test did not acknowledge after zero-motion intermediate' unless paced_wheel(StringIO.new, zero_writer, zero_then_motion, TraceNull.new, 64, 1, 1, {}, frame_log: zero_log, phase: 'unit-zero', baseline: zero_before, validator: valid_up_continuity, pump_fn: zero_pump, clock: -> { zero_time += 0.1 })
  raise 'self-test did not feed zero-motion frame' unless zero_fed >= 1
  raise 'self-test resent after zero-motion intermediate' unless zero_writer.string.scan("\e[<64;").length == 1

  zero_only = make_screen.call
  zero_only_writer = StringIO.new
  zero_only_queue = [frame.call(['oldest000 alpha', 'oldest001 beta'])]
  zero_only_fed = 0
  zero_only_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    unless zero_only_queue.empty?
      value.feed(zero_only_queue.shift.to_s)
      zero_only_fed += 1
    end
    true
  end
  zero_only_time = 0.0
  zero_only_clock_values = [0.0, 0.1, 0.2, 0.3, 5.1]
  zero_only_log = []
  raise 'self-test accepted zero-row-only crop change' if paced_wheel(StringIO.new, zero_only_writer, zero_only, TraceNull.new, 64, 1, 1, {}, frame_log: zero_only_log, phase: 'unit-zero-only', baseline: record_frame(zero_only_log, zero_only, 'unit-zero-only'), pump_fn: zero_only_pump, clock: -> { zero_only_clock_values.shift || 5.1 })
  raise 'self-test did not feed zero-only frame' unless zero_only_fed >= 1
  raise 'self-test resent after zero-row-only crop change' unless zero_only_writer.string.scan("\e[<64;").length == 1

  one_down_screen = Screen.new(8, 40)
  one_down_screen.feed(frame.call(['', '', 'oldest000', 'oldest001']))
  one_down_writer = StringIO.new
  one_down_queue = [frame.call(['', 'oldest000', 'oldest001'])]
  one_down_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    value.feed(one_down_queue.shift.to_s) unless one_down_queue.empty?
    true
  end
  one_down_log = []
  one_down_time = 0.0
  one_down_before = record_frame(one_down_log, one_down_screen, 'unit-one-down')
  raise 'self-test rejected full one-down call shape' unless paced_wheel(StringIO.new, one_down_writer, one_down_screen, TraceNull.new, 65, 1, 1, {}, frame_log: one_down_log, phase: 'unit-one-down', baseline: one_down_before, validator: lambda { |before_entry, after_entry| down_frame_continuity(before_entry, after_entry, require_row_motion: true, max_rows: 3) }, max_rows: 3, pump_fn: one_down_pump, clock: -> { one_down_time += 0.1 })

  invalid_first = make_screen.call
  invalid_writer = StringIO.new
  invalid_queue = [frame.call(['', '', '', '', '', '', 'oldest000', 'oldest001']) + frame.call(['', '', '', 'oldest000', 'oldest001'])]
  invalid_fed = 0
  invalid_pump = lambda do |_r, _w, _seconds, value, _raw, _counts|
    unless invalid_queue.empty?
      value.feed(invalid_queue.shift.to_s)
      invalid_fed += 1
    end
    true
  end
  invalid_log = []
  invalid_time = 0.0
  raise 'self-test accepted invalid first motion frame' if paced_wheel(StringIO.new, invalid_writer, invalid_first, TraceNull.new, 64, 1, 1, {}, frame_log: invalid_log, phase: 'unit-invalid', baseline: record_frame(invalid_log, invalid_first, 'unit-invalid'), validator: valid_up, pump_fn: invalid_pump, clock: -> { invalid_time += 0.1 })
  raise 'self-test resent after invalid first motion' unless invalid_writer.string.scan("\e[<64;").length == 1
  raise 'self-test did not feed concatenated invalid/valid read' unless invalid_fed == 1
  raise 'self-test did not record invalid frame' unless invalid_log.any? { |entry| entry['continuity_failure'] }

  timeout_writer = StringIO.new
  timeout_screen = make_screen.call
  timeout_pump = ->(*) { true }
  timeout_clock = 0.0
  timeout_log = []
  raise 'self-test did not timeout without motion' if paced_wheel(StringIO.new, timeout_writer, timeout_screen, TraceNull.new, 64, 1, 1, {}, frame_log: timeout_log, phase: 'unit-timeout', baseline: record_frame(timeout_log, timeout_screen, 'unit-timeout'), pump_fn: timeout_pump, clock: -> { timeout_clock += 6 })
  raise 'self-test resent after timeout' unless timeout_writer.string.scan("\e[<64;").length == 1

  overflow = Screen.new(4, 24)
  overflow_failed = false
  begin
    129.times { overflow.feed(frame.call(['oldest000'])) }
  rescue RuntimeError => error
    overflow_failed = error.message.include?('queue overflow')
  end
  raise 'self-test did not fail closed on completed-frame overflow' unless overflow_failed
  malformed_failed = false
  begin
    record_rows(nil, ['oldest000'], 'malformed')
  rescue ArgumentError => error
    malformed_failed = error.message.include?('requires frame_log')
  end
  raise 'self-test did not reject missing frame_log' unless malformed_failed
  puts 'observer self-test PASS'
end

def visible_indices(screen)
  screen.text.scan(/oldest\d{3,}/).filter_map do |marker|
    value = marker.delete_prefix('oldest').to_i
    value if value >= 0
  end.uniq.sort
end

def marker_positions(screen)
  marker_positions_rows(screen.rows)
end

def marker_positions_rows(rows)
  positions = {}
  rows.each_with_index do |row, index|
    row.scan(/oldest\d{3,}/).each do |marker|
      positions[marker.delete_prefix('oldest').to_i] ||= index
    end
  end
  positions
end

def consecutive_visible?(indices)
  indices.each_cons(2).all? { |a, b| b == a + 1 }
end

def down_frame_continuity(previous, current, require_row_motion: false, max_rows: 3)
  previous_indices = previous['visible_indices']
  current_indices = current['visible_indices']
  return false if previous_indices.empty? || current_indices.empty?
  return false unless consecutive_visible?(previous_indices) && consecutive_visible?(current_indices)
  return false unless current_indices.min >= previous_indices.min
  shared = previous['marker_rows'].keys & current['marker_rows'].keys
  return false if shared.empty?
  deltas = shared.map { |index| current['marker_rows'][index] - previous['marker_rows'][index] }
  return false unless deltas.uniq.one? && deltas.first.between?(-max_rows, 0)
  return false if require_row_motion && (!deltas.first.between?(-max_rows, -1) || previous['transcript_crop_sha256'] == current['transcript_crop_sha256'])
  (current_indices.max - previous_indices.max).between?(0, max_rows)
end

def up_frame_continuity(previous, current, require_row_motion: false, max_rows: 3)
  previous_indices = previous['visible_indices']
  current_indices = current['visible_indices']
  return false if previous_indices.empty? || current_indices.empty?
  return false unless consecutive_visible?(previous_indices) && consecutive_visible?(current_indices)
  return false unless current_indices.min <= previous_indices.min
  shared = previous['marker_rows'].keys & current['marker_rows'].keys
  return false if shared.empty?
  deltas = shared.map { |index| current['marker_rows'][index] - previous['marker_rows'][index] }
  return false unless deltas.uniq.one? && deltas.first.between?(0, max_rows)
  return false if require_row_motion && (!deltas.first.between?(1, max_rows) || previous['transcript_crop_sha256'] == current['transcript_crop_sha256'])
  (previous_indices.min - current_indices.min).between?(0, max_rows)
end

def record_frame(frame_log, screen, phase, trace = nil)
  record_rows(frame_log, screen.rows, phase, trace)
end

def record_rows(frame_log, rows, phase, trace = nil)
  raise ArgumentError, 'record_rows requires frame_log' unless frame_log.is_a?(Array)
  marker_rows = marker_positions_rows(rows)
  min_row = marker_rows.values.min
  max_row = marker_rows.values.max
  crop = min_row && max_row ? rows[min_row..max_row].join("\n") : ''
  entry = {
    'phase' => phase,
    'visible_indices' => marker_rows.keys.sort,
    'marker_rows' => marker_rows,
    'transcript_crop_sha256' => Digest::SHA256.hexdigest([min_row, max_row, crop].to_json),
    'transcript_crop_rows' => [min_row, max_row],
    'screen_changed' => false
  }
  previous = frame_log.last
  entry['screen_changed'] = previous.nil? || entry['transcript_crop_sha256'] != previous['transcript_crop_sha256']
  frame_log << entry
  frame_log.shift while frame_log.length > 5000
  trace.frame(entry) if trace
  entry
end

def move_until(reader, writer, screen, raw, button, x, y, counters, max_steps, phase:, frame_log:, trace:, validator: nil, input_strategy: nil, max_rows: 3)
  max_steps.times do
    return true if yield
    before = record_frame(frame_log, screen, phase, trace)
    return false unless paced_wheel(reader, writer, screen, raw, button, x, y, counters,
                                    frame_log: frame_log, phase: phase, trace: trace,
                                    baseline: before, validator: validator, input_strategy: input_strategy, max_rows: max_rows)
  end
  yield
end

if ARGV.include?('--observer-self-test')
  observer_self_test!
  exit 0
end

def stop_owned_daemon(data_dir, client_pid)
  path = File.join(data_dir, 'runtime', 'backend.json')
  return unless File.file?(path)
  descriptor = JSON.parse(File.read(path)) rescue nil
  daemon_pid = descriptor && descriptor['pid']
  return unless daemon_pid.is_a?(Integer) && daemon_pid > 1 && daemon_pid != client_pid
  Process.kill('TERM', daemon_pid)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 2
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
    begin
      Process.kill(0, daemon_pid)
    rescue Errno::ESRCH
      return
    end
    sleep 0.05
  end
  Process.kill('KILL', daemon_pid) rescue nil
rescue Errno::ESRCH
  nil
end

native_lib = ENV['DYLD_LIBRARY_PATH'].to_s
native_lib = File.expand_path('../../../target/ui-native/lib', __dir__) unless File.directory?(native_lib)
raise "native library directory missing: #{native_lib}" unless File.directory?(native_lib)
lib_files = Dir[File.join(native_lib, '*')].select { |p| File.file?(p) }.sort
lib_hashes = lib_files.to_h { |p| [File.basename(p), Digest::SHA256.file(p).hexdigest] }
  result = { 'binary' => BINARY, 'binary_sha256' => BINARY_SHA, 'native_library_dir' => native_lib, 'native_library_sha256' => lib_hashes, 'fixture_count' => MESSAGE_COUNT, 'input_mode' => SCROLL_INPUT_MODE, 'input_rows' => INPUT_ROWS, 'terminal' => { 'rows' => ROWS, 'cols' => COLS }, 'cursor_queries' => 0 }
$journey_deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + GLOBAL_DEADLINE_SECONDS
pid = reader = writer = nil
data = File.join(ROOT, 'data'); project = File.join(ROOT, 'project'); config = File.join(ROOT, 'config')
[data, project, config, File.join(ROOT, 'home'), File.join(ROOT, 'xdg-config'), File.join(ROOT, 'xdg-data'), File.join(ROOT, 'xdg-state'), File.join(ROOT, 'xdg-cache')].each { |p| FileUtils.mkdir_p(p) }
trace = Trace.new(ROOT)
raw = trace
env = { 'HOME'=>File.join(ROOT,'home'), 'XDG_CONFIG_HOME'=>File.join(ROOT,'xdg-config'), 'XDG_DATA_HOME'=>File.join(ROOT,'xdg-data'), 'XDG_STATE_HOME'=>File.join(ROOT,'xdg-state'), 'XDG_CACHE_HOME'=>File.join(ROOT,'xdg-cache'), 'OPENCODE_CONFIG_DIR'=>config, 'OPENCODE_RK_CONFIG_DIR'=>config, 'OPENCODE_DATA_DIR'=>data, 'OPENCODE_RK_HOME'=>data, 'TERM'=>'xterm-256color', 'COLUMNS'=>COLS.to_s, 'LINES'=>ROWS.to_s, 'PATH'=>'/usr/bin:/bin', 'LANG'=>'C', 'LC_ALL'=>'C', 'DYLD_LIBRARY_PATH'=>native_lib, 'LD_LIBRARY_PATH'=>native_lib }

begin
  PTY.spawn(env, BINARY, '--native', '--data-dir', data, { unsetenv_others: true, chdir: project }) do |r, w, child|
    reader, writer, pid = r, w, child; winsize(reader, ROWS, COLS); screen = Screen.new(ROWS, COLS)
    counters = result
    pump(reader, writer, 2, screen, raw, counters)
    descriptor_path = File.join(data, 'runtime', 'backend.json'); deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 20
    until File.file?(descriptor_path)
      raise 'backend descriptor missing' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      pump(reader, writer, 0.20, screen, raw, counters)
    end
    raise 'backend descriptor missing' unless File.file?(descriptor_path)
    descriptor = JSON.parse(File.read(descriptor_path)); result['daemon_origin'] = descriptor['http_origin']
    sessions = api(descriptor, 'get', '/api/sessions')[1].fetch('sessions'); raise 'no native session' if sessions.empty?
    session = sessions.first.fetch('id'); result['session_id'] = session
    MESSAGE_COUNT.times do |i|
      status, = api(descriptor, 'post', "/api/sessions/#{session}/messages", { 'text' => format('oldest%03d', i) })
      raise "append #{i} HTTP #{status}" unless status == 201
      pump(reader, writer, 0.015, screen, raw, counters) if (i + 1) % 20 == 0
    end
    listed = api(descriptor, 'get', "/api/sessions/#{session}/messages?limit=500"); raise "fixture GET HTTP #{listed[0]}" unless listed[0] == 200
    messages = all_fixture_messages(descriptor, session, MESSAGE_COUNT); texts = messages.map { |m| m.dig('body', 'text') }; result['api_message_count'] = messages.length
    raise 'API fixture is not exact' unless texts == (0...MESSAGE_COUNT).map { |i| format('oldest%03d', i) }
    result['api_message_count_exact'] = result['api_message_count'] == MESSAGE_COUNT

    # The API append is deliberately performed after the initial native client
    # has started.  Close that client normally, then resume the exact persisted
    # UUID in a fresh native client; external appends must not be mistaken for
    # an already-materialized in-memory transcript.
    writer.write("\x04"); writer.flush
    begin
      Timeout.timeout(5) { Process.wait(pid) }
    rescue StandardError
      Process.kill('TERM', pid) rescue nil
      Process.wait(pid) rescue nil
    end
    pid = reader = writer = nil

    PTY.spawn(env, BINARY, '--native', '--data-dir', data, '--session', session,
              { unsetenv_others: true, chdir: project }) do |resumed_reader, resumed_writer, resumed_child|
      reader, writer, pid = resumed_reader, resumed_writer, resumed_child
      winsize(reader, ROWS, COLS)
      screen = Screen.new(ROWS, COLS)
      pump(reader, writer, 2, screen, raw, counters)
      frame_log = []
      wait_for(reader, writer, screen, format('oldest%03d', MESSAGE_COUNT - 1), raw, 25, counters)
      result['initial_tail'] = screen.text.include?(format('oldest%03d', MESSAGE_COUNT - 1))
    # One native wheel event advances three transcript rows; this is bounded
    # but deliberately exceeds the worst-case 501-line transcript distance.
    up_limit = [MESSAGE_COUNT * 3, 3000].max
    up_key = SCROLL_INPUT_MODE == 'keyboard'
    up_input = up_key ? -> { "\x02" } : nil
    down_input = up_key ? -> { "\x06" } : nil
    up_validator = lambda { |before, after| up_frame_continuity(before, after, max_rows: INPUT_ROWS) }
    down_validator = lambda { |before, after| down_frame_continuity(before, after, max_rows: INPUT_ROWS) }
    result['oldest_reached'] = move_until(reader, writer, screen, raw, 64, 4, 4, counters, up_limit, phase: 'up', frame_log: frame_log, trace: trace, validator: up_validator, input_strategy: up_input, max_rows: INPUT_ROWS) { screen.text.include?('oldest000') }
    before_one_down = record_frame(frame_log, screen, 'one-down', trace)
    down_ack = paced_wheel(reader, writer, screen, raw, 65, 4, 4, counters,
                           frame_log: frame_log, phase: 'one-down', trace: trace,
                           baseline: before_one_down,
                           validator: lambda { |before, after| down_frame_continuity(before, after, require_row_motion: true, max_rows: INPUT_ROWS) },
                           input_strategy: down_input, max_rows: INPUT_ROWS)
    next_sequence = visible_indices(screen)
    after_one_down = frame_log.last
    result['one_down_moves_adjacent'] = down_ack && down_frame_continuity(before_one_down, after_one_down, require_row_motion: true, max_rows: INPUT_ROWS)
    down_limit = [MESSAGE_COUNT * 3, 3000].max
    result['return_to_tail'] = move_until(reader, writer, screen, raw, 65, 4, 4, counters, down_limit, phase: 'down', frame_log: frame_log, trace: trace, validator: down_validator, input_strategy: down_input, max_rows: INPUT_ROWS) { screen.text.include?(format('oldest%03d', MESSAGE_COUNT - 1)) }
    # Blank rows are legal around the composer/sidebar; the transcript viewport
    # must nevertheless contain typed content and never contain a fabricated
    # marker outside the fixture's range.
    visible_markers = screen.text.scan(/oldest\d{3,}/).uniq
    result['no_blank_rows'] = visible_markers.any?
    result['no_out_of_range'] = visible_markers.any? && visible_markers.all? { |marker| (0...MESSAGE_COUNT).cover?(marker.delete_prefix('oldest').to_i) }
    result['frame_log_entries'] = frame_log.length
    result['frame_trace_path'] = File.join(ROOT, 'frames.jsonl')
    resize_rows = 24; resize_cols = 80
    winsize(reader, resize_rows, resize_cols); screen.resize(resize_rows, resize_cols)
    resized_frame = pump(reader, writer, 1, screen, raw, counters)
    result['resize_blocked_by_return_to_tail'] = !result['return_to_tail']
    result['resize_stable'] = result['return_to_tail'] && resized_frame && screen.rows.length == resize_rows && screen.rows.all? { |row| row.length == resize_cols } && screen.text.include?(format('oldest%03d', MESSAGE_COUNT - 1))
    winsize(reader, ROWS, COLS); screen.resize(ROWS, COLS); restored_frame = pump(reader, writer, 1, screen, raw, counters)
    result['resize_restored'] = result['return_to_tail'] && restored_frame && screen.rows.length == ROWS && screen.rows.all? { |row| row.length == COLS } && screen.text.include?(format('oldest%03d', MESSAGE_COUNT - 1))
    File.write(File.join(ROOT, 'screen.txt'), screen.text)
      writer.write("\x04"); writer.flush
      begin
        Timeout.timeout(5) { Process.wait(pid) }
      rescue StandardError
        Process.kill('TERM', pid) rescue nil
        Process.wait(pid) rescue nil
      end
      pid = reader = writer = nil
    end
  end
rescue StandardError => e
  result['error'] = "#{e.class}: #{e.message}"
  result['status'] = 'INCOMPLETE'
ensure
  if pid
    Process.kill('TERM', pid) rescue nil
    Process.wait(pid) rescue nil
  end
  stop_owned_daemon(data, pid)
  result['elapsed_seconds'] = Process.clock_gettime(Process::CLOCK_MONOTONIC) - trace.started_at
  trace.frame(result.merge('phase' => 'final', 'event' => 'summary')) rescue nil
  trace.close
end

result['required_checks'] = %w[api_message_count_exact initial_tail oldest_reached one_down_moves_adjacent return_to_tail no_blank_rows no_out_of_range resize_stable resize_restored]
result['all_required_checks_passed'] = !result['error'] && result['api_message_count'] == MESSAGE_COUNT && result['required_checks'].all? { |key| result[key] == true }
result['status'] = result['all_required_checks_passed'] ? 'PASS' : (result['status'] || 'FAIL')
File.write(File.join(ROOT, 'summary.json'), JSON.pretty_generate(result) + "\n")
puts JSON.pretty_generate(result)
exit(result['all_required_checks_passed'] ? 0 : 1)
