import assert from "node:assert/strict";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  countOpenApiPaths,
  normalisePythonNameToCamelCase,
  parseArgs,
  readClonedHead,
  sampleUpstreamSource,
  scanEnvdOrchestratorOnlyPaths,
  scanEnvdVersionGates,
  scanEnvRestEndpoints,
  scanErrorFamilies,
  scanGrpcErrorCodes,
  scanOpenApiOperations,
  scanPerformanceConstant,
  scanProtobufSurface,
  scanPythonPublicMembers,
  scanStreamingMethods,
  scanTypeScriptPublicMembers,
} from "../../tools/audit-sandbox-e2b-upstream-source-parity.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const toolPath = path.join(repoRoot, "tools/audit-sandbox-e2b-upstream-source-parity.mjs");

/** Run `body` against a throwaway directory tree, removing it afterwards. */
function withTempTree(body) {
  const tree = mkdtempSync(path.join(tmpdir(), "e2b-source-parity-"));
  try {
    return body(tree);
  } finally {
    rmSync(tree, { recursive: true, force: true });
  }
}

test("the envd orchestrator-only marker is line-anchored, so prose that mentions it is not a marker", () => {
  // Two places where the marker's own text appears without being a marker, both real shapes.
  //
  // One is the file-level comment above `paths:` that upstream uses to explain the convention. It
  // sits before every path heading, so a file-wide substring count reads seven markers where the
  // file declares six — which is exactly how this trap was first hit, by hand.
  //
  // The other is a description inside a path block that names the marker to say it does *not*
  // apply. Block scoping alone does not save this one: an unanchored pattern matches prose that
  // lives inside the block, and would close an endpoint that is open. Line anchoring is what makes
  // a comment unable to impersonate a declaration.
  const document = [
    "openapi: 3.0.0",
    "info:",
    "  title: envd",
    "",
    "# Operations marked `x-internal: true` are the orchestrator's control plane.",
    "paths:",
    "  /health:",
    "    get:",
    "      summary: Check the health of the service",
    "      description: Always public. Not tagged `x-internal: true`.",
    "",
    "  /init:",
    "    post:",
    "      x-internal: true",
    "      summary: Initialize the sandbox",
    "",
    "  /files:",
    "    post:",
    "      summary: Read a file",
    "",
  ].join("\n");
  assert.deepEqual(scanEnvdOrchestratorOnlyPaths(document), ["/init"]);

  // Negative control: the same text without the marker line yields nothing, so the assertion above
  // is reading the marker and not merely returning the shape it expects.
  const unmarked = document.replace("      x-internal: true\n", "");
  assert.deepEqual(scanEnvdOrchestratorOnlyPaths(unmarked), []);

  // And the prose must not be reachable by the naive reading either, so the difference is stated
  // rather than assumed: an unanchored match over the same text finds three sites, of which two are
  // prose.
  const naive = [...document.matchAll(/x-internal: true/gu)];
  assert.equal(naive.length, 3, "the marker's text appears twice in prose and once as a declaration");
  assert.equal(scanEnvdOrchestratorOnlyPaths(document).length, 1);
});

test("the envd marker is scoped to its own path block and cannot leak onto a neighbour", () => {
  const document = [
    "paths:",
    "  /freeze:",
    "    post:",
    "      x-internal: true",
    "",
    "  /unfreeze:",
    "    post:",
    "      x-internal: true",
    "",
    "  /envs:",
    "    get:",
    "      summary: List environment variables",
    "",
    "  /files/compose:",
    "    post:",
    "      summary: Compose files",
    "",
  ].join("\n");
  assert.deepEqual(scanEnvdOrchestratorOnlyPaths(document), ["/freeze", "/unfreeze"]);

  // A file-wide count cannot express which operation is closed, which is the property upstream
  // relies on: the sandbox proxy builds a per-operation rejection list from this marker. The
  // document below carries the same number of markers as the one above, and only the names say so.
  const movedMarker = [
    "paths:",
    "  /freeze:",
    "    post:",
    "      x-internal: true",
    "",
    "  /unfreeze:",
    "    post:",
    "      x-internal: true",
    "",
    "  /envs:",
    "    get:",
    "      x-internal: true",
    "",
    "  /files/compose:",
    "    post:",
    "      summary: Compose files",
    "",
  ].join("\n");
  const before = scanEnvdOrchestratorOnlyPaths(document);
  const after = scanEnvdOrchestratorOnlyPaths(movedMarker);
  assert.equal(after.length, before.length + 1, "the moved document carries one more marker");
  assert.deepEqual(after, ["/freeze", "/unfreeze", "/envs"]);
  assert.notDeepEqual(after, before, "the set, not the count, is what reports a marker that moved");
});

test("envd REST endpoints are the two-space path keys only, not consumer properties or declared methods", () => {
  const document = [
    "openapi: 3.0.0",
    "paths:",
    "  /health:",
    "    get:",
    "      responses:",
    "        \"204\":",
    "          description: healthy",
    "",
    "  /files/compose:",
    "    post:",
    "      requestBody:",
    "        content:",
    "          application/json:",
    "            schema:",
    "              properties:",
    "                /not-a-path:",
    "                  type: string",
    "",
  ].join("\n");
  assert.deepEqual(scanEnvRestEndpoints(document), ["/health", "/files/compose"]);
  assert.deepEqual(scanEnvRestEndpoints("paths: {}\n"), []);
});

test("protobuf RPC extraction is indentation-agnostic, because the upstream protos disagree", () => {
  // The real pair: `filesystem.proto` indents `rpc` by two spaces and `process.proto` by four. A
  // pattern pinned to either width still parses and silently drops half the surface, which is the
  // failure mode this sampler was built to stop repeating.
  const twoSpace = ["service Filesystem {", "  rpc Stat(StatRequest) returns (StatResponse);", "  rpc ListDir(ListDirRequest) returns (ListDirResponse);", "}"].join("\n");
  const fourSpace = ["service Process {", "    rpc Start(StartRequest) returns (StartResponse);", "    rpc StreamInput(stream StreamInputRequest) returns (StreamInputResponse);", "}"].join("\n");

  const shallow = scanProtobufSurface(twoSpace);
  const deep = scanProtobufSurface(fourSpace);
  assert.deepEqual([...shallow.methods], ["Stat", "ListDir"]);
  assert.deepEqual([...deep.methods], ["Start", "StreamInput"]);
  assert.deepEqual([...shallow.services], ["Filesystem"]);
  assert.deepEqual([...deep.services], ["Process"]);

  // A width-pinned pattern is the negative control: it must lose the methods the real scan keeps.
  const pinned = /^ {2}rpc\s+([A-Za-z][A-Za-z0-9_]*)/gmu;
  assert.equal([...twoSpace.matchAll(pinned)].length, 2);
  assert.equal([...fourSpace.matchAll(pinned)].length, 0, "a two-space pattern must miss the four-space proto");
});

test("OpenAPI operations carry the path they were declared under and stop at the next top-level key", () => {
  const document = [
    "openapi: 3.0.0",
    "paths:",
    "  /v2/sandboxes:",
    "    get:",
    "      summary: List sandboxes",
    "    post:",
    "      summary: Create a sandbox",
    "  /v2/sandboxes/{sandboxID}:",
    "    delete:",
    "      summary: Kill a sandbox",
    "components:",
    "  schemas:",
    "    Sandbox:",
    "      get:",
    "        notAnOperation: true",
    "",
  ].join("\n");
  assert.deepEqual(scanOpenApiOperations(document), [
    "GET /v2/sandboxes",
    "POST /v2/sandboxes",
    "DELETE /v2/sandboxes/{sandboxID}",
  ]);
  assert.equal(countOpenApiPaths(document), 2);
});

test("a clone's HEAD is read from the loose ref, then packed-refs, without invoking git", () => {
  const head = "0c21aa2277b59a1d040761ed3fbbb29a78775470";
  withTempTree((tree) => {
    const git = path.join(tree, ".git");
    mkdirSync(path.join(git, "refs", "heads"), { recursive: true });

    writeFileSync(path.join(git, "HEAD"), "ref: refs/heads/main\n");
    writeFileSync(path.join(git, "refs", "heads", "main"), `${head}\n`);
    assert.equal(readClonedHead(tree), head);

    // Shallow clones sometimes keep the ref packed instead.
    rmSync(path.join(git, "refs"), { recursive: true, force: true });
    writeFileSync(path.join(git, "packed-refs"), `# pack-refs with: peeled fully-peeled sorted\n${head} refs/heads/main\n`);
    assert.equal(readClonedHead(tree), head);

    // A detached HEAD is the SHA itself.
    writeFileSync(path.join(git, "HEAD"), `${head}\n`);
    assert.equal(readClonedHead(tree), head);
  });

  // Not a clone at all: a readable null, never a silent empty string.
  withTempTree((tree) => assert.equal(readClonedHead(tree), null));
});

test("error families are normalised across the two languages, and a multi-base class keeps its first base", () => {
  const typescript = [
    "export class SandboxError extends Error {",
    "export class NotFoundError extends SandboxError {",
    "export class VolumeError extends Error {",
    "export class VolumeNotFoundError extends VolumeError {",
    "class NotExported extends SandboxError {",
  ].join("\n");
  const python = [
    "class SandboxException(Exception):",
    "class NotFoundException(SandboxException):",
    "class VolumeException(Exception):",
    "class VolumeNotFoundException(NotFoundException):",
    "class CommandExitException(SandboxException, CommandResult):",
  ].join("\n");

  const js = scanErrorFamilies(typescript, "javascript");
  const py = scanErrorFamilies(python, "python");

  // A class declared without a parent is recorded as the language root, and the two root names
  // collapse to the same token, so "not in the sandbox hierarchy" does not read as a divergence.
  assert.equal(js.sandbox, "root");
  assert.equal(py.sandbox, "root");
  assert.equal(js.notfound, "sandbox");
  assert.equal(py.notfound, "sandbox");

  // The volume family is the real divergence this scan exists to find: the same family name has a
  // different parent on each side.
  assert.equal(js.volumenotfound, "volume");
  assert.equal(py.volumenotfound, "notfound");

  // A non-exported TypeScript class is not public surface and must not be counted.
  assert.equal("notexported" in js, false, "only exported classes are public error surface");

  // Python's `class CommandExitException(SandboxException, CommandResult)` is the trap: taking the
  // last base would read `commandresult` and fabricate a divergence that does not exist.
  assert.equal(py.commandexit, "sandbox");
});

test("envd version gates are read from both spellings and compare feature by feature", () => {
  const typescript = [
    "export const ENVD_DEFAULT_USER = '0.4.0'",
    "export const ENVD_ENVD_CLOSE = '0.5.2'",
    "export const NOT_A_GATE = '1.2.3'",
  ].join("\n");
  const python = [
    'ENVD_DEFAULT_USER = Version("0.4.0")',
    'ENVD_ENVD_CLOSE = Version("0.5.2")',
  ].join("\n");

  assert.deepEqual(scanEnvdVersionGates(typescript), {
    ENVD_DEFAULT_USER: "0.4.0",
    ENVD_ENVD_CLOSE: "0.5.2",
  });
  assert.deepEqual(scanEnvdVersionGates(python), {
    ENVD_DEFAULT_USER: "0.4.0",
    ENVD_ENVD_CLOSE: "0.5.2",
  });
  // Only `ENVD_`-prefixed constants are gates; a version-looking constant elsewhere is not one.
  assert.equal("NOT_A_GATE" in scanEnvdVersionGates(typescript), false);
});

test("streaming direction is read from which side of the signature carries `stream`", () => {
  const proto = [
    "service Filesystem {",
    "  rpc Stat(StatRequest) returns (StatResponse);",
    "  rpc WatchDir(WatchDirRequest) returns (stream WatchDirResponse);",
    "}",
    "service Process {",
    "    rpc Connect(ConnectRequest) returns (stream ConnectResponse);",
    "    rpc StreamInput(stream StreamInputRequest) returns (StreamInputResponse);",
    "}",
  ].join("\n");
  const streaming = scanStreamingMethods(proto);
  assert.deepEqual(streaming.server, ["WatchDir", "Connect"]);
  assert.deepEqual(streaming.client, ["StreamInput"]);

  // Counting `stream` occurrences cannot say which side streams, which is the whole distinction: a
  // client that only reads must implement two of these three differently from the third.
  assert.equal([...proto.matchAll(/\bstream\b/gu)].length, 3);
  assert.notEqual(streaming.server.length, 3);
});

test("gRPC status codes are read from both SDK spellings and normalised to compare equal", () => {
  const typescript = [
    "const DEFAULT_ERROR_MAP = {",
    "  [Code.InvalidArgument]: (message) => new InvalidArgumentError(message),",
    "  [Code.DeadlineExceeded]: (message) => new TimeoutError(message),",
    "};",
  ].join("\n");
  const python = [
    "_DEFAULT_RPC_ERROR_MAP: dict[Code, Callable[[str], Exception]] = {",
    "    Code.INVALID_ARGUMENT: InvalidArgumentException,",
    "    Code.DEADLINE_EXCEEDED: TimeoutException,",
    "}",
  ].join("\n");

  assert.deepEqual(scanGrpcErrorCodes(typescript), ["deadlineexceeded", "invalidargument"]);
  assert.deepEqual(scanGrpcErrorCodes(python), ["deadlineexceeded", "invalidargument"]);
  assert.deepEqual(scanGrpcErrorCodes(""), []);
});

test("a compiled-in constant and a runtime flag default are reported apart", () => {
  const go = [
    "const (",
    "\tinPlaceStateFlipTimeout = 40 * time.Second",
    "\tMaxSandboxesPerNode = NewIntFlag(\"max-sandboxes-per-node\", 200)",
    ")",
  ].join("\n");

  // The distinction is the point of the reading, not a detail of it: `const` can only be moved by a
  // rebuild, while a flag default can be overridden at runtime, so a capacity model that reads the
  // second as a hard ceiling disagrees with the service. A scan that returns bare numbers cannot
  // tell the two apart, and both of these are the same shape of line in the file.
  const compiled = scanPerformanceConstant(go, "inPlaceStateFlipTimeout");
  assert.equal(compiled.kind, "const");
  assert.equal(compiled.value, "40 * time.Second");
  assert.equal(compiled.remoteKey, null);

  const flag = scanPerformanceConstant(go, "MaxSandboxesPerNode");
  assert.equal(flag.kind, "flag-default");
  assert.equal(flag.value, "200");

  // The remote key, not the Go identifier, is what an operator sets, so it is part of the reading.
  assert.equal(flag.remoteKey, "max-sandboxes-per-node");
});

test("a trailing line comment is stripped in both of the ways it breaks the reading", () => {
  // Upstream writes both forms, and they fail differently. When the comment ends in `)` the flag
  // still parses and swallows the comment into the value; when it does not, the flag stops parsing
  // altogether and the line is demoted to a plain constant whose value is an unevaluated call.
  const endsInParen = "\tBestOfKAlpha = NewIntFlag(\"best-of-k-alpha\", 50) // Default Alpha=0.5 (percentage)";
  const noParen = "\tBestOfKSampleSize = NewIntFlag(\"best-of-k-sample-size\", 3) // Default K=3";
  const plainConst = "\tReturnDelay = 3 * time.Second // so the old request can drain";

  const alpha = scanPerformanceConstant(endsInParen, "BestOfKAlpha");
  assert.equal(alpha.kind, "flag-default");
  assert.equal(alpha.value, "50");
  assert.equal(alpha.remoteKey, "best-of-k-alpha");

  const sampleSize = scanPerformanceConstant(noParen, "BestOfKSampleSize");
  assert.equal(sampleSize.kind, "flag-default", "a comment that does not end in `)` must not hide the flag");
  assert.equal(sampleSize.value, "3");
  assert.equal(sampleSize.remoteKey, "best-of-k-sample-size");

  const delay = scanPerformanceConstant(plainConst, "ReturnDelay");
  assert.equal(delay.kind, "const");
  assert.equal(delay.value, "3 * time.Second");
});

test("the stored value is returned, not the ratio the comment describes", () => {
  // `BestOfKAlpha` is 50 because the flag stores a percentage, and the comment says Alpha=0.5. A
  // reading that returned the semantic value would be undoing the very unit convention a caller has
  // to apply, and the two agree right up until someone multiplies by 100 twice.
  const go = "\tBestOfKAlpha = NewIntFlag(\"best-of-k-alpha\", 50) // Default Alpha=0.5 (stored as percentage for int flag, current usage weight)";
  assert.equal(scanPerformanceConstant(go, "BestOfKAlpha").value, "50");

  const overcommit = "\tBestOfKMaxOvercommit = NewIntFlag(\"best-of-k-max-overcommit\", 400) // Default R=4 (stored as percentage, max over-commit ratio)";
  assert.equal(scanPerformanceConstant(overcommit, "BestOfKMaxOvercommit").value, "400");
});

test("an ambiguous assignment is reported rather than silently resolved to the first", () => {
  const go = ["\tmaxRequestsInProgress = 4096", "\tmaxRequestsInProgress = 8192"].join("\n");
  const read = scanPerformanceConstant(go, "maxRequestsInProgress");
  assert.equal(read.occurrences, 2, "two assignments make the value a guess about which one counts");
  assert.equal(read.value, "4096");
});

test("a mention is not an assignment", () => {
  const go = [
    "\t// maxRequestsInProgress bounds the serve loop, see the WP-resolve pool below.",
    "\tu.wg.SetLimit(maxRequestsInProgress)",
  ].join("\n");
  assert.equal(scanPerformanceConstant(go, "maxRequestsInProgress"), null);
});

test("privately declared TypeScript members are not public surface", () => {
  const typescript = [
    "export class Git {",
    "  private async runGit(cmd: string) {",
    "  private async runShell(cmd: string) {",
    "  async clone(url: string) {",
    "  async commit(path: string) {",
    "}",
  ].join("\n");

  // JavaScript's `private async runGit` and Python's `_run_git` are the same design decision.
  // Counting the JavaScript one as public invents five divergences on the git surface while the two
  // SDKs agree member for member — which is exactly what a naive scan reported on the real file.
  assert.deepEqual(scanTypeScriptPublicMembers(typescript), ["clone", "commit"]);
});

test("a generic static is a member, not a miss", () => {
  const typescript = [
    "export class Volume {",
    "  static async create<V extends typeof Volume>(name: string): Promise<V> {",
    "  static async connect<V extends typeof Volume>(volumeId: string): Promise<V> {",
    "  static async list(opts?: ConnectionOpts): Promise<VolumeInfo[]> {",
    "}",
  ].join("\n");

  // `create<V extends typeof Volume>(` puts a type parameter list between the name and the `(`.
  // A pattern that requires `(` there reads eight members on the volume surface where there are
  // thirteen, and the five it drops are every static the surface has.
  assert.deepEqual(scanTypeScriptPublicMembers(typescript), ["connect", "create", "list"]);
});

test("a statement keyword at member indentation is not a member", () => {
  const typescript = [
    "function validateSecretName(name: string): void {",
    "  if (name.length === 0) {",
    "    throw new Error(name)",
    "  }",
    "  return (",
    "    name",
    "  )",
    "}",
    "export class Secret {",
    "  static fill(secret: string): string {",
    "}",
  ].join("\n");

  // A module-level function's body sits at the same two-space indentation as a class member, so
  // `  if (` inside `function validateSecretName` matches the member pattern. On the real file this
  // added a member named `if` to the secret surface.
  assert.deepEqual(scanTypeScriptPublicMembers(typescript), ["fill"]);
});

test("a Python property is an attribute in both languages, not a method", () => {
  const python = [
    "class Volume(ClientFactory):",
    "    @property",
    "    def volume_id(self) -> str:",
    "    @property",
    "    def name(self) -> str:",
    "    @cached_property",
    "    def token(self) -> Optional[str]:",
    "    @classmethod",
    "    def create(cls, name: str) -> Self:",
    "    @classmethod",
    "    def _class_list(cls) -> List[VolumeInfo]:",
    "    @overload",
    "    def read_file(",
    "    async def write_file(self, path: str) -> None:",
    "    def _run_git(self) -> None:",
  ].join("\n");

  // `Volume` exposes `volumeId` / `name` / `token` as `readonly` fields in TypeScript and as
  // `@property` accessors in Python. Counting the accessors as methods reports three Python-only
  // "methods" that are one public surface written the way each language writes it. The other
  // decorators must not disqualify a member: `@overload`'s accessor is still `read_file`, and
  // looking no further than the first decorator would drop it.
  assert.deepEqual(scanPythonPublicMembers(python), ["create", "read_file", "write_file"]);
});

test("the naming convention is normalised away, leaving only real divergences", () => {
  assert.equal(normalisePythonNameToCamelCase("send_stdin"), "sendStdin");
  assert.equal(normalisePythonNameToCamelCase("update_metadata"), "updateMetadata");
  assert.equal(normalisePythonNameToCamelCase("list"), "list");

  // `send_stdin` normalises to `sendStdin`, which is still not JavaScript's `sendInput` — so the
  // divergence survives normalisation, which is the whole point of doing it. A pair that differs
  // only by convention must *not* survive.
  assert.notEqual(normalisePythonNameToCamelCase("send_stdin"), "sendInput");
  assert.equal(normalisePythonNameToCamelCase("create_branch"), "createBranch");
});

test("sampler arguments default to a local read and fail closed on unusable input", () => {
  assert.deepEqual(parseArgs([]), { fetch: false, json: false });
  assert.deepEqual(parseArgs(["--fetch", "--json"]), { fetch: true, json: true });
  assert.equal(parseArgs(["--help"]).help, true);
  assert.throws(() => parseArgs(["--bogus"]), /unsupported argument/u);
});

test("a tree without the vendored clones exits 3, which is not the clean exit 0", () => {
  withTempTree((tree) => {
    // The tool resolves its repository root from its own location, so relocating a copy into a tree
    // that has no `external/` reproduces the fresh-clone case exactly.
    mkdirSync(path.join(tree, "tools"), { recursive: true });
    const relocated = path.join(tree, "tools", path.basename(toolPath));
    copyFileSync(toolPath, relocated);

    const result = spawnSync(process.execPath, [relocated], { cwd: tree, encoding: "utf8" });
    assert.equal(result.status, 3, `expected the unsampled exit, got ${result.status}: ${result.stderr}`);
    assert.match(result.stderr, /not vendored/u);
    assert.match(result.stderr, /must not look alike/u);

    // The distinction is the contract: exit 0 must be unreachable when nothing was sampled.
    assert.notEqual(result.status, 0);
  });
});

test("the sampler against this repository reports a defined outcome and never invents a clean run", () => {
  // This test must hold on a fresh clone, where `external/` is legitimately absent. It therefore
  // asserts the invariant rather than a fixed code: 3 when nothing could be sampled, and 0 or 1
  // only when the vendored trees were actually present.
  const result = spawnSync(process.execPath, [toolPath, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  const externalPresent = existsSync(path.join(repoRoot, "external"));

  if (!externalPresent) {
    assert.equal(result.status, 3, result.stderr);
    return;
  }

  assert.ok([0, 1].includes(result.status), `expected a sampled outcome, got ${result.status}: ${result.stderr}`);
  const report = JSON.parse(result.stdout);
  assert.equal(report.kind, "sdkwork.sandbox.e2b-upstream-source-sample");
  assert.equal(report.readings.envdRestEndpoints !== undefined, true);
  assert.equal(
    (report.readings.envdOrchestratorOnlyEndpoints ?? 0) + (report.readings.envdClientReachableEndpoints ?? 0),
    report.readings.envdRestEndpoints,
    "the two envd trust zones must partition the declared endpoints",
  );
  assert.equal(report.findings.length === 0, result.status === 0, "an exit code must agree with the finding list");
});

test("a sampled tree reports drift as findings rather than re-baselining them away", () => {
  // Guards the sampler's central promise: recorded values are compared, not rewritten. Mutating the
  // vendored tree is not an option here (it is gitignored and absent on a fresh clone), so the
  // assertion is made against the shape the sampler must return.
  if (!existsSync(path.join(repoRoot, "external"))) {
    return;
  }
  const { readings, findings } = sampleUpstreamSource();
  assert.equal(typeof readings.controlPlaneOperations, "number");
  assert.ok(Array.isArray(findings));
  for (const finding of findings) {
    assert.equal(typeof finding.check, "string");
    assert.equal(typeof finding.message, "string");
    assert.ok(finding.message.length > 0, "a finding must say what moved");
  }
});
