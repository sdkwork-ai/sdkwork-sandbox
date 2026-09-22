-- demos 能力探针 —— Lua 实现。协议见 demos/SPEC.md。
--
-- 模式（SPEC §4 的四个 + 一个并发用）：
--   默认                   跑全部检查
--   --echo-child           往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
--   --read-stdin           读一行 stdin，内容为 ping 时回 pong
--   --argv-probe A B       回显实参个数与内容
--   --count-worker <out>   往 <out> 追加 1000 行（A18 并发用；走实参而非环境变量）
--
-- 设计取舍（都在 SPEC 允许范围内）：
--  1) 只用 Lua 5.4 标准库。模式分派**必须先于任何检查**：探针自我调用，派错会指数级派生。
--  2) Lua 的 os.execute/io.popen 都经 shell（POSIX sh / Windows cmd），所以命令要按平台拼引号，
--     环境变量前缀也是两种写法（`K=V cmd` vs `set K=V && cmd`）。
--  3) A09 目录操作：Lua 标准库**没有**任何目录 API（无 mkdir/opendir/readdir/rmdir），
--     只能经 os.execute 调 shell（`mkdir`/`dir`/`rmdir`）。这测的是运行环境的能力，
--     与 awk 探针用 od/wc/cmp 量字节同一性质，detail 里写明走了 shell。
--  4) A18 并发：两个 `--count-worker` 子进程。**join 靠管道 EOF** ——
--     `io.popen(cmd,"r"):read("a")` 会一直读到所有继承了写端的进程退出（实测后台孙进程会继承），
--     所以「读完」就是「两个子进程都结束」。
--  5) A13 / A19 声明 EXEMPT：Lua 标准库无亚秒级挂钟（os.time 整秒、os.clock 是 CPU 时间），
--     也没有任何 socket API。

local IS_WIN = package.config:sub(1, 1) == "\\"

local LUA = os.getenv("DEMO_LUA")
local SELF = os.getenv("DEMO_SELF")

local UNICODE_S = "中文-日本語-한국어-🚀"
local UNICODE_CODEPOINTS = 12
local UNICODE_BYTES = 31
local LARGE_BYTES = 262144
local MAX_DEPTH = 3

local DEPTH = tonumber(os.getenv("DEMO_DEPTH") or "0") or 0

local npass, nfail, nskip = 0, 0, 0

-- ---------------------------------------------------------------- 进程外工具
local function q(s)
  if IS_WIN then
    return '"' .. s .. '"'
  end
  return "'" .. s:gsub("'", "'\\''") .. "'"
end

-- 目录操作/引号以外的环境变量前缀：两种 shell 两种写法。
local function envprefix(vars)
  local keys = {}
  for k in pairs(vars) do keys[#keys + 1] = k end
  if #keys == 0 then return "" end
  table.sort(keys)
  local parts = {}
  for _, k in ipairs(keys) do parts[#parts + 1] = k .. "=" .. tostring(vars[k]) end
  if IS_WIN then
    return "set " .. table.concat(parts, " && set ") .. " && "
  end
  return table.concat(parts, " ") .. " "
end

local function readfile(path)
  local f = io.open(path, "rb")
  if not f then return "" end
  local d = f:read("a")
  f:close()
  return d or ""
end

local function trimnl(s)
  return (tostring(s):gsub("[\r\n]+$", ""))
end

local function exists(path)
  local f = io.open(path, "rb")
  if f then f:close() return true end
  return false
end

-- 启动自身的子进程；返回 退出码, stdout, stderr。
--
-- ⚠️ Windows 上命令**绝不能以引号开头**：Lua 的 os.execute 走 MSVC system()，
-- 它转成 `cmd.exe /c <命令>`，而 cmd 的规则是「整行以 `"` 开头就剥掉首尾两个引号」，
-- 于是 `"lua.exe" "probe.lua"` 会被拆坏，报「文件名、目录名或卷标语法不正确」（实测踩过）。
-- 所以这里**永远**带 DEMO_DEPTH 前缀（`set X=Y && ` / `X=Y `），既修引号问题又守住递归护栏。
local OUTP, ERRP = "self.out", "self.err"

local function run_self(argstr, vars, stdin_text)
  local env = { DEMO_DEPTH = DEPTH + 1 }
  for k, v in pairs(vars or {}) do env[k] = v end

  local prefix = envprefix(env)
  if prefix == "" then
    -- 兜底：没有前缀时用一个恒真且无副作用的命令占位，避免命令以引号开头
    prefix = IS_WIN and "ver >nul && " or ": "
  end

  local cmd = prefix .. q(LUA) .. " " .. q(SELF)
  if argstr and argstr ~= "" then cmd = cmd .. " " .. argstr end
  cmd = cmd .. " >" .. q(OUTP) .. " 2>" .. q(ERRP)
  if stdin_text then
    local sf = io.open("self.stdin", "wb")
    sf:write(stdin_text)
    sf:close()
    cmd = cmd .. " <" .. q("self.stdin")
  end
  os.remove(OUTP)
  os.remove(ERRP)
  -- DEMO_CMD_DEBUG=1 时把拼出的命令行回显到 stderr（排查 shell 引号问题用）
  if os.getenv("DEMO_CMD_DEBUG") == "1" then
    io.stderr:write("CMD = " .. cmd .. "\n")
  end
  local code = select(3, os.execute(cmd))
  return (code or -1), readfile(OUTP), readfile(ERRP)
end

local function countlines(path)
  local d = readfile(path)
  local n = 0
  for _ in d:gmatch("\n") do n = n + 1 end
  return n
end

-- ---------------------------------------------------------------- 主流程
if DEPTH > MAX_DEPTH then
  io.stderr:write("FATAL: probe recursion depth " .. DEPTH .. " exceeded\n")
  os.exit(3)
end

local mode = arg[1] or ""

-- ⚠️ 模式分派必须先于任何检查执行。
if mode == "--echo-child" then
  io.write("child-ok\n")
  io.stderr:write("child-err\n")
  os.exit(0)
end

if mode == "--read-stdin" then
  local line = io.read("l") or ""
  line = trimnl(line)
  io.write(line == "ping" and "pong\n" or ("unexpected:" .. line .. "\n"))
  os.exit(0)
end

if mode == "--argv-probe" then
  local rest = {}
  for i = 2, #arg do rest[#rest + 1] = arg[i] end
  io.write("argv-probe:" .. #rest .. ":" .. table.concat(rest, ":") .. "\n")
  os.exit(0)
end

if mode == "--count-worker" then
  local out = arg[2]
  if not out then os.exit(4) end
  local fh = io.open(out, "ab")
  if not fh then os.exit(5) end
  local rows = {}
  for _ = 1, 1000 do rows[#rows + 1] = "x\n" end
  fh:write(table.concat(rows))
  fh:close()
  os.exit(0)
end

-- ------------------------------------------------------------------ Tier A
local function a01()
  local _, out = run_self("--argv-probe alpha beta")
  local got = trimnl(out)
  if got ~= "argv-probe:2:alpha:beta" then
    return "FAIL:argv 回显不符: " .. got
  end
  return "2 args round-tripped"
end

local function a02()
  local v = os.getenv("DEMO_LANG_TAG") or ""
  if v ~= "demos-capability" then return "FAIL:DEMO_LANG_TAG=" .. v end
  return "env visible"
end

local function a03()
  local _, out, err = run_self("--echo-child")
  local o = trimnl(out)
  if o ~= "child-ok" then return "FAIL:stdout=" .. o end
  if not err:find("child-err", 1, true) then return "FAIL:stderr missing child-err" end
  if o:find("child-err", 1, true) then return "FAIL:stderr leaked into stdout" end
  return "stdout/stderr separated"
end

local function a04()
  local _, out = run_self("--read-stdin", nil, "ping\n")
  local s = trimnl(out)
  if s ~= "pong" then return "FAIL:reply=" .. s end
  return "ping->pong"
end

local function a05()
  local fh = io.open("a05.txt", "wb")
  if not fh then return "FAIL:open" end
  fh:write("hello-io")
  fh:close()
  local got = readfile("a05.txt")
  if got ~= "hello-io" then return "FAIL:read back " .. got end
  return "text round-trip ok"
end

local function a06()
  -- 不带换行：SPEC 与其他语言都按 "ab"（无换行）对齐。
  local fh = io.open("a06.txt", "wb")
  if not fh then return "FAIL:open" end
  fh:write("a")
  fh:close()
  fh = io.open("a06.txt", "ab")
  if not fh then return "FAIL:reopen" end
  fh:write("b")
  fh:close()
  local got = readfile("a06.txt")
  if got ~= "ab" then return "FAIL:append 结果 " .. got end
  return "append ok"
end

local function a07()
  local t = {}
  for i = 0, 255 do t[#t + 1] = string.char(i) end
  local payload = table.concat(t)
  local fh = io.open("a07.bin", "wb")
  if not fh then return "FAIL:open" end
  fh:write(payload)
  fh:close()
  local back = readfile("a07.bin")
  if #back ~= 256 or back ~= payload then
    return "FAIL:" .. #back .. " bytes, round-trip mismatch"
  end
  return "256 bytes incl 0x00/0xFF"
end

local function a08()
  local fh = io.open("a08.txt", "wb")
  if not fh then return "FAIL:open" end
  fh:write("statted")          -- 7 字节，不带换行
  fh:close()
  if not exists("a08.txt") then return "FAIL:file not created" end
  local rf = io.open("a08.txt", "rb")
  local size = rf:seek("end")
  rf:close()
  if size ~= 7 then return "FAIL:size=" .. tostring(size) end
  os.remove("a08.txt")
  if exists("a08.txt") then return "FAIL:remove failed" end
  return "size=7 then removed"
end

local function a09()
  local sub = IS_WIN and "a09dir\\inner.txt" or "a09dir/inner.txt"
  -- 预清：只跑本平台的命令，并吞掉"不存在"之类的噪声，避免污染 runner 捕获的 stderr。
  os.execute(IS_WIN and "rmdir /s /q a09dir 2>nul" or "rm -rf a09dir 2>/dev/null")
  if select(3, os.execute("mkdir a09dir")) ~= 0 then return "FAIL:mkdir failed" end
  local fh = io.open(sub, "wb")
  if not fh then return "FAIL:create inner" end
  fh:write("x")
  fh:close()

  local listcmd = IS_WIN and "dir /b a09dir" or "ls a09dir"
  local ph = io.popen(listcmd, "r")
  local listing = ph:read("a") or ""
  ph:close()
  if not listing:find("inner.txt", 1, true) then
    return "FAIL:listing missing inner.txt"
  end

  -- 用标准库删文件（os.remove 接受反斜杠路径），再交给 shell 删空目录。
  os.remove(sub)
  if select(3, os.execute("rmdir a09dir")) ~= 0 then return "FAIL:rmdir failed" end
  return "mkdir/list/rmdir ok (via os.execute shell)"
end

local function a10()
  local path = os.tmpname()
  if path == nil or path == "" then return "FAIL:tmpname empty" end
  if exists(path) then os.remove(path) end
  local fh = io.open(path, "wb")
  if not fh then return "FAIL:open temp " .. path end
  fh:write("temp-content")
  fh:close()
  local got = readfile(path)
  os.remove(path)
  if got ~= "temp-content" then return "FAIL:temp read " .. got end
  return "unique temp file (os.tmpname)"
end

local function a11()
  local utf8lib = require("utf8")
  local cps = utf8lib.len(UNICODE_S)
  if cps ~= UNICODE_CODEPOINTS then return "FAIL:codepoints=" .. tostring(cps) end
  local nbytes = #UNICODE_S
  if nbytes ~= UNICODE_BYTES then return "FAIL:bytes=" .. nbytes end

  local fh = io.open("a11.txt", "wb")
  if not fh then return "FAIL:open" end
  fh:write(UNICODE_S)
  fh:close()
  local raw = readfile("a11.txt")
  if #raw ~= UNICODE_BYTES then return "FAIL:onfile=" .. #raw end
  if raw ~= UNICODE_S then return "FAIL:round-trip mismatch" end
  if utf8lib.len(raw) ~= UNICODE_CODEPOINTS then
    return "FAIL:reread codepoints=" .. tostring(utf8lib.len(raw))
  end
  return cps .. " codepoints / " .. nbytes .. " bytes"
end

local function a12()
  if LARGE_BYTES ~= 262144 then return "FAIL:constant drift" end
  local chunk = ("abcdefgh"):rep(1024)      -- 8192
  local payload = chunk:rep(32)             -- 262144
  local fh = io.open("a12.bin", "wb")
  if not fh then return "FAIL:open" end
  fh:write(payload)
  fh:close()
  local got = readfile("a12.bin")
  if #got ~= LARGE_BYTES or got ~= payload then
    return "FAIL:len=" .. #got
  end
  return LARGE_BYTES .. " bytes ok"
end

local function a14()
  local v = math.random(0, 999)
  if v < 0 or v >= 1000 then return "FAIL:out of range " .. v end
  return "in [0,1000) (math.random)"
end

local function a15()
  local data = { 5, 3, 9, 1, 7, 3 }
  table.sort(data)
  local joined = table.concat(data, ",")
  if joined ~= "1,3,3,5,7,9" then return "FAIL:sorted=" .. joined end

  local m = { a = 1 }
  m.b = 2
  if m.a ~= 1 or m.b ~= 2 then return "FAIL:table map broken" end
  local n = 0
  for _ in pairs(m) do n = n + 1 end
  if n ~= 2 then return "FAIL:table len=" .. n end
  m.a = nil
  n = 0
  for _ in pairs(m) do n = n + 1 end
  if n ~= 1 then return "FAIL:table delete" end
  return "table.sort + table map insert/delete ok"
end

local function a16()
  local ok, err = pcall(function()
    local fh = assert(io.open("definitely-missing-file-xyz", "rb"))
    fh:close()
  end)
  if ok then return "FAIL:no error raised for missing file" end
  if err == nil then return "FAIL:no error object" end
  return "pcall/assert caught missing-file error"
end

local function a17()
  local rc, out = run_self("--echo-child")
  if rc ~= 0 then return "FAIL:child exit=" .. rc end
  local s = trimnl(out)
  if s ~= "child-ok" then return "FAIL:child stdout=" .. s end
  return "child exit=0, stdout captured"
end

local function a18()
  os.remove("a18.1.txt")
  os.remove("a18.2.txt")
  local c1 = q(LUA) .. " " .. q(SELF) .. " --count-worker " .. q("a18.1.txt")
  local c2 = q(LUA) .. " " .. q(SELF) .. " --count-worker " .. q("a18.2.txt")

  local cmd
  if IS_WIN then
    -- `set` 与两个 `start /b` 在同一个 cmd 进程里 ⇒ 两个孙进程都继承 DEMO_DEPTH。
    cmd = "set DEMO_DEPTH=" .. (DEPTH + 1) .. " && start /b \"\" " .. c1
        .. " & start /b \"\" " .. c2
  else
    local pre = "DEMO_DEPTH=" .. (DEPTH + 1) .. " "
    cmd = "(" .. pre .. c1 .. ") & (" .. pre .. c2 .. ") & wait"
  end

  local ph = io.popen(cmd, "r")
  if not ph then return "FAIL:popen" end
  ph:read("a")   -- 读到 EOF = 两个后台子进程都退出了（它们继承了管道写端）
  ph:close()

  local n1 = countlines("a18.1.txt")
  local n2 = countlines("a18.2.txt")
  if n1 + n2 ~= 2000 then return "FAIL:total=" .. (n1 + n2) end
  return "2 concurrent workers joined -> " .. n1 .. "+" .. n2
end

-- ------------------------------------------------------------------ Tier B
local function b01()
  return "SKIP:Lua 标准库无 SHA-256（须 openssl 或 luarocks 第三方模块）"
end

local function b02()
  return "SKIP:Lua 标准库无 Base64（须第三方模块或自写编码表）"
end

local function b03()
  return "SKIP:Lua 标准库无 JSON（须 dkjson/cjson 等第三方模块）"
end

local function b04()
  -- Lua 的 string.match 用的是 Lua 模式串（pattern），不是正则：无 {n} 重复量词。
  -- 这里用等价的 %l / %d 写法测同一个谓词，detail 里写明机制差异。
  if not ("abc-123"):match("^%l+%-%d%d%d$") then return "FAIL:positive match failed" end
  if ("ABC-123"):match("^%l+%-%d%d%d$") then return "FAIL:negative match unexpected" end
  return "match+reject ok (Lua patterns；无 {n} 量词)"
end

local function b05()
  return "SKIP:Lua 标准库无 gzip/deflate（须 zlib/lzlib 等第三方模块）"
end

local CHECKS = {
  { "A01_argv", a01 },
  { "A02_env", a02 },
  { "A03_streams", a03 },
  { "A04_stdin", a04 },
  { "A05_file_rw", a05 },
  { "A06_file_append", a06 },
  { "A07_file_binary", a07 },
  { "A08_file_stat", a08 },
  { "A09_dir_ops", a09 },
  { "A10_temp_file", a10 },
  { "A11_unicode", a11 },
  { "A12_large_io", a12 },
  { "A14_random", a14 },
  { "A15_container", a15 },
  { "A16_error", a16 },
  { "A17_subprocess", a17 },
  { "A18_concurrency", a18 },
  { "B01_sha256", b01 },
  { "B02_base64", b02 },
  { "B03_json", b03 },
  { "B04_regex", b04 },
  { "B05_gzip", b05 },
}

local function record(cid, status, detail)
  detail = tostring(detail or ""):gsub("[\t\r\n]+", " ")
  if #detail > 160 then detail = detail:sub(1, 160) end
  if status == "PASS" then npass = npass + 1
  elseif status == "FAIL" then nfail = nfail + 1
  else nskip = nskip + 1 end
  io.write("CAP ", cid, " ", status, " ", detail, "\n")
end

local function probe(cid, fn)
  local ok, res = pcall(fn)
  if not ok then
    return record(cid, "FAIL", "uncaught: " .. tostring(res))
  end
  res = trimnl(res or "")
  local skip = res:match("^SKIP:%s*(.*)$")
  if skip then
    if skip == "" then skip = "SKIP declared without reason" end
    return record(cid, "SKIP", skip)
  end
  local fail = res:match("^FAIL:%s*(.*)$")
  if fail then
    if fail == "" then fail = "FAIL declared without reason" end
    return record(cid, "FAIL", fail)
  end
  return record(cid, "PASS", res)
end

for _, c in ipairs(CHECKS) do
  probe(c[1], c[2])
end

-- 两项 Tier A 的 EXEMPT（不计入 pass/fail/skip；runner 按 ^EXEMPT 单独解析）
io.write("EXEMPT A13_time ", "Lua 标准库无亚秒挂钟：os.time 只到整秒，os.clock 是 CPU 时间不是墙钟\n")
io.write("EXEMPT A19_tcp_loopback ", "Lua 标准库无任何 socket API（须 luasocket 等第三方模块）\n")

io.write(("SUMMARY lua %d %d %d\n"):format(npass, nfail, nskip))
os.exit(nfail > 0 and 1 or 0)
