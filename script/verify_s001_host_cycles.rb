#!/usr/bin/env ruby
# Native host construction/destruction on the actual eframe shutdown path.
require_relative 'lib/canvas_control'
require 'open3'
require 'tmpdir'
require 'fileutils'
root = File.expand_path('..', __dir__)
port = Integer(ARGV.fetch(0, '17981'))
folder = File.join(root, 'dist/s001-evidence/host-cycles')
FileUtils.mkdir_p(folder)
log_path = File.join(root, 'dist/canvas-stderr.log')
wait_stopped = lambda do
  50.times do
    _, status = Open3.capture2('pgrep', '-x', 'KodmaCanvas')
    break if !status.success?
    sleep 0.1
  end
  _, status = Open3.capture2('pgrep', '-x', 'KodmaCanvas')
  raise 'validation app did not stop' if status.success?
end
# Refuse to stop a default/reference host. This probe restarts only the opt-in validation app.
client = CanvasControl.new(port)
raise 'requires canvas validation host' unless client.request('ui.inspect')['canvasHost']
client.request('app.quit'); client.close
wait_stopped.call
Dir.mktmpdir('kodma-s001-cycles-') do |config|
  5.times do |index|
    offset = File.exist?(log_path) ? File.size(log_path) : 0
    ok = system({ 'KODMA_CANVAS_CONFIG_DIR' => config }, File.join(root, 'script/build_and_run.sh'), '--no-build', '--verify', '--control', port.to_s, out: File::NULL, err: File::NULL)
    raise 'validation launch failed' unless ok
    client = CanvasControl.new(port)
    raise 'host missing' unless client.request('ui.inspect')['canvasHost']
    client.request('app.quit'); client.close
    wait_stopped.call
    log = File.binread(log_path).byteslice(offset..)
    File.binwrite(File.join(folder, "cycle-#{index + 1}.log"), log)
    raise 'native cleanup was bypassed' unless log.scan('native_canvas installed').length == 1 && log.scan('native_canvas detached').length == 1
    puts "PASS: native install/drop cycle #{index + 1}"
  end
end
