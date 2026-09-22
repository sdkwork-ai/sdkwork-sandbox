#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/**
 * Host capability evidence runner.
 *
 * Gate 0 review packets repeatedly require "real Linux KVM evidence", "real OS evidence",
 * and "real platform conformance", but the repository has no reusable way to capture what a
 * target host can actually do. This runner probes a host read-only, classifies every capability
 * as verified / unsupported / denied / unverifiable, and emits machine-readable evidence.
 *
 * It is a diagnostic. It does not create a Provider, isolation policy, API route, or deployable
 * profile, and it must never attempt privilege escalation.
 */

const MARKER = "@@SBX@@|";
const STATUSES = new Set(["verified", "unsupported", "denied", "unverifiable"]);
const TARGETS = new Set(["local", "wsl"]);

const REQUIRED_CAPABILITY_IDS = [
  "namespace.mount-userns",
  "namespace.pid-userns",
  "namespace.net-userns",
  "cgroup.v2-mounted",
  "cgroup.controllers",
  "filesystem.overlayfs-supported",
  "security.seccomp-mode",
];

/**
 * Every capability id the probe can emit, including the parametric `namespace.*`, `toolchain.*`
 * and opt-in write-probe ids.
 *
 * `tools/check-sandbox-evidence-traceability.mjs` binds this declared set both to the emitted
 * probe text and to the evidence ids the repository contracts require, so a host-evidence claim
 * cannot silently drift away from what the probe actually reports.
 */
export const HOST_CAPABILITY_IDS = Object.freeze([
  "platform.kernel",
  "platform.arch",
  "platform.distro",
  ...[
    "mount",
    "pid",
    "uts",
    "ipc",
    "net",
    "cgroup",
    "time",
    "mount-userns",
    "pid-userns",
    "uts-userns",
    "ipc-userns",
    "net-userns",
    "cgroup-userns",
  ].map((name) => `namespace.${name}`),
  "cgroup.v2-mounted",
  "cgroup.controllers",
  "cgroup.root-writable",
  "cgroup.pids-controller",
  "cgroup.subtree-control",
  "cgroup.kill-file",
  "cgroup.events-file",
  "cgroup.delegation-writable",
  "cgroup.delegated-subtree-files",
  "filesystem.overlayfs-supported",
  "filesystem.tmpfs-supported",
  "filesystem.root-type",
  "filesystem.openat2-kernel-version",
  "filesystem.overlayfs-usable",
  "security.seccomp-mode",
  "security.no-new-privs",
  "security.caps-effective",
  "security.userns-budget",
  "security.lsm",
  "isolation.kvm-device",
  "isolation.numa-nodes",
  ...[
    "unshare",
    "mount",
    "ip",
    "nft",
    "iptables",
    "capsh",
    "firecracker",
    "jailer",
    "socat",
    "nsenter",
  ].map((name) => `toolchain.${name}`),
]);

function fail(message) {
  throw new Error(message);
}

export function parseHostCapabilityArgs(argv) {
  let target = "local";
  let distro = null;
  let out = null;
  let json = false;
  let probeWrite = false;
  let require = [];
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const next = () => argv[index + 1] ?? fail(`${argument} requires a value`);
    if (argument === "--target") {
      const value = next();
      index += 1;
      const [name, suffix] = value.split(":");
      if (!TARGETS.has(name)) {
        fail("--target must be local or wsl[:<distro>]");
      }
      target = name;
      distro = suffix ?? null;
      if (distro !== null && !/^[A-Za-z0-9._-]+$/u.test(distro)) {
        fail("--target wsl distro name must contain only ASCII letters, digits, dot, underscore or hyphen");
      }
    } else if (argument === "--out") {
      out = next();
      index += 1;
    } else if (argument === "--require") {
      require = next().split(",").map((item) => item.trim()).filter(Boolean);
      index += 1;
    } else if (argument === "--json") {
      json = true;
    } else if (argument === "--probe-write") {
      probeWrite = true;
    } else {
      fail(`unsupported argument: ${argument}`);
    }
  }
  return { target, distro, out, json, probeWrite, require };
}

/**
 * The probe script is intentionally POSIX-sh only, runs read-only by default, and never escalates
 * privilege: it invokes nothing but unprivileged inspection commands, and the opt-in write probes
 * are self-cleaning inside a user namespace.
 * It emits `@@SBX@@|<id>|ok|fail|na|<detail>` lines that parseHostCapabilityOutput classifies.
 */
export function buildHostCapabilityProbeScript({ probeWrite = false } = {}) {
  const lines = [
    "set -u",
    "emit() { printf '@@SBX@@|%s|%s|%s\\n' \"$1\" \"$2\" \"$3\"; }",
    "run() {",
    "  id=\"$1\"; shift",
    "  out=$(\"$@\" 2>&1); rc=$?",
    "  first=$(printf '%s' \"$out\" | head -n 1 | tr -d '\\r')",
    "  if [ \"$rc\" -eq 0 ]; then emit \"$id\" ok \"$*\"; else emit \"$id\" fail \"exit=$rc $first\"; fi",
    "}",
    "have() { command -v \"$1\" >/dev/null 2>&1; }",
    "",
    "# --- platform ---",
    "emit platform.kernel ok \"$(uname -srmo 2>&1 | tr -d '\\r')\"",
    "emit platform.arch ok \"$(uname -m 2>&1 | tr -d '\\r')\"",
    "if [ -r /etc/os-release ]; then",
    "  pretty=$(grep -m1 '^PRETTY_NAME=' /etc/os-release 2>/dev/null | cut -d= -f2- | tr -d '\\\"' | tr -d '\\r')",
    "  if [ -n \"$pretty\" ]; then emit platform.distro ok \"$pretty\"; else emit platform.distro na 'PRETTY_NAME absent'; fi",
    "else",
    "  emit platform.distro na 'no /etc/os-release'",
    "fi",
    "",
    "# --- namespaces (unshare is non-destructive; it must be, it only creates a namespace) ---",
    "if have unshare; then",
    "  run namespace.mount unshare --mount true",
    "  run namespace.pid unshare --pid --fork true",
    "  run namespace.uts unshare --uts true",
    "  run namespace.ipc unshare --ipc true",
    "  run namespace.net unshare --net true",
    "  run namespace.cgroup unshare --cgroup true",
    "  run namespace.time unshare --time true",
    "  run namespace.mount-userns unshare --user --map-root-user --mount true",
    "  run namespace.pid-userns unshare --user --map-root-user --pid --fork true",
    "  run namespace.uts-userns unshare --user --map-root-user --uts true",
    "  run namespace.ipc-userns unshare --user --map-root-user --ipc true",
    "  run namespace.net-userns unshare --user --map-root-user --net true",
    "  run namespace.cgroup-userns unshare --user --map-root-user --cgroup true",
    "else",
    "  for id in mount pid uts ipc net cgroup time mount-userns pid-userns uts-userns ipc-userns net-userns cgroup-userns; do emit \"namespace.$id\" na 'unshare not installed'; done",
    "fi",
    "",
    "# --- cgroup v2 ---",
    "if [ -d /sys/fs/cgroup ]; then",
    "  fs=$(stat -fc %T /sys/fs/cgroup 2>/dev/null | tr -d '\\r')",
    "  if [ \"$fs\" = 'cgroup2fs' ]; then emit cgroup.v2-mounted ok \"cgroup2fs\"; else emit cgroup.v2-mounted fail \"filesystem=$fs\"; fi",
    "  if [ -r /sys/fs/cgroup/cgroup.controllers ]; then",
    "    ctrl=$(cat /sys/fs/cgroup/cgroup.controllers 2>/dev/null | tr -d '\\r')",
    "    emit cgroup.controllers ok \"$ctrl\"",
    "    if [ -r /sys/fs/cgroup/cgroup.subtree_control ]; then sub=$(cat /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null | tr -d '\\r'); else sub=''; fi",
    "    emit cgroup.subtree-control ok \"enabled=[$sub]\"",
    "    case \" $ctrl \" in *' pids '*) avail=yes;; *) avail=no;; esac",
    "    case \" $sub \" in *' pids '*) en=yes;; *) en=no;; esac",
    "    if [ \"$avail\" = yes ] && [ \"$en\" = yes ]; then emit cgroup.pids-controller ok 'pids available and enabled in root subtree_control'; else emit cgroup.pids-controller fail \"pids available=$avail enabled-in-root-subtree=$en\"; fi",
    "  else",
    "    emit cgroup.controllers na 'cgroup.controllers not readable'",
    "    emit cgroup.subtree-control na 'cgroup.controllers not readable'",
    "    emit cgroup.pids-controller na 'cgroup.controllers not readable'",
    "  fi",
    "  if [ -e /sys/fs/cgroup/cgroup.kill ]; then emit cgroup.kill-file ok 'cgroup.kill present at the cgroup2 root'; else emit cgroup.kill-file fail 'cgroup.kill absent at the cgroup2 root (needs Linux >= 5.14, or only exists on non-root cgroups)'; fi",
    "  if [ -e /sys/fs/cgroup/cgroup.events ]; then emit cgroup.events-file ok 'cgroup.events present at the cgroup2 root'; else emit cgroup.events-file fail 'cgroup.events absent at the cgroup2 root'; fi",
    "  if [ -w /sys/fs/cgroup ]; then emit cgroup.root-writable ok 'root of cgroup2 is writable by this identity'; else emit cgroup.root-writable fail 'cgroup2 root is not writable by this identity'; fi",
    "else",
    "  emit cgroup.v2-mounted fail 'no /sys/fs/cgroup'",
    "  for id in controllers subtree-control pids-controller kill-file events-file root-writable; do emit \"cgroup.$id\" na 'no /sys/fs/cgroup'; done",
    "fi",
    "",
    "# --- filesystem ---",
    "if [ -r /proc/filesystems ]; then",
    "  if grep -qw overlay /proc/filesystems; then emit filesystem.overlayfs-supported ok 'overlay listed in /proc/filesystems'; else emit filesystem.overlayfs-supported fail 'overlay missing from /proc/filesystems'; fi",
    "  if grep -qw tmpfs /proc/filesystems; then emit filesystem.tmpfs-supported ok 'tmpfs listed in /proc/filesystems'; else emit filesystem.tmpfs-supported fail 'tmpfs missing from /proc/filesystems'; fi",
    "else",
    "  emit filesystem.overlayfs-supported na '/proc/filesystems not readable'",
    "  emit filesystem.tmpfs-supported na '/proc/filesystems not readable'",
    "fi",
    "if [ -d / ]; then emit filesystem.root-type ok \"stat -fc %T / -> $(stat -fc %T / 2>/dev/null | tr -d '\\r')\"; else emit filesystem.root-type na '/ not inspectable'; fi",
    "# The Local Host Boundary contract prefers an openat2 resolve-beneath primitive, which needs Linux >= 5.6.",
    "# Report the kernel release and the derived verdict so the reviewer judges the preferred primitive, not a guess.",
    "rel=$(uname -r 2>/dev/null | tr -d '\\r')",
    "maj=$(printf '%s' \"$rel\" | cut -d. -f1)",
    "min=$(printf '%s' \"$rel\" | cut -d. -f2)",
    "if [ -n \"$maj\" ] && [ -n \"$min\" ] && [ \"$maj\" -eq \"$maj\" ] 2>/dev/null && [ \"$min\" -eq \"$min\" ] 2>/dev/null; then",
    "  if [ \"$maj\" -gt 5 ] || { [ \"$maj\" -eq 5 ] && [ \"$min\" -ge 6 ]; }; then emit filesystem.openat2-kernel-version ok \"kernel $rel meets the openat2 floor (Linux >= 5.6)\"; else emit filesystem.openat2-kernel-version fail \"kernel $rel predates openat2 (needs Linux >= 5.6)\"; fi",
    "else",
    "  emit filesystem.openat2-kernel-version na \"unparseable kernel release: $rel\"",
    "fi",
    "",
    "# --- security ---",
    "if [ -r /proc/self/status ]; then",
    "  sec=$(grep -m1 '^Seccomp:' /proc/self/status 2>/dev/null | awk '{print $2}' | tr -d '\\r')",
    "  if [ -n \"$sec\" ]; then emit security.seccomp-mode ok \"mode=$sec\"; else emit security.seccomp-mode na 'Seccomp field absent'; fi",
    "  nnp=$(grep -m1 '^NoNewPrivs:' /proc/self/status 2>/dev/null | awk '{print $2}' | tr -d '\\r')",
    "  if [ -n \"$nnp\" ]; then emit security.no-new-privs ok \"NoNewPrivs=$nnp\"; else emit security.no-new-privs na 'NoNewPrivs field absent'; fi",
    "  ceff=$(grep -m1 '^CapEff:' /proc/self/status 2>/dev/null | awk '{print $2}' | tr -d '\\r')",
    "  cbnd=$(grep -m1 '^CapBnd:' /proc/self/status 2>/dev/null | awk '{print $2}' | tr -d '\\r')",
    "  if [ -n \"$ceff\" ] && [ -n \"$cbnd\" ]; then emit security.caps-effective ok \"CapEff=$ceff CapBnd=$cbnd\"; else emit security.caps-effective na 'CapEff/CapBnd fields absent'; fi",
    "else",
    "  for id in seccomp-mode no-new-privs caps-effective; do emit \"security.$id\" na '/proc/self/status not readable'; done",
    "fi",
    "if [ -r /proc/sys/user/max_user_namespaces ]; then",
    "  emit security.userns-budget ok \"max_user_namespaces=$(cat /proc/sys/user/max_user_namespaces | tr -d '\\r')\"",
    "else",
    "  emit security.userns-budget na 'user namespace budget not readable'",
    "fi",
    "if [ -r /sys/kernel/security/lsm ]; then emit security.lsm ok \"$(cat /sys/kernel/security/lsm | tr -d '\\r')\"; else emit security.lsm na 'LSM list not readable'; fi",
    "",
    "# --- isolation / virtualization ---",
    "if [ -e /dev/kvm ]; then",
    "  perm=$(ls -l /dev/kvm 2>/dev/null | awk '{print $1\" \"$3\" \"$4}' | tr -d '\\r')",
    "  if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then emit isolation.kvm-device ok \"read-write for current identity ($perm)\"; else emit isolation.kvm-device fail \"present but not read-write ($perm)\"; fi",
    "else",
    "  emit isolation.kvm-device fail 'no /dev/kvm'",
    "fi",
    "if [ -d /sys/devices/system/node ]; then",
    "  n=$(ls /sys/devices/system/node 2>/dev/null | grep -c '^node')",
    "  emit isolation.numa-nodes ok \"numa_nodes=$n\"",
    "else",
    "  emit isolation.numa-nodes na 'no /sys/devices/system/node'",
    "fi",
    "",
    "# --- toolchain ---",
    "for tool in unshare mount ip nft iptables capsh firecracker jailer socat nsenter; do",
    "  if have \"$tool\"; then emit \"toolchain.$tool\" ok \"$(command -v \"$tool\" | tr -d '\\r')\"; else emit \"toolchain.$tool\" na \"$tool not installed\"; fi",
    "done",
    "",
  ];

  if (probeWrite) {
    lines.push(
      "# --- opt-in write probes (self-cleaning, never touch existing data) ---",
      "# The Local Host Boundary contract makes per-binding delegated cgroup v2 membership mandatory and",
      "# names cgroup.kill, cgroup.events and the PID controller. Those files exist on non-root cgroups,",
      "# so creating and removing a real probe cgroup is the only honest way to observe them.",
      "d=/sys/fs/cgroup/sdkwork-capability-probe-$$",
      "if mkdir \"$d\" 2>/dev/null; then",
      "  present=''",
      "  for f in cgroup.kill cgroup.events cgroup.procs cgroup.subtree_control cgroup.type; do if [ -e \"$d/$f\" ]; then present=\"$present $f\"; fi; done",
      "  rmdir \"$d\" 2>/dev/null",
      "  emit cgroup.delegation-writable ok 'created and removed a probe cgroup'",
      "  emit cgroup.delegated-subtree-files ok \"present:$present\"",
      "else",
      "  why=$(mkdir \"$d\" 2>&1 | head -n 1 | tr -d '\\r')",
      "  emit cgroup.delegation-writable fail \"cannot create a delegated sub-cgroup: ${why:-unknown reason}\"",
      "  emit cgroup.delegated-subtree-files na 'no delegated sub-cgroup could be created'",
      "fi",
      "tmp=$(mktemp -d 2>/dev/null)",
      "if [ -n \"$tmp\" ]; then",
      "  mkdir -p \"$tmp/lower\" \"$tmp/upper\" \"$tmp/work\" \"$tmp/merged\" 2>/dev/null",
      "  if unshare --user --map-root-user --mount mount -t overlay overlay -o lowerdir=\"$tmp/lower\",upperdir=\"$tmp/upper\",workdir=\"$tmp/work\" \"$tmp/merged\" 2>/dev/null; then",
      "    umount \"$tmp/merged\" 2>/dev/null; emit filesystem.overlayfs-usable ok 'overlay mount succeeded inside a user+mount namespace'",
      "  else",
      "    emit filesystem.overlayfs-usable fail 'overlay mount failed inside a user+mount namespace'",
      "  fi",
      "  rm -rf \"$tmp\" 2>/dev/null",
      "else",
      "  emit filesystem.overlayfs-usable na 'mktemp unavailable'",
      "fi",
    );
  } else {
    lines.push(
      "# --- write probes disabled ---",
      "emit cgroup.delegation-writable na 'requires --probe-write'",
      "emit cgroup.delegated-subtree-files na 'requires --probe-write'",
      "emit filesystem.overlayfs-usable na 'requires --probe-write'",
    );
  }

  return lines.join("\n");
}

function classifyFailure(detail) {
  if (/Operation not permitted|Permission denied|must be superuser|not permitted/iu.test(detail)) {
    return "denied";
  }
  return "unsupported";
}

export function parseHostCapabilityOutput(stdout) {
  const checks = [];
  for (const rawLine of String(stdout).split("\n")) {
    const line = rawLine.trim();
    if (!line.startsWith(MARKER)) {
      continue;
    }
    const parts = line.slice(MARKER.length).split("|");
    if (parts.length < 3) {
      fail(`malformed probe line: ${line}`);
    }
    const [id, rawStatus, ...detailParts] = parts;
    const detail = detailParts.join("|");
    if (!/^[a-z0-9][a-z0-9.-]*$/u.test(id)) {
      fail(`malformed probe id: ${id}`);
    }
    let status;
    if (rawStatus === "ok") {
      status = "verified";
    } else if (rawStatus === "na") {
      status = "unverifiable";
    } else if (rawStatus === "fail") {
      status = classifyFailure(detail);
    } else {
      fail(`unsupported probe status: ${rawStatus}`);
    }
    if (!STATUSES.has(status)) {
      fail(`unsupported classified status: ${status}`);
    }
    checks.push({ id, status, detail });
  }
  return checks;
}

export function summarizeHostCapabilityChecks(checks) {
  const summary = { verified: 0, unsupported: 0, denied: 0, unverifiable: 0 };
  for (const check of checks) {
    summary[check.status] += 1;
  }
  const byId = new Map(checks.map((check) => [check.id, check]));
  const blocking = REQUIRED_CAPABILITY_IDS.filter(
    (id) => byId.get(id)?.status !== "verified",
  ).map((id) => ({
    id,
    status: byId.get(id)?.status ?? "missing",
    detail: byId.get(id)?.detail ?? "probe did not report this capability",
  }));
  return { ...summary, total: checks.length, blocking };
}

export function resolveProbeCommand({ target, distro }, script) {
  if (target === "local") {
    return { command: "bash", args: [], input: script };
  }
  if (target === "wsl") {
    const args = distro ? ["-d", distro, "--"] : [];
    args.push("bash", "-s");
    return { command: "wsl.exe", args, input: script };
  }
  fail(`unsupported target: ${target}`);
}

export function runHostCapabilityProbe(options) {
  const script = buildHostCapabilityProbeScript({ probeWrite: options.probeWrite });
  const invocation = resolveProbeCommand(options, script);
  const result = spawnSync(invocation.command, invocation.args, {
    input: invocation.input,
    encoding: "utf8",
    shell: false,
    maxBuffer: 4 * 1024 * 1024,
  });
  const stdout = result.stdout ?? "";
  const stderr = (result.stderr ?? "").trim();
  const checks = parseHostCapabilityOutput(stdout);
  if (checks.length === 0) {
    fail(
      `host capability probe produced no checks (exit=${result.status ?? "null"}${stderr ? `, stderr=${stderr.split("\n")[0]}` : ""})`,
    );
  }
  return {
    schemaVersion: 1,
    kind: "sdkwork.sandbox.host-capability-evidence",
    generatedAt: new Date().toISOString(),
    target: options.target,
    distro: options.distro ?? null,
    probeWrite: options.probeWrite === true,
    probeExitCode: result.status ?? null,
    checks,
    summary: summarizeHostCapabilityChecks(checks),
  };
}

export function reportHostCapabilityEvidence(evidence, { require = [] } = {}) {
  const lines = [
    `SDKWork Sandbox host capability evidence (target=${evidence.target}${evidence.distro ? `:${evidence.distro}` : ""}, probe-write=${evidence.probeWrite})`,
    "status      capability                          detail",
  ];
  for (const check of evidence.checks) {
    lines.push(`${check.status.padEnd(11)} ${check.id.padEnd(35)} ${check.detail}`);
  }
  const s = evidence.summary;
  lines.push("");
  lines.push(
    `summary: total=${s.total} verified=${s.verified} unsupported=${s.unsupported} denied=${s.denied} unverifiable=${s.unverifiable}`,
  );
  if (s.blocking.length > 0) {
    lines.push("blocking (required capability not verified):");
    for (const item of s.blocking) {
      lines.push(`- ${item.id}: ${item.status} (${item.detail})`);
    }
  } else {
    lines.push("blocking: none");
  }
  const missingRequired = require.filter(
    (id) => !evidence.checks.some((check) => check.id === id && check.status === "verified"),
  );
  if (missingRequired.length > 0) {
    lines.push(`--require not satisfied: ${missingRequired.join(", ")}`);
  }
  return { text: lines.join("\n"), missingRequired };
}

function main() {
  const options = parseHostCapabilityArgs(process.argv.slice(2));
  const evidence = runHostCapabilityProbe(options);
  const { text, missingRequired } = reportHostCapabilityEvidence(evidence, {
    require: options.require,
  });
  if (options.out) {
    const outPath = resolve(repositoryRoot, options.out);
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
    process.stdout.write(`host capability evidence written: ${outPath}\n`);
  }
  process.stdout.write(options.json ? `${JSON.stringify(evidence, null, 2)}\n` : `${text}\n`);
  if (missingRequired.length > 0) {
    process.exitCode = 1;
  }
}

const isMain = process.argv[1]
  && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));

if (isMain) {
  main();
}
