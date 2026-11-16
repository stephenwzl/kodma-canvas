#!/usr/bin/env ruby
# Protocol/MCP integration and short resource probe; native mouse/IME are separate evidence.
require_relative 'lib/canvas_control'
require 'open3'
require 'fileutils'
root=File.expand_path('..',__dir__)
port=Integer(ARGV.fetch(0,'17982'))
manifest_path=ENV.fetch('KODMA_S002_MANIFEST',File.join(root,'dist/s002-i4/latest.json'))
m=JSON.parse(File.read(manifest_path))
c=CanvasControl.new(port)
report={checks:[],samples:[]}
check=lambda{|name,ok|raise name unless ok; report[:checks]<<name}
switch=lambda{|tag| ds=c.run('doc.list').find{|d|d['path']==m.fetch(tag).fetch('path')};raise 'open both generated files first' unless ds;c.run('doc.switch',{index:ds.fetch('index')});c.request('ui.set',{home:false})}
agent=lambda do |color,success=true|
  env={'KODMA_S002_MANIFEST'=>manifest_path}
  env['KODMA_CANVAS_CLI']=ENV['KODMA_CANVAS_CLI'] if ENV['KODMA_CANVAS_CLI']
  out,err,status=Open3.capture3(env,'ruby',File.join(root,'script/s002_agent_edit.rb'),port.to_s,'A',color)
  raise "unexpected MCP status: #{err}" unless status.success? == success
  report[:checks]<< (success ? "attached MCP #{color}" : 'readonly MCP rejected')
  out
end
begin
  switch.call('A'); c.run('select.set',{ids:[m['A']['node']]})
  original=c.run('node.get',{id:m['A']['node']})
  switch.call('B');b=c.request('document.inspect')
  agent.call('#FF9687')
  check.call('background file unchanged',c.request('document.inspect')==b)
  switch.call('A');node=c.run('node.get',{id:m['A']['node']})
  check.call('background result visible',node.dig('fills',0,'color')=='#FF9687')
  check.call('untouched appearance preserved',%w[cornerRadius strokes effects].all?{|key|node[key]==original[key]})
  agent.call('#AADAFF')
  c.run('doc.home');agent.call('#FF9687');check.call('home remains visible',c.request('ui.workspace').dig('snapshot','home'))
  switch.call('A');c.run('file.saveVersion',{name:'S002 readonly fixture'});c.run('version.preview',{index:0})
  agent.call('#112233',false)
  check.call('readonly data unchanged',c.run('node.get',{id:m['A']['node']}).dig('fills',0,'color')=='#FF9687')
  c.run('version.exitPreview')
  expected={}
  %w[A B].each{|tag|switch.call(tag);c.run('file.save');expected[tag]=c.run('node.get',{id:m[tag]['node']})}
  pid=ARGV[1]&.to_i
  sample=lambda do
    if pid&&pid>0
      out,status=Open3.capture2('ps','-p',pid.to_s,'-o','rss=,time=');raise 'app ended' unless status.success?
      rss,cpu=out.split; report[:samples]<<{rssKiB:Integer(rss),cpuTime:cpu,monotonic:Process.clock_gettime(Process::CLOCK_MONOTONIC)}
    end
  end
  sample.call
  count=c.run('doc.list').length
  30.times do |i|
    switch.call('A');c.run('file.close');c.run('file.open',{path:m['A']['path']})
    raise 'saved data changed' unless c.run('node.get',{id:m['A']['node']})==expected['A']
    raise 'tab count changed' unless c.run('doc.list').length==count
    c.run('doc.home');switch.call('B')
    sample.call if (i+1)%5==0
  end
  check.call('30 reopen/home/A-B cycles',true)
  %w[A B].each do |tag|
    switch.call(tag);c.run('file.close');c.run('file.open',{path:m[tag]['path']})
    check.call("#{tag} saved data reopened",c.run('node.get',{id:m[tag]['node']})==expected[tag])
  end
  report[:cycles]=30
  report[:scope]='short protocol/MCP probe; not native input acceptance or a long-term leak guarantee'
  puts JSON.pretty_generate(report)
ensure
  c.close
end
