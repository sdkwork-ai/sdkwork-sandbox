#!/usr/bin/env node
/**
 * Static gate: every markdown relative link must resolve, and every command this repository's
 * live documentation tells you to run must be runnable as written.
 *
 * Why this exists. On 2026-09-22 the repository carried fourteen dead relative links and four
 * command prescriptions naming scripts that do not exist:
 *
 *   - `docs/guides/developer/README.md` prescribed `node scripts/documentation-checker.mjs`,
 *     `scripts/component-port-checker.mjs`, `scripts/packages-layout-checker.mjs` and
 *     `scripts/repository-baseline-audit.mjs`. `scripts/` holds only a README; the real tools are
 *     `check-repository-docs-standard.mjs`, `check-component-port-bindings.mjs`,
 *     `check-workspace-packages-layout.mjs` and `audit-repository-baseline.mjs` under
 *     `../sdkwork-specs/tools/`.
 *   - The three `docs/guides/<role>/README.md` files linked `../../../sdkwork-specs/...`, one `..`
 *     short, and `docs/releases/RELEASE-v0.1.0.md` linked repository-root-relative paths.
 *   - Six requirement records and two guides still prescribed `cargo fmt --all -- --check`, which
 *     formats local path dependencies and therefore reports formatting diffs owned by sibling
 *     repositories this repository must not edit (see `AGENTS.md`).
 *
 * None of that failed a gate, because `check-repository-docs-standard.mjs` validates document
 * structure and path ownership, not link resolvability or command executability.
 *
 * Two rule families, both deterministic:
 *
 *   1. LINKS. For every markdown document, each `[text](target)` whose target is relative must
 *      resolve against the document's own directory. Fenced code blocks and inline code spans are
 *      excluded because markdown does not render links inside them.
 *
 *   2. COMMANDS. Inside the fenced code blocks of live documentation:
 *      2a. `cargo fmt --all` is forbidden in any spelling; the gate is `cargo fmt --check`.
 *      2b. A `node <path>.mjs` target must resolve from at least one plausible working directory:
 *          the document's own directory (crate-local READMEs), the repository root (most guides),
 *          or the multi-repository checkout root (the parent of the repository root, which
 *          `DEPENDENCY_MANAGEMENT_SPEC.md` section 1.4 names as a real anchor). A target that
 *          resolves from none of them cannot be run from anywhere. The document's *parent* is not
 *          a candidate: a `../`-prefixed path that resolves only there was written for a directory
 *          the document does not occupy, and accepting it admitted a command whose `--root` pointed
 *          at the checkout root and therefore checked nothing while exiting 0.
 *      2c. A `pnpm run <script>` script must exist in this repository's `package.json`.
 *      2d. A shell-script target with a directory component (`bin/doctor.sh`, `./bin/backup.sh`,
 *          `sdkwork-specs/tools/x.sh`) must exist, because the operational runbooks and `bin/`
 *          entry points are prescribed the same way as the `node` tools and go stale the same way.
 *          A bare `foo.sh` basename is not checked: it names no location.
 *
 * Live documentation means every markdown document except the point-in-time evidence records
 * under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and `docs/archive/`.
 * A recorded command is a fact about the past and may legitimately name a command that has since
 * been replaced; a prescription is an instruction that must work today.
 *
 * Usage:
 *   node tools/check-sandbox-doc-integrity.mjs [--json] [--root <dir>]
 *
 * `--root` audits another tree, which is how the contract test proves this gate can go red.
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Directories this gate does not audit, because they hold no content this repository authors.
 * The criterion is ownership, not interest: `.git` is version-control metadata; `node_modules`,
 * `target` and `.workbuddy` are gitignored generated, build and agent-local state; and `external`
 * holds gitignored read-only clones of upstream reference sources studied for capability parity.
 * Auditing vendored upstream prose here would report defects this repository has no authority to
 * fix, and a rule satisfiable only by editing someone else's files measures the wrong tree.
 */
const SKIPPED_DIRECTORIES = new Set([".git", "node_modules", "target", ".workbuddy", "external"]);

/**
 * Point-in-time evidence records. Their commands are historical facts, not prescriptive
 * instructions, so rule family 2 does not apply to them.
 */
const HISTORICAL_EVIDENCE_DIRECTORIES = [
  "docs/archive/",
  "docs/changelogs/",
  "docs/engineering/reviews/",
  "docs/releases/",
];

const FORBIDDEN_FMT = /\bcargo\s+fmt\s+--all\b/gu;
const NODE_TARGET = /\bnode\s+(?:--[A-Za-z-]+\s+)*([A-Za-z0-9_./*-]+\.mjs)/gu;
const PNPM_RUN = /\bpnpm\s+run\s+([A-Za-z0-9:_-]+)/gu;
/**
 * A shell-script target that names its own directory. The lookbehind rejects an absolute path
 * (`/usr/local/bin/x.sh`), a variable-prefixed one (`$BIN/x.sh`), and a placeholder (`<bin>/x.sh`),
 * none of which claim to live inside this checkout. A bare basename is deliberately not matched.
 */
const SHELL_TARGET = /(?<![\w./$<{:-])((?:\.\/)?(?:[A-Za-z0-9_.-]+\/)+[A-Za-z0-9_.-]+\.sh)/gu;
const LINK = /!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/gu;

/** List every markdown document under `root`, skipping generated and tooling directories. */
export function discoverMarkdownDocuments(root) {
  const documents = [];
  function walk(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (SKIPPED_DIRECTORIES.has(entry.name)) {
        continue;
      }
      const full = join(directory, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else if (entry.name.endsWith(".md")) {
        documents.push(full);
      }
    }
  }
  if (!existsSync(root) || !statSync(root).isDirectory()) {
    throw new Error(`no directory at ${root}; the documentation gate would be vacuous`);
  }
  walk(root);
  return documents.sort();
}

/**
 * Split a document into prose and code, because markdown renders neither links nor commands the
 * same way inside fenced code blocks as outside them.
 *
 * @returns `{ prose, blocks }` where `blocks` is an array of
 * `{ language, body, startLine }`. `prose` retains line structure with code blocks blanked so
 * that reported line numbers stay correct.
 */
export function splitDocument(markdown) {
  const lines = String(markdown).split(/\r?\n/u);
  const prose = [];
  const blocks = [];
  let open = null;
  let buffer = [];
  let startLine = 0;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const fence = line.match(/^\s*```+\s*([A-Za-z0-9_-]*)\s*$/u);
    if (open === null && fence) {
      open = fence[1] || "";
      buffer = [];
      startLine = index + 2;
      prose.push("");
      continue;
    }
    if (open !== null && /^\s*```+\s*$/u.test(line)) {
      blocks.push({ language: open, body: buffer.join("\n"), startLine });
      open = null;
      prose.push("");
      continue;
    }
    if (open !== null) {
      buffer.push(line);
      prose.push("");
      continue;
    }
    prose.push(line);
  }
  if (open !== null) {
    blocks.push({ language: open, body: buffer.join("\n"), startLine, unterminated: true });
  }
  return { prose: prose.join("\n"), blocks };
}

/** Blank inline code spans so a link-looking example inside backticks is not treated as a link. */
export function blankInlineCode(prose) {
  return String(prose).replace(/(`+)[^`]*\1/gu, (match) => " ".repeat(match.length));
}

/**
 * Rule family 1: relative markdown links must resolve from the document's own directory.
 *
 * @returns an array of `{ line, target, reason, message }`.
 */
export function findDeadLinks(documentPath, markdown) {
  const { prose } = splitDocument(markdown);
  const searchable = blankInlineCode(prose);
  const directory = dirname(documentPath);
  const failures = [];
  for (const match of searchable.matchAll(LINK)) {
    const raw = match[1].trim();
    if (/^[A-Za-z][A-Za-z0-9+.-]*:/u.test(raw) || raw.startsWith("#") || raw.startsWith("//")) {
      continue;
    }
    const target = raw.split("#")[0].split("?")[0];
    if (target.length === 0) {
      continue;
    }
    if (existsSync(resolve(directory, target))) {
      continue;
    }
    const line = searchable.slice(0, match.index).split("\n").length;
    failures.push({
      line,
      target: raw,
      reason: "dead-relative-link",
      message:
        `dead relative link '${raw}': resolves to ${resolve(directory, target)}, ` +
        "which does not exist",
    });
  }
  return failures;
}

/** Rules 2a-2c over the fenced code blocks of one live document. */
export function findUnrunnableCommands({
  documentPath,
  markdown,
  repoRoot,
  multiRepoRoot,
  packageScripts = null,
} = {}) {
  const { blocks } = splitDocument(markdown);
  // The document's parent is deliberately NOT a candidate. A `../`-prefixed target that resolves
  // only one level above the document is a claim about where the document lives, and accepting it
  // admitted `node ../../sdkwork-specs/tools/check-database-framework-standard.mjs --root ../..` in
  // `crates/sdkwork-intelligence-sandbox-repository-sqlx/README.md`: from that crate directory the
  // tool path did not resolve, from `crates/` it did, and there `--root ../..` pointed at the
  // checkout root instead of this repository, so the tool reported "Database framework standard
  // skipped (no database/ directory)" and exited 0. A verification command that silently checks
  // nothing is worse than one that fails. Measured before removing the candidate: of 203 `node`
  // prescriptions in live documents, exactly one resolved only from the document's parent, and it
  // was this one; after correcting it, none does, and removing the candidate retires no other
  // command. The checkout root stays because `deployments/webserver/README.md` legitimately anchors
  // `sdkwork-specs/tools/webserver/*.mjs` there.
  const workingDirectories = [dirname(documentPath), repoRoot, multiRepoRoot].filter(
    (value) => typeof value === "string" && value.length > 0,
  );

  const failures = [];
  for (const block of blocks) {
    for (const match of block.body.matchAll(FORBIDDEN_FMT)) {
      const line = block.startLine + block.body.slice(0, match.index).split("\n").length - 1;
      failures.push({
        line,
        reason: "forbidden-fmt-scope",
        message:
          `'${match[0].trim()}' at line ${line} formats local path dependencies, so it reports and ` +
          "writes formatting diffs owned by sibling repositories this repository must not edit; " +
          "use 'cargo fmt --check' (AGENTS.md Build, Test, And Verification)",
      });
    }

    for (const match of block.body.matchAll(NODE_TARGET)) {
      const target = match[1];
      const line = block.startLine + block.body.slice(0, match.index).split("\n").length - 1;
      // A glob names its directory: `tests/contract/*.test.mjs` needs `tests/contract` to exist.
      const probe = target.includes("*")
        ? target.slice(0, target.indexOf("*")).replace(/\/+$/u, "")
        : target;
      const resolvedFrom = workingDirectories.find((base) => existsSync(resolve(base, probe)));
      if (!resolvedFrom) {
        failures.push({
          line,
          reason: "unrunnable-command-target",
          message:
            `'node ${target}' at line ${line} names a path that resolves from none of the ` +
            "document directory, the repository root or the multi-repository root; " +
            "the command cannot be run as written",
        });
      }
    }

    for (const match of block.body.matchAll(SHELL_TARGET)) {
      const target = match[1];
      const probe = target.replace(/^\.\//u, "").replace(/\/+$/u, "");
      if (workingDirectories.some((base) => existsSync(resolve(base, probe)))) {
        continue;
      }
      const line = block.startLine + block.body.slice(0, match.index).split("\n").length - 1;
      failures.push({
        line,
        reason: "unrunnable-command-target",
        message:
          `'${target}' at line ${line} names a script that resolves from none of the ` +
          "document directory, its parent, the repository root or the multi-repository root; " +
          "the prescribed entry point does not exist",
      });
    }

    if (packageScripts) {
      for (const match of block.body.matchAll(PNPM_RUN)) {
        if (packageScripts.has(match[1])) {
          continue;
        }
        const line = block.startLine + block.body.slice(0, match.index).split("\n").length - 1;
        failures.push({
          line,
          reason: "unknown-package-script",
          message:
            `'pnpm run ${match[1]}' at line ${line} is not a script in this repository's ` +
            "package.json",
        });
      }
    }
  }
  return failures;
}

/**
 * @param repoRootPath the repository whose documentation is audited
 * @param multiRepoRootPath the checkout that contains the repository; defaults to its parent
 */
export function assessDocumentationIntegrity({
  repoRoot = repositoryRoot,
  multiRepoRoot = dirname(repoRoot),
} = {}) {
  const documents = discoverMarkdownDocuments(repoRoot);
  if (documents.length === 0) {
    throw new Error("no markdown document found; the documentation gate would be vacuous");
  }

  const packagePath = join(repoRoot, "package.json");
  const packageScripts = existsSync(packagePath)
    ? new Set(Object.keys(JSON.parse(readFileSync(packagePath, "utf8")).scripts ?? {}))
    : null;

  const failures = [];
  let linksChecked = 0;
  let blocksChecked = 0;
  let liveDocuments = 0;
  let scriptTargetsChecked = 0;

  for (const documentPath of documents) {
    const label = relative(repoRoot, documentPath).split(sep).join("/");
    const markdown = readFileSync(documentPath, "utf8");

    const { prose, blocks } = splitDocument(markdown);
    const searchable = blankInlineCode(prose);
    for (const _match of searchable.matchAll(LINK)) {
      linksChecked += 1;
    }
    for (const failure of findDeadLinks(documentPath, markdown)) {
      failures.push({ document: label, ...failure });
    }

    if (HISTORICAL_EVIDENCE_DIRECTORIES.some((prefix) => label.startsWith(prefix))) {
      continue;
    }
    liveDocuments += 1;
    blocksChecked += blocks.length;
    for (const block of blocks) {
      for (const _match of block.body.matchAll(NODE_TARGET)) {
        scriptTargetsChecked += 1;
      }
      for (const _match of block.body.matchAll(SHELL_TARGET)) {
        scriptTargetsChecked += 1;
      }
    }
    for (const failure of findUnrunnableCommands({
      documentPath,
      markdown,
      repoRoot,
      multiRepoRoot,
      packageScripts,
    })) {
      failures.push({ document: label, ...failure });
    }
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    multiRepoRoot,
    documentsChecked: documents.length,
    liveDocumentsChecked: liveDocuments,
    linksChecked,
    codeBlocksChecked: blocksChecked,
    scriptTargetsChecked,
    failures,
  };
}

export function formatDocumentationIntegrityReport(assessment) {
  if (assessment.ok) {
    return (
      `Documentation integrity: ${assessment.linksChecked} relative link(s) across ` +
      `${assessment.documentsChecked} markdown document(s) resolve, and ` +
      `${assessment.codeBlocksChecked} code block(s) across ` +
      `${assessment.liveDocumentsChecked} live document(s) prescribe ` +
      `${assessment.scriptTargetsChecked} existing script target(s)\n`
    );
  }
  const lines = [
    `Documentation integrity failed: ${assessment.failures.length} problem(s) across ` +
      `${assessment.documentsChecked} markdown document(s)`,
  ];
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.document}:${failure.line} ${failure.message}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseDocumentationIntegrityArgs(argv) {
  const options = { json: false, root: repositoryRoot };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
    } else if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("--root requires a directory argument");
      }
      options.root = resolve(value);
      index += 1;
    } else {
      throw new Error(`unsupported argument: ${argument}`);
    }
  }
  return options;
}

function isMainModule() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  try {
    const options = parseDocumentationIntegrityArgs(process.argv.slice(2));
    const assessment = assessDocumentationIntegrity({ repoRoot: options.root });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatDocumentationIntegrityReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox documentation integrity check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
