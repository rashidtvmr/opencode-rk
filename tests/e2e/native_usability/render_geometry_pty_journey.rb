#!/usr/bin/env ruby
# frozen_string_literal: true

# Native render evidence probe.  This intentionally does not claim pixel or
# continuous-squircle geometry: a PTY gives us terminal cells only.  The probe
# retains the real ANSI stream, replays cursor motion/SGR, and verifies the
# actual RGB foreground/background cells at the observed rectangles.
require 'digest'
require 'fileutils'
require 'json'
require 'pty'
require 'timeout'

ROOT = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/render-geometry-pty-verification', __dir__)
RUN = File.join(ROOT, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY = File.expand_path(ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__)))
ROWS = Integer(ENV.fetch('PTY_ROWS', '40'), 10)
COLS = Integer(ENV.fetch('PTY_COLS', '140'), 10)
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
STYLES = {
  'rounded' => { 'preset' => 'rounded', 'overrides' => { 'dialog' => { 'preset' => 'rounded' } } },
  'double' => { 'preset' => 'sharp', 'overrides' => { 'chatcomposer' => { 'border_style' => 'double' }, 'dialog' => { 'border_style' => 'double' } } },
  'heavy' => { 'preset' => 'sharp', 'overrides' => { 'chatcomposer' => { 'border_style' => 'heavy' }, 'dialog' => { 'border_style' => 'heavy' } } }
}.freeze
FileUtils.mkdir_p(RUN)
raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && EXPECTED_SHA != BINARY_SHA

def rgb(value)
  value.is_a?(Array) && value.length == 3 && value.all? { |n| n.is_a?(Integer) && n.between?(0, 255) } ? value : nil
end

Cell = Struct.new(:char, :fg, :bg, keyword_init: true)

class AnsiGrid
  attr_reader :rows, :cols
  def initialize(rows, cols)
    @rows, @cols = rows, cols
    @cells = Array.new(rows) { Array.new(cols) { Cell.new(char: ' ', fg: nil, bg: nil) } }
    @x = @y = 0
    @fg = @bg = nil
    @state = nil
    @csi = +''
    @osc = +''
    @utf8 = +''.b
  end

  def cell(x, y)
    return nil unless x.between?(0, @cols - 1) && y.between?(0, @rows - 1)
    @cells[y][x]
  end

  def feed(bytes)
    bytes.each_byte do |byte|
      char = byte.chr
      case @state
      when :esc
        @state = char == '[' ? :csi : char == ']' ? :osc : nil
        @csi.clear if @state == :csi
        @osc.clear if @state == :osc
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
          @y = [@y + 1, @rows - 1].min
        elsif byte == 8
          @x = [@x - 1, 0].max
        elsif byte >= 32 && byte != 127
          @utf8 << char
          candidate = @utf8.dup.force_encoding(Encoding::UTF_8)
          if candidate.valid_encoding?
            put(candidate)
            @utf8.clear
          elsif @utf8.bytesize >= 4
            put("�")
            @utf8.clear
          end
        end
      end
    end
  end

  def put(char)
    return if @y >= @rows || @x >= @cols
    @cells[@y][@x] = Cell.new(char: char, fg: @fg, bg: @bg)
    @x += 1
  end

  def color_dump(path)
    File.open(path, 'w') do |io|
      io.puts "x\ty\tchar\tfg_rgb\tbg_rgb"
      @cells.each_with_index do |row, y|
        row.each_with_index do |cell, x|
          next if cell.char == ' ' && cell.fg.nil? && cell.bg.nil?
          io.puts [x, y, cell.char.inspect, cell.fg&.join(','), cell.bg&.join(',')].join("\t")
        end
      end
    end
  end

  def text
    @cells.map { |row| row.map(&:char).join }.join("\n")
  end

  private

  def apply_csi(command, body)
    values = body.sub(/\A[?>=]/, '').split(';').map { |v| v.empty? ? 1 : v.to_i }
    case command
    when 'H', 'f'
      @y = [[(values[0] || 1) - 1, 0].max, @rows - 1].min
      @x = [[(values[1] || 1) - 1, 0].max, @cols - 1].min
    when 'J'
      @cells = Array.new(@rows) { Array.new(@cols) { Cell.new(char: ' ', fg: @fg, bg: @bg) } }
    when 'K'
      (@x...@cols).each { |x| @cells[@y][x] = Cell.new(char: ' ', fg: @fg, bg: @bg) }
    when 'm'
      parse_sgr(values)
    when 's'
      @saved = [@x, @y]
    when 'u'
      @x, @y = *@saved if @saved
    when 'd'
      @y = [[(values[0] || 1) - 1, 0].max, @rows - 1].min
    end
  end

  def parse_sgr(values)
    values = [0] if values.empty?
    i = 0
    while i < values.length
      case values[i]
      when 0 then @fg = @bg = nil
      when 1, 2, 3, 4, 5, 7, 22, 23, 24, 25, 27, 39, 49 then @fg = nil if values[i] == 39; @bg = nil if values[i] == 49
      when 38, 48
        target = values[i] == 38 ? :@fg : :@bg
        if values[i + 1] == 2 && values[i + 4]
          instance_variable_set(target, values[(i + 2)..(i + 4)].map(&:to_i)); i += 4
        elsif values[i + 1] == 5
          i += 2 # indexed colors are retained as non-RGB limitation, not guessed
        end
      end
      i += 1
    end
  end
end

def drain(reader, writer, seconds)
  out = +''.b
  tail = +''.b
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([reader], nil, nil, [left, 0.1].min)
    begin
      chunk = reader.read_nonblock(65_536)
      tail = (tail + chunk).byteslice(-64, 64).to_s
      if tail.include?("\e[6n")
        writer.write("\e[1;1R"); writer.flush; tail.clear
      end
      out << chunk
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO
      break
    end
  end
  out
end

def rectangles(grid, style)
  glyphs = {
    'rounded' => ['╭', '╮', '╰', '╯', '─', '│'],
    'double' => ['╔', '╗', '╚', '╝', '═', '║'],
    'heavy' => ['┏', '┓', '┗', '┛', '━', '┃']
  }.fetch(style)
  tl, tr, bl, br, h, v = glyphs
  found = []
  grid.rows.times do |y|
    grid.cols.times do |x|
      next unless grid.cell(x, y)&.char == tl
      (x + 2...grid.cols).each do |right|
        next unless grid.cell(right, y)&.char == tr && (x + 1...right).all? { |xx| grid.cell(xx, y)&.char == h }
        (y + 2...grid.rows).each do |bottom|
          next unless grid.cell(x, bottom)&.char == bl && grid.cell(right, bottom)&.char == br
          next unless (x + 1...right).all? { |xx| grid.cell(xx, bottom)&.char == h }
          next unless (y + 1...bottom).all? { |yy| grid.cell(x, yy)&.char == v && grid.cell(right, yy)&.char == v }
          found << { x: x, y: y, right: right, bottom: bottom }
        end
      end
    end
  end
  found.uniq
end

def require_rgb_cell(grid, x, y, label)
  cell = grid.cell(x, y)
  raise "#{label} missing cell #{x},#{y}" unless cell
  raise "#{label} #{x},#{y} lacks RGB background" unless rgb(cell.bg)
  cell
end

def verify_frame(grid, layout, style, cols)
  rects = rectangles(grid, style)
  raise "#{layout}/#{style}: no actual #{style} rectangle in ANSI cells" if rects.empty?
  box = rects.max_by { |r| (r[:right] - r[:x]) * (r[:bottom] - r[:y]) }
  tl = require_rgb_cell(grid, box[:x], box[:y], 'border top-left')
  inner = require_rgb_cell(grid, box[:x] + 1, box[:y] + 1, 'border interior')
  raise "#{layout}/#{style}: border/interior fill backgrounds differ" unless tl.bg == inner.bg
  outside_xy = if box[:x] > 0
                 [box[:x] - 1, box[:y]]
               elsif box[:right] + 1 < cols
                 [box[:right] + 1, box[:y]]
               elsif box[:y] > 0
                 [box[:x], box[:y] - 1]
               else
                 nil
               end
  raise "#{layout}/#{style}: no physical outside cell available" unless outside_xy
  outside = require_rgb_cell(grid, outside_xy[0], outside_xy[1], 'outside border')
  raise "#{layout}/#{style}: interior and outside backgrounds are identical" if inner.bg == outside.bg
  evidence = { 'rectangle' => box, 'border_top_left' => tl.to_h, 'interior' => inner.to_h, 'outside' => outside.to_h }
  if layout == 'normal'
    start = cols - 42
    boundary_center = require_rgb_cell(grid, start - 1, 1, 'normal center boundary')
    boundary_sidebar = require_rgb_cell(grid, start, 1, 'normal sidebar boundary')
    sidebar_cells = []
    (start...cols).each do |x|
      (0...grid.rows).each do |y|
        cell = grid.cell(x, y)
        sidebar_cells << [x, y, cell] if cell && cell.char != ' '
      end
    end
    boundary_changed = boundary_center.bg != boundary_sidebar.bg
    actual_sidebar_text = sidebar_cells.find { |_x, _y, cell| rgb(cell.fg) && rgb(cell.bg) }
    raise "#{layout}/#{style}: no actual sidebar boundary change or RGB sidebar text/frame" unless boundary_changed || actual_sidebar_text
    evidence['sidebar_geometry'] = {
      'start_x' => start, 'width' => 42, 'outside_x' => start - 1,
      'boundary_center' => boundary_center.to_h, 'boundary_sidebar' => boundary_sidebar.to_h,
      'boundary_background_changed' => boundary_changed,
      'actual_sidebar_cell' => actual_sidebar_text && { 'x' => actual_sidebar_text[0], 'y' => actual_sidebar_text[1], 'cell' => actual_sidebar_text[2].to_h }
    }
  else
    compact_cell = require_rgb_cell(grid, 0, 1, 'compact captured geometry cell')
    evidence['compact_geometry'] = {
      'viewport_width' => cols, 'viewport_height' => grid.rows,
      'captured_cell' => { 'x' => 0, 'y' => 1, 'cell' => compact_cell.to_h }
    }
  end
  evidence
end

def wait_visible(reader, writer, grid, raw, needle, timeout = 15)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
  until grid.text.include?(needle)
    left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)
    raise "timeout waiting for #{needle.inspect}" if left <= 0
    chunk = drain(reader, writer, [left, 0.25].min)
    raw << chunk
    grid.feed(chunk)
  end
end

def open_appearance(reader, writer, grid, raw)
  writer.write('/appearance'); writer.flush
  wait_visible(reader, writer, grid, raw, 'Commands')
  writer.write("\r"); writer.flush
  # Depending on completion state, one Enter accepts the command and a second
  # submits it.  The second Enter is conditional on the visible palette.
  if grid.text.include?('Commands')
    writer.write("\r"); writer.flush
  end
  wait_visible(reader, writer, grid, raw, 'Box appearance')
end

def descriptor_pid(data)
  path = File.join(data, 'runtime', 'backend.json')
  return nil unless File.file?(path)
  value = JSON.parse(File.read(path)) rescue nil
  pid = value && value['pid']
  pid.is_a?(Integer) && pid > 1 ? pid : nil
end

def cleanup_owned_daemon(data)
  pid = descriptor_pid(data)
  return unless pid
  Process.kill('TERM', pid) rescue nil
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 3
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
    begin
      Process.kill(0, pid)
      sleep 0.05
    rescue Errno::ESRCH
      break
    rescue Errno::EPERM
      break
    end
  end
end

def run_case(style_name, cols, rows)
  dir = File.join(RUN, "#{style_name}-#{cols}x#{rows}")
  FileUtils.mkdir_p(dir)
  data = File.join(dir, 'data'); project = File.join(dir, 'project'); home = File.join(dir, 'home')
  config = File.join(dir, 'config'); xdg_config = File.join(dir, 'xdg-config')
  xdg_data = File.join(dir, 'xdg-data'); xdg_state = File.join(dir, 'xdg-state'); xdg_cache = File.join(dir, 'xdg-cache')
  [data, project, home, config, xdg_config, xdg_data, xdg_state, xdg_cache].each { |p| FileUtils.mkdir_p(p) }
  FileUtils.mkdir_p(File.join(data, 'state'))
  File.write(File.join(data, 'state', 'kv.json'), JSON.generate('box_appearance' => STYLES.fetch(style_name)))
  lib_path = ENV['DYLD_LIBRARY_PATH'].to_s
  env = {
    'HOME' => home, 'XDG_CONFIG_HOME' => xdg_config, 'XDG_DATA_HOME' => xdg_data,
    'XDG_STATE_HOME' => xdg_state, 'XDG_CACHE_HOME' => xdg_cache,
    'OPENCODE_CONFIG_DIR' => config, 'OPENCODE_RK_CONFIG_DIR' => config,
    'OPENCODE_DATA_DIR' => data, 'OPENCODE_RK_HOME' => data, 'OPENCODE_CACHE_DIR' => xdg_cache,
    'TERM' => 'xterm-256color', 'COLUMNS' => cols.to_s, 'LINES' => rows.to_s,
    'PATH' => '/usr/bin:/bin'
  }
  raise 'DYLD_LIBRARY_PATH must be explicitly supplied' if lib_path.empty?
  env['DYLD_LIBRARY_PATH'] = lib_path
  env['LD_LIBRARY_PATH'] = ENV['LD_LIBRARY_PATH'] if ENV['LD_LIBRARY_PATH']
  raw = +''.b; frame_raw = +''.b; grid = AnsiGrid.new(rows, cols); pid = nil
  begin
    PTY.spawn(env, BINARY, '--native', '--data-dir', data, { unsetenv_others: true, chdir: project }) do |reader, writer, child|
      pid = child
      begin reader.ioctl(0x80087467, [rows, cols, 0, 0].pack('S4')); rescue StandardError; end
      raw << drain(reader, writer, 2.5)
      grid.feed(raw)
      open_appearance(reader, writer, grid, raw)
      # The frame is captured before Ctrl-C/TERM allows alternate-screen
      # restoration to repaint the parser's buffer.
      frame_raw = raw.dup
      File.binwrite(File.join(dir, 'frame-before-stop.ansi'), frame_raw)
      writer.write("\x03"); writer.flush rescue nil
    end
  rescue StandardError => e
    File.write(File.join(dir, 'error.txt'), "#{e.class}: #{e.message}\n")
  ensure
    cleanup_owned_daemon(data)
    if pid
      Process.kill('TERM', pid) rescue nil
      Process.wait(pid) rescue nil
    end
  end
  File.binwrite(File.join(dir, 'frame.ansi'), frame_raw.empty? ? raw : frame_raw)
  File.write(File.join(dir, 'frame.txt'), grid.text)
  grid.color_dump(File.join(dir, 'color-cells.tsv'))
  result = { 'style' => style_name, 'cols' => cols, 'rows' => rows, 'ansi_bytes' => raw.bytesize }
  result['style_config_exact'] = JSON.parse(JSON.generate(STYLES.fetch(style_name))) == JSON.parse(File.read(File.join(data, 'state', 'kv.json'))).fetch('box_appearance')
  result['geometry'] = verify_frame(grid, cols > 120 ? 'normal' : 'compact', style_name, cols)
  result
end

summary = {
  'binary' => BINARY, 'binary_sha256' => BINARY_SHA, 'dyld_library_path' => ENV['DYLD_LIBRARY_PATH'],
  'library_digests' => (ENV['DYLD_LIBRARY_PATH'].to_s.split(':').flat_map { |p| Dir[File.join(p, 'libopentui*.dylib')] }.uniq.to_h { |p| [p, Digest::SHA256.file(p).hexdigest] }),
  'pty_cell_evidence_only' => true, 'continuous_squircle_pixels_claimed' => false, 'cases' => {}
}
raise 'binary SHA digest unexpectedly empty' if BINARY_SHA.empty?
raise 'no installed native library digests found' if summary['library_digests'].empty?
begin
  %w[rounded double heavy].each do |style|
    [ ['normal', 140, 40], ['compact', 100, 32] ].each do |layout, cols, rows|
      key = "#{style}-#{layout}"
      begin
        summary['cases'][key] = run_case(style, cols, rows)
      rescue StandardError => e
        summary['cases'][key] = { 'style' => style, 'layout' => layout, 'error' => "#{e.class}: #{e.message}" }
      end
    end
  end
rescue StandardError => e
  summary['error'] = "#{e.class}: #{e.message}"
end
summary['case_count'] = summary['cases'].length
summary['all_six_cases_present'] = summary['case_count'] == 6
summary['no_case_errors'] = summary['cases'].values.none? { |value| value['error'] }
summary['all_cases_passed'] = summary['all_six_cases_present'] && summary['no_case_errors']
File.write(File.join(RUN, 'summary.json'), JSON.pretty_generate(summary) + "\n")
puts JSON.pretty_generate(summary)
exit(summary['error'] || !summary['all_cases_passed'] ? 1 : 0)
