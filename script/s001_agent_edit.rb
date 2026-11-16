#!/usr/bin/env ruby
# A real attached-MCP edit scoped to the last generated synthetic acceptance draft.
require_relative 'lib/canvas_control'
require 'open3'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17981'))
draft = JSON.parse(File.read(File.join(root, 'dist/s001-acceptance/latest.json')))
client = CanvasControl.new(port)
begin
  doc = client.request('document.inspect')
  raise 'open the generated acceptance draft first' unless doc['path'] == draft.fetch('path') && doc['name'] == 'S001 验收卡片'
  node = client.request('node.get', { id: draft.fetch('ids').fetch('Blue') })
  raise 'the Blue fixture object is missing' unless node['name'] == 'Blue'
  Open3.popen3(ENV.fetch('KODMA_CANVAS_CLI', File.join(root, 'target/debug/figcraft-cli')), 'mcp', '--attach', port.to_s, '--timeout', '10') do |input, output, errors, wait|
    call = lambda do |id, method, params|
      input.puts(JSON.generate(jsonrpc: '2.0', id: id, method: method, params: params)); input.flush
      raise 'MCP timeout' unless IO.select([output], nil, nil, 15)
      reply = JSON.parse(output.gets || raise('MCP ended'))
      raise "MCP failed: #{reply}" if reply['error'] || reply.dig('result', 'isError')
      reply
    end
    call.call(1, 'initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'S001 acceptance', version: '1' } })
    input.puts(JSON.generate(jsonrpc: '2.0', method: 'notifications/initialized'))
    call.call(2, 'tools/call', { name: 'run_command', arguments: { document: draft.fetch('path'), command: 'node.set', params: { ids: [node.fetch('id')], props: { fills: ['#FF9687'] } } } })
    input.close
    raise 'MCP did not stop' unless wait.join(10)
    raise "MCP failed: #{errors.read}" unless wait.value.success?
  end
  result = client.request('node.get', { id: node.fetch('id') })
  raise 'MCP edit did not reach the canvas' unless result.dig('fills', 0, 'color') == '#FF9687'
  puts 'PASS: attached MCP changed Blue to coral; Cmd+Z restores its previous fill.'
ensure
  client.close
end
