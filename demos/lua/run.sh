#!/usr/bin/env bash
# Lua 探针入口。cygpath 说明见 demos/python/run.sh。
#
# Windows 上的 lua 由 winget（DEVCOM.Lua）装到**用户级 Programs** 目录，同 php 的情况：
# 只写进注册表 PATH，当前 shell 未必继承到，所以显式探测几个落点。
# WSL 侧是 lua5.4（系统包），`command -v lua5.4` 直接命中。
#
# 探针需要 Lua 5.4 的 `utf8` 标准库与 os.execute 的三返回值（5.2+），所以拒绝 5.1/5.2-lua5.3 之外的老版本。
set -u

HERE_POSIX="$(cd "$(dirname "$0")" && pwd)"
HERE="${HERE_POSIX}"
# 工作目录必须钉在 <repo>/target/demos-work/<lang>（SPEC §6）：探针的临时产物
# 绝不落进仓库。runner 本来就会 cd 到这里，但**直接 `bash demos/<lang>/run.sh`**
# 时不会 —— 实测直接跑会把 a05.txt / a07.bin / self.out 等生成物拉在 demos/<lang>/ 里。
DEMO_ROOT="$(cd "${HERE_POSIX}/../.." && pwd)"
DEMO_WORK="${DEMO_ROOT}/target/demos-work/$(basename "${HERE_POSIX}")"
mkdir -p "${DEMO_WORK}"
cd "${DEMO_WORK}"
if command -v cygpath >/dev/null 2>&1; then
  HERE="$(cygpath -w "${HERE_POSIX}")"
fi

LUA=""
for cand in \
    "$(command -v lua 2>/dev/null)" \
    "$(command -v lua5.4 2>/dev/null)" \
    "$(command -v lua54 2>/dev/null)" \
    "$(command -v lua5.3 2>/dev/null)" \
    "${LOCALAPPDATA:-}/Programs/Lua/bin/lua.exe" \
    "${USERPROFILE:-}/AppData/Local/Programs/Lua/bin/lua.exe" ; do
  [ -n "${cand}" ] || continue
  [ -x "${cand}" ] || continue
  # 必须真能跑，且版本 >= 5.3（探针用 utf8 库与 os.exit 的整数返回）
  ver="$("${cand}" -e 'io.write(_VERSION)' 2>/dev/null)"
  case "${ver}" in
    Lua\ 5.[345]*) LUA="${cand}"; break ;;
  esac
done

if [ -z "${LUA}" ]; then
  echo "UNAVAILABLE lua >= 5.3 not found on PATH or known install roots"
  exit 2
fi

DEMO_LUA="${LUA}"
DEMO_SELF="${HERE}/probe.lua"
export DEMO_LUA DEMO_SELF

exec "${LUA}" "${HERE}/probe.lua" "$@"
