#!/usr/bin/env ruby
# Synthetic managed drafts for the isolated S003 workspace, with persisted-data readback.
require_relative 'lib/canvas_control'
require 'fileutils'
require 'open3'

port = Integer(ARGV.fetch(1, '17992'))
manifest = File.expand_path(ARGV.fetch(2, 'dist/s003-i5/regression.json'))
client = CanvasControl.new(port)

def snapshot(client)
  client.request('ui.workspace').fetch('snapshot')
end

def check(condition, message)
  raise message unless condition
end

def open_draft(client, draft)
  client.run('file.open', {path: draft.fetch('path')})
  client.request('ui.set', {home: false})
  navigation = snapshot(client)
  active = navigation.fetch('documents').find { |doc| doc['id'] == navigation['active'] }
  check(active && active['name'] == draft['name'], 'wrong draft opened')
end

def remember(client, draft)
  draft['expected_nodes'] = %w[frame node child gradient].to_h do |field|
    [field, client.request('node.get', {id: draft.fetch(field)})]
  end
  draft['expected_pages'] = client.run('page.list').map { |page| page.reject { |key| key == 'current' } }
end

def verify_content(client, draft)
  draft.fetch('expected_nodes').each do |field, expected|
    check(client.request('node.get', {id: draft.fetch(field)}) == expected, "#{draft.fetch('name')}: #{field} changed")
  end
  pages = client.run('page.list').map { |page| page.reject { |key| key == 'current' } }
  check(pages == draft['expected_pages'], "#{draft.fetch('name')}: pages changed")
end

def wait_for_scale(client, choice, native)
  start = client.request('ui.inspect').fetch('framesDrawn')
  client.run('pref.uiScale', {scale: choice})
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 3
  matched_frame = nil
  loop do
    inspect = client.request('ui.inspect')
    scale = inspect.fetch('uiScale')
    frame = inspect.fetch('framesDrawn')
    if frame > start && scale['choice'] == choice && scale['zoom'] == choice &&
        scale['drawn'] == native * choice && !scale['limited']
      # egui applies the plan on the next frame. Read the AppKit/egui geometry after it does.
      if matched_frame && frame > matched_frame
        return {scale: scale, frames: frame, workspace: client.request('ui.workspace').slice('canvas', 'host')}
      end
      matched_frame = frame
    end
    raise "expected #{choice} at #{native}× display scale, got #{scale}; frames #{start} → #{frame}" if
      Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    sleep 0.1
  end
end

begin
  action = ARGV.fetch(0, 'seed')
  check(client.request('ui.workspace').dig('host', 'respectHome'), 'requires the native workspace')
  case action
  when 'seed'
    config = File.realpath(ENV.fetch('KODMA_CANVAS_CONFIG_DIR'))
    check(File.basename(config).start_with?('kodma-s003-i5-'), 'requires a disposable kodma-s003-i5-* configuration')
    check(client.run('draft.list').fetch('files').empty? && client.run('doc.list').empty?, 'use an empty isolated workspace')
    client.request('ui.set', {theme: 'light'})
    drafts = {}
    %w[A B].each_with_index do |tag, index|
      name = index.zero? ? '阅读记录 App · 春季阅读计划与书架详情交互设计稿' : '流程草稿 · Flow notes'
      draft = client.run('draft.create', {id: format('%032x', index + 1), name: name}).merge('name' => name)
      check(File.realpath(File.dirname(File.dirname(draft.fetch('path')))) == config, 'host config differs from the requested test configuration')
      color = index.zero? ? '#61B8F6' : '#99D9BD'
      client.run('page.rename', {name: "Design #{tag}"})
      draft['page'] = client.run('page.list').first.fetch('id')
      draft['frame'] = client.run('frame.create', {name: "#{tag} · Frame", x: 100, y: 110, width: 400, height: 360}).fetch('id')
      draft['node'] = client.run('shape.rectangle', {
        name: "#{tag} · Accent", parent: draft['frame'], x: 24, y: 100, width: 145 + index * 25, height: 80,
        props: {fills: [color], cornerRadius: 12, strokes: ['#394963']}
      }).fetch('id')
      draft['child'] = client.run('text.create', {
        name: "#{tag} · Label", parent: draft['frame'], text: "Kodma #{tag} · mixed styles", x: 24, y: 34,
        props: {fontFamily: 'Helvetica', fontSize: 24}
      }).fetch('id')
      client.run('text.style', {id: draft['child'], start: 0, end: 5, fontSize: 32})
      draft['gradient'] = client.run('shape.rectangle', {
        name: "#{tag} · Gradient", parent: draft['frame'], x: 24, y: 210, width: 290, height: 82,
        props: {fills: [{type: 'linearGradient', stops: [color, '#F4A59A']}]}
      }).fetch('id')
      draft['empty'] = client.run('page.add', {name: "Empty #{tag}"}).fetch('id')
      client.run('page.select', {id: draft['page']})
      client.run('draft.save')
      remember(client, draft)
      drafts[tag] = draft
    end
    check(drafts['A']['node'] == drafts['B']['node'], 'fixtures must have overlapping node IDs')
    open_draft(client, drafts['A'])
    client.run('view.set', {zoom: 0.9, cx: 300, cy: 290})
    FileUtils.mkdir_p(File.dirname(manifest))
    File.write(manifest, JSON.pretty_generate(drafts))
    puts manifest
  when 'scale'
    native = Float(ARGV.fetch(3, '2'))
    before = wait_for_scale(client, 1, native)
    target = wait_for_scale(client, 1.25, native)
    check(before.dig(:workspace, 'canvas', 'width').to_f.positive?, 'requires a visible canvas')
    %w[x y width height].each do |key|
      baseline = before.fetch(:workspace).fetch('canvas').fetch(key)
      scaled = target.fetch(:workspace).fetch('canvas').fetch(key)
      check((baseline - scaled * 1.25).abs < 1.0, "#{key}: egui and native canvas bounds diverged")
    end
    puts JSON.pretty_generate(before: before, target: target)
  when 'mcp'
    drafts = JSON.parse(File.read(manifest))
    open_draft(client, drafts.fetch('A'))
    verify_content(client, drafts.fetch('A'))
    open_draft(client, drafts.fetch('B'))
    before = snapshot(client).slice('active', 'page', 'selection', 'zoom')
    script = File.join(__dir__, 's002_agent_edit.rb')
    out, err, status = Open3.capture3({'KODMA_S002_MANIFEST' => manifest}, 'ruby', script, port.to_s, 'A', '#FF9687')
    check(status.success?, err)
    check(before == snapshot(client).slice('active', 'page', 'selection', 'zoom'), 'foreground changed after MCP edit')
    verify_content(client, drafts.fetch('B'))
    open_draft(client, drafts.fetch('A'))
    edited = client.request('node.get', {id: drafts.dig('A', 'node')})
    original = drafts.dig('A', 'expected_nodes', 'node')
    check(edited.dig('fills', 0, 'color') == '#FF9687', 'background edit missing')
    check(edited.reject { |key| key == 'fills' } == original.reject { |key| key == 'fills' }, 'background edit changed unrelated properties')
    drafts['A']['expected_nodes']['node'] = edited
    drafts['A']['expected_nodes']['frame'].fetch('children').each do |child|
      child['fills'] = edited['fills'] if child['id'] == edited['id']
    end
    verify_content(client, drafts.fetch('A'))
    client.run('draft.save')
    File.write(manifest, JSON.pretty_generate(drafts))
    puts out
  when 'restart'
    drafts = JSON.parse(File.read(manifest))
    indexed = client.run('draft.list').fetch('files')
    drafts.each_value do |draft|
      check(indexed.any? { |entry| %w[id name path].all? { |key| entry[key] == draft[key] } }, 'stable draft identity missing')
      open_draft(client, draft)
      verify_content(client, draft)
    end
    puts 'Both managed drafts reopened with the same identity, pages, hierarchy, gradient and rich text.'
  else
    raise "unknown phase #{action}"
  end
ensure
  client.close
end
