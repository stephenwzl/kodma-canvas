#!/usr/bin/env ruby
# I1 only: a real file, native-host observations, hidden-home scheduling and attached MCP.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'open3'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17982'))
client = CanvasControl.new(port)
begin
  state = client.request('ui.workspace')
  raise 'requires --native-workspace' unless state.dig('host', 'respectHome')
  client.run('file.new', { name: 'S002 I1 · 工作区' })
  client.request('ui.set', { home: false, theme: 'light' })
  node = client.run('shape.rectangle', { name: 'I1 · 蓝色矩形', x: 120, y: 160, width: 180, height: 120 }).fetch('id')
  client.run('node.set', { ids: [node], props: { fills: ['#61B8F6'], cornerRadius: 16 } })
  client.run('view.set', { zoom: 1, cx: 240, cy: 220 })
  directory = File.join(root, 'dist/s002-i1')
  FileUtils.mkdir_p(directory)
  path = File.join(directory, "workspace-#{Time.now.strftime('%Y%m%d-%H%M%S')}-#{Process.pid}.figcraft")
  client.run('file.saveAs', { path: path })
  original = client.request('ui.workspace').fetch('snapshot').fetch('active')
  docs = client.run('doc.list')
  index = docs.find { |d| d['id'] == original }.fetch('index')
  client.run('file.close', { index: index })
  client.run('file.open', { path: path })
  client.request('ui.set', { home: false })
  client.run('select.set', { ids: [node] })
  # Observe after presentation has reached a rendered frame, never rely on an arbitrary sleep.
  wait_for = lambda do |&predicate|
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 10
    loop do
      state = client.request('ui.workspace')
      break state if predicate.call(state)
      raise 'workspace observation timeout' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      sleep 0.1
    end
  end
  opened = wait_for.call { |s| !s.dig('snapshot', 'home') && s.dig('canvas', 'x').to_f > 0 }
  uid = opened.fetch('snapshot').fetch('active')
  raise 'reopen reused the closed runtime identity' if uid == original
  raise 'selection does not match the file' unless opened.dig('snapshot', 'selection', 0, 'id') == node
  host = opened.fetch('host')
  raise 'missing native regions' unless [host['left'], host['top'], host['right']].all? { |x| x.to_f > 0 }
  client.run('doc.home')
  wait_for.call { |s| s.dig('snapshot', 'home') && s['canvas'].nil? }
  # Attached MCP writes the explicitly named on-disk document while native home is visible.
  Open3.popen3(File.join(root, 'target/debug/figcraft-cli'), 'mcp', '--attach', port.to_s, '--timeout', '10') do |input, output, errors, process|
    call = lambda do |id, method, params|
      input.puts(JSON.generate(jsonrpc: '2.0', id: id, method: method, params: params)); input.flush
      raise 'MCP timeout' unless IO.select([output], nil, nil, 15)
      reply = JSON.parse(output.gets || raise('MCP ended'))
      raise "MCP error: #{reply}" if reply['error'] || reply.dig('result', 'isError')
    end
    call.call(1, 'initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'S002 I1 verification', version: '1' } })
    input.puts(JSON.generate(jsonrpc: '2.0', method: 'notifications/initialized'))
    call.call(2, 'tools/call', { name: 'run_command', arguments: { document: path, command: 'node.set', params: { ids: [node], props: { width: 220 } } } })
    input.close
    raise 'MCP did not stop' unless process.join(10)
    raise "MCP process failed: #{errors.read}" unless process.value.success?
  end
  raise 'home changed during MCP edit' unless client.request('ui.workspace').dig('snapshot', 'home')
  client.request('ui.set', { home: false })
  updated = wait_for.call { |s| s.dig('snapshot', 'selection', 0, 'width') == 220 }
  raise 'document identity changed during MCP edit' unless updated.dig('snapshot', 'active') == uid
  client.run('edit.undo')
  wait_for.call { |s| s.dig('snapshot', 'selection', 0, 'width') == 180 }
  client.run('edit.redo')
  wait_for.call { |s| s.dig('snapshot', 'selection', 0, 'width') == 220 }
  client.run('file.save')
  client.run('view.set', { zoom: 1, cx: 240, cy: 220 })
  result = { path: path, document: uid, node: node, state: client.request('ui.workspace'), checks: ['real file reopen', 'new UID after reopen', 'native regions', 'home background attached MCP', 'snapshot refresh', 'undo/redo'] }
  File.write(File.join(directory, 'latest.json'), JSON.pretty_generate(result))
  puts "PASS: S002 I1 file, identity, native regions, background MCP and undo/redo.\n#{path}"
ensure
  client.close
end
