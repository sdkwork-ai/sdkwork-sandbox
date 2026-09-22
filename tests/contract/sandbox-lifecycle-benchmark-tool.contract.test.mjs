import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  computeStats,
  extractBalancedJson,
  parseBenchmarkArgs,
  parseBenchmarkResultLine,
  parseCpuClock,
  parseCsvRow,
  parseProcStat,
  parseProcStatusCounters,
  parseWindowsTasklistRow,
  renderBenchmarkReport,
  summarizeCapabilityEvidence,
} from "../../tools/bench-sandbox-lifecycle.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const toolPath = path.join(repoRoot, "tools/bench-sandbox-lifecycle.mjs");

test("benchmark arguments default to the control-plane lane with inert sampling", () => {
  const options = parseBenchmarkArgs([]);
  assert.equal(options.lane, "control-plane");
  assert.equal(options.iterations, 20_000);
  assert.equal(options.resourceIterations, 200_000);
  assert.equal(options.profile, "release");
  assert.equal(options.sampleIntervalMs, 500);
  assert.equal(options.out, null);
  assert.equal(options.json, false);
});

test("benchmark arguments accept every documented option", () => {
  const options = parseBenchmarkArgs([
    "--lane",
    "all",
    "--iterations",
    "10",
    "--resource-iterations",
    "20",
    "--profile",
    "debug",
    "--crate",
    "some-crate",
    "--host-floor-samples",
    "3",
    "--host-floor-files",
    "7",
    "--sample-interval-ms",
    "25",
    "--label",
    "test-label",
    "--out",
    "target/report.json",
    "--json",
  ]);
  assert.equal(options.lane, "all");
  assert.equal(options.iterations, 10);
  assert.equal(options.resourceIterations, 20);
  assert.equal(options.profile, "debug");
  assert.equal(options.crate, "some-crate");
  assert.equal(options.hostFloorSamples, 3);
  assert.equal(options.hostFloorFiles, 7);
  assert.equal(options.sampleIntervalMs, 25);
  assert.equal(options.label, "test-label");
  assert.equal(options.out, "target/report.json");
  assert.equal(options.json, true);
});

test("benchmark arguments fail closed on unusable input", () => {
  assert.throws(() => parseBenchmarkArgs(["--lane", "everything"]), /--lane must be/u);
  assert.throws(() => parseBenchmarkArgs(["--profile", "fast"]), /--profile must be/u);
  assert.throws(() => parseBenchmarkArgs(["--iterations", "0"]), /--iterations must be/u);
  assert.throws(() => parseBenchmarkArgs(["--iterations", "1.5"]), /--iterations must be/u);
  assert.throws(() => parseBenchmarkArgs(["--iterations"]), /requires a value/u);
  assert.throws(() => parseBenchmarkArgs(["--nope"]), /unsupported argument/u);
});

test("CSV rows keep embedded thousands separators inside quoted fields", () => {
  const cells = parseCsvRow('"node.exe","16020","Console","1","35,504 K","Unknown","host","0:00:00","N/A"');
  assert.deepEqual(cells.slice(0, 6), ["node.exe", "16020", "Console", "1", "35,504 K", "Unknown"]);
  assert.equal(cells[7], "0:00:00");
  assert.deepEqual(parseCsvRow('"a","b"'), ["a", "b"]);
  assert.deepEqual(parseCsvRow("plain,row"), ["plain", "row"]);
});

test("tasklist rows are read by shape, not by column position", () => {
  const row = parseWindowsTasklistRow(
    '"node.exe","16020","Console","1","35,504 K","Unknown","host\\\\user","0:01:02","N/A"',
  );
  assert.equal(row.rssBytes, 35_504 * 1024);
  assert.equal(row.cpuSeconds, 62);

  // A code-page-mangled trailing column and a reordered layout must still resolve.
  const reordered = parseWindowsTasklistRow('"proc","7","0:00:07","Console","12,000 K","junk"');
  assert.equal(reordered.rssBytes, 12_000 * 1024);
  assert.equal(reordered.cpuSeconds, 7);

  assert.equal(parseWindowsTasklistRow("INFO: No tasks are running which match the specified criteria."), null);
  assert.equal(parseWindowsTasklistRow(""), null);
});

test("CPU clock text parses in every shape tasklist emits", () => {
  assert.equal(parseCpuClock("0:00:00"), 0);
  assert.equal(parseCpuClock("0:01:02"), 62);
  assert.equal(parseCpuClock("2:00:00"), 7200);
  assert.equal(parseCpuClock("1:02:03"), 3723);
  assert.equal(parseCpuClock("not-a-clock"), null);
});

test("proc stat parses past a comm field containing spaces and parentheses", () => {
  // Real shape: pid (comm) state ppid ... Fields after comm: state is index 0, so minflt is index
  // 7, majflt is index 9, utime is index 11 and stime is index 12.
  const line =
    "4242 (some worker (v2)) R 1 1 0 -1 4194304 0 1200 0 7 0 150 50 0 0 0";
  assert.deepEqual(parseProcStat(line, 100), {
    cpuSeconds: 2,
    minorPageFaults: 1200,
    majorPageFaults: 7,
  });
  assert.deepEqual(parseProcStat(line, 250), {
    cpuSeconds: 0.8,
    minorPageFaults: 1200,
    majorPageFaults: 7,
  });
  assert.equal(parseProcStat("no parenthesis here", 100), null);
});

test("proc status counters expose both context-switch classes", () => {
  const status = [
    "Name:\tnode",
    "VmRSS:\t  35104 kB",
    "VmHWM:\t  40000 kB",
    "voluntary_ctxt_switches:\t12",
    "nonvoluntary_ctxt_switches:\t3",
  ].join("\n");
  assert.deepEqual(parseProcStatusCounters(status), {
    voluntaryContextSwitches: 12,
    involuntaryContextSwitches: 3,
  });
  assert.deepEqual(parseProcStatusCounters("Name:\tnode"), {
    voluntaryContextSwitches: null,
    involuntaryContextSwitches: null,
  });
});

test("host capability evidence reduces to the facts a benchmark must record", () => {
  const summary = summarizeCapabilityEvidence(
    JSON.stringify({
      target: "wsl",
      checks: {
        capabilities: [
          { id: "platform.kernel", status: "verified", detail: "Linux 6.6.87.2-microsoft-standard-WSL2" },
          { id: "platform.distro", status: "verified", detail: "Ubuntu 22.04.5 LTS" },
          { id: "cgroup.v2-mounted", status: "verified", detail: "cgroup2fs" },
          { id: "security.seccomp-mode", status: "verified", detail: "mode=2" },
          { id: "isolation.kvm-device", status: "unsupported", detail: "crw-rw---- root kvm" },
          { id: "namespace.mount", status: "denied", detail: "Operation not permitted" },
        ],
      },
    }),
  );
  assert.equal(summary.total, 6);
  assert.deepEqual(summary.classification, { verified: 4, unsupported: 1, denied: 1 });
  assert.equal(summary.distro, "Ubuntu 22.04.5 LTS");
  assert.equal(summary.cgroupV2, "cgroup2fs");
  assert.equal(summary.kvmDevice, "crw-rw---- root kvm");
  assert.equal(summary.seccompMode, "mode=2");
});

test("capability evidence counts each probe once even when the report repeats it under blocking", () => {
  // Real shape: `checks` lists every probe, `blocking` repeats the interesting ones. Counting every
  // object with id+status counted those twice and reported 54 checks for a 47-check host, which
  // then travelled into the rendered report as the machine's virtualization capability.
  const summary = summarizeCapabilityEvidence(
    JSON.stringify({
      target: "local",
      summary: { verified: 1, unsupported: 1, denied: 0, unverifiable: 1, total: 3 },
      checks: [
        { id: "platform.kernel", status: "verified", detail: "MINGW64_NT-10.0-26200" },
        { id: "cgroup.v2-mounted", status: "unsupported", detail: "no /sys/fs/cgroup" },
        { id: "security.seccomp-mode", status: "unverifiable", detail: "Seccomp field absent" },
      ],
      blocking: [
        { id: "cgroup.v2-mounted", status: "unsupported", detail: "no /sys/fs/cgroup" },
        { id: "security.seccomp-mode", status: "unverifiable", detail: "Seccomp field absent" },
      ],
    }),
  );
  assert.equal(summary.total, 3);
  assert.deepEqual(summary.classification, { verified: 1, unsupported: 1, unverifiable: 1 });
  assert.equal(summary.declaredTotal, 3);
  assert.equal(summary.totalMatchesDeclared, true);
  assert.equal(summary.cgroupV2, "no /sys/fs/cgroup");
});

test("a summarizer that disagrees with the probe's own census is flagged, not hidden", () => {
  const summary = summarizeCapabilityEvidence(
    JSON.stringify({
      target: "local",
      summary: { total: 47 },
      checks: [{ id: "platform.kernel", status: "verified", detail: "k" }],
    }),
  );
  assert.equal(summary.total, 1);
  assert.equal(summary.declaredTotal, 47);
  assert.equal(summary.totalMatchesDeclared, false);
});

test("statistics use exact nearest-rank percentiles", () => {
  const nanos = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((value) => value * 1_000_000);
  const stats = computeStats(nanos);
  assert.equal(stats.count, 10);
  assert.equal(stats.minMs, 1);
  assert.equal(stats.p50Ms, 5);
  assert.equal(stats.p90Ms, 9);
  assert.equal(stats.p95Ms, 10);
  assert.equal(stats.p99Ms, 10);
  assert.equal(stats.maxMs, 10);
  assert.equal(stats.meanMs, 5.5);
  assert.equal(stats.stdDevMs, 2.872);

  const single = computeStats([7_000_000]);
  assert.equal(single.count, 1);
  assert.equal(single.p99Ms, 7);
  assert.equal(single.stdDevMs, 0);

  assert.throws(() => computeStats([]), /at least one sample/u);
});

test("balanced JSON extraction ignores braces inside strings and nesting", () => {
  const text = 'noise {"a":{"b":[1,2]},"c":"} not a brace"} tail {"ignored":true}';
  assert.equal(extractBalancedJson(text, text.indexOf("{")), '{"a":{"b":[1,2]},"c":"} not a brace"}');
  assert.equal(extractBalancedJson('{"unterminated":1', 0), null);
});

test("the benchmark result is found even when libtest prefixes it on the same line", () => {
  // Regression: under --nocapture libtest writes `test <name> ... ` with no newline, so the result
  // lands mid-line. A line-anchored parser reported that the benchmark produced nothing.
  const payload = { iterations: 2, warmup: 1, createNanos: [100], startNanos: [200] };
  const interleaved = `running 1 test\ntest tests::bench ... ${"SDKWORK_BENCH_RESULT "}${JSON.stringify(payload)}\nok\n`;
  assert.deepEqual(parseBenchmarkResultLine(interleaved), payload);

  const onOwnLine = `noise\n${"SDKWORK_BENCH_RESULT "}${JSON.stringify(payload)}\nmore noise\n`;
  assert.deepEqual(parseBenchmarkResultLine(onOwnLine), payload);

  assert.throws(() => parseBenchmarkResultLine("test tests::bench ... ok"), /produced no/u);
  assert.throws(() => parseBenchmarkResultLine("SDKWORK_BENCH_RESULT "), /not followed by/u);
  assert.throws(() => parseBenchmarkResultLine("SDKWORK_BENCH_RESULT {broken"), /not terminated/u);
});

test("the rendered report states the measured layer and the Windows peak-RSS caveat", () => {
  const perPhase = {
    count: 10,
    minMs: 1,
    p50Ms: 2,
    p90Ms: 3,
    p95Ms: 4,
    p99Ms: 5,
    maxMs: 6,
    meanMs: 2.5,
    stdDevMs: 0.5,
  };
  const report = {
    schemaVersion: 1,
    kind: "sdkwork-sandbox-lifecycle-benchmark",
    generatedAt: "2026-09-22T00:00:00.000Z",
    label: "unit-test",
    machine: {
      cpuModel: "Test CPU",
      logicalCpus: 8,
      totalMemoryBytes: 16 * 1024 * 1024 * 1024,
      platform: "win32",
      arch: "x64",
      kernel: "10.0.26200",
      node: "v22.0.0",
    },
    capabilityEvidence: {
      path: "target/caps.json",
      summary: {
        target: "local",
        total: 3,
        classification: { verified: 2, unsupported: 1 },
        kernel: "MINGW64_NT-10.0",
        distro: null,
        cgroupV2: "no /sys/fs/cgroup",
        kvmDevice: "absent",
        seccompMode: "Seccomp field absent",
      },
    },
    conformance: {
      authority: "TECH-performance-and-capacity.md sections 5, 6 and 9",
      recorded: {
        machineSpec: "Test CPU · 8 logical cpus",
        kernelVersion: "10.0.26200",
        virtualizationCapability: "target/caps.json",
        templateVersion: "not applicable",
        artifactTuple: "not applicable",
        sampleSize: "10 measured + 2 warmup",
        concurrencyLevel: "1",
        statisticalMethod: "nearest-rank percentiles",
      },
      missingRequiredElements: [],
      releaseGateEligible: false,
      releaseGateReason: "no real-hardware benchmark",
    },
    lanes: [
      {
        lane: "control-plane",
        benchmarkTest: "tests::bench",
        iterations: 10,
        warmupIterations: 2,
        latencyWallMs: 5,
        coldFirstPass: { createMs: 0.9, startMs: 1.2, endToEndMs: 2.1, note: "first pass" },
        create: perPhase,
        start: perPhase,
        endToEnd: perPhase,
        errors: { total: 0, policyRejections: 0, systemFaults: 0, note: "scripted provider" },
        resources: {
          pass: "second",
          iterations: 100,
          wallMs: 50,
          requestedIntervalMs: 500,
          effectiveIntervalMs: 600,
          samplerCostMs: 540,
          sampleCount: 3,
          peakRssBytes: 10 * 1024 * 1024,
          peakRssSource: "maximum of sampled working set (Windows exposes no portable peak counter)",
          baselineRssBytes: 5 * 1024 * 1024,
          finalRssBytes: 9 * 1024 * 1024,
          rssGrowthBytesPerIteration: 40_960,
          cpuSeconds: 0.5,
          cpuSecondsSource: "tasklist /v total processor time delta",
          voluntaryContextSwitches: null,
          involuntaryContextSwitches: null,
          minorPageFaults: null,
          majorPageFaults: null,
          counterAvailability: "not available on this platform",
        },
        scope: "Control-plane orchestration only.",
      },
    ],
  };
  const rendered = renderBenchmarkReport(report);
  assert.match(rendered, /Control plane \(create -> start\)/u);
  assert.match(rendered, /no sampler attached/u);
  assert.match(rendered, /one sampling call costs 540 ms/u);
  assert.match(rendered, /no portable peak counter/u);
  assert.match(rendered, /Cold path/u);
  assert.match(rendered, /Control-plane orchestration only\./u);
  assert.match(rendered, /10\.0 MiB/u);
  assert.match(rendered, /Measurement conformance/u);
  assert.match(rendered, /release-gate eligible: false/u);
  assert.match(rendered, /no real-hardware benchmark/u);
  assert.match(rendered, /not available on this platform/u);
  assert.match(rendered, /Test CPU · 8 logical cpus/u);
});

test("the host-floor lane runs end to end and reports a complete shape", () => {
  const result = spawnSync(
    process.execPath,
    [
      toolPath,
      "--lane",
      "host-floor",
      "--host-floor-samples",
      "3",
      "--host-floor-files",
      "6",
      "--json",
    ],
    { cwd: repoRoot, encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
  );
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.kind, "sdkwork-sandbox-lifecycle-benchmark");
  assert.equal(report.lanes.length, 1);
  const lane = report.lanes[0];
  assert.equal(lane.lane, "host-floor");
  assert.equal(lane.processSpawn.count, 3);
  assert.ok(lane.processSpawn.p50Ms >= 0);
  assert.equal(lane.directoryMaterialization.directories, 6);
  assert.equal(lane.directoryMaterialization.files, 6);
  assert.equal(lane.directoryMaterialization.bytes, 6 * 4096);
  assert.ok(lane.directoryMaterialization.filesPerSecond > 0);
  assert.equal(lane.processSpawn.includesHarnessOverhead, true);
});

test("the CLI fails loudly on an unsupported argument", () => {
  const result = spawnSync(process.execPath, [toolPath, "--bogus"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /unsupported argument/u);
});
