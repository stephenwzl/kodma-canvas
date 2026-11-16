# Bounded JSON-lines client for local, explicitly started validation hosts.
require 'socket'
require 'json'
require 'timeout'
class CanvasControl
  def initialize(port)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15
    begin
      @socket = TCPSocket.new('127.0.0.1', Integer(port))
    rescue Errno::ECONNREFUSED
      raise if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      sleep 0.1
      retry
    end
    @serial = 0
  end
  def request(method, params = {})
    @serial += 1
    @socket.puts(JSON.generate(id: @serial, method: method, params: params))
    line = Timeout.timeout(20) { @socket.gets } or raise 'control connection ended'
    reply = JSON.parse(line)
    raise 'control id mismatch' unless reply['id'] == @serial
    raise "#{method}: #{reply['error']}" unless reply['ok']
    reply['result']
  end
  def run(command, params = {})
    request('engine.execute', { command: command, params: params })
  end
  def close
    @socket.close
  end
end
