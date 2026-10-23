#!/usr/bin/env ruby
# frozen_string_literal: true

# Minimal real-native child roster journey.  This intentionally uses only the
# Ruby standard library: the provider is a loopback fixture, while the child,
# task tool, persistence, and native roster are all supplied by oc2.
require 'pty'
require 'fileutils'
require 'json'
require 'digest'
require 'net/http'
require 'uri'

ARTIFACT_DIR = File.expand_path('../../../target/phase1-convergence-20261021/native-usability/subagent-pty-verification', __dir__)
ROOT = File.join(ARTIFACT_DIR, "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
BINARY = ARGV.fetch(0, File.expand_path('../../../target/debug/oc2', __dir__))
EXPECTED_SHA = ENV['EXPECTED_BINARY_SHA256']
FileUtils.mkdir_p(ROOT)
%w[home config data project xdg-config xdg-data xdg-state xdg-cache].each { |n| FileUtils.mkdir_p(File.join(ROOT, n)) }

raise "binary missing: #{BINARY}" unless File.file?(BINARY)
BINARY_SHA = Digest::SHA256.file(BINARY).hexdigest
raise "binary SHA mismatch expected=#{EXPECTED_SHA} actual=#{BINARY_SHA}" if EXPECTED_SHA && EXPECTED_SHA != BINARY_SHA

HOME_DIR = File.join(ROOT, 'home')
CONFIG_DIR = File.join(ROOT, 'config')
DATA_DIR = File.join(ROOT, 'data')
PROJECT_DIR = File.join(ROOT, 'project')
CUSTOM = 'native child explicit follow-up'
CHILD_PROMPT = 'NATIVE_CHILD_INSPECT read-only child inspection'
NATIVE_LIBRARY = begin
  explicit = ENV['DYLD_LIBRARY_PATH'].to_s.split(':').find { |path| File.directory?(path) }
  fallback = File.expand_path('../../../target/ui-native/lib', __dir__)
  explicit || (File.directory?(fallback) ? fallback : nil)
end
NATIVE_LIBRARY_FILE = NATIVE_LIBRARY && Dir[File.join(NATIVE_LIBRARY, 'libopentui*')].find { |path| File.file?(path) }

class FakeProvider
  attr_reader :port
  def initialize
    @server = TCPServer.new('127.0.0.1', 0)
    @port = @server.addr[1]
    @requests = []; @errors = []; @lock = Mutex.new; @closed = false
    @thread = Thread.new { accept_loop }
  end
  def requests; @lock.synchronize { @requests.map(&:dup) }; end
  def errors; @lock.synchronize { @errors.dup }; end
  def close
    @closed = true; @server.close rescue nil
    @thread.join(2); @thread.kill if @thread.alive?
  end
  private
  def accept_loop
    until @closed
      begin
        ready = IO.select([@server], nil, nil, 0.15); next unless ready
        socket = @server.accept; Thread.new(socket) { |s| serve(s) }
      rescue IOError, Errno::EBADF
        break if @closed
      rescue StandardError => e
        @lock.synchronize { @errors << "accept #{e.class}: #{e.message}" }
      end
    end
  end
  def serve(socket)
    request = read_request(socket); return unless request
    method, path, headers, body = request
    parsed = JSON.parse(body) rescue nil
    @lock.synchronize { @requests << { 'method'=>method, 'path'=>path, 'json'=>parsed } }
    if method == 'GET' && path == '/v1/models'
      respond(socket, 200, 'application/json', JSON.generate('data'=>[{'id'=>'fixture-model','name'=>'Native child fixture'}]))
    elsif method == 'POST' && path == '/v1/responses'
      respond(socket, 200, 'text/event-stream', response_for(parsed))
    else
      respond(socket, 404, 'application/json', '{"error":"not found"}')
    end
  rescue StandardError => e
    @lock.synchronize { @errors << "request #{e.class}: #{e.message}" }
  ensure
    socket.close rescue nil
  end
  def response_for(body)
    items = body.fetch('input')
    user = items.reverse.find { |x| x['role'] == 'user' }
    text = user && user['content'].to_s
    child_prompt = items.any? { |item| item['role'] == 'user' && item['content'].to_s.include?('NATIVE_CHILD_INSPECT') }
    custom_prompt = items.any? { |item| item['role'] == 'user' && item['content'].to_s.include?(CUSTOM) }
    output_done = items.drop_while { |item| item != user }.drop(1).any? { |item| item['type'] == 'function_call_output' }
    if child_prompt
      delta = custom_prompt ? 'child follow-up complete' : 'child inspection complete'
    elsif output_done
      delta = 'parent delegation complete'
    else
      # The first parent round must be a real task call with the production
      # task schema; no child/session is fabricated by this fixture.
      delta = nil
    end
    event = if delta
      {'type'=>'response.output_text.delta', 'delta'=>delta}
    else
      {'type'=>'response.output_item.done', 'item'=>{'type'=>'function_call', 'id'=>'native_fc_task', 'call_id'=>'native_parent_task', 'name'=>'task', 'arguments'=>JSON.generate('description'=>'native child inspect', 'prompt'=>CHILD_PROMPT, 'subagent_type'=>'general', 'background'=>true)}}
    end
    "data: #{JSON.generate(event)}\n\ndata: #{JSON.generate('type'=>'response.completed', 'response'=>{'status'=>'completed'})}\n\n"
  end
  def read_request(socket)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5; bytes = ''.b; end_at = nil
    until end_at
      return nil unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096); raise 'headers too large' if bytes.bytesize > 16 * 1024
      end_at = bytes.index("\r\n\r\n")
    end
    head = bytes.byteslice(0, end_at).force_encoding('UTF-8').scrub; lines = head.split("\r\n")
    method, path = lines.shift.to_s.split(' ', 3); headers = {}
    lines.each { |line| k, v = line.split(':', 2); headers[k.to_s.downcase] = v.to_s.strip }
    length = Integer(headers.fetch('content-length', '0')); raise 'body too large' if length > 1024 * 1024
    start = end_at + 4
    while bytes.bytesize < start + length
      return nil unless IO.select([socket], nil, nil, [deadline - Process.clock_gettime(Process::CLOCK_MONOTONIC), 0].max)
      bytes << socket.readpartial(4096)
    end
    [method, path, headers, bytes.byteslice(start, length).to_s]
  rescue EOFError
    nil
  end
  def respond(socket, status, type, body)
    bytes = body.b; reason = status == 200 ? 'OK' : 'Not Found'
    socket.write("HTTP/1.1 #{status} #{reason}\r\nContent-Type: #{type}\r\nContent-Length: #{bytes.bytesize}\r\nConnection: close\r\n\r\n#{bytes}")
  end
end

class Screen
  attr_reader :rows
  def initialize(rows=40, cols=120); @rows=Array.new(rows){' '*cols}; @x=0; @y=0; @esc=''; end
  def feed(raw)
    raw.each_byte do |b|
      c=b.chr
      if @esc.empty?
        if b==27; @esc=c elsif b==13; @x=0 elsif b==10; @y=[@y+1,@rows.length-1].min
        elsif b>=32 && b<127; @rows[@y][@x]=c if @x<@rows[@y].length; @x=[@x+1,@rows[@y].length-1].min end
      elsif @esc=="\e" && c=='['; @esc='CSI:'
      elsif @esc.start_with?('CSI:')
        @esc=@esc.dup<<c
        if c.ord.between?(0x40,0x7e)
          n=@esc[4...-1].to_s.split(';').map{|v| v.empty? ? 1 : v.to_i}
          case c; when 'H','f'; @y=[(n[0]||1)-1,0].max.clamp(0,@rows.length-1); @x=[(n[1]||1)-1,0].max.clamp(0,@rows.first.length-1); when 'J'; @rows.map!{' '*@rows.first.length}; @x=@y=0; when 'K'; @rows[@y]=@rows[@y][0...@x]+' '*(@rows[@y].length-@x); end
          @esc=''
        end
      else; @esc=''; end
    end
  end
  def text; @rows.join("\n"); end
  def row_containing(s); @rows.index { |r| r.include?(s) }; end
end

def read_for(io, writer, seconds)
  out=+''; query=+''; deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+seconds
  while (left=deadline-Process.clock_gettime(Process::CLOCK_MONOTONIC))>0
    next unless IO.select([io],nil,nil,[left,0.15].min)
    begin
      chunk=io.read_nonblock(65_536); query=(query+chunk)[-32..].to_s
      if query.include?("\e[6n"); writer.write("\e[1;1R"); writer.flush; query=+''; end
      out<<chunk
    rescue IO::WaitReadable; rescue EOFError, Errno::EIO; break; end
  end
  out.force_encoding('UTF-8').scrub
end
def wait_for(io, writer, screen, needle, timeout=15, raw=+'')
  deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+timeout
  until screen.text.include?(needle)
    raise "timeout waiting for #{needle.inspect}" if Process.clock_gettime(Process::CLOCK_MONOTONIC)>=deadline
    chunk=read_for(io,writer,0.25); raw<<chunk; screen.feed(chunk)
  end
  raw
end
def mouse(writer, button, x, y, release=false); writer.write("\e[<#{button};#{x};#{y}#{release ? 'm':'M'}"); writer.flush; end
def api(origin, token, method, path, body=nil)
  uri=URI("#{origin}#{path}"); req=Net::HTTP.const_get(method.capitalize).new(uri); req['Authorization']="Bearer #{token}"; req['Content-Type']='application/json'; req.body=JSON.generate(body) if body
  Net::HTTP.start(uri.host,uri.port,read_timeout:3) do |h|
    r=h.request(req)
    parsed = begin JSON.parse(r.body) rescue {} end
    [r.code.to_i, parsed]
  end
end
def eventually(origin, token, path, timeout=20, pump=nil)
  deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+timeout; last=nil
  loop do
    status,last=api(origin,token,'get',path); return last if yield(last)
    pump.call if pump
    raise "timeout polling #{path}: #{last.inspect}" if Process.clock_gettime(Process::CLOCK_MONOTONIC)>=deadline
    sleep 0.15
  end
end

results={'binary_sha256'=>BINARY_SHA,'custom_prompt'=>CUSTOM}
results['resolved_library_path']=NATIVE_LIBRARY
results['native_library_file']=NATIVE_LIBRARY_FILE
results['native_library_sha256']=NATIVE_LIBRARY_FILE && Digest::SHA256.file(NATIVE_LIBRARY_FILE).hexdigest
screen=Screen.new; raw=+''
provider=nil; child_id=nil; daemon_pid=nil; pty_reader=nil; pty_writer=nil
begin
  provider=FakeProvider.new
  File.write(File.join(CONFIG_DIR,'opencode.json'), JSON.pretty_generate('model'=>'acme/fixture-model','providers'=>{'acme'=>{'name'=>'Acme native child','base_url'=>"http://127.0.0.1:#{provider.port}/v1"}})+"\n")
   raise 'native library fixture not found' unless NATIVE_LIBRARY_FILE && results['native_library_sha256'].to_s.match?(/\A[0-9a-f]{64}\z/)
   env={'HOME'=>HOME_DIR,'XDG_CONFIG_HOME'=>File.join(ROOT,'xdg-config'),'XDG_DATA_HOME'=>File.join(ROOT,'xdg-data'),'XDG_STATE_HOME'=>File.join(ROOT,'xdg-state'),'XDG_CACHE_HOME'=>File.join(ROOT,'xdg-cache'),'OPENCODE_RK_CONFIG_DIR'=>CONFIG_DIR,'OPENCODE_CONFIG_DIR'=>File.join(ROOT,'xdg-config'),'OPENCODE_DATA_DIR'=>DATA_DIR,'OPENCODE_RK_HOME'=>DATA_DIR,'OPENCODE_RK_TURN_TOOLS'=>'task,read','TERM'=>'xterm-256color','COLUMNS'=>'120','LINES'=>'40','PATH'=>'/usr/bin:/bin','DYLD_LIBRARY_PATH'=>NATIVE_LIBRARY}
   env['OPENCODE_RK_TUI_SUBMIT_KEY']='enter'
   PTY.spawn(env,BINARY,'--native','--data-dir',DATA_DIR,{unsetenv_others:true,chdir:PROJECT_DIR}) do |r,w,p|
     pty_reader=r; pty_writer=w; daemon_pid=p
     begin r.ioctl(0x80087467, [40,120,0,0].pack('S4')); rescue StandardError; end
     chunk=read_for(r,w,2); raw << chunk; screen.feed(chunk)
    wait_for(r,w,screen,'Ask anything',15,raw)
    # Native startup may restore an example draft; clear the composer before
    # entering the actual parent request so the provider sees the exact text.
    # Erase the restored example draft using the same ordinary edit path as a
    # user (the native composer intentionally does not treat Ctrl-U specially).
    w.write(("\x7f" * 40) + "delegate native child"); w.flush; sleep 0.4; w.write("\r"); w.flush
    # Task permission is approved through the real daemon API, not by faking a child.
    descriptor=File.join(DATA_DIR,'runtime','backend.json'); deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+15
    until File.file?(descriptor) || Process.clock_gettime(Process::CLOCK_MONOTONIC)>=deadline; sleep 0.1; end
    desc=JSON.parse(File.read(descriptor)); origin=desc['http_origin']; token=desc['auth_token']; results['daemon_origin']=origin
    # backend.json deliberately carries daemon identity, not a session id.
    # Resolve the actual native session through the public API.
    pump = -> { chunk=read_for(r,w,0.12); raw << chunk; screen.feed(chunk) }
    sessions=eventually(origin,token,'/api/sessions',20,pump) { |v| v['sessions'].is_a?(Array) && !v['sessions'].empty? }
    parent_id=sessions['sessions'][0]['id']; results['parent_session_id']=parent_id
    permission=eventually(origin,token,"/api/session/#{parent_id}/permission",20,pump) { |v| v['data'].is_a?(Array) && !v['data'].empty? }
    status,_=api(origin,token,'post',"/api/session/#{parent_id}/permission/#{permission['data'][0]['id']}/reply",'reply'=>'once'); raise "permission reply #{status}" unless status==204
    children=eventually(origin,token,"/api/sessions/#{parent_id}/children",20,pump) { |v| v['children'].is_a?(Array) && !v['children'].empty? && v['children'][0]['state']=='completed' }
    child=children['children'][0]; child_id=child['session_id']; results['child_session_id_api']=child_id; results['child_initial_state']=child['state']; results['child_initial_generation']=child['generation']
    # The API poll above proves completion; drain the already-rendered native
    # frame before opening the roster rather than waiting on a transcript phrase
    # that may be below the current chat viewport.
     chunk=read_for(r,w,2); raw << chunk; screen.feed(chunk)
     w.write("/subagents"); w.flush; sleep 0.5; w.write("\r"); w.flush
     sleep 0.5; chunk=read_for(r,w,1); raw << chunk; screen.feed(chunk)
     # A freshly painted frame can race the command decoder; retry the same
     # explicit native command only while the visible draft still owns focus.
     unless screen.text.include?('Subagents')
       w.write("\r"); w.flush; sleep 0.5
       chunk=read_for(r,w,1); raw << chunk; screen.feed(chunk)
     end
     wait_for(r,w,screen,'Subagents',15,raw)
     row=screen.row_containing('native child inspect'); raise 'child roster row not visible' unless row
    modal_x=screen.rows[screen.row_containing('Subagents')].index('Subagents') || 0
    child_col=screen.rows[row].index('general') || 0
    # The roster's rendered row is already in terminal coordinates; the panel
    # translates it internally. Click the visible row, then use Enter for the
    # same detail transition as a second click. Wait on the detail-only footer,
    # not on transcript text that may also exist in the parent chat.
    mouse(w,0,child_col+1,row+1); sleep 0.4
    w.write("\r"); w.flush
    # A detail render has a stable metadata line and the footer. Refresh once
    # after the click so the footer predicate cannot observe a stale list frame.
    chunk=read_for(r,w,1); raw << chunk; screen.feed(chunk)
    wait_for(r,w,screen,'c follow-up',15,raw)
    footer=screen.row_containing('c follow-up'); raise 'follow-up footer not visible' unless footer
    follow_col=screen.rows[footer].index('c follow-up') || 0
     # The footer coordinates are absolute terminal coordinates: `Screen#rows`
     # already contains the modal's rendered x/y origin.  Wait for the empty
     # focused composer before writing, otherwise the first character can be
     # consumed by the mouse/paint transition.
     mouse(w,0,follow_col+1,footer+1); w.flush
     chunk=read_for(r,w,1); raw << chunk; screen.feed(chunk)
     raise 'follow-up composer did not open' unless screen.text.include?('Follow-up:')
     w.write(CUSTOM); w.flush
     chunk=read_for(r,w,1); raw << chunk; screen.feed(chunk)
     results['followup_composer_exact']=screen.text.include?("Follow-up: #{CUSTOM}")
     send_row=screen.row_containing('Enter send'); raise 'send footer not visible' unless send_row
     send_col=screen.rows[send_row].index('Enter send') || 0
     mouse(w,0,send_col+1,send_row+1); w.flush
    follow=eventually(origin,token,"/api/sessions/#{parent_id}/children",20,pump) { |v| v['children'].is_a?(Array) && v['children'][0]['generation']==2 && v['children'][0]['state']=='completed' }
    results['child_followup_state']=follow['children'][0]['state']; results['child_followup_generation']=follow['children'][0]['generation']; results['child_transcript_visible']=screen.text.include?('child inspection complete')
    turns=provider.requests.select{|x| x['path']=='/v1/responses'}
    latest_user = ->(x) { x.dig('json','input').to_a.reverse.find{|item| item['role']=='user'}&.dig('content') }
     parent_turn=turns.find{|x| latest_user.call(x)=='delegate native child' && x.dig('json','input').to_a.any?{|item| item['type']=='function_call_output' && item['call_id']=='native_parent_task'}}
    child_turns=turns.select{|x| x.dig('json','input').to_a.any?{|item| item['role']=='system' && item['content'].to_s.start_with?('Agent: general')} && x.dig('json','input').to_a.any?{|item| item['role']=='user' && item['content'].to_s.include?('NATIVE_CHILD_INSPECT')}}
    results['parent_task_request_exact']=!!parent_turn
     results['parent_task_arguments_exact']=begin
       call=parent_turn && parent_turn.dig('json','input').find{|item| item['type']=='function_call' && item['call_id']=='native_parent_task'}
       output=parent_turn && parent_turn.dig('json','input').find{|item| item['type']=='function_call_output' && item['call_id']=='native_parent_task'}
       args=call && JSON.parse(call.fetch('arguments'))
       output && args == {'description'=>'native child inspect','prompt'=>CHILD_PROMPT,'subagent_type'=>'general','background'=>true} && output['output'].to_s.include?(child_id)
    rescue StandardError
      false
    end
     results['provider_child_requests']=child_turns.length
     results['child_identity_correlated']=child_turns.length==2 && child_turns.all?{|x| x.dig('json','model')=='fixture-model' && x.dig('json','input').to_a.any?{|item| item['role']=='system' && item['content'].to_s.include?('Agent: general')}}
     results['child_custom_latest_user']=child_turns.any?{|x| latest_user.call(x)==CUSTOM}
     results['followup_prompt_carried']=results['child_custom_latest_user'] == true
    results['followup_provider_prompt_bytes']=child_turns.map{|x| x.dig('json','input').to_a.select{|item| item['role']=='user'}.map{|item| item['content']}}
    File.write(File.join(ROOT,'native-session.txt'),screen.text)
  end
rescue StandardError => e
  results['error']="#{e.class}: #{e.message}"
  results['provider_requests']=provider ? provider.requests : []
  File.write(File.join(ROOT,'failure-screen.txt'), screen.text)
  File.binwrite(File.join(ROOT,'failure-raw.ansi'), raw)
ensure
  if daemon_pid && pty_writer
    pty_writer.write("\x03") rescue nil; pty_writer.flush rescue nil
    Process.kill('TERM',daemon_pid) rescue nil; Process.wait(daemon_pid) rescue nil
  end
  provider.close if provider
end
results['provider_errors']=provider ? provider.errors : []
results['provider_request_count']=provider ? provider.requests.length : 0
results['provider_requests']=provider ? provider.requests : []
 results['required_checks']=%w[native_library_pair_bound parent_task_request_exact parent_task_arguments_exact provider_child_requests child_session_id_api child_initial_state child_initial_generation child_transcript_visible child_followup_state child_followup_generation followup_composer_exact child_identity_correlated child_custom_latest_user followup_prompt_carried]
 results['assertions']={
   'native_library_pair_bound'=>NATIVE_LIBRARY_FILE && results['resolved_library_path']==NATIVE_LIBRARY && results['native_library_sha256'].to_s.match?(/\A[0-9a-f]{64}\z/),
  'parent_task_request_exact'=>results['parent_task_request_exact'] == true,
  'parent_task_arguments_exact'=>results['parent_task_arguments_exact'] == true,
  'provider_child_requests'=>results['provider_child_requests'] == 2,
  'child_session_id_api'=>results['child_session_id_api'].is_a?(String) && !!(results['child_session_id_api'] =~ /\A[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\z/),
  'child_initial_state'=>results['child_initial_state'] == 'completed',
  'child_initial_generation'=>results['child_initial_generation'] == 1,
  'child_transcript_visible'=>results['child_transcript_visible'] == true,
  'child_followup_state'=>results['child_followup_state'] == 'completed',
  'child_followup_generation'=>results['child_followup_generation'] == 2,
  'followup_composer_exact'=>results['followup_composer_exact'] == true,
   'followup_prompt_carried'=>results['followup_prompt_carried'] == true,
   'child_identity_correlated'=>results['child_identity_correlated'] == true,
   'child_custom_latest_user'=>results['child_custom_latest_user'] == true
}
results['all_required_checks_passed']=results['error'].nil? && results['assertions'].values.all?
results['artifact_dir']=ROOT
File.write(File.join(ROOT,'summary.json'),JSON.pretty_generate(results)+"\n")
puts JSON.pretty_generate(results)
exit(results['all_required_checks_passed'] ? 0 : 1)
