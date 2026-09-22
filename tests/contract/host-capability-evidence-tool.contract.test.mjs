import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  buildHostCapabilityProbeScript,
  parseHostCapabilityArgs,
  parseHostCapabilityOutput,
  resolveProbeCommand,
  summarizeHostCapabilityChecks,
} from "../../tools/testing/sandbox-host-capability-evidence.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const toolPath = path.join(repoRoot, "tools/testing/sandbox-host-capability-evidence.mjs");

test("host capability evidence defaults to a read-only local probe", () => {
  assert.deepEqual(parseHostCapabilityArgs([]), {
    target: "local",
    distro: null,
    out: null,
    json: false,
    probeWrite: false,
    require: [],
  });
});

test("host capability evidence accepts only supported targets and distro names", () => {
  assert.equal(parseHostCapabilityArgs(["--target", "wsl:Ubuntu-22.04"]).distro, "Ubuntu-22.04");
  assert.equal(parseHostCapabilityArgs(["--target", "wsl"]).distro, null);
  assert.throws(
    () => parseHostCapabilityArgs(["--target", "wsl:my distro"]),
    undefined,
    "distro names with spaces must be rejected",
  );
  assert.throws(() => parseHostCapabilityArgs(["--target", "wsl:bad/name"]));
  assert.throws(() => parseHostCapabilityArgs(["--target", "docker"]));
  assert.throws(() => parseHostCapabilityArgs(["--unknown"]));
  assert.throws(() => parseHostCapabilityArgs(["--target"]));
});

test("host capability evidence parses require lists and write opt-in", () => {
  const options = parseHostCapabilityArgs([
    "--probe-write",
    "--require",
    "namespace.mount-userns, cgroup.v2-mounted",
    "--json",
    "--out",
    "target/host.json",
  ]);

  assert.equal(options.probeWrite, true);
  assert.equal(options.json, true);
  assert.equal(options.out, "target/host.json");
  assert.deepEqual(options.require, ["namespace.mount-userns", "cgroup.v2-mounted"]);
});

test("host capability evidence keeps write probes opt-in", () => {
  const readOnly = buildHostCapabilityProbeScript();
  const writable = buildHostCapabilityProbeScript({ probeWrite: true });

  assert.match(readOnly, /cgroup\.delegation-writable na 'requires --probe-write'/u);
  assert.match(readOnly, /filesystem\.overlayfs-usable na 'requires --probe-write'/u);
  assert.doesNotMatch(readOnly, /capability-probe/u);

  assert.match(writable, /capability-probe/u);
  assert.match(writable, /mktemp -d/u);
  assert.match(writable, /filesystem\.overlayfs-usable/u);
  assert.doesNotMatch(writable, /requires --probe-write/u);
});

test("host capability probe never escalates privilege or touches foreign state", () => {
  const script = buildHostCapabilityProbeScript({ probeWrite: true });

  assert.doesNotMatch(script, /sudo/u);
  assert.doesNotMatch(script, /\bsu\b/u);
  assert.doesNotMatch(script, /setfacl|chown|chmod/u);
  assert.match(script, /rm -rf "\$tmp"/u);
  assert.match(script, /rmdir "\$d"/u);
  assert.match(script, /umount "\$tmp\/merged"/u);
  // cleanup must be scoped to the probe's own cgroup and the mktemp directory
  assert.doesNotMatch(script, /rm -rf \/(?!sys)/u);
});

test("host capability evidence classifies probe output into four states", () => {
  const stdout = [
    "noise before",
    "@@SBX@@|namespace.mount-userns|ok|unshare --user --map-root-user --mount true",
    "@@SBX@@|namespace.mount|fail|exit=1 unshare: unshare failed: Operation not permitted",
    "@@SBX@@|filesystem.tmpfs-supported|fail|tmpfs missing from /proc/filesystems",
    "@@SBX@@|toolchain.firecracker|na|firecracker not installed",
    "",
  ].join("\n");

  assert.deepEqual(parseHostCapabilityOutput(stdout), [
    {
      id: "namespace.mount-userns",
      status: "verified",
      detail: "unshare --user --map-root-user --mount true",
    },
    {
      id: "namespace.mount",
      status: "denied",
      detail: "exit=1 unshare: unshare failed: Operation not permitted",
    },
    {
      id: "filesystem.tmpfs-supported",
      status: "unsupported",
      detail: "tmpfs missing from /proc/filesystems",
    },
    {
      id: "toolchain.firecracker",
      status: "unverifiable",
      detail: "firecracker not installed",
    },
  ]);
});

test("host capability evidence rejects malformed probe output", () => {
  assert.throws(() => parseHostCapabilityOutput("@@SBX@@|only-two-fields|ok\n"));
  assert.throws(() => parseHostCapabilityOutput("@@SBX@@|Bad ID|ok|detail\n"));
  assert.throws(() => parseHostCapabilityOutput("@@SBX@@|id.here|weird|detail\n"));
});

test("host capability evidence preserves the pipe character inside details", () => {
  const parsed = parseHostCapabilityOutput("@@SBX@@|platform.kernel|ok|Linux 6.6 x86_64 | extra\n");

  assert.deepEqual(parsed, [
    { id: "platform.kernel", status: "verified", detail: "Linux 6.6 x86_64 | extra" },
  ]);
});

test("host capability evidence reports blocking required capabilities", () => {
  const summary = summarizeHostCapabilityChecks([
    { id: "namespace.mount-userns", status: "verified", detail: "ok" },
    { id: "namespace.pid-userns", status: "denied", detail: "exit=1 EPERM" },
    { id: "cgroup.v2-mounted", status: "verified", detail: "cgroup2fs" },
  ]);

  assert.equal(summary.total, 3);
  assert.equal(summary.verified, 2);
  assert.equal(summary.denied, 1);
  assert.equal(summary.blocking.length, 5, "the five unreported required capabilities must block");
  assert.deepEqual(
    summary.blocking.map((item) => item.id),
    [
      "namespace.pid-userns",
      "namespace.net-userns",
      "cgroup.controllers",
      "filesystem.overlayfs-supported",
      "security.seccomp-mode",
    ],
  );
  assert.equal(summary.blocking[0].status, "denied");
  assert.equal(summary.blocking[1].status, "missing");
});

test("host capability evidence resolves probe invocations without a shell", () => {
  assert.deepEqual(resolveProbeCommand({ target: "local" }, "script-body"), {
    command: "bash",
    args: [],
    input: "script-body",
  });
  assert.deepEqual(resolveProbeCommand({ target: "wsl", distro: "Ubuntu-22.04" }, "script-body"), {
    command: "wsl.exe",
    args: ["-d", "Ubuntu-22.04", "--", "bash", "-s"],
    input: "script-body",
  });
  assert.deepEqual(resolveProbeCommand({ target: "wsl", distro: null }, "script-body"), {
    command: "wsl.exe",
    args: ["bash", "-s"],
    input: "script-body",
  });
});

test("host capability evidence uses argument arrays and cannot create runtime authority", () => {
  const source = readFileSync(toolPath, "utf8");

  assert.match(source, /spawnSync\(invocation\.command, invocation\.args,/u);
  assert.match(source, /shell: false/u);
  assert.doesNotMatch(source, /execSync|shell:\s*true/u);
  assert.doesNotMatch(source, /sudo/u);
  // a diagnostic tool must not declare authorization or provider authority
  assert.doesNotMatch(source, /implementationAuthorized\s*:\s*true/u);
  assert.doesNotMatch(source, /SandboxProvider\b/u);
  assert.match(source, /implementationAuthorized|diagnostic/u);
});
