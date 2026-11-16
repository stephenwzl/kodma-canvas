#!/usr/bin/env ruby
# Real attached MCP, scoped to an explicit generated file; never to the frontmost tab.
require_relative 'lib/canvas_control'
require 'open3'
root=File.expand_path('..',__dir__)
port=Integer(ARGV.fetch(0,'17982'))
tag=ARGV.fetch(1,'A')
raise 'target must be A or B' unless %w[A B].include?(tag)
manifest=JSON.parse(File.read(ENV.fetch('KODMA_S002_MANIFEST',File.join(root,'dist/s002-i4/latest.json'))))
target=manifest.fetch(tag)
color=ARGV.fetch(2,'#FF9687')
raise 'requires an opaque hex color' unless /\A#[0-9a-fA-F]{6}\z/.match?(color)
control=CanvasControl.new(port)
begin
  before=control.request('ui.workspace').fetch('snapshot')
  result=nil
  Open3.popen3(ENV.fetch('KODMA_CANVAS_CLI',File.join(root,'target/debug/figcraft-cli')),'mcp','--attach',port.to_s,'--timeout','10') do |input,output,errors,process|
    call=lambda do |id,method,params|
      input.puts(JSON.generate(jsonrpc:'2.0',id:id,method:method,params:params));input.flush
      raise 'MCP timeout' unless IO.select([output],nil,nil,15)
      reply=JSON.parse(output.gets || raise('MCP ended'))
      raise 'MCP request failed' if reply['error'] || reply.dig('result','isError')
      reply.fetch('result')
    end
    call.call(1,'initialize',{protocolVersion:'2025-06-18',capabilities:{},clientInfo:{name:'S002 acceptance',version:'1.2'}})
    input.puts(JSON.generate(jsonrpc:'2.0',method:'notifications/initialized'))
    args={document:target.fetch('path'),command:'node.get',params:{id:target.fetch('node')}}
    node=call.call(2,'tools/call',{name:'run_command',arguments:args})
    data=JSON.parse(node.fetch('content').find{|c|c['type']=='text'}.fetch('text'))
    raise 'fixture identity mismatch' unless data['name'].start_with?("#{tag} ·") && data['type']=='RECTANGLE'
    call.call(3,'tools/call',{name:'run_command',arguments:{document:target.fetch('path'),command:'node.set',params:{ids:[target.fetch('node')],props:{fills:[color]}}}})
    result=call.call(4,'tools/call',{name:'run_command',arguments:args})
    data=JSON.parse(result.fetch('content').find{|c|c['type']=='text'}.fetch('text'))
    raise 'MCP readback mismatch' unless data.dig('fills',0,'color')==color.upcase
    input.close
    raise 'MCP did not exit' unless process.join(10)
    raise 'MCP process failed' unless process.value.success?
  end
  after=control.request('ui.workspace').fetch('snapshot')
  %w[active home page selection].each{|key|raise "foreground #{key} changed" unless before[key]==after[key]}
  puts JSON.pretty_generate({result:'PASS',target:tag,path:target['path'],color:color,foreground:after.slice('active','home','page','selection')})
ensure
  control.close
end
