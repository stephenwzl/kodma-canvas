#!/usr/bin/env ruby
# I4 synthetic A/B, mixed values, complex fills and rich text; no private content.
require_relative 'lib/canvas_control'
require 'fileutils'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17982'))
client = CanvasControl.new(port)
begin
  raise 'requires an empty isolated native workspace' unless client.request('ui.workspace').dig('host','respectHome') && client.run('doc.list').empty?
  client.request('ui.set',{theme:'light'})
  directory=File.join(root,'dist/s002-i4',"run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
  FileUtils.mkdir_p(directory)
  manifest={config:ENV['KODMA_I4_CONFIG_DIR'],port:port}
  %w[A B].each do |tag|
    a=tag=='A'
    client.run('file.new',{name:"S002 #{tag}"})
    node=client.run('shape.rectangle',{name:a ? 'A · 蓝色' : 'B · 薄荷绿',x:a ? 120 : 360,y:200,width:a ? 150 : 220,height:a ? 100 : 120}).fetch('id')
    client.run('node.set',{ids:[node],props:{fills:[a ? '#61B8F6' : '#99D9BD'],opacity:a ? 1 : 0.6}})
    text=client.run('text.create',{text:"你好，Kodma #{tag}",x:120,y:390,props:{fontSize:a ? 32 : 24,fontFamily:"Helvetica"}}).fetch('id')
    helper=client.run('shape.rectangle',{name:"#{tag} · 辅助",x:120,y:480,width:100,height:60,props:{fills:['#F4A59A'],opacity:0.5}}).fetch('id')
    gradient=nil; rich=nil
    if a
      gradient=client.run('shape.rectangle',{name:'A · 渐变',x:360,y:480,width:150,height:100,props:{fills:[{type:'linearGradient',stops:['#61B8F6','#99D9BD']}]}}).fetch('id')
      rich=client.run('text.create',{name:'A · 混合文字',text:'保留混合样式',x:120,y:620,props:{fontFamily:'Helvetica',fontSize:24}}).fetch('id')
      client.run('text.style',{id:rich,start:0,end:2,fontSize:36})
    end
    client.run('select.set',{ids:[node]})
    client.run('view.set',{zoom:a ? 1 : 0.8,cx:a ? 240 : 420,cy:300})
    path=File.join(directory,"S002 #{tag}.figcraft")
    client.run('file.saveAs',{path:path})
    FileUtils.cp(path,"#{path}.original")
    client.run('file.close',{index:0})
    manifest[tag]={path:path,node:node,text:text,helper:helper,gradient:gradient,rich:rich}
  end
  raise 'fixture main NodeIds must match' unless manifest['A'][:node]==manifest['B'][:node]
  client.run('doc.home')
  File.write(File.join(root,'dist/s002-i4/latest.json'),JSON.pretty_generate(manifest))
  puts "I4 fixtures prepared; native UI acceptance has not run.\n#{JSON.pretty_generate(manifest)}"
ensure
  client.close
end
