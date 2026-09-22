#!/usr/bin/env bash
# Python 探针入口。
#
# runner 会 cd 到 target/demos-work/python 再调本脚本，所以探针里的相对路径 IO
# 都落在临时工作目录，不会污染仓库。
#
# 关于 cygpath：Windows 上的解释器（python3.exe 等）**看不懂** MSYS 的 `/d/...`
# 路径，而且 MSYS 还会在传参时把它改写成并不存在的形式（多套一层盘符目录）。
# cygpath -w 负责把它转成 Windows 原生路径（前导 `/d` 换成盘符加反斜杠）；
# Linux 上没有 cygpath，保持 POSIX 路径即可。
#
# 注意：这段说明**不要**写出盘符绝对路径的字面量（例如盘符 + 冒号 + 斜杠的完整形式），
# check-workspace-path-portability 会把反斜杠归一后当成机器绝对路径报 MACHINE-ABS。
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

if command -v python3 >/dev/null 2>&1; then
  PY=python3
elif command -v python >/dev/null 2>&1; then
  PY=python
else
  echo "UNAVAILABLE python3/python not on PATH"
  exit 2
fi

exec "${PY}" "${HERE}/probe.py" "$@"
