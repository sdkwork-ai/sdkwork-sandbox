#!/usr/bin/env node
// Sandbox lifecycle performance harness.
//
// Measures two lanes that answer different questions:
//
//   control-plane  drives the real provider-neutral lifecycle service (create -> start) through its
//                  public port and reports the latency distribution plus the process resource cost.
//                  This is the cost an agent pays *before* any machine boots, so it is a floor, not
//                  a sandbox boot time.
//   host-floor     measures what the host itself contributes: the cost of spawning a process and of
//                  materializing a directory tree. These are the two platform costs a local provider
//                  cannot avoid, and they are what makes Windows and Ubuntu differ.
//
// Two deliberate design constraints, both learned the hard way on Windows:
//
//   1. Latency and resources are measured in **separate passes**. Sampling a process on Windows
//      means running `tasklist`, which costs ~540 ms of blocked event loop per call, so sampling
//      during the latency pass would both inflate the latencies and stretch the sampling interval
//      to ~600 ms. The latency pass therefore runs with no sampler at all; only the resource pass
//      attaches one, and it is sized to be long enough for a handful of samples.
//   2. Every OS-specific call lives here, never in the Rust crate, so the crate stays free of
//      platform-conditional code and the portability gate can keep it that way.
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, release, tmpdir, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const RESULT_MARKER = "SDKWORK_BENCH_RESULT ";
const DEFAULT_CRATE = "sdkwork-intelligence-sandbox-service";
const BENCH_TEST_NAME = "tests::sandbox_lifecycle_create_start_benchmark";

function fail(message) {
  throw new Error(message);
}

export function parseBenchmarkArgs(argv) {
  const options = {
    lane: "control-plane",
    iterations: 20_000,
    resourceIterations: 200_000,
    profile: "release",
    crate: DEFAULT_CRATE,
    hostFloorSamples: 200,
    hostFloorFiles: 200,
    sampleIntervalMs: 500,
    label: null,
    capabilityEvidence: null,
    out: null,
    json: false,
  };
  const positive = (name, value) => {
    const parsed = Number(value);
    if (!Number.isInteger(parsed) || parsed <= 0) {
      fail(`${name} must be a positive integer`);
    }
    return parsed;
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const next = () => {
      const value = argv[index + 1];
      if (value === undefined) {
        fail(`${argument} requires a value`);
      }
      index += 1;
      return value;
    };
    if (argument === "--lane") {
      const value = next();
      if (!["control-plane", "host-floor", "all"].includes(value)) {
        fail("--lane must be control-plane, host-floor or all");
      }
      options.lane = value;
    } else if (argument === "--iterations") {
      options.iterations = positive("--iterations", next());
    } else if (argument === "--resource-iterations") {
      options.resourceIterations = positive("--resource-iterations", next());
    } else if (argument === "--profile") {
      const value = next();
      if (!["release", "debug"].includes(value)) {
        fail("--profile must be release or debug");
      }
      options.profile = value;
    } else if (argument === "--crate") {
      options.crate = next();
    } else if (argument === "--host-floor-samples") {
      options.hostFloorSamples = positive("--host-floor-samples", next());
    } else if (argument === "--host-floor-files") {
      options.hostFloorFiles = positive("--host-floor-files", next());
    } else if (argument === "--sample-interval-ms") {
      options.sampleIntervalMs = positive("--sample-interval-ms", next());
    } else if (argument === "--label") {
      options.label = next();
    } else if (argument === "--capability-evidence") {
      options.capabilityEvidence = next();
    } else if (argument === "--out") {
      options.out = next();
    } else if (argument === "--json") {
      options.json = true;
    } else if (argument === "--help" || argument === "-h") {
      options.help = true;
    } else {
      fail(`unsupported argument ${argument}`);
    }
  }
  return options;
}

export const BENCHMARK_HELP = `Sandbox lifecycle performance harness

  --lane <lane>                control-plane | host-floor | all   (default control-plane)
  --iterations <n>             latency-pass lifecycle iterations   (default 20000)
  --resource-iterations <n>    resource-pass lifecycle iterations  (default 200000)
  --profile <profile>          release | debug                     (default release)
  --crate <name>               crate owning the benchmark test     (default ${DEFAULT_CRATE})
  --host-floor-samples <n>     process spawn samples               (default 200)
  --host-floor-files <n>       files materialized by the directory floor (default 200)
  --sample-interval-ms <n>     resource sampling interval          (default 500)
  --label <text>               platform label recorded in the report
  --capability-evidence <path> host capability evidence JSON to record as the virtualization-capability element
  --out <path>                 write the JSON report to a file
  --json                       print the JSON report to stdout
`;

/// Reduces a `sandbox-host-capability-evidence.mjs` report to the facts
/// `TECH-performance-and-capacity.md` section 5 requires every benchmark result to record as the
/// machine's virtualization capability.
///
/// The evidence report lists each probe once under `checks` and repeats the interesting ones under
/// `blocking`. Counting every object that carries `id` + `status` therefore counted those twice and
/// inflated the Windows reading from 47 to 54 (WSL was unaffected only because its `blocking` array
/// is empty). `checks` is authoritative when present; the walk is a fallback for other shapes.
export function summarizeCapabilityEvidence(content) {
  const parsed = JSON.parse(content);
  const capabilities = [];
  const add = (node) => {
    if (!capabilities.some((capability) => capability.id === node.id)) {
      capabilities.push(node);
    }
  };
  if (Array.isArray(parsed.checks)) {
    for (const check of parsed.checks) {
      if (check && check.id && check.status) {
        add(check);
      }
    }
  } else {
    const walk = (node) => {
      if (Array.isArray(node)) {
        node.forEach(walk);
        return;
      }
      if (node && typeof node === "object") {
        if (node.id && node.status) {
          add(node);
          return;
        }
        Object.values(node).forEach(walk);
      }
    };
    walk(parsed);
  }
  const classification = {};
  for (const capability of capabilities) {
    classification[capability.status] = (classification[capability.status] ?? 0) + 1;
  }
  const detail = (id) => {
    const hit = capabilities.find((capability) => capability.id === id);
    return hit ? hit.detail : null;
  };
  const declaredTotal =
    parsed.summary && typeof parsed.summary.total === "number" ? parsed.summary.total : null;
  return {
    target: parsed.target ?? null,
    total: capabilities.length,
    // Recorded so a summarizer that disagrees with the probe's own census is visible in the report
    // rather than silently replacing it.
    declaredTotal,
    totalMatchesDeclared: declaredTotal === null || declaredTotal === capabilities.length,
    classification,
    kernel: detail("platform.kernel"),
    arch: detail("platform.arch"),
    distro: detail("platform.distro"),
    kvmDevice: detail("isolation.kvm-device"),
    seccompMode: detail("security.seccomp-mode"),
    cgroupV2: detail("cgroup.v2-mounted"),
  };
}

/// Minimal RFC4180-style row parser. `tasklist /FO CSV` quotes fields and embeds thousands
/// separators inside them ("35,504 K"), and the trailing column can be mangled by the console code
/// page, so index-based splitting is not safe.
export function parseCsvRow(line) {
  const cells = [];
  let current = "";
  let inQuotes = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (inQuotes) {
      if (character === '"') {
        if (line[index + 1] === '"') {
          current += '"';
          index += 1;
        } else {
          inQuotes = false;
        }
      } else {
        current += character;
      }
    } else if (character === '"') {
      inQuotes = true;
    } else if (character === ",") {
      cells.push(current);
      current = "";
    } else {
      current += character;
    }
  }
  cells.push(current);
  return cells.map((cell) => cell.trim());
}

/// Parses a `tasklist /v` row into working set and total processor time. Fields are found by shape
/// rather than by position, because column order and code page differ by locale.
export function parseWindowsTasklistRow(line) {
  if (!line || /No tasks are running/i.test(line)) {
    return null;
  }
  const cells = parseCsvRow(line);
  const memoryCell = cells.find((cell) => /^[\d.,]+ K$/i.test(cell));
  const cpuCell = cells.find((cell) => /^\d+:\d{2}:\d{2}$/.test(cell));
  const rssKilobytes = memoryCell ? Number.parseInt(memoryCell.replace(/[^\d]/g, ""), 10) : Number.NaN;
  return {
    rssBytes: Number.isNaN(rssKilobytes) ? null : rssKilobytes * 1024,
    cpuSeconds: cpuCell ? parseCpuClock(cpuCell) : null,
  };
}

/// Parses `H:MM:SS` or `M:SS` clock text into seconds.
export function parseCpuClock(value) {
  const parts = String(value).trim().split(":");
  if (!parts.every((part) => /^\d+$/.test(part))) {
    return null;
  }
  return parts.reduce((total, part) => total * 60 + Number.parseInt(part, 10), 0);
}

/// Parses `/proc/<pid>/stat`. Field 2 (`comm`) may contain spaces and parentheses, so the split
/// starts after the final `)`; `minflt` is field 10, `majflt` is field 12, `utime` is field 14 and
/// `stime` is field 15.
export function parseProcStat(content, clockTicks) {
  const closingParenthesis = content.lastIndexOf(")");
  if (closingParenthesis < 0) {
    return null;
  }
  const fields = content.slice(closingParenthesis + 2).trim().split(/\s+/u);
  const userTicks = Number.parseInt(fields[11], 10);
  const systemTicks = Number.parseInt(fields[12], 10);
  if (Number.isNaN(userTicks) || Number.isNaN(systemTicks)) {
    return null;
  }
  const minorPageFaults = Number.parseInt(fields[7], 10);
  const majorPageFaults = Number.parseInt(fields[9], 10);
  return {
    cpuSeconds: (userTicks + systemTicks) / clockTicks,
    minorPageFaults: Number.isNaN(minorPageFaults) ? null : minorPageFaults,
    majorPageFaults: Number.isNaN(majorPageFaults) ? null : majorPageFaults,
  };
}

/// Reads the context-switch counters `TECH-performance-and-capacity.md` section 6 requires.
export function parseProcStatusCounters(status) {
  const read = (name) => {
    const match = new RegExp(`^${name}:\\s+(\\d+)`, "mu").exec(status);
    return match ? Number.parseInt(match[1], 10) : null;
  };
  return {
    voluntaryContextSwitches: read("voluntary_ctxt_switches"),
    involuntaryContextSwitches: read("nonvoluntary_ctxt_switches"),
  };
}

function linuxClockTicks() {
  const probed = spawnSync("getconf", ["CLK_TCK"], { encoding: "utf8" });
  const parsed = Number.parseInt((probed.stdout || "").trim(), 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : 100;
}

/// Samples one process. Returns null once the process is gone. On Linux `VmHWM` is the kernel's
/// exact peak resident set; on Windows there is no portable peak counter, so the caller reduces
/// samples to a sampled maximum and the report says exactly that.
export function sampleProcess(pid, platform, clockTicks) {
  if (platform === "win32") {
    const probed = spawnSync("tasklist.exe", ["/v", "/FI", `PID eq ${pid}`, "/FO", "CSV", "/NH"], {
      encoding: "utf8",
    });
    const firstLine = (probed.stdout || "")
      .split(/\r?\n/u)
      .find((line) => line.trim().length > 0);
    return parseWindowsTasklistRow(firstLine || "");
  }
  try {
    const status = readFileSync(`/proc/${pid}/status`, "utf8");
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    const resident = /VmRSS:\s+(\d+) kB/u.exec(status);
    const peak = /VmHWM:\s+(\d+) kB/u.exec(status);
    const cpu = parseProcStat(stat, clockTicks);
    const counters = parseProcStatusCounters(status);
    return {
      rssBytes: resident ? Number.parseInt(resident[1], 10) * 1024 : null,
      peakRssBytes: peak ? Number.parseInt(peak[1], 10) * 1024 : null,
      cpuSeconds: cpu ? cpu.cpuSeconds : null,
      minorPageFaults: cpu ? cpu.minorPageFaults : null,
      majorPageFaults: cpu ? cpu.majorPageFaults : null,
      voluntaryContextSwitches: counters.voluntaryContextSwitches,
      involuntaryContextSwitches: counters.involuntaryContextSwitches,
    };
  } catch {
    return null;
  }
}

/// Difference between the first and last sample of a nullable per-process counter.
function counterDelta(samples, key) {
  const values = samples.map((sample) => sample[key]).filter((value) => typeof value === "number");
  if (values.length < 2) {
    return null;
  }
  return values[values.length - 1] - values[0];
}

/// Latency summary in milliseconds. Percentiles use the nearest-rank method on the sorted sample,
/// which is exact for the discrete sample and needs no interpolation assumption.
export function computeStats(samplesNanos) {
  if (samplesNanos.length === 0) {
    fail("computeStats requires at least one sample");
  }
  const sorted = [...samplesNanos].map(Number).sort((left, right) => left - right);
  const percentile = (fraction) => {
    const rank = Math.ceil(fraction * sorted.length);
    return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1];
  };
  const total = sorted.reduce((sum, value) => sum + value, 0);
  const mean = total / sorted.length;
  const variance = sorted.reduce((sum, value) => sum + (value - mean) ** 2, 0) / sorted.length;
  const toMillis = (nanos) => Math.round((nanos / 1e6) * 1000) / 1000;
  return {
    count: sorted.length,
    minMs: toMillis(sorted[0]),
    p50Ms: toMillis(percentile(0.5)),
    p90Ms: toMillis(percentile(0.9)),
    p95Ms: toMillis(percentile(0.95)),
    p99Ms: toMillis(percentile(0.99)),
    maxMs: toMillis(sorted[sorted.length - 1]),
    meanMs: toMillis(mean),
    stdDevMs: toMillis(Math.sqrt(variance)),
  };
}

/// Extracts one balanced JSON value starting at `startIndex`, ignoring braces inside strings.
/// Needed because the marker is not guaranteed to be at the start of its line — see
/// `parseBenchmarkResultLine`.
export function extractBalancedJson(text, startIndex) {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = startIndex; index < text.length; index += 1) {
    const character = text[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }
    if (character === '"') {
      inString = true;
    } else if (character === "{" || character === "[") {
      depth += 1;
    } else if (character === "}" || character === "]") {
      depth -= 1;
      if (depth === 0) {
        return text.slice(startIndex, index + 1);
      }
    }
  }
  return null;
}

/// Finds the benchmark result anywhere in the captured stdout and parses it.
///
/// The marker must be searched for, not matched line-by-line. Under libtest `--nocapture`, the
/// runner writes `test <name> ... ` **without a trailing newline** and the test's own stdout lands
/// on the same physical line, so the result arrives as
/// `test tests::sandbox_lifecycle_create_start_benchmark ... SDKWORK_BENCH_RESULT {...}`. A
/// line-anchored check silently reports that the benchmark produced nothing at all.
export function parseBenchmarkResultLine(stdout) {
  const text = String(stdout);
  const markerIndex = text.indexOf(RESULT_MARKER);
  if (markerIndex < 0) {
    fail(`benchmark produced no ${RESULT_MARKER} line`);
  }
  const jsonStart = text.indexOf("{", markerIndex + RESULT_MARKER.length);
  if (jsonStart < 0) {
    fail(`benchmark result marker was not followed by a JSON object`);
  }
  const jsonText = extractBalancedJson(text, jsonStart);
  if (!jsonText) {
    fail(`benchmark result JSON object was not terminated`);
  }
  return JSON.parse(jsonText);
}

/// Builds the benchmark test binary and returns its path. `cargo test --no-run` with JSON messages
/// is the only supported way to learn the path without parsing unstable human-readable output.
///
/// A crate's own unit tests are built from the **lib** target, so the artifact reports
/// `target.kind: ["lib"]` with `profile.test: true` — filtering on `kind` containing `"test"` finds
/// nothing and silently reports the crate as unbuildable. Build scripts also carry an `executable`,
/// so `profile.test` is the field that actually distinguishes a runnable test binary.
export function resolveTestExecutable(crate, profile, cwd) {
  const args = ["test", "--no-run", "-p", crate, "--message-format=json"];
  if (profile === "release") {
    args.splice(1, 0, "--release");
  }
  const built = spawnSync("cargo", args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (built.status !== 0) {
    fail(`cargo test --no-run failed:\n${(built.stderr || "").slice(-2000)}`);
  }
  const candidates = [];
  for (const line of (built.stdout || "").split(/\r?\n/u)) {
    if (!line.startsWith("{")) {
      continue;
    }
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      continue;
    }
    if (message.reason !== "compiler-artifact" || !message.executable) {
      continue;
    }
    if (!message.profile || message.profile.test !== true) {
      continue;
    }
    const kinds = (message.target && message.target.kind) || [];
    candidates.push({ executable: message.executable, kinds });
  }
  const preferred = candidates.find((candidate) => candidate.kinds.includes("lib"));
  if (preferred) {
    return preferred.executable;
  }
  if (candidates.length > 0) {
    return candidates[0].executable;
  }
  fail(`no test executable found for crate ${crate}`);
}

async function runBenchmarkProcess(executable, iterations, options, cwd, sampler) {
  const startedAt = Date.now();
  const sampled = [];
  const child = spawn(executable, ["--exact", BENCH_TEST_NAME, "--nocapture", "--test-threads=1"], {
    cwd,
    env: { ...process.env, SDKWORK_SANDBOX_BENCH_ITERATIONS: String(iterations) },
  });
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  let timer = null;
  if (sampler) {
    timer = setInterval(() => {
      const sample = sampler(child.pid);
      if (sample) {
        sampled.push(sample);
      }
    }, options.sampleIntervalMs);
  }
  const exitCode = await new Promise((resolveExit) => {
    child.on("close", resolveExit);
  });
  if (timer) {
    clearInterval(timer);
  }
  if (exitCode !== 0) {
    fail(`benchmark test process exited with ${exitCode}:\n${stderr.slice(-2000)}`);
  }
  return { payload: parseBenchmarkResultLine(stdout), sampled, wallMs: Date.now() - startedAt };
}

function measureSamplerCostMs(platform, clockTicks, samples = 3) {
  const pid = process.pid;
  const startedAt = process.hrtime.bigint();
  for (let index = 0; index < samples; index += 1) {
    sampleProcess(pid, platform, clockTicks);
  }
  return Math.round((Number(process.hrtime.bigint() - startedAt) / 1e6 / samples) * 100) / 100;
}

async function runControlPlaneLane(options, cwd) {
  const platform = process.platform;
  const clockTicks = platform === "win32" ? null : linuxClockTicks();
  const executable = resolveTestExecutable(options.crate, options.profile, cwd);
  const samplerCostMs = measureSamplerCostMs(platform, clockTicks);

  // Pass 1: latency, no sampler attached, so nothing perturbs the run.
  const latency = await runBenchmarkProcess(executable, options.iterations, options, cwd, null);

  // Pass 2: resources, sampler attached and sized to outlast a single sampling call.
  const effectiveIntervalMs = Math.max(options.sampleIntervalMs, Math.ceil(samplerCostMs));
  const resourceOptions = { ...options, sampleIntervalMs: effectiveIntervalMs };
  const resources = await runBenchmarkProcess(
    executable,
    options.resourceIterations,
    resourceOptions,
    cwd,
    (pid) => sampleProcess(pid, platform, clockTicks),
  );

  const peakSamples = resources.sampled
    .map((sample) => sample.peakRssBytes ?? sample.rssBytes)
    .filter((value) => typeof value === "number");
  const baseline = resources.sampled.find((sample) => typeof sample.rssBytes === "number");
  const rssSeries = resources.sampled
    .map((sample) => sample.rssBytes)
    .filter((value) => typeof value === "number");
  const cpuSamples = resources.sampled.filter((sample) => typeof sample.cpuSeconds === "number");
  const peakRssBytes = peakSamples.length > 0 ? Math.max(...peakSamples) : null;

  const counterAvailability =
    platform === "win32"
      ? "not available: this platform exposes no per-process context-switch or page-fault counter through tasklist, and reading them would require Windows API calls that would put platform-conditional code into the crate"
      : null;

  return {
    lane: "control-plane",
    benchmarkTest: BENCH_TEST_NAME,
    iterations: latency.payload.iterations,
    warmupIterations: latency.payload.warmup,
    latencyWallMs: latency.wallMs,
    // TECH-performance-and-capacity.md section 2: hot allocation latency and cold start latency
    // must be counted separately and never merged.
    coldFirstPass: {
      createMs: Math.round((latency.payload.coldCreateNanos / 1e6) * 1000) / 1000,
      startMs: Math.round((latency.payload.coldStartNanos / 1e6) * 1000) / 1000,
      endToEndMs:
        Math.round(((latency.payload.coldCreateNanos + latency.payload.coldStartNanos) / 1e6) * 1000) /
        1000,
      note: "The very first create -> start of the process, before any warmup. Reported separately from the steady-state distribution because the spec forbids merging them.",
    },
    create: computeStats(latency.payload.createNanos),
    start: computeStats(latency.payload.startNanos),
    endToEnd: computeStats(
      latency.payload.createNanos.map((value, index) => value + latency.payload.startNanos[index]),
    ),
    errors: {
      total: 0,
      policyRejections: 0,
      systemFaults: 0,
      note: "Not a meaningful measurement at this layer: the lane runs a scripted in-process provider and an in-memory repository, so neither policy rejection nor system fault can occur by construction. A failure aborts the run instead of being counted.",
    },
    resources: {
      pass: "second, dedicated resource pass; the latency pass ran with no sampler attached",
      iterations: resources.payload.iterations,
      wallMs: resources.wallMs,
      requestedIntervalMs: options.sampleIntervalMs,
      effectiveIntervalMs,
      samplerCostMs,
      sampleCount: resources.sampled.length,
      peakRssBytes,
      peakRssSource:
        platform === "win32"
          ? "maximum of sampled working set (Windows exposes no portable peak counter)"
          : "VmHWM, the kernel's exact peak resident set",
      baselineRssBytes: baseline ? baseline.rssBytes : null,
      finalRssBytes: rssSeries.length > 0 ? rssSeries[rssSeries.length - 1] : null,
      rssGrowthBytesPerIteration:
        rssSeries.length > 1
          ? Math.round((rssSeries[rssSeries.length - 1] - rssSeries[0]) / resources.payload.iterations)
          : null,
      cpuSeconds: counterDelta(resources.sampled, "cpuSeconds"),
      cpuSecondsSource:
        platform === "win32"
          ? "tasklist /v total processor time delta"
          : "/proc/<pid>/stat utime+stime delta",
      voluntaryContextSwitches: counterDelta(resources.sampled, "voluntaryContextSwitches"),
      involuntaryContextSwitches: counterDelta(resources.sampled, "involuntaryContextSwitches"),
      minorPageFaults: counterDelta(resources.sampled, "minorPageFaults"),
      majorPageFaults: counterDelta(resources.sampled, "majorPageFaults"),
      counterAvailability,
    },
    scope:
      "Control-plane orchestration only: repository persistence, lease handling, provider selection, and the provider SPI calls of an in-process fake provider. No VM, container, or process was started, so this is the latency floor beneath a real sandbox creation, not the sandbox boot itself.",
  };
}

/// The host-side floor: process spawn and directory materialization. Both platforms are probed from
/// this same Node runtime, so the comparison isolates the OS rather than the harness. The spawn
/// probe uses the smallest real executable available on each platform; on Windows `cmd.exe` was
/// rejected as a floor probe because its own startup dominates the measurement.
function runHostFloorLane(options) {
  const probe =
    process.platform === "win32"
      ? { command: "where.exe", args: ["cmd.exe"] }
      : { command: "/bin/true", args: [] };
  const spawnSamples = [];
  for (let index = 0; index < options.hostFloorSamples; index += 1) {
    const startedAt = process.hrtime.bigint();
    const result = spawnSync(probe.command, probe.args, { stdio: "ignore" });
    const elapsed = process.hrtime.bigint() - startedAt;
    if (result.status !== 0) {
      fail(`host-floor spawn probe failed with status ${result.status}`);
    }
    spawnSamples.push(Number(elapsed));
  }

  const root = mkdtempSync(join(tmpdir(), "sdkwork-bench-"));
  const materializeStartedAt = process.hrtime.bigint();
  const directoryCount = Math.min(20, options.hostFloorFiles);
  const filesPerDirectory = Math.ceil(options.hostFloorFiles / directoryCount);
  const payload = "x".repeat(4096);
  let written = 0;
  let bytesWritten = 0;
  for (let directory = 0; directory < directoryCount; directory += 1) {
    const directoryPath = join(root, `workspace-${directory}`);
    mkdirSync(directoryPath, { recursive: true });
    for (let file = 0; file < filesPerDirectory; file += 1) {
      writeFileSync(join(directoryPath, `payload-${file}.bin`), payload);
      written += 1;
      bytesWritten += payload.length;
    }
  }
  const materializeElapsed = Number(process.hrtime.bigint() - materializeStartedAt);
  rmSync(root, { recursive: true, force: true });

  return {
    lane: "host-floor",
    processSpawn: {
      ...computeStats(spawnSamples),
      command: `${probe.command}${probe.args.length > 0 ? ` ${probe.args.join(" ")}` : ""}`,
      includesHarnessOverhead: true,
      note: "Measured through spawnSync, so the absolute value includes this Node runtime's own process-setup cost identically on both platforms. The cross-platform difference is the signal; the absolute number is not a sandbox boot time.",
    },
    directoryMaterialization: {
      directories: directoryCount,
      files: written,
      bytes: bytesWritten,
      elapsedMs: Math.round((materializeElapsed / 1e6) * 1000) / 1000,
      filesPerSecond: Math.round((written / materializeElapsed) * 1e9),
      mebibytesPerSecond:
        Math.round((bytesWritten / (1024 * 1024) / (materializeElapsed / 1e9)) * 100) / 100,
      note: "4 KiB payloads written with writeFileSync after mkdirSync, then removed. This is the file-materialization floor a local provider pays when it prepares a workspace.",
    },
  };
}

export async function runBenchmark(options, cwd = process.cwd()) {
  const lanes = [];
  if (options.lane === "control-plane" || options.lane === "all") {
    lanes.push(await runControlPlaneLane(options, cwd));
  }
  if (options.lane === "host-floor" || options.lane === "all") {
    lanes.push(runHostFloorLane(options));
  }
  const machine = {
    cpuModel: cpus()[0] ? cpus()[0].model : null,
    logicalCpus: cpus().length,
    totalMemoryBytes: totalmem(),
    platform: process.platform,
    arch: process.arch,
    kernel: release(),
    node: process.version,
  };
  const capabilityEvidence = options.capabilityEvidence
    ? {
        path: options.capabilityEvidence,
        summary: summarizeCapabilityEvidence(
          readFileSync(resolve(cwd, options.capabilityEvidence), "utf8"),
        ),
      }
    : null;
  const controlPlane = lanes.find((lane) => lane.lane === "control-plane");
  const missingRequiredElements = [];
  if (!capabilityEvidence) {
    missingRequiredElements.push("virtualizationCapability (pass --capability-evidence)");
  }
  if (!controlPlane) {
    missingRequiredElements.push("control-plane lane (this run measured the host floor only)");
  }
  return {
    schemaVersion: 1,
    kind: "sdkwork-sandbox-lifecycle-benchmark",
    generatedAt: new Date().toISOString(),
    label: options.label,
    machine,
    capabilityEvidence,
    conformance: {
      authority: "TECH-performance-and-capacity.md sections 5, 6 and 9",
      recorded: {
        machineSpec: `${machine.cpuModel} · ${machine.logicalCpus} logical cpus · ${machine.totalMemoryBytes} bytes RAM`,
        kernelVersion: machine.kernel,
        virtualizationCapability: capabilityEvidence
          ? `${capabilityEvidence.path} (${JSON.stringify(capabilityEvidence.summary.classification)})`
          : "NOT RECORDED",
        templateVersion: "not applicable: this repository has no Template artifact or Template contract (Phase 0)",
        artifactTuple: "not applicable: this repository has no runtime artifact or artifact tuple (Phase 0)",
        sampleSize: controlPlane ? `${controlPlane.iterations} measured + ${controlPlane.warmupIterations} warmup` : "n/a",
        concurrencyLevel: "1 — sequential, single process, in-process provider",
        statisticalMethod:
          "nearest-rank percentiles over raw in-process samples (hot and cold counted separately)",
      },
      missingRequiredElements,
      releaseGateEligible: false,
      releaseGateReason:
        "TECH-performance-and-capacity.md section 9 forbids a latency, concurrency or resource number from entering a PRD, release evidence or external material until it has passed a real-hardware benchmark with a recorded reference environment, Template and workload. This run has no Template, no runtime artifact and no KVM, so it is a floor measurement and must not be quoted as a capacity claim.",
    },
    lanes,
  };
}

export function renderBenchmarkReport(report) {
  const lines = [];
  const millis = (value) => (value === null || value === undefined ? "n/a" : value.toFixed(3));
  const short = (value) => (value === null || value === undefined ? "n/a" : String(value));
  const mebibytes = (value) =>
    value === null || value === undefined ? "n/a" : `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  const machine = report.machine;
  lines.push(`# Sandbox lifecycle benchmark — ${report.label ?? machine.platform}`);
  lines.push("");
  lines.push(`Generated: ${report.generatedAt}`);
  lines.push("");
  lines.push("## Reference environment");
  lines.push("");
  lines.push(`- cpu: ${machine.cpuModel} · ${machine.logicalCpus} logical cpus`);
  lines.push(
    `- memory: ${mebibytes(machine.totalMemoryBytes)} · platform: ${machine.platform} ${machine.arch} · kernel: ${machine.kernel}`,
  );
  lines.push(`- node: ${machine.node}`);
  if (report.capabilityEvidence) {
    const summary = report.capabilityEvidence.summary;
    lines.push(
      `- virtualization capability: ${summary.distro ?? summary.target ?? "unknown"} · ${JSON.stringify(summary.classification)}`,
    );
    lines.push(
      `- kernel / cgroup / seccomp as probed: ${short(summary.kernel)} · cgroup.v2-mounted: ${short(summary.cgroupV2)} · seccomp: ${short(summary.seccompMode)} · kvm: ${short(summary.kvmDevice)}`,
    );
  } else {
    lines.push("- virtualization capability: NOT RECORDED");
  }
  for (const lane of report.lanes) {
    lines.push("");
    if (lane.lane === "control-plane") {
      lines.push("## Control plane (create -> start)");
      lines.push("");
      lines.push("Steady state (hot path), after warmup:");
      lines.push("");
      lines.push("| Phase | n | p50 ms | p90 ms | p95 ms | p99 ms | max ms | mean ms | stddev ms |");
      lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
      for (const [name, stats] of [
        ["create", lane.create],
        ["start", lane.start],
        ["create + start", lane.endToEnd],
      ]) {
        lines.push(
          `| ${name} | ${stats.count} | ${millis(stats.p50Ms)} | ${millis(stats.p90Ms)} | ${millis(stats.p95Ms)} | ${millis(stats.p99Ms)} | ${millis(stats.maxMs)} | ${millis(stats.meanMs)} | ${millis(stats.stdDevMs)} |`,
        );
      }
      lines.push("");
      lines.push(
        `Cold path (the first create -> start of the process, reported separately as the spec requires): create ${millis(lane.coldFirstPass.createMs)} ms · start ${millis(lane.coldFirstPass.startMs)} ms · end to end ${millis(lane.coldFirstPass.endToEndMs)} ms`,
      );
      lines.push("");
      lines.push(
        `- latency pass: ${lane.iterations} measured iterations (${lane.warmupIterations} warmup) in ${lane.latencyWallMs} ms, no sampler attached`,
      );
      lines.push(
        `- resource pass: ${lane.resources.iterations} iterations in ${lane.resources.wallMs} ms, ${lane.resources.sampleCount} samples at an effective ${lane.resources.effectiveIntervalMs} ms interval (one sampling call costs ${lane.resources.samplerCostMs} ms)`,
      );
      lines.push(
        `- peak RSS: ${mebibytes(lane.resources.peakRssBytes)} (${lane.resources.peakRssSource})`,
      );
      lines.push(
        `- baseline RSS: ${mebibytes(lane.resources.baselineRssBytes)} · final RSS: ${mebibytes(lane.resources.finalRssBytes)}`,
      );
      lines.push(
        `- control-plane state growth: ${lane.resources.rssGrowthBytesPerIteration ?? "n/a"} bytes per sandbox session`,
      );
      lines.push(
        `- CPU time: ${short(lane.resources.cpuSeconds)} s (${lane.resources.cpuSecondsSource})`,
      );
      lines.push(
        `- context switches: voluntary ${short(lane.resources.voluntaryContextSwitches)} · involuntary ${short(lane.resources.involuntaryContextSwitches)}${lane.resources.counterAvailability ? ` — ${lane.resources.counterAvailability}` : ""}`,
      );
      lines.push(
        `- page faults: minor ${short(lane.resources.minorPageFaults)} · major ${short(lane.resources.majorPageFaults)}`,
      );
      lines.push(`- errors: ${lane.errors.total} — ${lane.errors.note}`);
      lines.push(`- scope: ${lane.scope}`);
    } else {
      lines.push("## Host floor (platform cost, no sandbox)");
      lines.push("");
      lines.push(
        `Process spawn (${lane.processSpawn.command}): p50 ${millis(lane.processSpawn.p50Ms)} ms · p95 ${millis(lane.processSpawn.p95Ms)} ms · max ${millis(lane.processSpawn.maxMs)} ms over ${lane.processSpawn.count} samples`,
      );
      lines.push("");
      lines.push(
        `Directory materialization (IOPS / bandwidth proxy): ${lane.directoryMaterialization.files} files / ${lane.directoryMaterialization.directories} dirs · ${lane.directoryMaterialization.elapsedMs} ms · ${lane.directoryMaterialization.filesPerSecond} files/s · ${lane.directoryMaterialization.mebibytesPerSecond} MiB/s`,
      );
      lines.push("");
      lines.push(`- ${lane.processSpawn.note}`);
      lines.push(`- ${lane.directoryMaterialization.note}`);
    }
  }
  lines.push("");
  lines.push("## Measurement conformance");
  lines.push("");
  lines.push(`Authority: ${report.conformance.authority}`);
  lines.push("");
  for (const [key, value] of Object.entries(report.conformance.recorded)) {
    lines.push(`- ${key}: ${value}`);
  }
  lines.push("");
  if (report.conformance.missingRequiredElements.length > 0) {
    lines.push(
      `- MISSING required elements: ${report.conformance.missingRequiredElements.join("; ")}`,
    );
  }
  lines.push(`- release-gate eligible: ${report.conformance.releaseGateEligible}`);
  lines.push(`- why not: ${report.conformance.releaseGateReason}`);
  return `${lines.join("\n")}\n`;
}

export async function main(argv) {
  const options = parseBenchmarkArgs(argv);
  if (options.help) {
    process.stdout.write(BENCHMARK_HELP);
    return;
  }
  const report = await runBenchmark(options, process.cwd());
  if (options.out) {
    const outPath = resolve(process.cwd(), options.out);
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
  }
  if (options.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    process.stdout.write(renderBenchmarkReport(report));
  }
}

const invokedDirectly =
  process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (invokedDirectly) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`sandbox lifecycle benchmark failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}
