#!/usr/bin/env ruby
# I2 synthetic files only. A missing and a damaged copy exercise real native home errors.
require_relative 'lib/canvas_control'
require 'fileutils'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17982'))
client = CanvasControl.new(port)
begin
  raise 'requires an empty native workspace in an isolated configuration' unless client.request('ui.workspace').dig('host','respectHome') && client.run('doc.list').empty?
  directory = File.join(root, 'dist/s002-i2', "run-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}")
  FileUtils.mkdir_p(directory)
  create = lambda do |name|
    client.run('file.new', {name: name})
    node = client.run('shape.rectangle', {name: 'I2 蓝色矩形', x:120, y:160, width:180, height:120}).fetch('id')
    client.run('node.set', {ids:[node], props:{fills:['#61B8F6'], cornerRadius:16}})
    client.run('view.set', {zoom:1, cx:240, cy:220})
    path = File.join(directory, "#{name}.figcraft")
    client.run('file.saveAs', {path:path})
    client.run('file.close', {index:0})
    [path,node]
  end
  missing, = create.call('I2 已移走稿件')
  File.rename(missing, "#{missing}.backup")
  invalid, = create.call('I2 损坏稿件')
  FileUtils.cp(invalid, "#{invalid}.backup")
  File.binwrite(invalid, 'invalid synthetic figcraft file')
  actual, node = create.call('I2 蓝色卡片')
  client.run('doc.home')
  manifest = {config:ENV['KODMA_I2_CONFIG_DIR'], port:port, actual:actual, node:node, missing:missing, invalid:invalid}
  File.write(File.join(root,'dist/s002-i2/latest.json'), JSON.pretty_generate(manifest))
  puts "I2 fixtures prepared; native UI acceptance has not run.\n#{JSON.pretty_generate(manifest)}"
ensure
  client.close
end
