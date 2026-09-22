#!/usr/bin/env ruby
# frozen_string_literal: true
# demos 能力探针 —— Ruby 实现。协议见 demos/SPEC.md。
#
# 模式（SPEC §4 的四个 + 一个并发用）：
#   默认                    跑全部检查
#   --echo-child            往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
#   --read-stdin            读一行 stdin，内容为 ping 时回 pong
#   --argv-probe A B        回显实参个数与内容
#   --count-worker <out>    往 <out> 追加 1000 行（A18 并发用）
#
# 设计取舍：
#  1) 只用默认 gem（open3 / json / digest / base64 / zlib / tempfile / socket），无 Bundler 依赖。
#  2) 启动子进程一律走 **argv 数组**（Open3.capture3 / Process.spawn），不过 shell
#     ⇒ Windows 与 POSIX 的引号差异完全不参与，比 bash/awk/lua 三个探针干净。
#  3) A18 用 Process.spawn 起两个真子进程再 Process.wait 收尸 —— 真并发 + 真 join。
#  4) A19 用 Thread 做服务端（Ruby 的线程是真 OS 线程），accept 阻塞在子线程里，主线程连接。
#  5) 模式分派必须先于任何检查：探针自我调用，派错会指数级派生进程。

require "open3"
require "socket"
require "tempfile"

UNICODE_S = "中文-日本語-한국어-🚀"
UNICODE_CODEPOINTS = 12
UNICODE_BYTES = 31
LARGE_BYTES = 262_144
SHA256_ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

SELF = File.expand_path(__FILE__)
RUBY = RbConfig.ruby
MAX_DEPTH = 3

DEPTH = (ENV["DEMO_DEPTH"] || "0").to_i

$npass = 0
$nfail = 0
$nskip = 0

# ---------------------------------------------------------------- 主流程
if DEPTH > MAX_DEPTH
  warn "FATAL: probe recursion depth #{DEPTH} exceeded"
  exit 3
end

mode = ARGV[0] || ""

# ⚠️ 模式分派必须先于任何检查执行。
case mode
when "--echo-child"
  $stdout.write("child-ok\n")
  $stderr.write("child-err\n")
  exit 0
when "--read-stdin"
  line = ($stdin.gets || "").to_s.sub(/\r?\n\z/, "")
  $stdout.write(line == "ping" ? "pong\n" : "unexpected:#{line}\n")
  exit 0
when "--argv-probe"
  rest = ARGV[1..] || []
  $stdout.write("argv-probe:#{rest.length}:#{rest.join(':')}\n")
  exit 0
when "--count-worker"
  out = ARGV[1] or exit 4
  File.open(out, "ab") { |fh| 1000.times { fh.write("x\n") } }
  exit 0
end

# ---------------------------------------------------------------- 小工具
def trimnl(str)
  str.to_s.sub(/\r?\n\z/, "")
end

# 启动自身子进程；返回 [退出码, stdout, stderr]。走数组形式，不经 shell。
def run_self(args, stdin_data: nil)
  env = { "DEMO_DEPTH" => (DEPTH + 1).to_s }
  out, err, st = Open3.capture3(env, RUBY, SELF, *args, stdin_data: stdin_data)
  [st.exitstatus || -1, out, err]
end

def with_depth_env(extra)
  { "DEMO_DEPTH" => (DEPTH + 1).to_s }.merge(extra)
end

def count_lines(path)
  return -1 unless File.exist?(path)

  File.binread(path).count("\n")
end

# ------------------------------------------------------------------ Tier A
def a01
  _rc, out = run_self(["--argv-probe", "alpha", "beta"])
  got = trimnl(out)
  return "FAIL:argv 回显不符: #{got}" unless got == "argv-probe:2:alpha:beta"

  "2 args round-tripped"
end

def a02
  v = ENV["DEMO_LANG_TAG"].to_s
  return "FAIL:DEMO_LANG_TAG=#{v}" unless v == "demos-capability"

  "env visible"
end

def a03
  _rc, out, err = run_self(["--echo-child"])
  o = trimnl(out)
  return "FAIL:stdout=#{o}" unless o == "child-ok"
  return "FAIL:stderr missing child-err" unless err.include?("child-err")
  return "FAIL:stderr leaked into stdout" if o.include?("child-err")

  "stdout/stderr separated"
end

def a04
  _rc, out = run_self(["--read-stdin"], stdin_data: "ping\n")
  s = trimnl(out)
  return "FAIL:reply=#{s}" unless s == "pong"

  "ping->pong"
end

def a05
  File.binwrite("a05.txt", "hello-io")
  got = File.binread("a05.txt")
  return "FAIL:read back #{got}" unless got == "hello-io"

  "text round-trip ok"
end

def a06
  # 不带换行：SPEC 与其他语言都按 "ab"（无换行）对齐。
  File.open("a06.txt", "wb") { |fh| fh.write("a") }
  File.open("a06.txt", "ab") { |fh| fh.write("b") }
  got = File.binread("a06.txt")
  return "FAIL:append 结果 #{got}" unless got == "ab"

  "append ok"
end

def a07
  payload = (0..255).map(&:chr).join.b
  File.binwrite("a07.bin", payload)
  back = File.binread("a07.bin")
  return "FAIL:#{back.bytesize} bytes, round-trip mismatch" unless back == payload && back.bytesize == 256

  "256 bytes incl 0x00/0xFF"
end

def a08
  File.binwrite("a08.txt", "statted")   # 7 字节，不带换行
  return "FAIL:file not created" unless File.exist?("a08.txt")

  size = File.size("a08.txt")
  return "FAIL:size=#{size}" unless size == 7

  File.delete("a08.txt")
  return "FAIL:remove failed" if File.exist?("a08.txt")

  "size=7 then removed"
end

def a09
  # 不引入 FileUtils：Dir 的标准库方法（mkdir/children/rmdir/empty?）就够。
  Dir.rmdir("a09dir") if Dir.exist?("a09dir") && Dir.empty?("a09dir")
  Dir.mkdir("a09dir") unless Dir.exist?("a09dir")
  File.open(File.join("a09dir", "inner.txt"), "wb") { |fh| fh.write("x") }

  entries = Dir.children("a09dir")
  return "FAIL:listing missing inner.txt" unless entries.include?("inner.txt")

  File.delete(File.join("a09dir", "inner.txt"))
  Dir.rmdir("a09dir")
  return "FAIL:still present after rmdir" if Dir.exist?("a09dir")

  "mkdir/list/rmdir ok"
end

def a10
  tmp = Tempfile.new(["demos-", ".tmp"])
  path = tmp.path
  tmp.write("temp-content")
  tmp.flush
  tmp.close
  got = File.binread(path)
  tmp.unlink
  return "FAIL:temp read #{got}" unless got == "temp-content"

  "unique temp file (Tempfile)"
end

def a11
  cps = UNICODE_S.length
  return "FAIL:codepoints=#{cps}" unless cps == UNICODE_CODEPOINTS

  nbytes = UNICODE_S.bytesize
  return "FAIL:bytes=#{nbytes}" unless nbytes == UNICODE_BYTES

  File.binwrite("a11.txt", UNICODE_S)
  raw = File.binread("a11.txt").force_encoding("UTF-8")
  return "FAIL:onfile=#{raw.bytesize}" unless raw.bytesize == UNICODE_BYTES
  return "FAIL:round-trip mismatch" unless raw == UNICODE_S

  "#{cps} codepoints / #{nbytes} bytes"
end

def a12
  return "FAIL:constant drift" unless LARGE_BYTES == 262_144

  payload = ("abcdefgh" * (LARGE_BYTES / 8)).b
  File.binwrite("a12.bin", payload)
  got = File.binread("a12.bin")
  return "FAIL:len=#{got.bytesize}" unless got.bytesize == LARGE_BYTES && got == payload

  "#{LARGE_BYTES} bytes ok"
end

def a13
  now_ms = (Time.now.to_f * 1000).to_i
  return "FAIL:epoch=#{now_ms}" if now_ms < 1_577_836_800_000

  t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  sleep 0.05
  delta_ms = (Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0) * 1000.0
  return format("FAIL:sleep 只测到 %.1f ms", delta_ms) if delta_ms < 40

  format("sleep %.0fms (CLOCK_MONOTONIC)", delta_ms)
end

def a14
  v = rand(1000)
  return "FAIL:out of range #{v}" unless v >= 0 && v < 1000

  "in [0,1000) (Kernel#rand)"
end

def a15
  data = [5, 3, 9, 1, 7, 3].sort
  return "FAIL:sorted=#{data.inspect}" unless data == [1, 3, 3, 5, 7, 9]

  m = { "a" => 1 }
  m["b"] = 2
  return "FAIL:hash broken" unless m["a"] == 1 && m["b"] == 2 && m.length == 2

  m.delete("a")
  return "FAIL:hash delete" unless m.length == 1

  "Array#sort + Hash insert/delete ok"
end

def a16
  caught = false
  begin
    File.binread("definitely-missing-file-xyz")
  rescue SystemCallError
    caught = true
  end
  return "FAIL:no error raised for missing file" unless caught

  "rescue SystemCallError ok"
end

def a17
  rc, out = run_self(["--echo-child"])
  return "FAIL:child exit=#{rc}" unless rc.zero?

  s = trimnl(out)
  return "FAIL:child stdout=#{s}" unless s == "child-ok"

  "child exit=0, stdout captured"
end

def a18
  File.delete("a18.1.txt") if File.exist?("a18.1.txt")
  File.delete("a18.2.txt") if File.exist?("a18.2.txt")

  # 两次 spawn 之间**不等待** ⇒ 两个子进程真的同时在跑；再各自 wait 收尸即 join。
  pids = [
    Process.spawn(with_depth_env("DEMO_OUT" => "a18.1.txt"), RUBY, SELF, "--count-worker", "a18.1.txt"),
    Process.spawn(with_depth_env("DEMO_OUT" => "a18.2.txt"), RUBY, SELF, "--count-worker", "a18.2.txt")
  ]
  codes = pids.map { |p| Process.wait2(p)[1].exitstatus }
  return "FAIL:worker exit=#{codes.inspect}" unless codes.all?(&:zero?)

  n1 = count_lines("a18.1.txt")
  n2 = count_lines("a18.2.txt")
  return "FAIL:total=#{n1 + n2}" unless n1 + n2 == 2000

  "2 spawned workers joined -> #{n1}+#{n2}"
end

# 读到恰好 n 字节：`IO#read(n)` 会一直阻塞到凑满 n 字节或 EOF，
# 用固定长度的小报文测回环就会死锁（实测 A19 卡死）。readpartial 有数据就返回。
def read_exact(io, n)
  buf = +""
  buf << io.readpartial(n - buf.bytesize) while buf.bytesize < n
  buf
end

def a19
  srv = TCPServer.new("127.0.0.1", 0)
  port = srv.addr[1]
  box = {}

  server = Thread.new do
    conn = srv.accept
    box[:recv] = read_exact(conn, 8)      # "tcp-ping"
    conn.write("tcp-pong")
    conn.close
  end

  cli = TCPSocket.new("127.0.0.1", port)
  cli.write("tcp-ping")
  reply = read_exact(cli, 8)
  cli.close
  server.join
  srv.close

  if box[:recv] != "tcp-ping" || reply != "tcp-pong"
    return "FAIL:recv=#{box[:recv].inspect} reply=#{reply.inspect}"
  end

  "loopback send/recv ok (port #{port})"
end

# ------------------------------------------------------------------ Tier B
def b01
  require "digest"
  got = Digest::SHA256.hexdigest("abc")
  return "FAIL:sha256=#{got}" unless got == SHA256_ABC

  "sha256(abc) ok (Digest::SHA256)"
end

def b02
  require "base64"
  enc = Base64.strict_encode64("abc")
  return "FAIL:b64=#{enc}" unless enc == "YWJj"
  return "FAIL:b64 decode mismatch" unless Base64.strict_decode64(enc) == "abc"

  "encode+decode ok (Base64)"
end

def b03
  require "json"
  obj = { "k" => [1, 2, 3], "n" => "v" }
  back = JSON.parse(JSON.generate(obj))
  return "FAIL:round-trip #{back.inspect}" unless back == obj

  "serialize+parse ok (JSON)"
end

def b04
  return "FAIL:positive match failed" unless "abc-123".match?(/\A[a-z]+-[0-9]{3}\z/)
  return "FAIL:negative match unexpected" if "ABC-123".match?(/\A[a-z]+-[0-9]{3}\z/)

  "match+reject ok (Regexp)"
end

def b05
  require "zlib"
  require "stringio"
  payload = "gzip-payload-" * 8
  io = StringIO.new
  gz = Zlib::GzipWriter.new(io)
  gz.write(payload)
  gz.close
  packed = io.string
  back = Zlib::GzipReader.new(StringIO.new(packed)).read
  return "FAIL:gzip round-trip mismatch" unless back == payload

  "compress+decompress ok (Zlib)"
end

CHECKS = [
  ["A01_argv", method(:a01)],
  ["A02_env", method(:a02)],
  ["A03_streams", method(:a03)],
  ["A04_stdin", method(:a04)],
  ["A05_file_rw", method(:a05)],
  ["A06_file_append", method(:a06)],
  ["A07_file_binary", method(:a07)],
  ["A08_file_stat", method(:a08)],
  ["A09_dir_ops", method(:a09)],
  ["A10_temp_file", method(:a10)],
  ["A11_unicode", method(:a11)],
  ["A12_large_io", method(:a12)],
  ["A13_time", method(:a13)],
  ["A14_random", method(:a14)],
  ["A15_container", method(:a15)],
  ["A16_error", method(:a16)],
  ["A17_subprocess", method(:a17)],
  ["A18_concurrency", method(:a18)],
  ["A19_tcp_loopback", method(:a19)],
  ["B01_sha256", method(:b01)],
  ["B02_base64", method(:b02)],
  ["B03_json", method(:b03)],
  ["B04_regex", method(:b04)],
  ["B05_gzip", method(:b05)]
].freeze

def record(cid, status, detail)
  detail = detail.to_s.gsub(/[\t\r\n]+/, " ")
  detail = detail[0, 160] if detail.length > 160
  case status
  when "PASS" then $npass += 1
  when "FAIL" then $nfail += 1
  else $nskip += 1
  end
  $stdout.write("CAP #{cid} #{status} #{detail}\n")
  $stdout.flush
end

def probe(cid, fn)
  res = begin
    fn.call
  rescue StandardError => e
    record(cid, "FAIL", "uncaught: #{e.class}: #{e.message}")
    return
  end
  res = trimnl(res)
  if (m = res.match(/\ASKIP:\s*(.*)\z/m))
    why = m[1]
    why = "SKIP declared without reason" if why.empty?
    record(cid, "SKIP", why)
  elsif (m = res.match(/\AFAIL:\s*(.*)\z/m))
    why = m[1]
    why = "FAIL declared without reason" if why.empty?
    record(cid, "FAIL", why)
  else
    record(cid, "PASS", res)
  end
end

CHECKS.each { |cid, fn| probe(cid, fn) }

$stdout.write(format("SUMMARY ruby %d %d %d\n", $npass, $nfail, $nskip))
exit($nfail.positive? ? 1 : 0)
