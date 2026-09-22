#!/usr/bin/env bash
# Ruby 探针入口。cygpath 说明见 demos/python/run.sh。
#
# Windows 上的 ruby 由 winget 装到 \Ruby33-x64（系统级），其 bin 目录只写进注册表 PATH，
# 当前 shell 未必继承到，所以显式探测几个落点（用环境变量拼，不写死绝对路径）。
# WSL 侧 `command -v ruby` 直接命中 /usr/bin/ruby。
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

# 逐个候选**实跑一次**再采纳：只看 command -v 会选中存在但跑不起来的解释器
# （WSL 上 libruby 缺库时正是如此），那会把探针误判成 UNAVAILABLE。
RUBY=""
for cand in \
    "$(command -v ruby 2>/dev/null)" \
    "/c/Ruby33-x64/bin/ruby.exe" \
    "${LOCALAPPDATA:-}/Programs/Ruby/bin/ruby.exe" \
    "/usr/bin/ruby" ; do
  [ -n "${cand}" ] || continue
  [ -x "${cand}" ] || continue
  if "${cand}" -e 'exit 0' >/dev/null 2>&1; then
    RUBY="${cand}"
    break
  fi
done

if [ -z "${RUBY}" ]; then
  echo "UNAVAILABLE ruby not found (or not executable) on PATH or known install roots"
  exit 2
fi

exec "${RUBY}" "${HERE}/probe.rb" "$@"
