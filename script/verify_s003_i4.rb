#!/usr/bin/env ruby
# Synthetic real-file evidence. All fault mutations are restricted to the named disposable root.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'digest'
root = File.expand_path('..', __dir__)
out = File.join(root, 'dist/s003-i4')
FileUtils.mkdir_p(out)
manifest = File.join(out, 'manifest.json')
phase = ARGV.fetch(0, 'seed')
c = CanvasControl.new(Integer(ARGV.fetch(1, '17996')))
m = File.exist?(manifest) ? JSON.parse(File.read(manifest)) : {}
def failure
  yield
  raise 'expected real error'
rescue RuntimeError => e
  raise unless e.message.start_with?('engine.execute:')
  e.message
end
begin
  case phase
  when 'seed'
    raise 'use an empty isolated workspace' unless c.run('draft.list').fetch('files').empty?
    names = (1..26).map{|n| "本地稿件 #{format('%02d',n)}"}
    names[0]='阅读记录 · 最早稿件'
    names[1]='流程草稿'
    names[2]=names[3]='同名稿件'
    names[24]='Visual App · English'
    names[25]='长名称 · 阅读与设计记录 · 超过二十份稿件之后仍然可以查找和打开'
    m={'files'=>[], 'checks'=>[]}
    names.each_with_index do |name,i|
      result=c.run('draft.create',{id:format('%032x',i+1),name:name})
      path=result.fetch('path')
      config=File.dirname(File.dirname(path))
      raise 'requires disposable I4 configuration' unless File.basename(config).start_with?('kodma-s003-i4-')
      m['config']=config
      node=c.run('shape.rectangle',{x:60+i*4,y:100,width:120+i,height:80,props:{fill:i.even? ? '#61B8F6' : '#99D9BD'}}).fetch('id')
      c.run('draft.save')
      m['files'] << result.merge('name'=>name,'node'=>node)
      c.run('file.close')
    end
    m['external']=File.join(out,'external-review.figcraft')
    c.run('file.new',{name:'外部文件'})
    c.run('file.saveAs',{path:m['external']})
    m['external_hash']=Digest::SHA256.file(m['external']).hexdigest
    c.run('file.close')
    m['checks'] << '26 real managed files created through commands, independent shapes, external file remains external'
    c.run('doc.home')
  when 'restart'
    files=c.run('draft.list').fetch('files')
    raise 'managed list was truncated' unless files.size==26
    m.fetch('files').each do |f|
      raise 'identity/name lost' unless files.any?{|actual| actual['id']==f['id'] && actual['name']==f['name'] && actual['path']==f['path']}
    end
    oldest=m.fetch('files').first
    c.run('file.open',{path:oldest['path']})
    raise 'oldest content lost' unless c.request('node.get',{id:oldest['node']})['width']==120
    c.run('doc.home')
    m['checks'] << 'normal restart recovered all 26 identities/names including oldest content'
  when 'faults'
    target=m.fetch('files')[4]
    path=target.fetch('path'); backup=path+'.i4-held'
    raise 'disposable root required' unless File.basename(m.fetch('config')).start_with?('kodma-s003-i4-') && path.start_with?(m['config']+'/Drafts/') && !File.exist?(backup)
    c.run('file.open',{path:path})
    before=c.run('draft.list').fetch('files')
    File.rename(path,backup)
    begin
      m['trash_error']=failure{c.run('draft.trash',{id:target['id'],confirmed:true})}
      raise 'failed trash removed a record' unless c.run('draft.list').fetch('files')==before
      raise 'failed trash closed target' unless c.request('document.inspect',{depth:0})['path']==path
    ensure
      File.rename(backup,path)
    end
    c.run('file.close')
    File.rename(path,backup)
    begin
      m['missing_error']=failure{c.run('file.open',{path:path})}
      raise 'missing open made an empty tab' unless c.run('doc.list').none?{|d|d['path']==path}
    ensure
      File.rename(backup,path)
    end
    corrupt=File.join(out,'corrupt.figcraft')
    File.binwrite(corrupt,'synthetic corrupt file')
    m['corrupt_error']=failure{c.run('file.open',{path:corrupt})}
    unreadable=File.join(out,'unreadable.figcraft')
    FileUtils.cp(m.fetch('external'),unreadable)
    File.chmod(0,unreadable)
    begin
      m['unreadable_error']=failure{c.run('file.open',{path:unreadable})}
    ensure
      File.chmod(0600,unreadable)
    end
    m['checks'] << 'real OS trash failure retained record/tab; missing/corrupt opens returned errors; original restored'
    c.run('doc.home')
  when 'trash-restore'
    target=m.fetch('files')[5]
    c.run('file.open',{path:target['path']})
    c.run('node.set',{ids:[target['node']],props:{width:206}})
    result=c.run('draft.trash',{id:target['id'],confirmed:true})
    trashed=result.fetch('trashed')
    raise 'OS did not return a recoverable destination' if trashed.empty? || !File.file?(trashed) || File.exist?(target['path'])
    c.run('file.open',{path:trashed})
    raise 'trash lost the last edit' unless c.request('node.get',{id:target['node']})['width']==206
    c.run('file.close')
    m['pending_restore']=target
    m['trash_destination']=trashed
    m['checks'] << 'OS returned actual Trash path; last dirty edit read back from trashed file'
    c.run('doc.home')
  when 'restore-check'
    target=m.fetch('pending_restore')
    # In Finder, restore the exact trash_destination (Put Back when available, or Move Here to the original Drafts folder).
    # Shell rename is denied by macOS Trash protections; do not weaken those protections.
    raise 'restore this synthetic file in Finder first' unless File.file?(target.fetch('path'))
    c.run('file.open',{path:target['path']})
    raise 'restored content lost' unless c.request('node.get',{id:target['node']})['width']==206
    c.run('draft.save')
    c.run('file.close')
    m.delete('pending_restore')
    m['checks'] << 'Finder restored this exact test file; reopened content and stable identity verified'
    c.run('doc.home')
  when 'external-prepare'
    c.run('file.open',{path:m.fetch('external')})
    c.run('file.close')
    m['external_hash']=Digest::SHA256.file(m.fetch('external')).hexdigest
    c.run('doc.home')
  when 'cancel-check'
    target=m.fetch('files')[6]
    raise 'cancel removed the file' unless File.file?(target.fetch('path'))
    raise 'cancel changed list' unless c.run('draft.list').fetch('files').size==26
    m['checks'] << 'native Trash Cancel retained all 26 files/records'
  when 'native-trash-prepare'
    target=m.fetch('files')[6]
    c.run('file.open',{path:target['path']})
    c.run('node.set',{ids:[target['node']],props:{width:207}})
    c.run('file.open',{path:m.fetch('files')[1]['path']})
    m['other_uid']=c.request('document.inspect',{depth:0})['id']
    c.run('doc.home')
  when 'native-trash-check'
    target=m.fetch('files')[6]
    raise 'native confirm did not trash target' if File.exist?(target['path']) || c.run('draft.list').fetch('files').any?{|f|f['id']==target['id']}
    raise 'target tab survived or other tab changed' if c.run('doc.list').any?{|d|d['path']==target['path']} || c.request('document.inspect',{depth:0})['id']!=m.fetch('other_uid')
    trashed=File.join(File.dirname(m.fetch('trash_destination')),File.basename(target.fetch('path')))
    c.run('file.open',{path:trashed})
    raise 'native trash lost final edits' unless c.request('node.get',{id:target['node']})['width']==207
    c.run('file.close')
    c.run('file.openRecent',{remove:trashed})
    m['checks'] << 'native Trash Confirm moved only target, closed its tab, preserved other active tab and final saved content'
    c.run('doc.home')
  when 'missing-record'
    result=c.run('draft.create',{id:'f'*32,name:'失效记录演练'})
    c.run('file.close')
    path=result.fetch('path')
    raise 'disposable root required' unless path.start_with?(m.fetch('config')+'/Drafts/') && File.basename(m['config']).start_with?('kodma-s003-i4-')
    held=File.join(out,'missing-held.figcraft')
    File.rename(path,held)
    count=c.run('doc.list').size
    failure{c.run('file.open',{path:path})}
    raise 'missing open created a false tab' unless c.run('doc.list').size==count
    c.run('draft.forgetMissing',{id:result['id']})
    raise 'missing record retained' if c.run('draft.list').fetch('files').any?{|f|f['id']==result['id']}
    raise 'missing file content was destroyed' unless File.file?(held)
    m['checks'] << 'explicit missing-record removal preserved the moved synthetic content and created no false tab'
    c.run('doc.home')
  when 'external-check'
    raise 'external source changed' unless Digest::SHA256.file(m.fetch('external')).hexdigest==m.fetch('external_hash')
    raise 'external reference still registered' if c.run('file.openRecent').fetch('files').any?{|f|f['path']==m['external']}
    m['checks'] << 'native external Remove removed the reference and preserved source bytes'
  when 'quit'
    c.request('app.quit')
  else
    raise 'unknown phase'
  end
  File.write(manifest,JSON.pretty_generate(m)+"\n")
  puts JSON.pretty_generate({phase:phase, checks:m['checks']})
ensure
  c.close
end
