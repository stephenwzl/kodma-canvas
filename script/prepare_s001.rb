#!/usr/bin/env ruby
# Generate an original synthetic draft through the running, visible FigCraft runtime.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'base64'
require 'zlib'
root = File.expand_path('..', __dir__)
client = CanvasControl.new(ARGV.fetch(0, '17981'))
begin
  raise 'requires the opt-in canvas host' unless client.request('ui.inspect')['canvasHost']
  fixture = JSON.parse(File.read(File.join(root, 'spec/macos/001-canvas-editor/fixtures/card.json')))
  client.run('file.new', { name: fixture.fetch('name') })
  ids = {}
  last = nil
  fixture.fetch('commands').each do |step|
    params = step.fetch('params').dup
    params['ids'] = [last || raise('node.set before creation')] if step['command'] == 'node.set'
    result = client.run(step.fetch('command'), params)
    if result['id']
      last = result['id']
      ids[params.fetch('name')] = last
    end
  end
  # Original 8x8 checkerboard, generated in memory; no bundled image or external asset.
  chunk = lambda { |type, bytes| [bytes.bytesize].pack('N') + type + bytes + [Zlib.crc32(type + bytes)].pack('N') }
  pixels = (0...8).map do |y|
    "\x00".b + (0...8).map { |x| ((x / 2 + y / 2).even? ? [97, 184, 246, 255] : [153, 217, 189, 255]).pack('C*') }.join
  end.join
  png = "\x89PNG\r\n\x1a\n".b + chunk.call('IHDR', [8, 8, 8, 6, 0, 0, 0].pack('NNC5')) + chunk.call('IDAT', Zlib.deflate(pixels)) + chunk.call('IEND', ''.b)
  image = client.run('image.place', { data: Base64.strict_encode64(png), x: 470, y: 350, width: 150, height: 100 })
  ids['Checker'] = image.fetch('id')
  client.run('node.set', { ids: [image.fetch('id')], props: { name: 'Checker' } })
  client.run('view.set', { zoom: 1, cx: 380, cy: 270 })
  client.run('select.set', { ids: [] })
  directory = File.join(root, 'dist/s001-acceptance')
  FileUtils.mkdir_p(directory)
  # Every invocation creates a fresh file; never overwrite a person's earlier acceptance edits.
  path = File.join(directory, "card-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}.figcraft")
  client.run('file.saveAs', { path: path })
  File.write(File.join(directory, 'latest.json'), JSON.pretty_generate({ path: path, ids: ids }))
  puts path
ensure
  client.close
end
