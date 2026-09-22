#!/usr/bin/env bash
# Bash 探针入口。
#
# 不需要 cygpath：Cygwin/MSYS 的 bash 与 Linux 的 bash 都认 POSIX 路径，
# 探针里所有"自我调用"都由 bash 直接执行。
set -u

HERE_POSIX="$(cd "$(dirname "$0")" && pwd)"

# 工作目录必须钉在 <repo>/target/demos-work/<lang>（SPEC §6）：探针的临时产物
# 绝不落进仓库。runner 本来就会 cd 到这里，但**直接 `bash demos/<lang>/run.sh`**
# 时不会 —— 实测直接跑会把 a05.txt / a07.bin / self.out 等生成物拉在 demos/<lang>/ 里。
DEMO_ROOT="$(cd "${HERE_POSIX}/../.." && pwd)"
DEMO_WORK="${DEMO_ROOT}/target/demos-work/$(basename "${HERE_POSIX}")"
mkdir -p "${DEMO_WORK}"
cd "${DEMO_WORK}"

if ! command -v bash >/dev/null 2>&1; then
  echo "UNAVAILABLE bash not on PATH"
  exit 2
fi

# 显式给出自身路径：探针靠它自我调用（argv/stdin/subprocess/并发检查）。
DEMO_SELF="${HERE_POSIX}/probe.sh"
export DEMO_SELF

exec bash "${HERE_POSIX}/probe.sh" "$@"
