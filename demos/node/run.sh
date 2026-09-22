#!/usr/bin/env bash
# Node.js / JavaScript 探针入口。cygpath 说明见 demos/python/run.sh。
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

if ! command -v node >/dev/null 2>&1; then
  echo "UNAVAILABLE node not on PATH"
  exit 2
fi

exec node "${HERE}/probe.mjs" "$@"
