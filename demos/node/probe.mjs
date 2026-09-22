#!/usr/bin/env node
// demos 能力探针 —— Node.js / JavaScript 实现。协议见 demos/SPEC.md。
//
// 模式：默认跑全部；--echo-child；--read-stdin；--argv-probe A B
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { gzipSync, gunzipSync } from "node:zlib";
import { createServer, createConnection } from "node:net";
import fs from "node:fs";
import path from "node:path";
import { Worker } from "node:worker_threads";

const SELF = path.resolve(process.argv[1]);
const NODE = process.execPath;

// 递归护栏（见 python 实现里的同款说明）：探针自我调用，模式分派写错就会指数派生。
const DEPTH = Number(process.env.DEMO_DEPTH || "0");
const MAX_DEPTH = 3;

const UNICODE_S = "中文-日本語-한국어-🚀";
const UNICODE_CHARS = 12;
const UNICODE_BYTES = 31;
const LARGE_BYTES = 262144;
const SHA256_ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

const results = [];
function record(id, status, detail = "") {
  const clean = String(detail).replace(/[\t\r\n]/g, " ").slice(0, 160);
  results.push([id, status, clean]);
  console.log(`CAP ${id} ${status} ${clean}`);
}

async function run(id, fn) {
  try {
    const detail = await fn();
    record(id, "PASS", detail || "");
  } catch (err) {
    record(id, "FAIL", `${err.name}: ${err.message}`);
  }
}

function spawn(args, stdinData) {
  const env = { ...process.env, DEMO_DEPTH: String(DEPTH + 1) };
  return execFileSync(NODE, [SELF, ...args], {
    input: stdinData === undefined ? "" : stdinData,
    env,
    timeout: 60000,
    encoding: "utf8",
    stdio: ["pipe", "pipe", "pipe"],
  });
}

// stdout 与 stderr 需要分开捕获，所以 A03/A17 单独走 spawnSync
function spawnSplit(args) {
  const env = { ...process.env, DEMO_DEPTH: String(DEPTH + 1) };
  return spawnSync(NODE, [SELF, ...args], { env, encoding: "utf8", timeout: 60000 });
}

// ---------------------------------------------------------------- 子进程模式
const argv = process.argv.slice(2);
if (argv[0] === "--echo-child") {
  process.stdout.write("child-ok\n");
  process.stderr.write("child-err\n");
  process.exit(0);
}
if (argv[0] === "--read-stdin") {
  let buf = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (c) => { buf += c; });
  process.stdin.on("end", () => {
    const line = buf.split("\n")[0].trim();
    process.stdout.write(line === "ping" ? "pong\n" : `unexpected:${line}\n`);
    process.exit(0);
  });
} else if (argv[0] === "--argv-probe") {
  const rest = argv.slice(1);
  process.stdout.write(`argv-probe:${rest.length}:${rest.join(":")}\n`);
  process.exit(0);
} else {
  if (DEPTH > MAX_DEPTH) {
    console.error(`FATAL: probe recursion depth ${DEPTH} exceeded`);
    process.exit(3);
  }
  main().then((code) => process.exit(code));
}

async function main() {
  await run("A01_argv", () => {
    const got = spawn(["--argv-probe", "alpha", "beta"]).trim();
    if (got !== "argv-probe:2:alpha:beta") throw new Error(`argv echo ${got}`);
    return "2 args round-tripped";
  });

  await run("A02_env", () => {
    const v = process.env.DEMO_LANG_TAG;
    if (v !== "demos-capability") throw new Error(`DEMO_LANG_TAG=${v}`);
    return "env visible";
  });

  await run("A03_streams", () => {
    const r = spawnSplit(["--echo-child"]);
    if (r.stdout.trim() !== "child-ok") throw new Error(`stdout=${r.stdout}`);
    if (!r.stderr.includes("child-err")) throw new Error(`stderr=${r.stderr}`);
    if (r.stdout.includes("child-err")) throw new Error("stderr leaked into stdout");
    return "stdout/stderr separated";
  });

  await run("A04_stdin", () => {
    const got = spawn(["--read-stdin"], "ping\n").trim();
    if (got !== "pong") throw new Error(`reply=${got}`);
    return "ping->pong";
  });

  await run("A05_file_rw", () => {
    fs.writeFileSync("a05.txt", "hello-io", "utf8");
    const got = fs.readFileSync("a05.txt", "utf8");
    if (got !== "hello-io") throw new Error(`read back ${got}`);
    return "text round-trip ok";
  });

  await run("A06_file_append", () => {
    fs.writeFileSync("a06.txt", "a", "utf8");
    fs.appendFileSync("a06.txt", "b", "utf8");
    const got = fs.readFileSync("a06.txt", "utf8");
    if (got !== "ab") throw new Error(`append ${got}`);
    return "append ok";
  });

  await run("A07_file_binary", () => {
    const payload = Buffer.from(Array.from({ length: 256 }, (_, i) => i));
    fs.writeFileSync("a07.bin", payload);
    const got = fs.readFileSync("a07.bin");
    if (!got.equals(payload)) throw new Error(`binary mismatch len=${got.length}`);
    return "256 bytes incl 0x00/0xFF";
  });

  await run("A08_file_stat", () => {
    fs.writeFileSync("a08.txt", "statted", "utf8");
    if (!fs.existsSync("a08.txt")) throw new Error("not created");
    const st = fs.statSync("a08.txt");
    if (st.size !== 7) throw new Error(`size=${st.size}`);
    fs.unlinkSync("a08.txt");
    if (fs.existsSync("a08.txt")) throw new Error("unlink failed");
    return "size=7 then removed";
  });

  await run("A09_dir_ops", () => {
    fs.rmSync("a09dir", { recursive: true, force: true });
    fs.mkdirSync("a09dir");
    fs.writeFileSync(path.join("a09dir", "inner.txt"), "x", "utf8");
    const entries = fs.readdirSync("a09dir");
    if (!entries.includes("inner.txt")) throw new Error(`listing=${entries}`);
    fs.unlinkSync(path.join("a09dir", "inner.txt"));
    fs.rmdirSync("a09dir");
    if (fs.existsSync("a09dir")) throw new Error("rmdir failed");
    return "mkdir/list/rmdir ok";
  });

  await run("A10_temp_file", () => {
    const dir = fs.mkdtempSync(path.join(process.cwd(), "a10-"));
    const f = path.join(dir, "t.tmp");
    fs.writeFileSync(f, "temp-content", "utf8");
    const got = fs.readFileSync(f, "utf8");
    if (got !== "temp-content") throw new Error(`temp read ${got}`);
    fs.rmSync(dir, { recursive: true, force: true });
    return "unique temp file";
  });

  await run("A11_unicode", () => {
    if ([...UNICODE_S].length !== UNICODE_CHARS) throw new Error("codepoint count");
    const bytes = Buffer.byteLength(UNICODE_S, "utf8");
    if (bytes !== UNICODE_BYTES) throw new Error(`bytes=${bytes}`);
    fs.writeFileSync("a11.txt", UNICODE_S, "utf8");
    const got = fs.readFileSync("a11.txt", "utf8");
    if (got !== UNICODE_S) throw new Error("round-trip mismatch");
    return `${UNICODE_CHARS} codepoints / ${UNICODE_BYTES} bytes`;
  });

  await run("A12_large_io", () => {
    const payload = Buffer.alloc(LARGE_BYTES, 0x61);
    fs.writeFileSync("a12.bin", payload);
    const got = fs.readFileSync("a12.bin");
    if (got.length !== LARGE_BYTES) throw new Error(`len=${got.length}`);
    return "262144 bytes ok";
  });

  await run("A13_time", async () => {
    const nowMs = Date.now();
    if (nowMs < 1577836800000) throw new Error(`epoch=${nowMs}`);
    const t0 = process.hrtime.bigint();
    await new Promise((r) => setTimeout(r, 50));
    const deltaMs = Number(process.hrtime.bigint() - t0) / 1e6;
    if (deltaMs < 40) throw new Error(`sleep 只测到 ${deltaMs.toFixed(1)} ms`);
    return `sleep ${deltaMs.toFixed(0)}ms`;
  });

  await run("A14_random", () => {
    const v = Math.floor(Math.random() * 1000);
    if (!(v >= 0 && v < 1000)) throw new Error(`out of range ${v}`);
    return "in [0,1000)";
  });

  await run("A15_container", () => {
    const data = [5, 3, 9, 1, 7, 3].sort((a, b) => a - b);
    if (data.join(",") !== "1,3,3,5,7,9") throw new Error(`sorted=${data}`);
    const m = new Map([["a", 1]]);
    m.set("b", 2);
    if (m.get("a") !== 1 || m.get("b") !== 2 || m.size !== 2) throw new Error("map");
    return "sort + map ok";
  });

  await run("A16_error", () => {
    let caught = false;
    try {
      fs.readFileSync("definitely-missing-file-xyz", "utf8");
    } catch {
      caught = true;
    }
    if (!caught) throw new Error("no error raised");
    return "exception caught";
  });

  await run("A17_subprocess", () => {
    const r = spawnSplit(["--echo-child"]);
    if (r.status !== 0) throw new Error(`child exit=${r.status}`);
    if (r.stdout.trim() !== "child-ok") throw new Error(`child stdout=${r.stdout}`);
    return "child exit=0, stdout captured";
  });

  await run("A18_concurrency", async () => {
    const sab = new SharedArrayBuffer(4);
    const view = new Int32Array(sab);
    const code = `
      const { workerData } = require('node:worker_threads');
      const v = new Int32Array(workerData);
      for (let i = 0; i < 1000; i++) Atomics.add(v, 0, 1);
    `;
    const mk = () => new Promise((res, rej) => {
      const w = new Worker(code, { eval: true, workerData: sab });
      w.on("exit", res);
      w.on("error", rej);
    });
    await Promise.all([mk(), mk()]);
    if (Atomics.load(view, 0) !== 2000) throw new Error(`total=${Atomics.load(view, 0)}`);
    return "2 workers -> 2000";
  });

  await run("A19_tcp_loopback", async () => {
    return await new Promise((resolve, reject) => {
      const srv = createServer((conn) => {
        conn.on("data", (d) => {
          if (d.toString() !== "tcp-ping") reject(new Error(`recv=${d}`));
          conn.end("tcp-pong");
        });
      });
      srv.on("error", reject);
      srv.listen(0, "127.0.0.1", () => {
        const port = srv.address().port;
        const cli = createConnection({ host: "127.0.0.1", port }, () => cli.write("tcp-ping"));
        let reply = "";
        cli.setEncoding("utf8");
        cli.on("data", (d) => { reply += d; });
        cli.on("end", () => {
          srv.close();
          if (reply !== "tcp-pong") reject(new Error(`reply=${reply}`));
          else resolve("loopback send/recv ok");
        });
        cli.on("error", reject);
      });
    });
  });

  await run("B01_sha256", () => {
    const got = createHash("sha256").update("abc").digest("hex");
    if (got !== SHA256_ABC) throw new Error(`sha256=${got}`);
    return "sha256(abc) ok";
  });

  await run("B02_base64", () => {
    const enc = Buffer.from("abc").toString("base64");
    if (enc !== "YWJj") throw new Error(`b64=${enc}`);
    if (Buffer.from(enc, "base64").toString("utf8") !== "abc") throw new Error("decode");
    return "encode+decode ok";
  });

  await run("B03_json", () => {
    const obj = { k: [1, 2, 3], n: "v" };
    const text = JSON.stringify(obj);
    const back = JSON.parse(text);
    if (back.k.join(",") !== "1,2,3" || back.n !== "v") throw new Error("round-trip");
    return "serialize+parse ok";
  });

  await run("B04_regex", () => {
    if (!/^[a-z]+-[0-9]{3}$/.test("abc-123")) throw new Error("positive match failed");
    if (/^[a-z]+-[0-9]{3}$/.test("ABC-123")) throw new Error("negative match unexpected");
    return "match+reject ok";
  });

  await run("B05_gzip", () => {
    const payload = Buffer.from("gzip-payload-".repeat(8));
    if (!gunzipSync(gzipSync(payload)).equals(payload)) throw new Error("gzip round-trip");
    return "compress+decompress ok";
  });

  const counts = { PASS: 0, FAIL: 0, SKIP: 0 };
  for (const [, status] of results) counts[status]++;
  console.log(`SUMMARY node ${counts.PASS} ${counts.FAIL} ${counts.SKIP}`);
  return counts.FAIL ? 1 : 0;
}
