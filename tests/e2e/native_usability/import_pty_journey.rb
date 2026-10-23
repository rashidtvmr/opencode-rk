#!/usr/bin/env ruby
# frozen_string_literal: true
# Focused native import PTY journey.  Stdlib only; writes evidence under target/.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/import-pty-verification', __dir__)
ROOT = File.join(ARTIFACT_DIR, 'run-' + Time.now.strftime('%Y%m%d-%H%M%S') + '-' + Process.pid.to_s)
FileUtils.mkdir_p(ROOT)
SRC = File.join(ROOT, 'source-opencode')
DEST = File.join(ROOT, 'own-config')
DATA = File.join(ROOT, 'data')
HOME = File.join(ROOT, 'home')
XDG = File.join(ROOT, 'xdg')
PROJECT = File.join(ROOT, 'project')
[SRC, DEST, DATA, HOME, XDG, PROJECT].each { |p| FileUtils.mkdir_p(p) }
source = { 'model' => 'fixture/source-model', 'providers' => { 'fixture' => { 'name' => 'source-provider', 'baseURL' => 'http://127.0.0.1:1/v1' } }, 'commands' => { 'safe' => { 'template' => 'inert' } } }
source_path = File.join(SRC, 'opencode.json')
source_bytes = JSON.pretty_generate(source) + "\n"
File.binwrite(source_path, source_bytes)
source_hash = Digest::SHA256.hexdigest(source_bytes)

binary = File.expand_path(ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__)))
raise "binary missing: #{binary}" unless File.file?(binary)

def resolve_native_library(binary)
  binary_dir = File.dirname(File.expand_path(binary))
  explicit_dirs = ENV.fetch('DYLD_LIBRARY_PATH', '').split(File::PATH_SEPARATOR).reject(&:empty?)
  candidates = explicit_dirs.map do |directory|
    [File.expand_path('libopentui.dylib', directory), 'explicit_dyld_library_path']
  end
  candidates << [File.expand_path('../lib/libopentui.dylib', binary_dir), 'installed_sibling']
  candidates << [File.expand_path('../ui-native/lib/libopentui.dylib', binary_dir), 'build_relative']
  selected = candidates.find { |path, _source| File.file?(path) }
  unless selected
    checked = candidates.map(&:first).uniq.join(', ')
    raise "native library unresolved: libopentui.dylib; checked=#{checked}"
  end

  path, source = selected
  resolved = File.realpath(path)
  [path, resolved, Digest::SHA256.file(resolved).hexdigest, source]
end

native_library_file, native_library_file_resolved, native_library_file_sha256, native_library_resolution_source = resolve_native_library(binary)
native_lib = File.dirname(native_library_file_resolved)
env = {
  'HOME'=>HOME, 'XDG_CONFIG_HOME'=>XDG, 'XDG_DATA_HOME'=>File.join(XDG,'data'),
  'XDG_STATE_HOME'=>File.join(XDG,'state'), 'OPENCODE_CONFIG_DIR'=>SRC,
  'OPENCODE_RK_CONFIG_DIR'=>DEST, 'OPENCODE_DATA_DIR'=>DATA,
  'OPENCODE_RK_HOME'=>DATA, 'TERM'=>'xterm-256color', 'COLUMNS'=>'100', 'LINES'=>'32',
  'DYLD_LIBRARY_PATH'=>native_lib,
  'PATH'=>'/usr/bin:/bin'
}

def spawn(binary, env)
  PTY.spawn(env, binary, '--native', '--data-dir', DATA, { unsetenv_others: true, chdir: PROJECT }) do |r,w,p|
    $pty_writer = w
    $pty_pid = p
    begin r.ioctl(0x80087467, [32,100,0,0].pack('S4')); rescue StandardError; end
    yield r,w,p
  end
end
def read_for(io, seconds)
  out = +''; deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + seconds
  while (left = deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC)) > 0
    next unless IO.select([io], nil, nil, [left, 0.15].min)
    begin
      chunk = io.read_nonblock(65536)
      $pty_query_pending = ($pty_query_pending.to_s + chunk)[-32..]
      if $pty_query_pending.include?("\e[6n")
        $pty_writer.write("\e[1;1R"); $pty_writer.flush
        $pty_query_pending = +''
      end
      out << chunk
    rescue IO::WaitReadable
    rescue EOFError, Errno::EIO then break end
  end
  out.force_encoding('UTF-8').scrub
end
class Screen
  attr_reader :rows
  def initialize(rows=32, cols=100); @rows=Array.new(rows){' '*cols}; @x=0; @y=0; @esc=''; end
  def feed(raw)
    raw.each_byte do |b|
      c=b.chr
      if @esc.empty?
        if b==27; @esc=c; elsif b==13; @x=0; elsif b==10; @y=[@y+1,@rows.length-1].min
        elsif b>=32 && b<127; @rows[@y][@x]=c if @x<@rows[@y].length; @x=[@x+1,@rows[@y].length-1].min end
      elsif @esc=="\e" && c=='['; @esc='CSI:'
      elsif @esc.start_with?('CSI:')
        @esc = @esc.dup << c
        if c.ord.between?(0x40,0x7e)
          body=@esc[4...-1].to_s; nums=body.split(';').map{|n| n.empty? ? 1 : n.to_i}
          case c
          when 'H','f'; @y=[(nums[0]||1)-1,0].max; @x=[(nums[1]||1)-1,0].max
          when 'J'; @rows.map!{' '*@rows.first.length}; @x=@y=0
          when 'K'; @rows[@y]=@rows[@y][0...@x]+' '*(@rows[@y].length-@x)
          end
          @esc=''
        end
      else; @esc=''; end
    end
  end
  def text; @rows.join("\n"); end
  def row_containing(value); @rows.index { |row| row.include?(value) }; end
end
def wait_screen(io, screen, needle, timeout=12, raw='')
  deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+timeout; parsed=+''
  until screen.text.include?(needle)
    left=deadline-Process.clock_gettime(Process::CLOCK_MONOTONIC)
    raise "timeout stage=#{needle.inspect} screen=#{screen.text.lines.first(12).join.inspect}" if left<=0
    chunk=read_for(io,[left,0.25].min); raw << chunk; screen.feed(chunk); parsed << chunk
  end
  $last_screen_text = screen.text
  [raw, parsed]
end
def stop(pid, io, w)
  w.write("\x03"); w.flush; read_for(io, 1); Process.kill('TERM', pid) rescue nil; Process.wait(pid) rescue nil
rescue Errno::EIO, Errno::ESRCH, Errno::ECHILD
ensure
  Process.kill('KILL', pid) rescue nil
  Process.wait(pid) rescue nil
end
def record(name, raw)
  File.binwrite(File.join(ROOT, name + '.ansi'), raw)
  File.write(File.join(ROOT, name + '.txt'), raw.gsub(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]/, ''))
end
def sgr_mouse(writer, button, x, y, release = false)
  writer.write("\e[<#{button};#{x};#{y}#{release ? 'm' : 'M'}"); writer.flush
end

results = {}
raw = +''
begin
  # First offer must be real and meaningful, then Esc is a persisted decline.
  spawn(binary, env) { |r,w,p| screen=Screen.new; raw << read_for(r, 2); screen.feed(raw); raw, = wait_screen(r,screen,'Import settings from OpenCode?',12,raw); record('01-startup-offer', raw); w.write("\e"); w.flush; raw << read_for(r, 1); stop(p,r,w) }
  results['startup_offer'] = $last_screen_text.to_s.include?('Import settings from OpenCode?')
  marker = File.join(DATA, 'import-offer.json')
  results['decline_marker'] = File.file?(marker) && JSON.parse(File.read(marker))['decision'] == 'declined'

  # Restart must be silent; then Settings route (/import) explicitly reopens.
  existing = { 'model'=>'existing/wins', 'existing'=>true }; File.write(File.join(DEST,'opencode.json'), JSON.pretty_generate(existing)+"\n"); before=File.read(File.join(DEST,'opencode.json'))
  spawn(binary, env) { |r,w,p| screen=Screen.new; silent=+''; silent << read_for(r,3); screen.feed(silent); record('02-restart-no-offer',silent); results['restart_no_offer']=!screen.text.include?('Import settings from OpenCode?'); w.write("/import\r"); w.flush; raw,=wait_screen(r,screen,'Import settings from OpenCode?',12,silent); record('03-settings-reopen',raw); results['settings_reopen']=screen.text.include?('Import settings from OpenCode?');
    # Exercise non-left-click and outside-modal pointer events while the offer owns input.
    offer_row=(screen.row_containing('Import (preview first)') || 12)+1; sgr_mouse(w,2,50,offer_row); sgr_mouse(w,64,50,offer_row); sgr_mouse(w,0,1,offer_row); raw << read_for(r,1); screen.feed(raw); results['offer_pointer_inert']=File.read(File.join(DEST,'opencode.json'))==before && screen.text.include?('Import (preview first)');
    w.write("\r"); w.flush; raw,=wait_screen(r,screen,'Import preview',12,raw); results['preview_visible']=screen.text.include?('Import preview') && File.read(File.join(DEST,'opencode.json'))==before; record('04-preview',raw);
    w.write("\e"); w.flush; raw << read_for(r,1); screen.feed(raw); results['preview_escape_cancel']=!screen.text.include?('Import preview') && File.read(File.join(DEST,'opencode.json'))==before; record('04b-preview-escape',raw);
    w.write("/import\r"); w.flush; raw,=wait_screen(r,screen,'Import settings from OpenCode?',12,raw); w.write("\r"); w.flush; raw,=wait_screen(r,screen,'Import preview',12,raw); w.write("\e[B\r"); w.flush; raw << read_for(r,1); screen.feed(raw); results['preview_cancel_selected']=!screen.text.include?('Import preview') && File.read(File.join(DEST,'opencode.json'))==before; record('04c-preview-cancel-selected',raw);
     w.write("/import\r"); w.flush; raw,=wait_screen(r,screen,'Import settings from OpenCode?',12,raw); w.write("\r"); w.flush; raw,=wait_screen(r,screen,'Import preview',12,raw); w.write("\r"); w.flush; raw,=wait_screen(r,screen,'Import applied',12,raw); record('05-apply',raw); results['apply_visible']=screen.text.include?('Import applied'); results['dest_written']=File.file?(File.join(DEST,'opencode.json')); merged=JSON.parse(File.read(File.join(DEST,'opencode.json'))); results['existing_conflict_wins']=merged['model']=='existing/wins' && merged['providers'].is_a?(Hash); results['dest_changed']=before != File.read(File.join(DEST,'opencode.json')); results['source_unchanged']=Digest::SHA256.file(source_path).hexdigest==source_hash; results['marker_after_apply']=File.file?(marker) && JSON.parse(File.read(marker))['decision']=='accepted'; stop(p,r,w) }
  spawn(binary, env) { |r,w,p| screen=Screen.new; raw=read_for(r,3); screen.feed(raw); results['post_apply_restart_no_offer']=!screen.text.include?('Import settings from OpenCode?'); w.write("\x0b"); w.flush; raw,=wait_screen(r,screen,'Commands',12,raw); results['commands_visible']=screen.text.include?('Commands'); w.write('/import'); w.flush; raw,=wait_screen(r,screen,'Import settings',12,raw); results['commands_import_visible']=screen.text.include?('Import settings'); w.write("\r"); w.flush; raw,=wait_screen(r,screen,'Import settings from OpenCode?',12,raw); results['commands_activation_reopens']=screen.text.include?('Import settings from OpenCode?'); record('06-post-apply-commands-reopen',raw); w.write("\e"); w.flush; stop(p,r,w) }
rescue Exception => e
  File.binwrite(File.join(ROOT, 'failure.ansi'), ($last_wait_raw || '').b)
  File.write(File.join(ROOT, 'failure.txt'), ($last_wait_raw || '').gsub(/\e\][^\a]*(?:\a|\e\\)|\e\[[0-?]*[ -\/]*[@-~]|\e[^\[]/, ''))
  results['error'] = "#{e.class}: #{e.message}"
end
results['source_hash_before'] = source_hash
results['source_hash_after'] = Digest::SHA256.file(source_path).hexdigest
results['binary_sha256'] = Digest::SHA256.file(binary).hexdigest
results['native_library_file'] = native_library_file
results['native_library_file_resolved'] = native_library_file_resolved
results['native_library_file_sha256'] = native_library_file_sha256
results['native_library_resolution_source'] = native_library_resolution_source
results['dyld_library_path_used'] = env.fetch('DYLD_LIBRARY_PATH')
results['root'] = ROOT
File.write(File.join(ROOT,'summary.json'), JSON.pretty_generate(results))
puts JSON.pretty_generate(results)
required = %w[startup_offer decline_marker restart_no_offer settings_reopen offer_pointer_inert preview_visible preview_escape_cancel preview_cancel_selected apply_visible dest_written existing_conflict_wins source_unchanged post_apply_restart_no_offer commands_visible commands_import_visible commands_activation_reopens]
results['required_checks'] = required
results['all_required_checks_passed'] = !results['error'] && required.all? { |key| results[key] == true } && results['source_hash_before'] == results['source_hash_after']
File.write(File.join(ROOT,'summary.json'), JSON.pretty_generate(results))
exit(results['all_required_checks_passed'] ? 0 : 1)
