#!/usr/bin/env ruby
# File reopen and document lifecycle probe, not proof of native view destruction or a leak test.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'open3'
client = CanvasControl.new(ARGV.fetch(0, '17981'))
root = File.expand_path('..', __dir__)
folder = File.join(root, 'dist/s001-evidence/lifecycle')
FileUtils.mkdir_p(folder)
pid = Integer(ARGV.fetch(1))
sample = lambda do
  text, status = Open3.capture2('ps', '-p', pid.to_s, '-o', 'rss=,time=')
  raise 'validation process ended' unless status.success?
  rss, cpu = text.split
  { rssKiB: Integer(rss), cpuTime: cpu, monotonic: Process.clock_gettime(Process::CLOCK_MONOTONIC) }
end
begin
  hosted = client.request('ui.inspect')['canvasHost']
  raise 'requires canvas host or explicit --reference' unless hosted || ARGV.include?('--reference')
  client.run('file.new', { name: 'S001 lifecycle synthetic' })
  node = client.run('shape.rectangle', { name: 'Persisted', x: 120, y: 100, width: 200, height: 140 })['id']
  path = File.join(folder, "reopen-#{Process.pid}.figcraft")
  client.run('file.saveAs', { path: path })
  count = client.request('ui.inspect')['tabs'].length
  samples = [sample.call]
  30.times do |index|
    client.run('file.close')
    client.run('file.open', { path: path })
    raise 'object changed after reopen' unless client.request('node.get', { id: node })['name'] == 'Persisted'
    state = client.request('ui.inspect')
    raise 'tabs leaked across reopen' unless state['tabs'].length == count
    raise 'host policy changed' unless !!state['canvasHost'] == !!hosted
    samples << sample.call if (index + 1) % 5 == 0
  end
  idle_start = sample.call
  sleep 5
  idle_finish = sample.call
  state = client.request('ui.inspect')
  File.write(File.join(folder, "probe-#{Process.pid}.json"), JSON.pretty_generate({ cycles: 30, samples: samples, idle: [idle_start, idle_finish], state: state }))
  puts JSON.generate({ result: 'PASS', cycles: 30, samples: samples, idle: [idle_start, idle_finish], frameTimes: state['frameTimes'], canvas: state['canvas'] })
ensure
  client.close
end
