#!/usr/bin/env bash
# PHP 探针入口。cygpath 说明见 demos/python/run.sh。
#
# Windows 上的 php 由 winget 装到用户级 Packages 目录，其 bin 目录**只写进注册表 PATH**，
# 当前 shell 未必继承到，所以这里显式探测几个常见落点（用环境变量拼，不写死绝对路径）。
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

PHP=""
if command -v php >/dev/null 2>&1; then
  PHP=php
else
  # winget 的 Links 目录（shim）与 Packages 目录；PATH 里的分号分隔项也扫一遍。
  for dir in \
      "${LOCALAPPDATA:-}/Microsoft/WinGet/Links" \
      "${USERPROFILE:-}/AppData/Local/Microsoft/WinGet/Links" ; do
    [ -n "${dir}" ] || continue
    if [ -x "${dir}/php.exe" ]; then PHP="${dir}/php.exe"; break; fi
  done
  if [ -z "${PHP}" ] && [ -n "${LOCALAPPDATA:-}" ]; then
    found="$(ls -d "${LOCALAPPDATA}"/Microsoft/WinGet/Packages/PHP.PHP.* 2>/dev/null | head -1)"
    if [ -n "${found}" ] && [ -x "${found}/php.exe" ]; then
      PHP="${found}/php.exe"
    fi
  fi
fi

if [ -z "${PHP}" ]; then
  echo "UNAVAILABLE php not found on PATH or winget package dirs"
  exit 2
fi

exec "${PHP}" "${HERE}/probe.php" "$@"
