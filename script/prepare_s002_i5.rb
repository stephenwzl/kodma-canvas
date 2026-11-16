#!/usr/bin/env ruby
# Extend freshly generated synthetic I4 files with a real second page and parent/child pair.
require_relative 'lib/canvas_control'
require 'fileutils'
root=File.expand_path('..',__dir__)
m=JSON.parse(File.read(File.join(root,'dist/s002-i4/latest.json')))
c=CanvasControl.new(Integer(ARGV.fetch(0,'17982')))
begin
  raise 'requires an empty isolated workspace' unless c.run('doc.list').empty?
  c.run('file.open',{path:m['A']['path']})
  c.request('ui.set',{home:false})
  page=c.request('ui.workspace').dig('snapshot','page')
  c.run('page.rename',{name:'属性与层级'})
  parent=c.run('node.create',{type:'frame',name:'层级验收',x:120,y:730,width:360,height:120,props:{fills:['#E9E2FA']}}).fetch('id')
  child=c.run('node.create',{type:'text',parent:parent,name:'子图层',text:'真实子图层',x:24,y:24,props:{fontSize:24}}).fetch('id')
  empty=c.run('page.add',{name:'空白页'}).fetch('id')
  c.run('page.select',{id:page})
  c.run('select.set',{ids:[m['A']['node']]})
  c.run('view.set',{zoom:1,cx:240,cy:300})
  c.run('file.save')
  FileUtils.cp(m['A']['path'],"#{m['A']['path']}.i5-original")
  c.run('file.close')
  m['A'].merge!('page'=>page,'emptyPage'=>empty,'parent'=>parent,'child'=>child)
  c.run('doc.home')
  FileUtils.mkdir_p(File.join(root,'dist/s002-i5'))
  File.write(File.join(root,'dist/s002-i5/latest.json'),JSON.pretty_generate(m))
  puts JSON.pretty_generate(m)
ensure
  c.close
end
