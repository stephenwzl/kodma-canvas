#!/usr/bin/env ruby
# Same synthetic workload for S001 and S002. Measurements are observations, not leak proofs.
require_relative 'lib/canvas_control'
require 'open3'

port = Integer(ARGV.fetch(0))
pid = Integer(ARGV.fetch(1))
label = ARGV.fetch(2)
manifest = JSON.parse(File.read(ENV.fetch('KODMA_S002_MANIFEST', File.expand_path('../dist/s002-i5/latest.json', __dir__))))
client = CanvasControl.new(port)
clock = -> { Process.clock_gettime(Process::CLOCK_MONOTONIC) }
sample = lambda do
  out, status = Open3.capture2('ps', '-p', pid.to_s, '-o', 'rss=,time=')
  raise 'validation host ended' unless status.success?
  rss, time = out.split
  cpu = time.split(':').map(&:to_f).reduce(0.0) { |total, part| total * 60 + part }
  { rssKiB: Integer(rss), cpuSeconds: cpu, monotonic: clock.call }
end
switch = lambda do |tag|
  file = manifest.fetch(tag).fetch('path')
  doc = client.run('doc.list').find { |d| d['path'] == file }
  raise "missing fixture #{tag}" unless doc
  client.run('doc.switch', { index: doc.fetch('index') })
  client.request('ui.set', { home: false })
end
report = { label: label, pid: pid, fixtures: %w[A B].to_h { |tag| [tag, manifest.fetch(tag).fetch('path')] }, warmupCycles: 30, measuredCycles: 120, samples: [] }
begin
  raise 'use a dedicated two-file validation host' unless client.run('doc.list').length == 2
  report[:workspace] = client.request('ui.workspace').slice('canvas', 'host')
  report[:uiScale] = client.request('ui.inspect').fetch('uiScale')
  expected = {}
  %w[A B].each do |tag|
    switch.call(tag)
    client.run('file.save')
    expected[tag] = client.request('node.get', { id: manifest.fetch(tag).fetch('node') })
  end
  150.times do |i|
    switch.call('A')
    client.run('file.close')
    client.run('file.open', { path: manifest.fetch('A').fetch('path') })
    raise 'reopen changed saved object' unless client.request('node.get', { id: manifest.fetch('A').fetch('node') }) == expected['A']
    raise 'duplicate or missing document' unless client.run('doc.list').length == 2
    client.run('doc.home')
    switch.call('B')
    report[:samples] << sample.call.merge(cycle: i + 1) if (i + 1) % 10 == 0
  end
  first = sample.call
  sleep 5
  last = sample.call
  report[:idle] = { seconds: last[:monotonic] - first[:monotonic], cpuSeconds: last[:cpuSeconds] - first[:cpuSeconds], rssBeforeKiB: first[:rssKiB], rssAfterKiB: last[:rssKiB] }
  report[:result] = 'PASS: 150 cycles retained two files and exact saved object data'
  report[:scope] = 'same binary and synthetic files; 30 warmup + 120 measured protocol cycles and five-second idle; not native interaction or a long-term leak guarantee'
  puts JSON.pretty_generate(report)
ensure
  client.close
end
