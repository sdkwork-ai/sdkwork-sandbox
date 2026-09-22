#!/usr/bin/env bash
# Java 探针入口。cygpath 说明见 demos/python/run.sh。
#
# javac 在两个宿主上的位置不同：
#   - Windows：Oracle JDK 装在 "Common Files/Oracle/Java/javapath"，已在 PATH 上；
#   - WSL：JDK 在 /opt/software/java/current，**只有登录 shell 才在 PATH 上**，
#          所以这里显式探测，避免 runner 的非登录 shell 把 java 判成"没有"。
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
PWD_POSIX="$(pwd)"
PWD_NATIVE="${PWD_POSIX}"
if command -v cygpath >/dev/null 2>&1; then
  HERE="$(cygpath -w "${HERE_POSIX}")"
  PWD_NATIVE="$(cygpath -w "${PWD_POSIX}")"
fi

JAVAC=""
JAVA=""
if command -v javac >/dev/null 2>&1; then
  JAVAC="$(command -v javac)"
  # ⚠️ **必须**从 javac 自己的 JDK 根推出 java，不能各取 PATH 上的第一个。
  # 否则会出现「javac 21 编译 / java 11 运行」的错配：class file version 65.0
  # 在 java 11 上直接 UnsupportedClassVersionError，探针一行 CAP 都发不出来
  # （WSL 上实测踩过：javac 来自 /opt/software/java/current，java 却来自
  #  /usr/lib/jvm/default-java，两者差了两个大版本）。
  JH="$(dirname "$(dirname "${JAVAC}")")"
  if [ -x "${JH}/bin/java" ]; then
    JAVA="${JH}/bin/java"
  else
    JAVA="$(command -v java 2>/dev/null)"
  fi
fi

if [ -z "${JAVAC}" ] || [ -z "${JAVA}" ]; then
  for jh in "${JAVA_HOME:-}" /opt/software/java/current /usr/lib/jvm/default-java; do
    [ -n "${jh}" ] || continue
    if [ -x "${jh}/bin/javac" ] && [ -x "${jh}/bin/java" ]; then
      JAVAC="${jh}/bin/javac"
      JAVA="${jh}/bin/java"
      break
    fi
  done
fi

if [ -z "${JAVAC}" ] || [ -z "${JAVA}" ]; then
  echo "UNAVAILABLE java/javac not found on PATH or JAVA_HOME"
  exit 2
fi

# 把选中的 java/javac 版本打到 stderr（runner 不解析，但排查错配时一眼可见）。
echo "java : ${JAVA} ($( "${JAVA}" -version 2>&1 | head -1 ))" >&2
echo "javac: ${JAVAC} ($( "${JAVAC}" -version 2>&1 | head -1 ))" >&2

BUILD="${PWD_POSIX}/_javabuild"
mkdir -p "${BUILD}"
cp "${HERE}/Probe.java" "${BUILD}/Probe.java"

# 编译失败 = 探针自己的问题，不是"本机没有 JDK"（见 demos/csharp/run.sh 的同款说明）。
if ! ( cd "${BUILD}" && "${JAVAC}" -encoding UTF-8 -d . Probe.java ) >"${PWD_POSIX}/_javac.log" 2>&1 ; then
  echo "java: javac failed (see _javac.log)" >&2
  tail -25 "${PWD_POSIX}/_javac.log" >&2
  exit 1
fi

# 编码属性写全：`file.encoding`（JEP 400 起只管 defaultCharset）与
# `stdout/stderr.encoding`（决定 System.out 的编码）是两回事。
# 探针内部还会把 System.out 换成显式 UTF-8 的 PrintStream；这里再钉一层。
exec "${JAVA}" \
  -Dfile.encoding=UTF-8 \
  -Dstdout.encoding=UTF-8 \
  -Dstderr.encoding=UTF-8 \
  -Dsun.stdout.encoding=UTF-8 \
  -Dsun.stderr.encoding=UTF-8 \
  -cp "${PWD_NATIVE}/_javabuild" Probe "$@"
