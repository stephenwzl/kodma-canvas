#!/usr/bin/env ruby
# Visual fixture only, not a functional acceptance run. Owner's original editable canvas
# artwork is exported separately to ignored dist/s002-visual/{shelf,detail}.svg.
require_relative 'lib/canvas_control'
require 'fileutils'
root = File.expand_path('..', __dir__)
c = CanvasControl.new(Integer(ARGV.fetch(0, '17982')))
begin
  c.request('ui.set', {theme: 'light', home: false})
  c.run('file.new', {name: '阅读记录 App'})
  c.run('page.rename', {name: '阅读体验'})
  c.run('page.setBackground', {color: '#FFFFFF'})
  frames = []
  [['shelf', '我的书架', 290], ['detail', '阅读详情', 674]].each do |file, name, x|
    result = c.run('file.import', {path: File.join(root, 'dist/s002-visual', "#{file}.svg"), x: x, y: 288})
    id = result.fetch('id')
    c.run('node.set', {ids: [id], props: {name: name, cornerRadius: 27, fills: ['#FFFCF7'], strokes: ['#D8D2DE'], strokeWeight: 1}})
    c.run('effect.add',{ids:[id],type:'dropShadow',effect:{type:'dropShadow',color:'#2B174B12',offset:[0,12],radius:28,spread:0}})
    frames << id
  end
  [['阅读体验',290,182,22,'#121212'],['2 个界面 · 1 个流程',290,224,12,'#736E69'],['打开书籍',565,486,12,'#736E69']].each do |text,x,y,size,fill|
    c.run('text.create',{text:text,x:x,y:y,props:{fontFamily:'PingFang SC',fontSize:size,fills:[fill]}})
  end
  c.run('node.create',{type:'vector',path:'M0 14 L110 14 M102 0 L110 14 L102 28',x:550,y:517,props:{fills:[],strokes:['#B5A2D1'],strokeWeight:1.5}})
  [['我的书架',362,'#AADAFF'],['阅读详情',562,'#E8DDF6'],['记录阅读',762,'#B9EDCA']].each do |text,x,fill|
    c.run('shape.rectangle',{x:x,y:839,width:170,height:32,props:{cornerRadius:10,fills:[fill]}})
    c.run('text.create',{text:text,x:x+42,y:846,props:{fontFamily:'PingFang SC',fontSize:12,fills:['#121212']}})
  end
  c.run('view.rulers',{on:false})
  c.run('view.set',{zoom:1,cx:637,cy:522})
  c.run('select.set',{ids:[frames.last]})
  path=File.join(root,'dist/s002-visual/阅读记录 App.figcraft')
  c.run('file.saveAs',{path:path})
  ['个人作品集','旅行计划'].each do |name|
    c.run('file.new',{name:name})
    c.run('file.saveAs',{path:File.join(root,'dist/s002-visual',"#{name}.figcraft")})
    c.run('file.close')
  end
  c.run('file.open',{path:path})
  c.run('select.set',{ids:[frames.last]})
  c.request('ui.set',{home:false})
  puts JSON.pretty_generate({path:path,frames:frames,kind:'visual fixture; no functionality acceptance'})
ensure
  c.close
end
