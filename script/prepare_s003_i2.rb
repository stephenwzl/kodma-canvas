#!/usr/bin/env ruby
# Original synthetic list fixtures for the isolated S003 visual review. Not a persistence test.
require_relative 'lib/canvas_control'
require 'fileutils'
port = Integer(ARGV.fetch(0, '17994'))
root = File.expand_path('..', __dir__)
dir = File.join(root, 'dist/s003-i2', "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
FileUtils.mkdir_p(dir)
c = CanvasControl.new(port)
begin
  raise 'requires the native workspace' unless c.request('ui.workspace').dig('host', 'respectHome')
  paths = []
  20.times do |i|
    name = case i
           when 19 then '中文长名称 · 阅读记录和界面流程设计的合成测试稿件'
           when 17, 18 then '同名稿件'
           else format('合成测试稿 %02d', i + 1)
           end
    folder = File.join(dir, i.to_s)
    FileUtils.mkdir_p(folder)
    path = File.join(folder, "#{name}.figcraft")
    index = c.run('file.new', {name: name}).fetch('index')
    c.run('shape.rectangle', {name:'合成矩形', x:120, y:180, width:150, height:100, props:{fill:'#61B8F6'}})
    c.run('file.saveAs', {path:path})
    c.run('file.close', {index:index})
    paths << path
  end
  c.run('doc.home')
  File.write(File.join(dir, 'manifest.json'), JSON.pretty_generate(paths: paths, port: port))
  File.write(File.join(root, 'dist/s003-i2/latest.json'), JSON.pretty_generate(paths: paths, port: port))
  puts 'Prepared 20 external synthetic list entries; managed drafts and >20 persistence are not validated.'
ensure
  c.close
end
