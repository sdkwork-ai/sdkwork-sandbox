#!/usr/bin/env python3
"""demos 能力探针 —— Python 参考实现。

按 demos/SPEC.md 的协议输出 `CAP <id> <PASS|FAIL|SKIP> <detail>`，
末尾一行 `SUMMARY python <pass> <fail> <skip>`；无 FAIL 时 exit 0。

模式：
  默认              跑全部检查
  --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
  --read-stdin      读一行 stdin，内容为 ping 时回 pong
  --argv-probe A B  回显 argv（argv-probe:2:A:B）
"""
import base64
import gzip
import hashlib
import json
import os
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time

SELF = os.path.abspath(__file__)
PY = sys.executable or "python3"

# 递归护栏：探针会自我调用，一旦模式分派写错就会指数级派生进程。
# 实测过一次 4 路递归爆到 8584 个进程，所以这里硬性封顶。
DEPTH = int(os.environ.get("DEMO_DEPTH", "0") or "0")
MAX_DEPTH = 3

UNICODE_S = "中文-日本語-한국어-🚀"
UNICODE_CHARS = 12
UNICODE_BYTES = 31
LARGE_BYTES = 262144
SHA256_ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

results = []


def record(cid, status, detail=""):
    # detail 必须单行、无制表符，否则会破坏 runner 的 TSV 解析
    clean = str(detail).replace("\t", " ").replace("\r", " ").replace("\n", " ")
    if len(clean) > 160:
        clean = clean[:160]
    results.append((cid, status, clean))
    # 立刻落盘：这样 runner 能流式看到进度，卡住时也能一眼看出卡在哪一项
    print("CAP %s %s %s" % (cid, status, clean), flush=True)


def check(cid):
    """注册一个检查。

    ⚠️ 装饰器**只能注册**，绝不能在装饰期调用被装饰函数。早期版本在这里直接
    调用 `fn()`，于是"检查"在**模块导入时**就开跑——比 main() 里的模式分派还早。
    后果是子进程模式失效、每个子进程都跑全套检查并再派生 4 个子进程，形成
    4 路指数递归（实测爆到 8584 个 python3 进程）。模式分派必须先于任何检查执行。
    """
    def wrap(fn):
        def runner():
            try:
                detail = fn()
                record(cid, "PASS", detail if detail else "")
            except SkipCheck as exc:
                record(cid, "SKIP", str(exc))
            except Exception as exc:  # noqa: BLE001 - 探针就是要兜住一切
                record(cid, "FAIL", "%s: %s" % (type(exc).__name__, exc))
        return runner
    return wrap


class SkipCheck(Exception):
    pass


def spawn(args, stdin_data=None):
    child_env = dict(os.environ)
    child_env["DEMO_DEPTH"] = str(DEPTH + 1)
    return subprocess.run(
        [PY, SELF] + args,
        input=stdin_data,
        capture_output=True,
        text=True,
        timeout=60,
        env=child_env,
    )


# --------------------------------------------------------------------- 子进程模式
def handle_modes(argv):
    if len(argv) >= 1 and argv[0] == "--echo-child":
        sys.stdout.write("child-ok\n")
        sys.stderr.write("child-err\n")
        return 0
    if len(argv) >= 1 and argv[0] == "--read-stdin":
        line = sys.stdin.readline().strip()
        sys.stdout.write("pong\n" if line == "ping" else "unexpected:%s\n" % line)
        return 0
    if len(argv) >= 1 and argv[0] == "--argv-probe":
        rest = argv[1:]
        sys.stdout.write("argv-probe:%d:%s\n" % (len(rest), ":".join(rest)))
        return 0
    return None


# ------------------------------------------------------------------------ Tier A
@check("A01_argv")
def a01():
    r = spawn(["--argv-probe", "alpha", "beta"])
    got = r.stdout.strip()
    if got != "argv-probe:2:alpha:beta":
        raise AssertionError("argv 回显不符: %r" % got)
    return "2 args round-tripped"


@check("A02_env")
def a02():
    v = os.environ.get("DEMO_LANG_TAG", "")
    if v != "demos-capability":
        raise AssertionError("DEMO_LANG_TAG=%r" % v)
    return "env visible"


@check("A03_streams")
def a03():
    r = spawn(["--echo-child"])
    if r.stdout.strip() != "child-ok":
        raise AssertionError("stdout=%r" % r.stdout)
    if "child-err" not in r.stderr:
        raise AssertionError("stderr=%r" % r.stderr)
    if "child-err" in r.stdout:
        raise AssertionError("stderr leaked into stdout")
    return "stdout/stderr separated"


@check("A04_stdin")
def a04():
    r = spawn(["--read-stdin"], stdin_data="ping\n")
    if r.stdout.strip() != "pong":
        raise AssertionError("reply=%r" % r.stdout)
    return "ping->pong"


@check("A05_file_rw")
def a05():
    name = "a05.txt"
    with open(name, "w", encoding="utf-8") as fh:
        fh.write("hello-io")
    with open(name, "r", encoding="utf-8") as fh:
        got = fh.read()
    if got != "hello-io":
        raise AssertionError("read back %r" % got)
    return "text round-trip ok"


@check("A06_file_append")
def a06():
    name = "a06.txt"
    with open(name, "w", encoding="utf-8") as fh:
        fh.write("a")
    with open(name, "a", encoding="utf-8") as fh:
        fh.write("b")
    with open(name, "r", encoding="utf-8") as fh:
        got = fh.read()
    if got != "ab":
        raise AssertionError("append 结果 %r" % got)
    return "append ok"


@check("A07_file_binary")
def a07():
    name = "a07.bin"
    payload = bytes(range(256))
    with open(name, "wb") as fh:
        fh.write(payload)
    with open(name, "rb") as fh:
        got = fh.read()
    if got != payload:
        raise AssertionError("二进制往返不一致 len=%d" % len(got))
    return "256 bytes incl 0x00/0xFF"


@check("A08_file_stat")
def a08():
    name = "a08.txt"
    with open(name, "w", encoding="utf-8") as fh:
        fh.write("statted")
    if not os.path.exists(name):
        raise AssertionError("file not created")
    size = os.path.getsize(name)
    if size != 7:
        raise AssertionError("size=%d" % size)
    os.remove(name)
    if os.path.exists(name):
        raise AssertionError("remove failed")
    return "size=7 then removed"


@check("A09_dir_ops")
def a09():
    d = "a09dir"
    if os.path.isdir(d):
        import shutil
        shutil.rmtree(d)
    os.mkdir(d)
    with open(os.path.join(d, "inner.txt"), "w", encoding="utf-8") as fh:
        fh.write("x")
    entries = os.listdir(d)
    if "inner.txt" not in entries:
        raise AssertionError("listing=%r" % entries)
    os.remove(os.path.join(d, "inner.txt"))
    os.rmdir(d)
    if os.path.isdir(d):
        raise AssertionError("rmdir failed")
    return "mkdir/list/rmdir ok"


@check("A10_temp_file")
def a10():
    fd, path = tempfile.mkstemp(prefix="demos-", suffix=".tmp", dir=".")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write("temp-content")
        with open(path, "r", encoding="utf-8") as fh:
            got = fh.read()
        if got != "temp-content":
            raise AssertionError("temp read %r" % got)
    finally:
        if os.path.exists(path):
            os.remove(path)
    return "unique temp file"


@check("A11_unicode")
def a11():
    if len(UNICODE_S) != UNICODE_CHARS:
        raise AssertionError("codepoints=%d" % len(UNICODE_S))
    raw = UNICODE_S.encode("utf-8")
    if len(raw) != UNICODE_BYTES:
        raise AssertionError("bytes=%d" % len(raw))
    name = "a11.txt"
    with open(name, "w", encoding="utf-8") as fh:
        fh.write(UNICODE_S)
    with open(name, "r", encoding="utf-8") as fh:
        got = fh.read()
    if got != UNICODE_S:
        raise AssertionError("round-trip mismatch")
    return "%d codepoints / %d bytes" % (UNICODE_CHARS, UNICODE_BYTES)


@check("A12_large_io")
def a12():
    if LARGE_BYTES != 262144:
        raise AssertionError("constant drift")
    payload = (b"abcdefgh" * (LARGE_BYTES // 8))
    name = "a12.bin"
    with open(name, "wb") as fh:
        fh.write(payload)
    with open(name, "rb") as fh:
        got = fh.read()
    if len(got) != LARGE_BYTES or got != payload:
        raise AssertionError("len=%d" % len(got))
    return "262144 bytes ok"


@check("A13_time")
def a13():
    now_ms = int(time.time() * 1000)
    if now_ms < 1577836800000:
        raise AssertionError("epoch=%d" % now_ms)
    t0 = time.monotonic()
    time.sleep(0.05)
    delta_ms = (time.monotonic() - t0) * 1000.0
    if delta_ms < 40:
        raise AssertionError("sleep 只测到 %.1f ms" % delta_ms)
    return "sleep %.0fms" % delta_ms


@check("A14_random")
def a14():
    import random
    v = random.randrange(0, 1000)
    if not 0 <= v < 1000:
        raise AssertionError("out of range %d" % v)
    return "in [0,1000)"


@check("A15_container")
def a15():
    data = [5, 3, 9, 1, 7, 3]
    data.sort()
    if data != [1, 3, 3, 5, 7, 9]:
        raise AssertionError("sorted=%r" % data)
    m = {"a": 1}
    m["b"] = 2
    if m.get("a") != 1 or m.get("b") != 2 or len(m) != 2:
        raise AssertionError("map=%r" % m)
    return "sort + map ok"


@check("A16_error")
def a16():
    caught = False
    try:
        with open("definitely-missing-file-xyz", "r", encoding="utf-8") as fh:
            fh.read()
    except OSError:
        caught = True
    if not caught:
        raise AssertionError("no error raised for missing file")
    return "OSError caught"


@check("A17_subprocess")
def a17():
    r = spawn(["--echo-child"])
    if r.returncode != 0:
        raise AssertionError("child exit=%d" % r.returncode)
    if r.stdout.strip() != "child-ok":
        raise AssertionError("child stdout=%r" % r.stdout)
    return "child exit=0, stdout captured"


@check("A18_concurrency")
def a18():
    total = [0]
    lock = threading.Lock()

    def worker():
        for _ in range(1000):
            with lock:
                total[0] += 1

    threads = [threading.Thread(target=worker) for _ in range(2)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    if total[0] != 2000:
        raise AssertionError("total=%d" % total[0])
    return "2 threads -> 2000"


@check("A19_tcp_loopback")
def a19():
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(1)
    port = srv.getsockname()[1]
    box = {}

    def server():
        conn, _ = srv.accept()
        data = conn.recv(64)
        box["recv"] = data
        conn.sendall(b"tcp-pong")
        conn.close()

    t = threading.Thread(target=server)
    t.start()
    cli = socket.create_connection(("127.0.0.1", port), timeout=10)
    cli.sendall(b"tcp-ping")
    reply = cli.recv(64)
    cli.close()
    t.join()
    srv.close()
    if box.get("recv") != b"tcp-ping" or reply != b"tcp-pong":
        raise AssertionError("recv=%r reply=%r" % (box.get("recv"), reply))
    return "loopback send/recv ok"


# ------------------------------------------------------------------------ Tier B
@check("B01_sha256")
def b01():
    got = hashlib.sha256(b"abc").hexdigest()
    if got != SHA256_ABC:
        raise AssertionError("sha256=%s" % got)
    return "sha256(abc) ok"


@check("B02_base64")
def b02():
    enc = base64.b64encode(b"abc").decode("ascii")
    if enc != "YWJj":
        raise AssertionError("b64=%s" % enc)
    if base64.b64decode(enc) != b"abc":
        raise AssertionError("b64 decode mismatch")
    return "encode+decode ok"


@check("B03_json")
def b03():
    obj = {"k": [1, 2, 3], "n": "v"}
    text = json.dumps(obj, separators=(",", ":"), sort_keys=True)
    back = json.loads(text)
    if back != obj:
        raise AssertionError("round-trip %r" % back)
    return "serialize+parse ok"


@check("B04_regex")
def b04():
    if not re.match(r"^[a-z]+-[0-9]{3}$", "abc-123"):
        raise AssertionError("positive match failed")
    if re.match(r"^[a-z]+-[0-9]{3}$", "ABC-123"):
        raise AssertionError("negative match unexpected")
    return "match+reject ok"


@check("B05_gzip")
def b05():
    payload = b"gzip-payload-" * 8
    packed = gzip.compress(payload)
    if gzip.decompress(packed) != payload:
        raise AssertionError("gzip round-trip mismatch")
    return "compress+decompress ok"


# ---------------------------------------------------------------------------- 主流程
def main():
    if DEPTH > MAX_DEPTH:
        sys.stderr.write("FATAL: probe recursion depth %d exceeded\n" % DEPTH)
        return 3

    argv = sys.argv[1:]
    handled = handle_modes(argv)
    if handled is not None:
        return handled

    for fn in (a01, a02, a03, a04, a05, a06, a07, a08, a09, a10, a11, a12, a13,
               a14, a15, a16, a17, a18, a19, b01, b02, b03, b04, b05):
        fn()

    npass = nfail = nskip = 0
    for _cid, status, _detail in results:
        if status == "PASS":
            npass += 1
        elif status == "FAIL":
            nfail += 1
        else:
            nskip += 1
    print("SUMMARY python %d %d %d" % (npass, nfail, nskip))
    return 1 if nfail else 0


if __name__ == "__main__":
    sys.exit(main())
