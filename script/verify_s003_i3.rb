#!/usr/bin/env ruby
# Real-file lifecycle evidence for an explicitly isolated, synthetic native S003 run.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'open3'
root = File.expand_path('..', __dir__)
phase = ARGV.fetch(0, 'seed')
port = Integer(ARGV.fetch(1, '17995'))
out = File.join(root, 'dist/s003-i3')
FileUtils.mkdir_p(out)
manifest_file = File.join(out, 'manifest.json')
c = CanvasControl.new(port)
def await_saved(c)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
  loop do
    return unless c.request('document.inspect', {depth:0}).fetch('dirty')
    raise 'automatic save did not finish' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    sleep 0.1
  end
end
def disk_info(root, path)
  output, error, status = Open3.capture3(File.join(root, 'target/debug/figcraft-cli'), 'info', '--json', path)
  raise error unless status.success?
  JSON.parse(output)
end
begin
  m = File.exist?(manifest_file) ? JSON.parse(File.read(manifest_file)) : {}
  case phase
  when 'seed'
    files = c.run('draft.list').fetch('files')
    raise 'click New once in an empty isolated workspace first' unless files.size == 1
    m = files.first.merge('config'=>File.dirname(File.dirname(files.first.fetch('path'))))
    raise 'requires the named disposable I3 configuration' unless File.basename(m.fetch('config')).start_with?('kodma-s003-i3-')
    m['uid'] = c.request('document.inspect', {depth:0}).fetch('id')
    m['node'] = c.run('shape.rectangle', {name:'I3 · 受管矩形',x:120,y:160,width:160,height:120,props:{fill:'#61B8F6'}}).fetch('id')
    m['text'] = c.run('text.create', {text:'本地保存 · 合成测试',x:120,y:350,props:{fontSize:28,fontFamily:'Helvetica'}}).fetch('id')
    m['gradient'] = c.run('shape.rectangle', {name:'I3 · 保留渐变',x:360,y:160,width:120,height:120,props:{fills:[{type:'linearGradient',stops:['#61B8F6','#99D9BD']}]}}).fetch('id')
    await_saved(c)
    m['disk'] = disk_info(root, m.fetch('path'))
    m['checks'] = ['native New created a real managed file', 'automatic save completed without enabling global auto-save', 'CLI independently read the saved file']
  when 'restart'
    files = c.run('draft.list').fetch('files')
    entry = files.find{|f| f['id'] == m.fetch('id')} or raise 'stable draft identity missing after restart'
    raise 'name or path changed' unless entry['path'] == m['path'] && entry['name'] == m['name']
    c.run('file.open', {path:entry.fetch('path')})
    node = c.request('node.get', {id:m.fetch('node')})
    raise 'content changed after restart' unless node['x'] == m.fetch('expected_x', 120) && node['width'] == 160
    raise 'text missing' unless c.request('node.get',{id:m.fetch('text')})['characters'] == '本地保存 · 合成测试'
    gradient = c.request('node.get',{id:m.fetch('gradient')})
    raise 'gradient was lost' unless gradient.fetch('fills').any?{|f| f['type'] == 'gradient' && f['kind'] == 'linear' && f.fetch('stops').size == 2}
    m['disk_after_restart'] = disk_info(root, m.fetch('path'))
    m['checks'] << 'normal process restart found the same id, name and saved content'
  when 'rename-record'
    entry = c.run('draft.list').fetch('files').find{|f| f['id'] == m.fetch('id')} or raise 'missing draft'
    raise 'native rename did not persist' if entry['name'] == '未命名稿件'
    m['name'] = entry.fetch('name')
    m['checks'] << 'native rename persisted without changing the file identity/path'
  when 'save-as'
    c.run('node.set', {ids:[m.fetch('node')],props:{x:210}})
    m['external'] = File.join(out, 'external-copy.figcraft')
    c.run('file.saveAs', {path:m.fetch('external')})
    c.run('file.rename',{name:'外部副本'})
    c.run('node.set',{ids:[m.fetch('node')],props:{x:400}})
    c.run('file.save')
    c.run('file.open',{path:m.fetch('path')})
    raise 'original and external copy share content' unless c.request('node.get',{id:m.fetch('node')})['x'] == 210
    raise 'original and external copy share runtime identity' if c.request('document.inspect',{depth:0})['id'] == m['uid']
    m['expected_x'] = 210
    m['checks'] << 'Save As first saved the original; external copy and original reopen independently'
  when 'quit'
    c.run('file.open',{path:m.fetch('path')})
    c.run('node.set',{ids:[m.fetch('node')],props:{x:260}})
    c.request('app.quit')
    m['expected_x'] = 260
    m['checks'] << 'normal Quit flushed the last dirty edit'
  when 'initial-fault'
    dir = File.join(m.fetch('config'),'Drafts')
    backup = dir + '.i3-backup'
    raise 'fault injection requires disposable I3 configuration' unless File.basename(m.fetch('config')).start_with?('kodma-s003-i3-') && !File.exist?(backup)
    before = c.run('draft.list').fetch('files').size
    File.rename(dir, backup)
    begin
      File.write(dir,'temporary synthetic creation fault')
      begin
        c.run('draft.create',{id:'44444444444444444444444444444444',name:'重试保留稿'})
        raise 'create unexpectedly succeeded'
      rescue RuntimeError => e
        raise unless e.message.include?('engine.execute:')
      end
      pending = c.request('document.inspect',{depth:0})
      raise 'initial failure published a ghost entry' unless pending.fetch('dirty') && c.run('draft.list').fetch('files').size == before
      c.run('shape.rectangle',{width:80,height:90})
    ensure
      File.delete(dir) if File.file?(dir)
      File.rename(backup, dir)
    end
    c.run('draft.save')
    raise 'retry changed runtime identity' unless c.request('document.inspect',{depth:0})['id'] == pending['id']
    raise 'retry created duplicate entries' unless c.run('draft.list').fetch('files').size == before + 1
    m['checks'] << 'real initial creation failure retained edits and retry published exactly one draft'
  when 'fault'
    dir = File.join(m.fetch('config'),'Drafts')
    backup = dir + '.i3-backup'
    raise 'fault injection requires disposable I3 configuration' unless File.basename(m.fetch('config')).start_with?('kodma-s003-i3-') && !File.exist?(backup)
    c.run('file.open',{path:m.fetch('path')})
    File.rename(dir, backup)
    begin
      File.write(dir,'temporary synthetic storage fault')
      c.run('node.set',{ids:[m.fetch('node')],props:{x:230}})
      begin
        c.run('draft.save')
        raise 'save unexpectedly succeeded'
      rescue RuntimeError => e
        raise unless e.message.include?('engine.execute:')
      end
      raise 'save failure cleared dirty' unless c.request('document.inspect',{depth:0}).fetch('dirty')
      begin
        c.request('app.quit')
        raise 'Quit unexpectedly bypassed the failed save'
      rescue RuntimeError => e
        raise unless e.message.include?('app.quit:')
      end
      sleep 0.3
      raise 'failed exit lost editable draft' unless c.request('document.inspect',{depth:0}).fetch('dirty')
    ensure
      File.delete(dir) if File.file?(dir)
      File.rename(backup, dir)
    end
    c.run('draft.save')
    m['expected_x'] = 230
    m['checks'] << 'real write fault retained dirty/editable state, cancelled Quit, and saved after retry'
  else
    raise "unknown phase #{phase}"
  end
  File.write(manifest_file+'.tmp', JSON.pretty_generate(m))
  File.rename(manifest_file+'.tmp',manifest_file)
  puts JSON.pretty_generate(m)
ensure
  c.close
end
