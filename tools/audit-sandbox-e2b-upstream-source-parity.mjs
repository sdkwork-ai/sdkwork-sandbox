#!/usr/bin/env node
// E2B upstream source-parity sampler.
//
// Why this is a sampler and not a gate: the upstream sources live in `external/`, which is
// gitignored because upstream code is not this repository's property. A gate must be runnable on a
// fresh clone, so a check whose subject may legitimately be absent cannot be one. This tool
// therefore re-derives the source-level facts, compares them against the values recorded below, and
// reports drift; it is deliberately absent from `_sdkwork:check`.
//
// What it answers, in the sense the repository's evidence discipline asks for:
//
//   * Did the vendored upstream move?           every repository's HEAD against its recorded SHA.
//   * Did the recorded readings move?           operations, RPC methods, versions, module count,
//                                              language mix, across the four upstream repositories.
//   * Did the documentation-derived baseline move?
//                                              with `--fetch`, re-fetch the authoritative
//                                              `openapi-public.yaml` and `llms.txt` and compare the
//                                              sha256 against `specs/sandbox-e2b-capability-baseline.json`.
//
// Two extraction traps are encoded here because both were hit while building this tool, and each is
// the same failure mode the repository's parity gates were hardened against — an extractor that
// silently drops already-covered items reports a gap that does not exist:
//
//   1. **Indentation is not uniform across the upstream proto files.** `filesystem.proto` indents
//      `rpc` by two spaces and `process.proto` by four. A pattern pinned to either width drops half
//      the methods and still parses. `^\s*rpc\s+` with `\s*` on both flanks counts 17; a two-space
//      pattern counts 9 and a four-space pattern counts 8.
//   2. **A hand count is not a count.** The first pass read 16 where the extractor said 17, because
//      `Stat` sorts between `Start` and `StreamInput` in a space-separated listing. The recorded
//      value below is the extractor's output, re-derived on every run, not a number typed from a
//      glance at the output.
//   3. **A comment can contain the thing it documents.** `envd.yaml` explains its `x-internal`
//      marker using the marker's own text, so counting matches over the whole file reads 7 markers
//      where the file declares 6. The scan is line-anchored and scoped to a path block; see
//      `scanEnvdOrchestratorOnlyPaths`. All three traps were found by the mutation proof beside this
//      tool, not by reading the code.
//
// Exit codes:
//   0  every recorded reading reproduced and no upstream commit moved.
//   1  drift: a recorded reading changed, or a vendored HEAD no longer matches what is recorded.
//   2  usage error.
//   3  not vendored: `external/` is absent, so nothing could be sampled. Deliberately distinct from
//      0 — "could not sample" and "sampled and clean" must not look the same.
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const externalDirectory = join(repositoryRoot, "external");
const evidenceDirectory = join(repositoryRoot, "target", "e2b-upstream-source-parity");
const docsCaptureDirectory = join(repositoryRoot, "target", "e2b-baseline");
const baselinePath = join(repositoryRoot, "specs", "sandbox-e2b-capability-baseline.json");

/**
 * The vendored repositories, with the commit each reading below was taken at. A HEAD that no longer
 * matches means every reading attributed to that repository is a statement about a different tree,
 * so the tool reports it as drift rather than re-baselining silently.
 */
const RECORDED_REPOSITORIES = Object.freeze({
  infra: "0c21aa2277b59a1d040761ed3fbbb29a78775470",
  E2B: "ccaf9fc0ffe6ac39c7ec786af7608ab1de19467b",
  desktop: "17ddc44f31080af9f2d0fa0fa767525fefd9882c",
  "code-interpreter": "f56a1edf750e20a96f847df57e0063eeeb5e13c3",
});

/**
 * Source-level readings, each re-derived on every run from the tree named in its `source` field.
 * Every value here was produced by the extractor in this file and checked against an independent
 * parser (PyYAML) where one was available; none was transcribed by eye.
 */
const RECORDED_READINGS = Object.freeze({
  controlPlaneOperations: 74,
  controlPlanePaths: 57,
  envdRpcMethods: 17,
  envdServices: 2,
  envdRestEndpoints: 11,
  /**
   * envd's streaming directions. Four of the seventeen RPCs stream, and which side streams decides
   * what a client must implement: `WatchDir`, `Connect` and `Start` stream from the server, while
   * `StreamInput` is the only one that streams from the client.
   */
  envdServerStreamingMethods: 3,
  envdClientStreamingMethods: 1,
  /**
   * envd's error surface: the gRPC status codes both SDKs translate into their own error families.
   * The two tables must map the same codes — a code handled on one side only is a request that
   * fails with a transport error in one language and a typed error in the other.
   */
  envdErrorCodesMapped: 7,
  /**
   * The `ENVD_*` version gates both SDKs negotiate features against. Nine of them, byte-identical in
   * name and value, and they matter because they are the only place a client learns which envd
   * capabilities it may use; a gate that exists on one side only is a feature one language cannot
   * reach. Read the gate table, not `spec/envd/envd.yaml`'s `info.version` (0.1.3), which is the
   * document's own revision and is far behind the 0.6.4 the gates require.
   */
  envdVersionGates: 9,
  /** Error families, compared across the two SDKs after stripping the language's suffix. */
  sdkErrorFamilies: 20,
  /**
   * envd's REST contract is split by an `x-internal: true` marker into the orchestrator's control
   * operations and the operations a sandbox client may reach. The two halves sum to
   * `envdRestEndpoints`; recording them separately is what keeps a marker added to the wrong
   * operation visible, since the total would not move. See `envdOrchestratorOnlyPaths` below.
   */
  envdOrchestratorOnlyEndpoints: 6,
  envdClientReachableEndpoints: 5,
  goWorkModules: 12,
  infraRustFiles: 0,
  /** Documented by the audit as the count the documentation-derived baseline carries. */
  documentedOperations: 71,
});

/**
 * The paths `spec/envd/envd.yaml` marks `x-internal: true`. Upstream's comment states the sandbox
 * proxy derives its rejection list from this marker, so the marker — not proxy code — is the
 * authority for what may not be reached through the public sandbox URL. Recorded by name, because a
 * marker moved from one operation to another leaves the count unchanged and only the names reveal it.
 */
const RECORDED_ENVD_ORCHESTRATOR_ONLY_PATHS = Object.freeze([
  "/init",
  "/freeze",
  "/unfreeze",
  "/collapse",
  "/fsfreeze",
  "/fsthaw",
]);

/**
 * Auxiliary contracts: documents that carry an independent surface but are deliberately not part of
 * the public SDK. Recorded as explicit `<repository>:<path>` pairs rather than a count.
 *
 * This replaced a bare count after the sampler's own first run reported a false drift — it recorded
 * four while listing three, because the number and the list disagreed about whether the primary
 * control-plane contract counted as auxiliary. A count cannot say *which* document moved, and when
 * the number and the list disagree the count silently wins and reads as "upstream changed" instead
 * of "this tool is counting the wrong tree". Naming each document makes the difference say which.
 */
const RECORDED_AUXILIARY_CONTRACTS = Object.freeze([
  "infra:spec/openapi-edge.yml",
  "infra:spec/openapi-hyperloop.yml",
  "infra:spec/openapi-dashboard.yml",
  "E2B:spec/openapi-volumecontent.yml",
  "E2B:spec/mcp-server.json",
  "E2B:spec/envd/envd.yaml",
]);

/**
 * The control-plane contract is vendored twice: `infra/spec/openapi.yml` is the control plane's own
 * copy and `E2B/spec/openapi.yml` is the copy the SDK repository carries. They agree on the
 * operation surface but their schema bodies have diverged, so both are sampled. Reading only one
 * would let the other drift unwatched while the sampler reported the single number it does read.
 */
const TWIN_CONTROL_PLANE_CONTRACTS = Object.freeze({
  E2B: "spec/openapi.yml",
  infra: "spec/openapi.yml",
});

/**
 * The two SDK-visible operations that exist in the upstream control-plane contract but are absent
 * from the documentation-derived baseline. Both appear in the generated clients, so a baseline that
 * omits them understates the SDK surface rather than merely lagging a spec.
 */
const RECORDED_SOURCE_ONLY_SDK_OPERATIONS = Object.freeze([
  "POST /v2/sandboxes",
  "POST /v2/sandboxes/{sandboxID}/connect",
]);

/** Versions read from each package's own manifest, not from a documentation page. */
const RECORDED_VERSIONS = Object.freeze({
  "packages/js-sdk/package.json": "2.51.0",
  "packages/python-sdk/pyproject.toml": "2.51.0",
  "packages/cli/package.json": "2.20.0",
  "packages/code-interpreter-js/package.json": "2.8.0",
  "packages/code-interpreter-python/pyproject.toml": "2.10.0",
  "packages/desktop-js/package.json": "2.4.0",
  "packages/desktop-python/pyproject.toml": "2.6.0",
});

/**
 * The two SDK packages whose public error surface is a cross-language contract. The names are
 * `Error`-suffixed in TypeScript and `Exception`-suffixed in Python, so families are compared after
 * stripping that suffix — otherwise every family looks like a divergence.
 */
const SDK_ERROR_SOURCES = Object.freeze({
  javascript: "packages/js-sdk/src/errors.ts",
  python: "packages/python-sdk/e2b/exceptions.py",
});

/** The two envd version-gate tables, which must declare the same features at the same versions. */
const ENVD_VERSION_GATES = Object.freeze({
  javascript: "packages/js-sdk/src/envd/versions.ts",
  python: "packages/python-sdk/e2b/envd/versions.py",
});

/** The two gRPC-status → error tables for the data plane. */
const ENVD_ERROR_MAPS = Object.freeze({
  javascript: "packages/js-sdk/src/envd/rpc.ts",
  python: "packages/python-sdk/e2b/envd/rpc.py",
});

/** The `infra` files that own the operational constants the review document's table cites. */
const PERFORMANCE_CONSTANT_SOURCES = Object.freeze({
  featureFlags: "packages/shared/pkg/featureflags/flags.go",
  inPlaceFlip: "packages/orchestrator/pkg/sandbox/sandbox.go",
  uffdLimits: "packages/orchestrator/pkg/sandbox/uffd/userfaultfd/userfaultfd.go",
  networkPool: "packages/orchestrator/pkg/sandbox/network/pool.go",
  templateCache: "packages/orchestrator/pkg/sandbox/template/cache.go",
  templateStatus: "packages/api/internal/template-manager/template_status.go",
  storage: "packages/shared/pkg/storage/storage.go",
  storageHeader: "packages/shared/pkg/storage/header/diff.go",
});

/**
 * The performance-relevant constants the review document's comparison table cites, each with the
 * file that owns it and the *kind* of value it is.
 *
 * `kind` is not decoration. A `const` is compiled in, so only a rebuild moves it; a `flag-default`
 * is overridden at runtime by the flag service without a deploy, which makes it a default rather
 * than a limit — a capacity model that reads one as a hard ceiling disagrees with the running
 * service. An earlier round of the same document flattened both kinds into one column and misnamed
 * `BestOfKMaxOvercommit` as `MaxOvercommit` while doing it. That is why every value below is
 * produced by `scanPerformanceConstant` and compared against a recorded set, never typed by hand:
 * a hand-typed value is exactly as reliable as whoever typed it, and nothing here would notice.
 */
const PERFORMANCE_CONSTANTS = Object.freeze([
  { name: "inPlaceStateFlipTimeout", source: "inPlaceFlip", kind: "const" },
  { name: "maxRequestsInProgress", source: "uffdLimits", kind: "const" },
  { name: "maxWPResolvesInProgress", source: "uffdLimits", kind: "const" },
  { name: "NewSlotsPoolSize", source: "networkPool", kind: "const" },
  { name: "ReusedSlotsPoolSize", source: "networkPool", kind: "const" },
  { name: "ReturnDelay", source: "networkPool", kind: "const" },
  { name: "templateExpiration", source: "templateCache", kind: "const" },
  { name: "templateExpirationBuffer", source: "templateCache", kind: "const" },
  { name: "buildCacheTTL", source: "templateCache", kind: "const" },
  { name: "buildCacheDelayEviction", source: "templateCache", kind: "const" },
  { name: "buildTimeout", source: "templateStatus", kind: "const" },
  { name: "MemoryChunkSize", source: "storage", kind: "const" },
  { name: "HugepageSize", source: "storageHeader", kind: "const" },
  { name: "MemoryPrefetchMaxFetchWorkers", source: "featureFlags", kind: "flag-default" },
  { name: "MemoryPrefetchMaxCopyWorkers", source: "featureFlags", kind: "flag-default" },
  { name: "MaxSandboxesPerNode", source: "featureFlags", kind: "flag-default" },
  { name: "MaxStartingInstancesPerNode", source: "featureFlags", kind: "flag-default" },
  { name: "BestOfKSampleSize", source: "featureFlags", kind: "flag-default" },
  { name: "BestOfKMaxOvercommit", source: "featureFlags", kind: "flag-default" },
  { name: "BestOfKAlpha", source: "featureFlags", kind: "flag-default" },
  { name: "BuildBaseRootfsSizeLimitMB", source: "featureFlags", kind: "flag-default" },
]);

/**
 * The values those twenty-one constants held at the vendored commits. Recorded as `name=value` strings
 * rather than a count, for the reason every reading here is: a count survives a value being
 * re-baselined, and a re-baselined default is the news.
 *
 * Three of the flags store a *percentage* integer rather than the ratio they name — `BestOfKAlpha`
 * is 50 for the α = 0.5 the placement comment describes, and `BestOfKMaxOvercommit` is 400 for the
 * 4× the comment describes. The recorded strings keep the stored value, not the semantic one, so
 * the unit convention cannot be lost by someone reading this list as the arithmetic value.
 */
const RECORDED_PERFORMANCE_CONSTANTS = Object.freeze([
  "BestOfKAlpha=50",
  "BestOfKMaxOvercommit=400",
  "BestOfKSampleSize=3",
  "BuildBaseRootfsSizeLimitMB=25000",
  "HugepageSize=2 << 20",
  "MaxSandboxesPerNode=200",
  "MaxStartingInstancesPerNode=3",
  "MemoryChunkSize=4 * 1024 * 1024",
  "MemoryPrefetchMaxCopyWorkers=8",
  "MemoryPrefetchMaxFetchWorkers=16",
  "NewSlotsPoolSize=32",
  "ReturnDelay=3 * time.Second",
  "ReusedSlotsPoolSize=100",
  "buildCacheDelayEviction=time.Second * 60",
  "buildCacheTTL=time.Hour * 25",
  "buildTimeout=time.Hour",
  "inPlaceStateFlipTimeout=40 * time.Second",
  "maxRequestsInProgress=4096",
  "maxWPResolvesInProgress=256",
  "templateExpiration=time.Hour * 25",
  "templateExpirationBuffer=time.Hour",
]);

/**
 * The runtime key each of the eight flag defaults is reachable under. Recorded apart from the values
 * because a key and a default fail differently: renaming a key breaks every override already set
 * while the default it falls back to is unchanged, so a reading that only tracks values reports
 * "no drift" through a change that silently reverts an operator's tuning.
 */
const RECORDED_PERFORMANCE_FLAG_KEYS = Object.freeze({
  BestOfKAlpha: "best-of-k-alpha",
  BestOfKMaxOvercommit: "best-of-k-max-overcommit",
  BestOfKSampleSize: "best-of-k-sample-size",
  BuildBaseRootfsSizeLimitMB: "build-base-rootfs-size-limit-mb",
  MaxSandboxesPerNode: "max-sandboxes-per-node",
  MaxStartingInstancesPerNode: "max-starting-instances-per-node",
  MemoryPrefetchMaxCopyWorkers: "memory-prefetch-max-copy-workers",
  MemoryPrefetchMaxFetchWorkers: "memory-prefetch-max-fetch-workers",
});

/**
 * The SDK surfaces the review document names, each with *every* file that declares part of it.
 *
 * A surface is a file list per language, not a file, because upstream does not split them the same
 * way in both languages: JavaScript keeps the whole `Secret` surface in `secret.ts`, while Python
 * splits it between `secret/base.py` (the static `fill` and `iam_token`) and `secret/secret_sync.py`
 * (the REST-backed members). Assuming one file per language reads six Python members where there are
 * nine, and the three that disappear are exactly the ones Python keeps in a base class.
 */
const SDK_METHOD_SURFACES = Object.freeze({
  git: {
    javascript: ["packages/js-sdk/src/sandbox/git/index.ts"],
    python: ["packages/python-sdk/e2b/sandbox_sync/git.py"],
  },
  volume: {
    javascript: ["packages/js-sdk/src/volume/index.ts"],
    python: ["packages/python-sdk/e2b/volume/volume_sync.py"],
  },
  secret: {
    javascript: ["packages/js-sdk/src/secret.ts"],
    python: [
      "packages/python-sdk/e2b/secret/base.py",
      "packages/python-sdk/e2b/secret/secret_sync.py",
    ],
  },
  pty: {
    javascript: ["packages/js-sdk/src/sandbox/commands/pty.ts"],
    python: ["packages/python-sdk/e2b/sandbox_sync/commands/pty.py"],
  },
});

/** TypeScript statement keywords that can be followed by `(` at class-member indentation. */
const TYPESCRIPT_STATEMENT_KEYWORDS = new Set([
  "if",
  "for",
  "while",
  "switch",
  "catch",
  "return",
  "throw",
  "await",
  "typeof",
  "else",
  "do",
  "delete",
  "new",
  "yield",
  "void",
  "case",
  "function",
]);

/** Decorators that make a Python `def` an attribute rather than a method. */
const PYTHON_PROPERTY_DECORATORS = new Set([
  "@property",
  "@cached_property",
  "@functools.cached_property",
]);

/**
 * Public-API names that exist on one side of a surface only, after the naming convention is
 * normalised away. Encoded `<surface>:<javascriptName>=<pythonName>`, with the empty side meaning the
 * name is absent there.
 *
 * This plane exists because of the PTY entry. JavaScript calls the operation `sendInput` and Python
 * calls it `send_stdin`; normalising snake_case to camelCase removes the *convention* difference and
 * leaves that one standing, which is the point — a caller porting a PTY handler keeps compiling
 * against its own SDK and has to rename the call. The volume pair is the same shape of news in a
 * different direction: `Volume.getInfo` and `Volume.list` are public statics in JavaScript and
 * private (`_class_get_info`, `_class_list`) in Python, so the capability is reachable in one SDK and
 * not in the other.
 *
 * The tool cannot know that `sendInput` and `send_stdin` name the same operation — that pairing is a
 * human judgement. It reports each one-sided name separately, and the review document records which
 * pairs correspond.
 */
const RECORDED_SDK_METHOD_DIVERGENCES = Object.freeze([
  "pty:=sendStdin",
  "pty:sendInput=",
  "volume:getInfo=",
  "volume:list=",
]);

/** Every public member name of every surface, as `surface:name`, across both languages together. */
const RECORDED_SDK_SURFACE_MEMBERS = Object.freeze([
  "git:add",
  "git:branches",
  "git:checkoutBranch",
  "git:clone",
  "git:commit",
  "git:configureUser",
  "git:createBranch",
  "git:dangerouslyAuthenticate",
  "git:deleteBranch",
  "git:getConfig",
  "git:init",
  "git:pull",
  "git:push",
  "git:remoteAdd",
  "git:remoteGet",
  "git:reset",
  "git:restore",
  "git:setConfig",
  "git:status",
  "pty:connect",
  "pty:create",
  "pty:kill",
  "pty:resize",
  "pty:sendInput",
  "pty:sendStdin",
  "secret:create",
  "secret:destroy",
  "secret:exists",
  "secret:fill",
  "secret:getInfo",
  "secret:iamToken",
  "secret:list",
  "secret:nextItems",
  "secret:update",
  "volume:connect",
  "volume:create",
  "volume:destroy",
  "volume:exists",
  "volume:getInfo",
  "volume:list",
  "volume:makeDir",
  "volume:readFile",
  "volume:remove",
  "volume:updateMetadata",
  "volume:writeFile",
]);

/**
 * Error families whose parent class differs between the two SDKs.
 *
 * `VolumeNotFoundError` / `VolumePathNotFoundError` extend the volume base in TypeScript but the
 * not-found base in Python. The consequence is a portability trap rather than a cosmetic
 * difference: `catch (e instanceof VolumeError)` covers a missing volume in JS and not in Python,
 * while `except NotFoundException` covers it in Python and not in JS. A caller porting a handler
 * between the two SDKs keeps compiling and silently stops catching the case.
 *
 * Recorded as a set so the sampler reports when it *moves in either direction* — a new divergence
 * appearing is the case to catch, and one disappearing upstream is worth a human look before this
 * list is shrunk.
 */
const RECORDED_SDK_INHERITANCE_DIVERGENCES = Object.freeze([
  "volumenotfound",
  "volumepathnotfound",
]);

const HTTP_METHODS = new Set(["get", "post", "put", "patch", "delete", "head", "options", "trace"]);

function fail(message) {
  throw new Error(message);
}

function readText(path) {
  return readFileSync(path, "utf8");
}

function safeReadText(path) {
  return existsSync(path) ? readText(path) : null;
}

/** Walk a tree and return every file whose extension matches. */
function walkFiles(root, extension) {
  const found = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const full = join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(full);
      } else if (entry.isFile() && extname(entry.name) === extension) {
        found.push(full);
      }
    }
  };
  visit(root);
  return found.sort();
}

/**
 * Resolve a clone's HEAD without invoking git, so the sampler works wherever the tree does.
 * A shallow single-branch clone keeps the branch ref loose; `packed-refs` is the fallback.
 */
export function readClonedHead(repositoryPath) {
  const headPath = join(repositoryPath, ".git", "HEAD");
  if (!existsSync(headPath)) {
    return null;
  }
  const head = readText(headPath).trim();
  const reference = /^ref:\s*(.+)$/u.exec(head);
  if (!reference) {
    return head;
  }
  const loose = join(repositoryPath, ".git", reference[1]);
  if (existsSync(loose)) {
    return readText(loose).trim();
  }
  const packedPath = join(repositoryPath, ".git", "packed-refs");
  if (existsSync(packedPath)) {
    const packed = readText(packedPath)
      .split("\n")
      .map((line) => /^([0-9a-f]{40})\s+(.+)$/u.exec(line))
      .find((match) => match && match[2] === reference[1]);
    if (packed) {
      return packed[1];
    }
  }
  return null;
}

/**
 * Extract `METHOD path` pairs from an OpenAPI document by line scanning.
 *
 * The upstream control-plane contract carries no `operationId`, so the method and path are the only
 * identifiers it has. The scan is indentation-sensitive on purpose: `paths:` is dropped to run
 * until the next top-level key, path keys sit at two spaces and method keys at four. It was
 * validated against PyYAML on `external/infra/spec/openapi.yml`: both report 74 operations across
 * 57 paths.
 */
export function scanOpenApiOperations(text) {
  const operations = [];
  let insidePaths = false;
  let currentPath = null;
  for (const line of text.split("\n")) {
    if (/^paths:\s*$/u.test(line)) {
      insidePaths = true;
      continue;
    }
    if (insidePaths && /^[A-Za-z]/u.test(line)) {
      break;
    }
    if (!insidePaths) {
      continue;
    }
    const path = /^ {2}(\/\S*):\s*$/u.exec(line);
    if (path) {
      currentPath = path[1];
      continue;
    }
    const method = /^ {4}([a-z]+):\s*$/u.exec(line);
    if (method && HTTP_METHODS.has(method[1]) && currentPath) {
      operations.push(`${method[1].toUpperCase()} ${currentPath}`);
    }
  }
  return operations;
}

/** Count the `paths:` entries regardless of the methods they declare. */
export function countOpenApiPaths(text) {
  let insidePaths = false;
  let count = 0;
  for (const line of text.split("\n")) {
    if (/^paths:\s*$/u.test(line)) {
      insidePaths = true;
      continue;
    }
    if (insidePaths && /^[A-Za-z]/u.test(line)) {
      break;
    }
    if (insidePaths && /^ {2}\/\S*:\s*$/u.test(line)) {
      count += 1;
    }
  }
  return count;
}

/**
 * Extract protobuf service and RPC names.
 *
 * `\s*` on both flanks of `rpc` is load-bearing: upstream indents `filesystem.proto` by two spaces
 * and `process.proto` by four, so any pattern pinned to one width silently halves the count while
 * still looking like a successful parse.
 */
export function scanProtobufSurface(text) {
  const services = [];
  const methods = [];
  for (const line of text.split("\n")) {
    const service = /^\s*service\s+([A-Za-z][A-Za-z0-9_]*)/u.exec(line);
    if (service) {
      services.push(service[1]);
    }
    const method = /^\s*rpc\s+([A-Za-z][A-Za-z0-9_]*)/u.exec(line);
    if (method) {
      methods.push(method[1]);
    }
  }
  return { services, methods };
}

/** Extract the path keys of an envd REST document. */
export function scanEnvRestEndpoints(text) {
  return text
    .split("\n")
    .map((line) => /^ {2}(\/[A-Za-z0-9/_-]*):\s*$/u.exec(line))
    .filter((match) => match !== null)
    .map((match) => match[1]);
}

/**
 * The envd REST paths whose block carries a line-anchored `x-internal: true`.
 *
 * Two traps are encoded here, both of which produce a plausible-looking wrong answer:
 *
 *   1. **A file-wide substring search counts the prose.** `envd.yaml` explains the marker in a
 *      comment that contains the marker's text verbatim. Counting matches over the whole file
 *      reports one more marker than the file declares (7 instead of 6); that is how this trap was
 *      first hit, by hand. The comment sits above every path heading, so block scoping would
 *      already exclude it — but the count is wrong before scoping is applied, and a count is what a
 *      reader takes away.
 *   2. **Line anchoring is what stops prose *inside* a block from impersonating a declaration.**
 *      A description that names the marker to say it does not apply matches an unanchored pattern
 *      and would close an endpoint that is open. Block scoping does not help here, because the
 *      prose is inside the block. Anchoring the pattern to the start of a line is the rule that
 *      makes a sentence unable to act as a declaration.
 *
 * Membership is decided by whether the marker line falls between one path heading and the next,
 * because that is the scope YAML gives it. A file-wide count cannot say *which* operation is
 * internal, and the whole point of the marker upstream is that the proxy derives a per-operation
 * rejection list from it.
 */
export function scanEnvdOrchestratorOnlyPaths(text) {
  const lines = text.split("\n");
  const headings = [];
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^ {2}(\/[A-Za-z0-9/_-]*):\s*$/u.exec(lines[index]);
    if (match !== null) {
      headings.push({ path: match[1], start: index });
    }
  }
  const markerLine = /^\s*x-internal:\s*true\s*$/u;
  return headings
    .filter((heading, position) => {
      const end = position + 1 < headings.length ? headings[position + 1].start : lines.length;
      for (let index = heading.start; index < end; index += 1) {
        if (markerLine.test(lines[index])) {
          return true;
        }
      }
      return false;
    })
    .map((heading) => heading.path);
}

/**
 * Families of a language's error classes: the class name with its `Error` / `Exception` suffix
 * stripped, mapped to the same-stripped name of its parent. A class declared without a parent is
 * recorded as extending the language's own root (`Error` in TypeScript, `Exception` in Python),
 * because "not part of the sandbox hierarchy" is the same design decision in both languages — the
 * comparison below treats those two root names as equal so that only real differences surface.
 */
export function scanErrorFamilies(text, language) {
  const families = {};
  const strip = (name) =>
    name === "Error" || name === "Exception"
      ? "root"
      : name.replace(/(Error|Exception)$/u, "").toLowerCase();
  if (language === "javascript") {
    for (const match of text.matchAll(/^export class ([A-Za-z][A-Za-z0-9_]*)(?:\s+extends\s+([A-Za-z][A-Za-z0-9_]*))?/gmu)) {
      families[strip(match[1])] = strip(match[2] ?? "Error");
    }
    return families;
  }
  for (const match of text.matchAll(/^class ([A-Za-z][A-Za-z0-9_]*)\(([^)]*)\):/gmu)) {
    const bases = match[2]
      .split(",")
      .map((base) => base.trim())
      .filter((base) => base.length > 0);
    families[strip(match[1])] = strip(bases[0] ?? "Exception");
  }
  return families;
}

/**
 * The `ENVD_*` version gates, read from whichever form the language uses: TypeScript declares them
 * as `export const NAME = '1.2.3'` and Python as `NAME = Version("1.2.3")`. Both spellings are
 * scanned so the two tables can be compared feature by feature rather than by count — a count
 * hides a feature renamed on one side, which is the drift that breaks version negotiation silently.
 */
export function scanEnvdVersionGates(text) {
  const gates = {};
  for (const match of text.matchAll(
    /^(?:export const )?(ENVD_[A-Z0-9_]+)\s*=\s*(?:Version\()?["']([0-9][0-9.]*)["']/gmu,
  )) {
    gates[match[1]] = match[2];
  }
  return gates;
}

/**
 * Server- and client-streaming methods of a protobuf surface. The direction is carried by where
 * `stream` appears in the signature, so a scan that only counts `stream` occurrences cannot say
 * which side is streaming — and only `WatchDir` streams from the server among the nine Filesystem
 * methods, so a mistaken direction changes what a client implementation must support.
 */
export function scanStreamingMethods(text) {
  const server = [];
  const client = [];
  for (const match of text.matchAll(
    /^\s*rpc\s+([A-Za-z][A-Za-z0-9_]*)\(([^)]*)\)\s+returns\s+\(([^)]*)\)/gmu,
  )) {
    if (/^\s*stream\b/u.test(match[2])) {
      client.push(match[1]);
    }
    if (/^\s*stream\b/u.test(match[3])) {
      server.push(match[1]);
    }
  }
  return { server, client };
}

/**
 * The gRPC status codes a data-plane error table maps, normalised so `Code.DeadlineExceeded` and
 * `Code.DEADLINE_EXCEEDED` compare equal. Only the code→family *correspondence* is compared here;
 * the message strings deliberately are not, because they are prose that upstream rewrites freely
 * and asserting them would turn every wording change into a false alarm. The prose differences that
 * matter are recorded in the review document instead.
 */
export function scanGrpcErrorCodes(text) {
  const codes = new Set();
  for (const match of text.matchAll(/\[Code\.([A-Za-z][A-Za-z0-9_]*)\]/gu)) {
    codes.add(match[1].replaceAll("_", "").toLowerCase());
  }
  for (const match of text.matchAll(/^\s{4}Code\.([A-Z][A-Z0-9_]*):/gmu)) {
    codes.add(match[1].replaceAll("_", "").toLowerCase());
  }
  return [...codes].sort();
}

/**
 * One Go assignment, read by name: a single `const NAME = value`, an entry inside a `const (...)`
 * block, or a flag declaration. A flag declaration is reported together with its remote key, because
 * that key — not the Go identifier — is what an operator sets, and what it sets is a default.
 *
 * `occurrences` is returned rather than discarded: a name declared in two places in the file the
 * reading attributes it to means the value this function returns is a guess about which one counts,
 * and a guess is not a reading. The caller turns any count other than one into a finding.
 */
export function scanPerformanceConstant(text, name) {
  const matches = [
    ...text.matchAll(new RegExp(`^\\s*(?:const\\s+|var\\s+)?${name}\\s*=\\s*(.+?)\\s*$`, "gmu")),
  ];
  if (matches.length === 0) {
    return null;
  }
  // A trailing `//` comment has to go before the expression is parsed, and the failure it causes is
  // silent in one direction: a flag line whose comment happens to end in `)` still parses as a flag
  // but with the comment glued onto the value, while one that does not end in `)` stops parsing as a
  // flag at all and is demoted to a plain constant. Both were observed on this file's first run. The
  // trim is not cosmetic: the whitespace the comment left behind is what stops `NewIntFlag(...)` from
  // reaching the end of the string the flag pattern anchors on.
  const expression = matches[0][1].replace(/\s\/\/.*$/u, "").trim();
  const flag = /^New[A-Za-z]*Flag\(\s*"([^"]+)"\s*,\s*(.+?)\s*\)$/u.exec(expression);
  const kind = flag === null ? "const" : "flag-default";
  return {
    occurrences: matches.length,
    kind,
    remoteKey: flag === null ? null : flag[1],
    value: flag === null ? expression : flag[2],
  };
}

/**
 * The public member names a TypeScript file declares at class-member indentation.
 *
 * Three rules, each of which was needed to stop a plausible wrong answer on a real file:
 *
 *  - `private` / `protected` are deliberately absent from the modifier alternation, so a privately
 *    declared helper cannot match at all: `  private async runGit(` leaves `async` where `(` would
 *    have to be. JavaScript's `private async runGit` and Python's `_run_git` are the same design
 *    decision, and counting the JavaScript one as public invents five divergences on the git surface
 *    while the two SDKs actually agree member for member.
 *  - `<...>` is allowed between the name and `(`, because a generic static is written
 *    `static async create<V extends typeof Volume>(`. Without it the five `Volume` statics vanish and
 *    the surface reads as eight members instead of thirteen.
 *  - statement keywords are filtered, because a module-level function's body sits at the same
 *    two-space indentation as a class member: the `  if (` inside `function validateSecretName` in
 *    `secret.ts` matches the member pattern and is added as a member literally named `if`.
 */
export function scanTypeScriptPublicMembers(text) {
  const names = new Set();
  for (const match of text.matchAll(
    /^[ \t]{2}(?:(?:public|static|async|readonly)\s+)*([a-zA-Z_][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*\(/gmu,
  )) {
    const name = match[1];
    if (name === "constructor" || TYPESCRIPT_STATEMENT_KEYWORDS.has(name)) {
      continue;
    }
    names.add(name);
  }
  return [...names].sort();
}

/**
 * The public member names a Python file declares at method indentation, excluding properties.
 *
 * Properties are excluded because they are not the same thing across the two languages: `Volume`
 * exposes `volumeId` / `name` / `token` as `readonly` fields in TypeScript and as `@property`
 * accessors in Python, so counting the accessors as methods reports three Python-only "methods" that
 * are the same public surface written the way each language writes it. Decorators other than the
 * property ones (`@overload`, `@classmethod`) do not disqualify a member, and the scan looks past
 * them rather than stopping at the first decorator it meets.
 */
export function scanPythonPublicMembers(text) {
  const lines = text.split("\n");
  const names = new Set();
  lines.forEach((line, index) => {
    const match = /^[ \t]{4}(?:async\s+)?def ([a-z_][A-Za-z0-9_]*)\s*\(/u.exec(line);
    if (match === null || match[1].startsWith("_")) {
      return;
    }
    for (let above = index - 1; above >= 0; above -= 1) {
      const candidate = lines[above].trim();
      if (candidate.length === 0) {
        continue;
      }
      if (!candidate.startsWith("@")) {
        break;
      }
      if (PYTHON_PROPERTY_DECORATORS.has(candidate)) {
        return;
      }
    }
    names.add(match[1]);
  });
  return [...names].sort();
}

/** `create_branch` → `createBranch`, so the two SDKs' naming conventions stop looking like drift. */
export function normalisePythonNameToCamelCase(name) {
  const [head, ...rest] = name.split("_");
  return head + rest.map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join("");
}

export function readJsonVersion(path) {
  try {
    const value = JSON.parse(readText(path));
    return typeof value.version === "string" ? value.version : null;
  } catch {
    return null;
  }
}

export function readTomlVersion(path) {
  const match = /^\s*version\s*=\s*"([^"]+)"/mu.exec(readText(path));
  return match ? match[1] : null;
}

export function parseArgs(argv) {
  const options = { fetch: false, json: false };
  for (const argument of argv) {
    if (argument === "--fetch") {
      options.fetch = true;
    } else if (argument === "--json") {
      options.json = true;
    } else if (argument === "--help" || argument === "-h") {
      options.help = true;
    } else {
      fail(`unsupported argument '${argument}'`);
    }
  }
  return options;
}

function sameSet(actual, expected) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/**
 * Re-derive every recorded reading from the vendored trees.
 *
 * @returns {{readings: object, findings: Array<{check: string, message: string}>}}
 */
export function sampleUpstreamSource() {
  const findings = [];
  const readings = {};
  const note = (check, message) => findings.push({ check, message });

  // --- repository identity -------------------------------------------------
  const heads = {};
  for (const [name, recorded] of Object.entries(RECORDED_REPOSITORIES)) {
    const repositoryPath = join(externalDirectory, name);
    if (!existsSync(repositoryPath)) {
      note("repository-present", `${name} is not vendored under external/`);
      heads[name] = null;
      continue;
    }
    const head = readClonedHead(repositoryPath);
    heads[name] = head;
    if (head !== recorded) {
      note(
        "repository-commit",
        `${name}: HEAD ${head ?? "<unreadable>"} differs from the recorded ${recorded}; every reading below that names this repository is now a statement about a different tree`,
      );
    }
  }
  readings.heads = heads;

  // --- control plane contract ---------------------------------------------
  const controlPlanePath = join(externalDirectory, "infra", "spec", "openapi.yml");
  const controlPlaneText = safeReadText(controlPlanePath);
  if (controlPlaneText === null) {
    note("control-plane-contract", "external/infra/spec/openapi.yml is missing");
  } else {
    const operations = scanOpenApiOperations(controlPlaneText);
    const paths = countOpenApiPaths(controlPlaneText);
    readings.controlPlaneOperations = operations.length;
    readings.controlPlanePaths = paths;
    readings.hasOperationId = /\boperationId\b/u.test(controlPlaneText);
    if (operations.length !== RECORDED_READINGS.controlPlaneOperations) {
      note(
        "control-plane-operations",
        `control plane declares ${operations.length} operation(s), recorded ${RECORDED_READINGS.controlPlaneOperations}`,
      );
    }
    if (paths !== RECORDED_READINGS.controlPlanePaths) {
      note(
        "control-plane-paths",
        `control plane declares ${paths} path(s), recorded ${RECORDED_READINGS.controlPlanePaths}`,
      );
    }
    if (readings.hasOperationId) {
      note(
        "control-plane-operation-id",
        "the control-plane contract now carries operationId; the path+method identifier this sampler and the audit both rely on may have been superseded",
      );
    }

    // The second vendored copy of the same contract. Both are sampled because they have already
    // diverged in their schema bodies while agreeing on the operation surface: reading only the
    // control plane's own copy leaves the SDK repository's copy free to drift in the operation set
    // without this tool noticing, and that copy is the one the SDK is generated from.
    const twinReadings = {};
    for (const [name, relativePath] of Object.entries(TWIN_CONTROL_PLANE_CONTRACTS)) {
      const text = safeReadText(join(externalDirectory, name, relativePath));
      if (text === null) {
        twinReadings[name] = null;
        note("twin-contract-absent", `${name}:${relativePath} is missing`);
        continue;
      }
      twinReadings[name] = {
        operations: scanOpenApiOperations(text).length,
        paths: countOpenApiPaths(text),
      };
    }
    readings.twinControlPlaneContracts = twinReadings;
    const twinNames = Object.keys(twinReadings).filter((name) => twinReadings[name] !== null);
    if (twinNames.length > 1) {
      const [first, ...rest] = twinNames;
      for (const name of rest) {
        if (
          twinReadings[name].operations !== twinReadings[first].operations ||
          twinReadings[name].paths !== twinReadings[first].paths
        ) {
          note(
            "twin-contract-operation-surface",
            `${first} and ${name} no longer declare the same operation surface: ${JSON.stringify(twinReadings[first])} vs ${JSON.stringify(twinReadings[name])}. They are read from different repositories, so one is stale rather than both being right`,
          );
        }
      }
    }

    // Auxiliary contracts: present, and deliberately not part of the SDK surface. Compared by name
    // rather than by count, so a failure says which document appeared or vanished.
    const presentAuxiliary = RECORDED_AUXILIARY_CONTRACTS.filter((identifier) => {
      const separator = identifier.indexOf(":");
      return existsSync(
        join(
          externalDirectory,
          identifier.slice(0, separator),
          identifier.slice(separator + 1),
        ),
      );
    });
    readings.auxiliaryContracts = presentAuxiliary;
    const absentAuxiliary = RECORDED_AUXILIARY_CONTRACTS.filter(
      (identifier) => !presentAuxiliary.includes(identifier),
    );
    if (absentAuxiliary.length > 0) {
      note(
        "auxiliary-specs",
        `recorded auxiliary contract(s) not found in the vendored trees: ${JSON.stringify(absentAuxiliary)}`,
      );
    }

    // Documentation-derived baseline, when its capture is still on disk.
    const publicPath = join(docsCaptureDirectory, "openapi-public.yaml");
    const publicText = safeReadText(publicPath);
    if (publicText === null) {
      readings.documentedOperations = null;
      note(
        "docs-capture-absent",
        `no capture at ${relative(repositoryRoot, publicPath)}; the two-way operation difference cannot be recomputed. Re-run the baseline's recaptureCommand, or pass --fetch`,
      );
    } else {
      const documented = scanOpenApiOperations(publicText);
      readings.documentedOperations = documented.length;
      const sourceSet = new Set(operations);
      const documentedSet = new Set(documented);
      readings.sourceOnlyOperations = [...sourceSet].filter((item) => !documentedSet.has(item));
      readings.documentedOnlyOperations = [...documentedSet].filter((item) => !sourceSet.has(item));
      if (documented.length !== RECORDED_READINGS.documentedOperations) {
        note(
          "documented-operations",
          `the captured documentation declares ${documented.length} operation(s), the audit records ${RECORDED_READINGS.documentedOperations}`,
        );
      }
      const missingSdkOperations = RECORDED_SOURCE_ONLY_SDK_OPERATIONS.filter(
        (item) => !readings.sourceOnlyOperations.includes(item),
      );
      if (missingSdkOperations.length > 0) {
        note(
          "source-only-sdk-operations",
          `recorded as absent from the documentation baseline but no longer source-only: ${missingSdkOperations.join(", ")}`,
        );
      }
    }
  }

  // --- data plane: envd ----------------------------------------------------
  const envdRoot = join(externalDirectory, "E2B", "spec", "envd");
  if (!existsSync(envdRoot)) {
    note("envd-contracts", "external/E2B/spec/envd is missing");
  } else {
    const services = new Set();
    const methods = new Set();
    const serverStreaming = new Set();
    const clientStreaming = new Set();
    for (const proto of walkFiles(envdRoot, ".proto")) {
      const surface = scanProtobufSurface(readText(proto));
      surface.services.forEach((value) => services.add(value));
      surface.methods.forEach((value) => methods.add(value));
      const streaming = scanStreamingMethods(readText(proto));
      streaming.server.forEach((value) => serverStreaming.add(value));
      streaming.client.forEach((value) => clientStreaming.add(value));
    }
    readings.envdServices = services.size;
    readings.envdRpcMethods = methods.size;
    readings.envdServerStreamingMethods = serverStreaming.size;
    readings.envdClientStreamingMethods = clientStreaming.size;
    readings.envdServerStreamingMethodNames = [...serverStreaming].sort();
    readings.envdClientStreamingMethodNames = [...clientStreaming].sort();
    if (readings.envdServerStreamingMethods !== RECORDED_READINGS.envdServerStreamingMethods) {
      note(
        "envd-server-streaming",
        `envd declares ${readings.envdServerStreamingMethods} server-streaming method(s) (${readings.envdServerStreamingMethodNames.join(", ")}), recorded ${RECORDED_READINGS.envdServerStreamingMethods}`,
      );
    }
    if (readings.envdClientStreamingMethods !== RECORDED_READINGS.envdClientStreamingMethods) {
      note(
        "envd-client-streaming",
        `envd declares ${readings.envdClientStreamingMethods} client-streaming method(s) (${readings.envdClientStreamingMethodNames.join(", ")}), recorded ${RECORDED_READINGS.envdClientStreamingMethods}`,
      );
    }
    if (services.size !== RECORDED_READINGS.envdServices) {
      note(
        "envd-services",
        `envd declares ${services.size} service(s), recorded ${RECORDED_READINGS.envdServices}`,
      );
    }
    if (methods.size !== RECORDED_READINGS.envdRpcMethods) {
      note(
        "envd-rpc-methods",
        `envd declares ${methods.size} RPC method(s), recorded ${RECORDED_READINGS.envdRpcMethods}. The two proto files indent differently on purpose; a pattern pinned to one width reports a smaller number here without failing to parse`,
      );
    }
    const restPath = join(envdRoot, "envd.yaml");
    const restText = safeReadText(restPath);
    if (restText === null) {
      note("envd-rest", "external/E2B/spec/envd/envd.yaml is missing");
    } else {
      const endpoints = scanEnvRestEndpoints(restText);
      readings.envdRestEndpoints = endpoints.length;
      if (endpoints.length !== RECORDED_READINGS.envdRestEndpoints) {
        note(
          "envd-rest-endpoints",
          `envd declares ${endpoints.length} REST endpoint(s), recorded ${RECORDED_READINGS.envdRestEndpoints}`,
        );
      }
      const orchestratorOnly = scanEnvdOrchestratorOnlyPaths(restText);
      readings.envdOrchestratorOnlyPaths = orchestratorOnly;
      readings.envdOrchestratorOnlyEndpoints = orchestratorOnly.length;
      readings.envdClientReachableEndpoints = endpoints.length - orchestratorOnly.length;
      if (orchestratorOnly.length !== RECORDED_READINGS.envdOrchestratorOnlyEndpoints) {
        note(
          "envd-orchestrator-only-endpoints",
          `envd marks ${orchestratorOnly.length} endpoint(s) x-internal, recorded ${RECORDED_READINGS.envdOrchestratorOnlyEndpoints}. A marker moved between operations keeps this count equal and only the names reveal it`,
        );
      }
      if (
        readings.envdClientReachableEndpoints !== RECORDED_READINGS.envdClientReachableEndpoints
      ) {
        note(
          "envd-client-reachable-endpoints",
          `envd leaves ${readings.envdClientReachableEndpoints} endpoint(s) reachable, recorded ${RECORDED_READINGS.envdClientReachableEndpoints}`,
        );
      }
      const expectedOrchestratorOnly = [...RECORDED_ENVD_ORCHESTRATOR_ONLY_PATHS];
      const gained = orchestratorOnly.filter((path) => !expectedOrchestratorOnly.includes(path));
      const lost = expectedOrchestratorOnly.filter((path) => !orchestratorOnly.includes(path));
      if (gained.length > 0 || lost.length > 0) {
        note(
          "envd-orchestrator-only-paths",
          `the set of x-internal paths moved: newly closed to clients ${JSON.stringify(gained)}, newly opened ${JSON.stringify(lost)}`,
        );
      }
    }
  }

  // --- SDK plane: the two languages' contracts with each other --------------
  //
  // The public error surface, the envd version gates and the gRPC error tables are not one SDK's
  // business: they are the contract the JavaScript and Python SDKs keep with each other, and the
  // only artifacts a generated SDK must reproduce in every language it emits. They are sampled as
  // cross-language comparisons rather than per-language counts, because a count agrees while the
  // two sides disagree about which member they are counting.
  const families = {};
  for (const [language, relativePath] of Object.entries(SDK_ERROR_SOURCES)) {
    const text = safeReadText(join(externalDirectory, "E2B", relativePath));
    if (text === null) {
      note("sdk-errors", `E2B/${relativePath} is missing`);
      continue;
    }
    families[language] = scanErrorFamilies(text, language);
    readings[`sdkErrorFamilies_${language}`] = Object.keys(families[language]).length;
  }
  if (families.javascript && families.python) {
    readings.sdkErrorFamilies = Object.keys(families.javascript).length;
    const foreign = (left, right) =>
      Object.keys(left).filter((family) => !Object.prototype.hasOwnProperty.call(right, family));
    const jsOnly = foreign(families.javascript, families.python);
    const pythonOnly = foreign(families.python, families.javascript);
    if (jsOnly.length > 0 || pythonOnly.length > 0) {
      note(
        "sdk-error-family-names",
        `the two SDKs no longer declare the same error families: JavaScript-only ${JSON.stringify(jsOnly)}, Python-only ${JSON.stringify(pythonOnly)}`,
      );
    }
    const divergences = Object.keys(families.javascript)
      .filter((family) => family in families.python)
      .filter((family) => families.javascript[family] !== families.python[family])
      .sort();
    readings.sdkInheritanceDivergences = divergences;
    const recorded = [...RECORDED_SDK_INHERITANCE_DIVERGENCES].sort();
    if (!sameSet(divergences, recorded)) {
      note(
        "sdk-error-inheritance",
        `the set of families whose parent class differs between the two SDKs moved: recorded ${JSON.stringify(recorded)}, now ${JSON.stringify(divergences)}. A new member is a portability trap; a removed one means upstream harmonised the hierarchy, which makes this list stale rather than wrong`,
      );
    }
    for (const family of divergences) {
      readings[`sdkParent_${family}`] = {
        javascript: families.javascript[family],
        python: families.python[family],
      };
    }
  }

  const gates = {};
  for (const [language, relativePath] of Object.entries(ENVD_VERSION_GATES)) {
    const text = safeReadText(join(externalDirectory, "E2B", relativePath));
    if (text === null) {
      note("envd-version-gates", `E2B/${relativePath} is missing`);
      continue;
    }
    gates[language] = scanEnvdVersionGates(text);
    readings[`envdVersionGates_${language}`] = Object.keys(gates[language]).length;
  }
  if (gates.javascript && gates.python) {
    readings.envdVersionGates = Object.keys(gates.javascript).length;
    if (readings.envdVersionGates !== RECORDED_READINGS.envdVersionGates) {
      note(
        "envd-version-gates",
        `the SDKs declare ${readings.envdVersionGates} ENVD_* gate(s), recorded ${RECORDED_READINGS.envdVersionGates}`,
      );
    }
    const disagrees = Object.keys(gates.javascript)
      .filter((gate) => gate in gates.python)
      .filter((gate) => gates.javascript[gate] !== gates.python[gate]);
    const jsOnlyGates = Object.keys(gates.javascript).filter((gate) => !(gate in gates.python));
    const pythonOnlyGates = Object.keys(gates.python).filter((gate) => !(gate in gates.javascript));
    if (disagrees.length > 0 || jsOnlyGates.length > 0 || pythonOnlyGates.length > 0) {
      note(
        "envd-version-gate-agreement",
        `the two SDKs disagree about the envd features they may use: different version ${JSON.stringify(disagrees.map((gate) => [gate, gates.javascript[gate], gates.python[gate]]))}, JavaScript-only ${JSON.stringify(jsOnlyGates)}, Python-only ${JSON.stringify(pythonOnlyGates)}`,
      );
    }
  }

  const errorCodes = {};
  for (const [language, relativePath] of Object.entries(ENVD_ERROR_MAPS)) {
    const text = safeReadText(join(externalDirectory, "E2B", relativePath));
    if (text === null) {
      note("envd-error-map", `E2B/${relativePath} is missing`);
      continue;
    }
    errorCodes[language] = scanGrpcErrorCodes(text);
    readings[`envdErrorCodesMapped_${language}`] = errorCodes[language].length;
  }
  if (errorCodes.javascript && errorCodes.python) {
    readings.envdErrorCodesMapped = errorCodes.javascript.length;
    readings.envdErrorCodes = errorCodes.javascript;
    if (readings.envdErrorCodesMapped !== RECORDED_READINGS.envdErrorCodesMapped) {
      note(
        "envd-error-codes",
        `the SDKs map ${readings.envdErrorCodesMapped} gRPC status code(s), recorded ${RECORDED_READINGS.envdErrorCodesMapped}`,
      );
    }
    if (!sameSet(errorCodes.javascript, errorCodes.python)) {
      note(
        "envd-error-code-agreement",
        `the two SDKs map different gRPC status codes: JavaScript ${JSON.stringify(errorCodes.javascript)}, Python ${JSON.stringify(errorCodes.python)}`,
      );
    }
  }

  // --- the SDK method plane -------------------------------------------------
  // The review document lists these surfaces by hand, and a hand list is a statement nobody checks:
  // it stays right until upstream moves and then it is silently wrong. Two readings are taken, and
  // they answer different questions. The *divergence* set is the compatibility question — a name
  // that exists on one side only is a call that does not port. The *member inventory* is the coverage
  // question — a member added to both sides in step is no divergence at all, and only the inventory
  // notices it, which is what keeps the document's list from going stale.
  const surfaceMembers = {};
  const divergences = [];
  for (const [surface, files] of Object.entries(SDK_METHOD_SURFACES)) {
    const collected = {};
    for (const [language, paths] of Object.entries(files)) {
      const names = new Set();
      for (const relativePath of paths) {
        const text = safeReadText(join(externalDirectory, "E2B", relativePath));
        if (text === null) {
          note("sdk-method-surface", `E2B/${relativePath} is missing`);
          continue;
        }
        const scan = language === "javascript" ? scanTypeScriptPublicMembers : scanPythonPublicMembers;
        for (const name of scan(text)) {
          names.add(name);
        }
      }
      collected[language] = names;
      readings[`sdkMembers_${surface}_${language}`] = names.size;
    }
    if (collected.javascript === undefined || collected.python === undefined) {
      continue;
    }
    const pythonAsCamel = new Set([...collected.python].map(normalisePythonNameToCamelCase));
    for (const name of collected.javascript) {
      if (!pythonAsCamel.has(name)) {
        divergences.push(`${surface}:${name}=`);
      }
    }
    for (const name of pythonAsCamel) {
      if (!collected.javascript.has(name)) {
        divergences.push(`${surface}:=${name}`);
      }
    }
    for (const name of new Set([...collected.javascript, ...pythonAsCamel])) {
      surfaceMembers[`${surface}:${name}`] = true;
    }
  }
  const measuredDivergences = divergences.sort();
  const measuredMembers = Object.keys(surfaceMembers).sort();
  readings.sdkMethodDivergences = measuredDivergences;
  readings.sdkSurfaceMembers = measuredMembers;
  const recordedDivergences = [...RECORDED_SDK_METHOD_DIVERGENCES].sort();
  if (!sameSet(measuredDivergences, recordedDivergences)) {
    note(
      "sdk-method-divergences",
      `the set of one-sided SDK members moved: recorded ${JSON.stringify(recordedDivergences)}, now ${JSON.stringify(measuredDivergences)}. A new entry is a call that no longer ports between the two SDKs; a removed one means upstream harmonised the surface, which makes the record stale rather than wrong`,
    );
  }
  const recordedMembers = [...RECORDED_SDK_SURFACE_MEMBERS].sort();
  if (!sameSet(measuredMembers, recordedMembers)) {
    const added = measuredMembers.filter((member) => !recordedMembers.includes(member));
    const removed = recordedMembers.filter((member) => !measuredMembers.includes(member));
    note(
      "sdk-method-inventory",
      `the SDK member inventory moved: added ${JSON.stringify(added)}, removed ${JSON.stringify(removed)}. A member added to both languages in step is not a divergence, so only this reading sees it — and the review document's hand-written surface list is what goes stale when it is not seen`,
    );
  }

  // --- the performance plane -----------------------------------------------
  // The review document's comparison table cites these constants, so the table's numbers have to be
  // recomputable rather than trusted. Three different kinds of news are reported apart, because they
  // call for different responses: a constant that is *gone* means the design moved, a constant whose
  // *value* changed means a default was re-baselined, and a constant whose *kind* changed means the
  // value stopped being compiled in — an operational fact, not a numerical one. A single count of
  // "constants read" would agree through all three.
  const constants = {};
  const constantSources = new Set();
  for (const entry of PERFORMANCE_CONSTANTS) {
    const relativePath = PERFORMANCE_CONSTANT_SOURCES[entry.source];
    if (!constantSources.has(relativePath)) {
      constantSources.add(relativePath);
      if (!existsSync(join(externalDirectory, "infra", relativePath))) {
        note("performance-constant-source", `infra/${relativePath} is missing`);
      }
    }
    constants[entry.name] = scanPerformanceConstant(
      safeReadText(join(externalDirectory, "infra", relativePath)) ?? "",
      entry.name,
    );
  }
  const measured = Object.entries(constants)
    .filter(([, read]) => read !== null)
    .map(([name, read]) => `${name}=${read.value}`)
    .sort();
  readings.performanceConstants = measured;
  readings.performanceConstantsByKind = Object.fromEntries(
    ["const", "flag-default"].map((kind) => [
      kind,
      Object.values(constants).filter((read) => read !== null && read.kind === kind).length,
    ]),
  );
  const absent = PERFORMANCE_CONSTANTS.filter((entry) => constants[entry.name] === null).map(
    (entry) => entry.name,
  );
  if (absent.length > 0) {
    note(
      "performance-constant-absent",
      `${JSON.stringify(absent)} no longer assigned in the file the table attributes it to. A constant that moved file is a reading taken from the wrong tree; one that vanished is a design change`,
    );
  }
  const ambiguous = Object.entries(constants)
    .filter(([, read]) => read !== null && read.occurrences !== 1)
    .map(([name, read]) => `${name} (${read.occurrences} assignments)`);
  if (ambiguous.length > 0) {
    note(
      "performance-constant-ambiguous",
      `${JSON.stringify(ambiguous)} is assigned more than once in the file attributed to it, so the value read is a guess about which assignment counts`,
    );
  }
  const rekinded = PERFORMANCE_CONSTANTS.filter(
    (entry) => constants[entry.name] !== null && constants[entry.name].kind !== entry.kind,
  ).map((entry) => `${entry.name}: recorded ${entry.kind}, read ${constants[entry.name].kind}`);
  if (rekinded.length > 0) {
    note(
      "performance-constant-kind",
      `the kind of value moved: ${JSON.stringify(rekinded)}. A constant promoted to a flag is now runtime-overridable without a deploy, so a capacity model that read it as a compiled-in limit is stale even though the number is unchanged`,
    );
  }
  if (!sameSet(measured, RECORDED_PERFORMANCE_CONSTANTS)) {
    note(
      "performance-constants",
      `the constant set moved: recorded ${JSON.stringify([...RECORDED_PERFORMANCE_CONSTANTS].sort())}, now ${JSON.stringify(measured)}. A changed value is a re-baselined default and the review document's table has to be re-derived from this reading`,
    );
  }
  const readKeys = Object.fromEntries(
    Object.entries(constants)
      .filter(([, read]) => read !== null && read.remoteKey !== null)
      .map(([name, read]) => [name, read.remoteKey]),
  );
  readings.performanceConstantFlagKeys = readKeys;
  const keyDrift = [
    ...Object.entries(readKeys)
      .filter(([name, key]) => RECORDED_PERFORMANCE_FLAG_KEYS[name] !== key)
      .map(
        ([name, key]) =>
          `${name}: recorded ${JSON.stringify(RECORDED_PERFORMANCE_FLAG_KEYS[name] ?? null)}, read ${JSON.stringify(key)}`,
      ),
    ...Object.keys(RECORDED_PERFORMANCE_FLAG_KEYS)
      .filter((name) => !(name in readKeys))
      .map((name) => `${name}: recorded ${JSON.stringify(RECORDED_PERFORMANCE_FLAG_KEYS[name])}, no flag read`),
  ];
  if (keyDrift.length > 0) {
    note(
      "performance-constant-flag-keys",
      `the runtime flag keys moved: ${JSON.stringify(keyDrift)}. A renamed key leaves the default unchanged and every override already set stops applying, so this is the one drift a value-only reading reports as clean`,
    );
  }

  // --- platform story ------------------------------------------------------
  const infraPath = join(externalDirectory, "infra");
  if (existsSync(infraPath)) {
    readings.infraRustFiles = walkFiles(infraPath, ".rs").length;
    if (readings.infraRustFiles !== RECORDED_READINGS.infraRustFiles) {
      note(
        "upstream-language-mix",
        `the upstream infrastructure now contains ${readings.infraRustFiles} Rust file(s) where the audit records ${RECORDED_READINGS.infraRustFiles}; "entirely Go" is no longer true`,
      );
    }
    const goWork = safeReadText(join(infraPath, "go.work"));
    if (goWork === null) {
      note("go-work", "external/infra/go.work is missing");
    } else {
      const modules = goWork
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.startsWith("."));
      readings.goWorkModules = modules.length;
      if (modules.length !== RECORDED_READINGS.goWorkModules) {
        note(
          "go-work-modules",
          `go.work lists ${modules.length} module(s), recorded ${RECORDED_READINGS.goWorkModules}`,
        );
      }
    }
  }

  // --- SDK versions --------------------------------------------------------
  const versions = {};
  for (const [relativePath, recorded] of Object.entries(RECORDED_VERSIONS)) {
    const full = join(externalDirectory, "E2B", relativePath);
    if (!existsSync(full)) {
      versions[relativePath] = null;
      note("sdk-manifest-present", `external/E2B/${relativePath} is missing`);
      continue;
    }
    const actual = relativePath.endsWith(".json")
      ? readJsonVersion(full)
      : readTomlVersion(full);
    versions[relativePath] = actual;
    if (actual !== recorded) {
      note(
        "sdk-version",
        `${relativePath}: version ${actual ?? "<unreadable>"} differs from the recorded ${recorded}`,
      );
    }
  }
  readings.versions = versions;

  return { readings, findings };
}

/**
 * Re-fetch the two authoritative documentation URLs and compare their digests with what
 * `specs/sandbox-e2b-capability-baseline.json` records. Uses curl, matching the baseline's own
 * `recaptureCommand`, so proxy configuration and certificate handling stay where the operator
 * already put them.
 */
export function verifyDocumentationSources() {
  const findings = [];
  const baseline = JSON.parse(readText(baselinePath));
  const wanted = {
    "openapi-public.yaml": "https://docs.e2b.dev/openapi-public.yaml",
    "llms.txt": "https://docs.e2b.dev/llms.txt",
  };
  mkdirSync(docsCaptureDirectory, { recursive: true });
  const digest = (buffer) =>
    spawnSync("sha256sum", [], { input: buffer, encoding: "utf8" }).stdout?.split(" ")[0] ?? null;
  const results = [];
  for (const [fileName, url] of Object.entries(wanted)) {
    const source = (baseline.sources ?? []).find((entry) => entry.url === url);
    if (!source) {
      findings.push({ check: "documentation-source-recorded", message: `${url} is not in the baseline sources` });
      continue;
    }
    const target = join(docsCaptureDirectory, fileName);
    const fetch = spawnSync("curl", ["-sS", "-o", target, url], { encoding: "utf8" });
    if (fetch.status !== 0 || !existsSync(target)) {
      findings.push({
        check: "documentation-source-fetch",
        message: `${url} could not be fetched (curl exit ${fetch.status}); an unfetched source is not a clean source`,
      });
      continue;
    }
    const bytes = statSync(target).size;
    const sha256 = digest(readFileSync(target));
    const moved = bytes !== source.bytes || sha256 !== source.sha256;
    results.push({ url, bytes, sha256, recordedBytes: source.bytes, recordedSha256: source.sha256, moved });
    if (moved) {
      findings.push({
        check: "documentation-source-drift",
        message: `${url} moved: ${source.bytes} B/${source.sha256.slice(0, 16)}… recorded, ${bytes} B/${sha256?.slice(0, 16)}… now`,
      });
    }
  }
  return { results, findings };
}

function formatReport(report) {
  const lines = [];
  lines.push("E2B upstream source-parity sample");
  lines.push("");
  lines.push("vendored commits:");
  for (const [name, head] of Object.entries(report.readings.heads ?? {})) {
    const recorded = RECORDED_REPOSITORIES[name];
    lines.push(`  ${head === recorded ? "=" : "!"} ${name}: ${head ?? "<absent>"}`);
  }
  lines.push("");
  lines.push("readings (actual / recorded):");
  const readings = report.readings;
  const pairs = [
    ["control-plane operations", readings.controlPlaneOperations, RECORDED_READINGS.controlPlaneOperations],
    ["control-plane paths", readings.controlPlanePaths, RECORDED_READINGS.controlPlanePaths],
    ["documented operations", readings.documentedOperations, RECORDED_READINGS.documentedOperations],
    ["envd services", readings.envdServices, RECORDED_READINGS.envdServices],
    ["envd RPC methods", readings.envdRpcMethods, RECORDED_READINGS.envdRpcMethods],
    ["envd REST endpoints", readings.envdRestEndpoints, RECORDED_READINGS.envdRestEndpoints],
    [
      "envd server-streaming methods",
      readings.envdServerStreamingMethods,
      RECORDED_READINGS.envdServerStreamingMethods,
    ],
    [
      "envd client-streaming methods",
      readings.envdClientStreamingMethods,
      RECORDED_READINGS.envdClientStreamingMethods,
    ],
    ["envd gRPC codes mapped", readings.envdErrorCodesMapped, RECORDED_READINGS.envdErrorCodesMapped],
    ["envd version gates", readings.envdVersionGates, RECORDED_READINGS.envdVersionGates],
    ["SDK error families", readings.sdkErrorFamilies, RECORDED_READINGS.sdkErrorFamilies],
    [
      "envd orchestrator-only (x-internal)",
      readings.envdOrchestratorOnlyEndpoints,
      RECORDED_READINGS.envdOrchestratorOnlyEndpoints,
    ],
    [
      "envd client-reachable",
      readings.envdClientReachableEndpoints,
      RECORDED_READINGS.envdClientReachableEndpoints,
    ],
    ["auxiliary contracts present", readings.auxiliaryContracts?.length, RECORDED_AUXILIARY_CONTRACTS.length],
    ["go.work modules", readings.goWorkModules, RECORDED_READINGS.goWorkModules],
    ["upstream Rust files", readings.infraRustFiles, RECORDED_READINGS.infraRustFiles],
    [
      "performance constants",
      readings.performanceConstants?.length,
      RECORDED_PERFORMANCE_CONSTANTS.length,
    ],
    ["SDK surface members", readings.sdkSurfaceMembers?.length, RECORDED_SDK_SURFACE_MEMBERS.length],
    [
      "SDK one-sided members",
      readings.sdkMethodDivergences?.length,
      RECORDED_SDK_METHOD_DIVERGENCES.length,
    ],
  ];
  for (const [label, actual, recorded] of pairs) {
    if (actual === undefined) {
      continue;
    }
    lines.push(`  ${actual === recorded ? "=" : "!"} ${label}: ${actual} / ${recorded}`);
  }
  if (readings.sourceOnlyOperations) {
    lines.push(
      `  = source-only operations: ${readings.sourceOnlyOperations.length}; documented-only: ${readings.documentedOnlyOperations.length}`,
    );
  }
  if (readings.performanceConstantsByKind) {
    const kinds = readings.performanceConstantsByKind;
    lines.push(
      `  = of which compiled in: ${kinds["const"]}; runtime-overridable flag defaults: ${kinds["flag-default"]}`,
    );
  }
  lines.push("");
  if (report.findings.length === 0) {
    lines.push("No drift: every recorded reading reproduced and no vendored commit moved.");
  } else {
    lines.push(`${report.findings.length} finding(s):`);
    for (const finding of report.findings) {
      lines.push(`  [${finding.check}] ${finding.message}`);
    }
  }
  if (report.documentationSources) {
    lines.push("");
    lines.push("documentation sources:")
    for (const result of report.documentationSources.results ?? []) {
      lines.push(`  ${result.moved ? "!" : "="} ${result.url}: ${result.bytes} B`);
    }
  }
  return `${lines.join("\n")}\n`;
}

export function main(argv) {
  const options = parseArgs(argv);
  if (options.help) {
    process.stdout.write(
      [
        "usage: node tools/audit-sandbox-e2b-upstream-source-parity.mjs [--fetch] [--json]",
        "",
        "  --fetch   additionally re-fetch the authoritative documentation URLs and compare digests",
        "  --json    print the evidence record instead of the human report",
        "",
      ].join("\n"),
    );
    return 0;
  }

  if (!existsSync(externalDirectory)) {
    process.stderr.write(
      [
        "not vendored: external/ is absent, so there is nothing to sample.",
        "This is exit 3, not 0: an unsampled tree and a sampled-clean tree must not look alike.",
        "Re-create it with depth-1 clones of the four upstream repositories named in",
        "docs/engineering/reviews/REVIEW-20260923-sandbox-e2b-upstream-source-parity.md",
        "",
      ].join("\n"),
    );
    return 3;
  }

  const { readings, findings } = sampleUpstreamSource();
  const report = {
    schemaVersion: 1,
    kind: "sdkwork.sandbox.e2b-upstream-source-sample",
    sampledAt: new Date().toISOString(),
    externalDirectory: relative(repositoryRoot, externalDirectory),
    readings,
    findings,
  };

  if (options.fetch) {
    const fetched = verifyDocumentationSources();
    report.documentationSources = fetched;
    findings.push(...fetched.findings);
  }

  mkdirSync(evidenceDirectory, { recursive: true });
  const evidencePath = join(evidenceDirectory, `${report.sampledAt.replace(/[:.]/gu, "-")}.json`);
  writeFileSync(evidencePath, `${JSON.stringify(report, null, 2)}\n`);

  if (options.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    process.stdout.write(formatReport(report));
    process.stdout.write(`evidence: ${relative(repositoryRoot, evidencePath)}\n`);
  }

  return findings.length === 0 ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 2;
  }
}
