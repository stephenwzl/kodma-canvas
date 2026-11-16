#!/usr/bin/env ruby
# Exercise a running S001 validation host through the actual control and attached MCP transports.
require 'socket'
require 'json'
require 'open3'
require 'fileutils'
require 'base64'

root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17981'))
evidence = File.expand_path(ENV.fetch('KODMA_CANVAS_EVIDENCE', 'dist/s001-evidence'), root)
FileUtils.mkdir_p(evidence)
deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
begin
  socket = TCPSocket.new('127.0.0.1', port)
rescue Errno::ECONNREFUSED
  raise if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
  sleep 0.1
  retry
end
serial = 0
request = lambda do |method, params = {}, expect_error = false|
  serial += 1
  socket.puts(JSON.generate(id: serial, method: method, params: params))
  raise "control timeout: #{method}" unless IO.select([socket], nil, nil, 15)
  reply = JSON.parse(socket.gets || raise('control connection ended'))
  raise "control id mismatch: #{method}" unless reply['id'] == serial
  raise "expected a rejected request: #{method}" if expect_error && reply['ok']
  raise "#{method}: #{reply['error']}" if !expect_error && !reply['ok']
  reply['result']
end
run = lambda { |command, params = {}| request.call('engine.execute', { command: command, params: params }) }
initial = request.call('ui.inspect')
raise 'this script only modifies an opt-in canvas validation host' unless initial['canvasHost']

run.call('file.new', { name: 'S001 synthetic verification' })
node = run.call('shape.rectangle', { name: 'S001 rectangle', x: 120, y: 100, width: 200, height: 140 })['id']
run.call('node.set', { ids: [node], props: { name: 'S001 modified' } })
raise 'edit not applied' unless request.call('node.get', { id: node })['name'] == 'S001 modified'
run.call('edit.undo')
raise 'undo not applied' unless request.call('node.get', { id: node })['name'] == 'S001 rectangle'
run.call('edit.redo')
raise 'redo not applied' unless request.call('node.get', { id: node })['name'] == 'S001 modified'
request.call('engine.execute', { command: 'shape.rectangle', params: {}, document: 'missing-s001-document' }, true)

Open3.popen3(ENV.fetch('KODMA_CANVAS_CLI', File.join(root, 'target', 'debug', 'figcraft-cli')), 'mcp', '--attach', port.to_s, '--timeout', '10') do |input, output, errors, wait|
  mcp = lambda do |packet|
    input.puts(JSON.generate(packet))
    input.flush
    raise 'MCP timeout' unless IO.select([output], nil, nil, 15)
    JSON.parse(output.gets || raise('MCP connection ended'))
  end
  init = mcp.call(jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'S001 verifier', version: '1' } })
  raise "MCP initialize: #{init['error']}" if init['error']
  input.puts(JSON.generate(jsonrpc: '2.0', method: 'notifications/initialized'))
  reply = mcp.call(jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'run_command', arguments: { command: 'node.set', params: { ids: [node], props: { name: 'S001 via attached MCP' } } } })
  raise "MCP tool failed: #{reply}" if reply['error'] || reply.dig('result', 'isError')
  input.close
  raise 'MCP process did not stop' unless wait.join(10)
  raise "MCP failed: #{errors.read}" unless wait.value.success?
end
raise 'MCP did not edit the visible runtime' unless request.call('node.get', { id: node })['name'] == 'S001 via attached MCP'
if ARGV.include?('--background')
  File.write(File.join(evidence, 'background-control-and-mcp.json'), JSON.pretty_generate({ port: port, node: node, state: request.call('ui.inspect'), checks: %w[background-control-read-write undo redo invalid-document attached-mcp shared-runtime] }))
  puts 'PASS: background control, undo/redo, invalid target and attached MCP'
  socket.close
  exit
end
run.call('view.set', { zoom: 1.0, cx: 300, cy: 240 })
run.call('select.set', { ids: [node] })
geometry = request.call('ui.inspect').dig('canvas', 'rect')
origin_x = geometry[0] + (geometry[2] - geometry[0]) / 2.0
origin_y = geometry[1] + (geometry[3] - geometry[1]) / 2.0
start = [origin_x + 220 - 300, origin_y + 170 - 240]
finish = [start[0] + 40, start[1] + 20]
request.call('ui.input', { events: [{ move: start }, { down: start }] })
request.call('ui.input', { events: [{ move: finish }] })
request.call('ui.input', { events: [{ up: finish }] })
raise 'window pointer input did not move the rectangle' unless request.call('node.get', { id: node })['x'] > 120
run.call('edit.undo')
raise 'pointer gesture was not one undo step' unless request.call('node.get', { id: node })['x'] == 120
request.call('ui.input', { events: [{ move: start }, { down: start }, { move: finish }, { up: finish }] })
raise 'batched pointer gesture lost its final position' unless request.call('node.get', { id: node })['x'] == 160
run.call('edit.undo')
raise 'batched pointer gesture was not one undo step' unless request.call('node.get', { id: node })['x'] == 120
request.call('ui.set', { home: true })
run.call('view.toggleUi', { on: false })
state = request.call('ui.inspect')
raise 'canvas host disappeared' unless state['canvasHost']
raise 'tab strip reappeared' unless state.dig('canvas', 'rect', 1) == 0.0
capture = request.call('ui.screenshot')
File.binwrite(File.join(evidence, 'egui-canvas.png'), Base64.decode64(capture.fetch('data')))
File.write(File.join(evidence, 'control-and-mcp.json'), JSON.pretty_generate({ port: port, node: node, state: state, checks: %w[control-read-write undo redo invalid-document attached-mcp shared-runtime window-pointer-input gesture-undo batched-pointer-input persistent-host-policy screenshot] }))
puts 'PASS: control, pointer input, undo/redo, invalid target, attached MCP, shared runtime and canvas capture'
puts "Evidence: #{evidence}"
socket.close
