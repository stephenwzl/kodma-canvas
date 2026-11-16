#!/usr/bin/env ruby
# I3 A/B navigation fixtures; complex appearance/field fixtures are supplied with I4.
require_relative 'lib/canvas_control'
require 'fileutils'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17982'))
client = CanvasControl.new(port)
begin
  raise 'requires an empty isolated native workspace' unless client.request('ui.workspace').dig('host','respectHome') && client.run('doc.list').empty?
  directory=File.join(root,'dist/s002-i3',"run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
  FileUtils.mkdir_p(directory)
  manifest={config:ENV['KODMA_I3_CONFIG_DIR'],port:port}
  %w[A B].each do |tag|
    a=tag=='A'
    client.run('file.new',{name:"S002 #{tag}"})
    node=client.run('shape.rectangle',{name:a ? 'A · 蓝色' : 'B · 薄荷绿',x:a ? 120 : 360,y:200,width:a ? 150 : 220,height:a ? 100 : 120}).fetch('id')
    client.run('node.set',{ids:[node],props:{fills:[a ? '#61B8F6' : '#99D9BD'],opacity:a ? 1 : 0.6}})
    client.run('text.create',{text:"你好，Kodma #{tag}",x:120,y:390,props:{fontSize:a ? 32 : 24}})
    client.run('shape.rectangle',{name:"#{tag} · 辅助",x:120,y:480,width:100,height:60,props:{fills:['#F4A59A'],opacity:0.5}})
    client.run('select.set',{ids:[node]})
    client.run('view.set',{zoom:a ? 1 : 0.8,cx:a ? 240 : 420,cy:300})
    path=File.join(directory,"S002 #{tag}.figcraft")
    client.run('file.saveAs',{path:path})
    FileUtils.cp(path,"#{path}.original")
    client.run('file.close',{index:0})
    manifest[tag]={path:path,node:node}
  end
  raise 'fixture main NodeIds must match' unless manifest['A'][:node]==manifest['B'][:node]
  client.run('doc.home')
  File.write(File.join(root,'dist/s002-i3/latest.json'),JSON.pretty_generate(manifest))
  puts "I3 fixtures prepared; native UI acceptance has not run.\n#{JSON.pretty_generate(manifest)}"
ensure
  client.close
end
