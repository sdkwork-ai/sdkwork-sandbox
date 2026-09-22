#!/usr/bin/env bash
# Perl 探针入口。cygpath 说明见 demos/python/run.sh。
#
# Windows 上的 perl 是 StrawberryPerl（装在 \Strawberry），其 bin 目录只写进**注册表 PATH**，
# 当前 shell 未必继承到，所以显式探测几个常见落点（用环境变量拼，不写死绝对路径）。
# WSL 侧 `command -v perl` 直接命中 /usr/bin/perl。
#
# ⚠️ LC_ALL=C 是必须的，不是保险：StrawberryPerl 解析不了 C.UTF-8，会对**每个**子进程
# 在 stderr 上刷 "Setting locale failed" 警告；探针会自我调用多次，噪声可观。
# Perl 的 Unicode 语义与 locale 无关（探针显式用 binmode/:encoding，不开 `use locale`），所以 C 安全。
set -u

LC_ALL=C
export LC_ALL

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

# 逐个候选**实跑一次**再采纳：MSYS 里存在一个可执行但缺 DLL 的 perl（跑起来 exit 127），
# 只看 `command -v` 会选中它并把探针判成 UNAVAILABLE。
PERL=""
for cand in \
    "$(command -v perl 2>/dev/null)" \
    "/c/Strawberry/perl/bin/perl.exe" \
    "${LOCALAPPDATA:-}/Programs/Strawberry/perl/bin/perl.exe" \
    "${USERPROFILE:-}/Strawberry/perl/bin/perl.exe" \
    "${PROGRAMFILES:-}/Strawberry/perl/bin/perl.exe" ; do
  [ -n "${cand}" ] || continue
  [ -x "${cand}" ] || continue
  if "${cand}" -e 'exit 0' >/dev/null 2>&1; then
    PERL="${cand}"
    break
  fi
done

if [ -z "${PERL}" ]; then
  echo "UNAVAILABLE perl not found (or not executable) on PATH or known StrawberryPerl roots"
  exit 2
fi

exec "${PERL}" "${HERE}/probe.pl" "$@"
