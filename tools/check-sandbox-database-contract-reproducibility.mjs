#!/usr/bin/env node
/**
 * Database contract reproducibility gate.
 *
 * `DATABASE_FRAMEWORK_SPEC.md` section 6.2 makes the materialized contract files
 * (`database/contract/schema.yaml`, `prefix-registry.json`, `table-registry.json`) generated
 * artifacts, and this repository registers exactly one generator for them in
 * `package.json` -> `db:materialize:contract`. A generated artifact that its own registered
 * generator cannot reproduce is unmaintainable: every future regeneration produces a spurious
 * diff, and the generator's own ordering logic exists specifically so that "unchanged content
 * produces no diff and real drift is not hidden in the noise". Worse, because a naive
 * regeneration *deletes* fields rather than adding them, an operator who runs the registered
 * command silently loses authored data.
 *
 * Measured 2026-09-22: the committed `table-registry.json` carried `contractVersion` and
 * `moduleId`, which the generator never emits; 68 of 70 workspace `sdkwork-*` repositories have
 * the generator's shape, and no consumer reads either field from this file. Both values already
 * live in `schema.yaml` and `database.manifest.json`. The committed `prefix-registry.json` also
 * carried a hand-compacted `forbidden_aliases` array that the generator reflows.
 *
 * This gate re-runs the *registered* command against a throwaway copy of the inputs and fails
 * when the committed artifacts are not what that command produces. It reads the argument list
 * from `package.json` rather than hard-coding it, so the gate cannot drift away from the
 * command operators actually run; only the `--root` value is redirected to the copy.
 *
 * The gate writes nothing inside the repository except a temporary directory under the
 * git-ignored `target/` tree, which it removes before returning.
 *
 * `check-database-framework-standard.mjs` and `verify-database-initialization-state.mjs` validate
 * the contract's shape and its relationship to the baseline; neither re-runs the generator, so
 * neither can detect non-reproducible artifacts.
 *
 * Usage:
 *   node tools/check-sandbox-database-contract-reproducibility.mjs [--root <dir>] [--json]
 */

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const MATERIALIZE_SCRIPT = "db:materialize:contract";
const CONTRACT_DIRECTORY = "database/contract";
const TEMPORARY_ROOT = "target/contract-reproducibility";
const INPUT_PATHS = [
  "database/database.manifest.json",
  "database/contract",
  "database/ddl/baseline",
];
const GENERATED_FILES = ["schema.yaml", "prefix-registry.json", "table-registry.json"];

/**
 * Split a `node <tool> --flag value ...` script into the tool path and its arguments.
 * Only the `--flag value` form is used by repository scripts, so no quoting rules are needed.
 */
export function parseRegisteredCommand(script) {
  const tokens = String(script).trim().split(/\s+/u);
  if (tokens[0] !== "node" || tokens.length < 2) {
    throw new Error(`${MATERIALIZE_SCRIPT} must be a 'node <tool> ...' command, saw '${script}'`);
  }
  const tool = tokens[1];
  const args = tokens.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    if (args[index].startsWith("--")) {
      index += 1;
    }
  }
  return { tool, args };
}

function readRegisteredCommand(repoRoot) {
  const packageJsonPath = join(repoRoot, "package.json");
  if (!existsSync(packageJsonPath)) {
    throw new Error("package.json is missing, so the registered materialization command is unknown");
  }
  const scripts = JSON.parse(readFileSync(packageJsonPath, "utf8")).scripts ?? {};
  const script = scripts[MATERIALIZE_SCRIPT];
  if (typeof script !== "string" || script.trim() === "") {
    throw new Error(`package.json has no '${MATERIALIZE_SCRIPT}' script to reproduce the contract with`);
  }
  return script;
}

/** Replace the value that follows `--root`, or append one, so the generator writes into `root`. */
export function redirectRoot(args, root) {
  const redirected = [...args];
  const index = redirected.indexOf("--root");
  if (index >= 0) {
    if (index + 1 >= redirected.length) {
      throw new Error("the registered command passes --root with no value");
    }
    redirected[index + 1] = root;
  } else {
    redirected.push("--root", root);
  }
  return redirected;
}

/** Compare two text files ignoring line-ending style, which git normalizes per checkout. */
export function normaliseText(text) {
  return String(text).replace(/\r\n/gu, "\n");
}

function firstDifferingLine(expected, actual) {
  const left = normaliseText(expected).split("\n");
  const right = normaliseText(actual).split("\n");
  const limit = Math.max(left.length, right.length);
  for (let index = 0; index < limit; index += 1) {
    if (left[index] !== right[index]) {
      return {
        line: index + 1,
        expected: left[index] === undefined ? "(missing line)" : left[index].trim(),
        actual: right[index] === undefined ? "(missing line)" : right[index].trim(),
      };
    }
  }
  return null;
}

export function assessDatabaseContractReproducibility({ repoRoot = repositoryRoot } = {}) {
  const failures = [];
  const command = readRegisteredCommand(repoRoot);
  const { tool, args } = parseRegisteredCommand(command);

  const toolPath = resolve(repoRoot, tool);
  if (!existsSync(toolPath)) {
    return {
      ok: false,
      repoRoot,
      command,
      temporaryRoot: null,
      // Kept as an array so every return path has the same shape; callers read `.length`.
      filesChecked: [],
      failures: [
        {
          file: tool,
          reason: "missing-generator",
          message: `the registered ${MATERIALIZE_SCRIPT} tool '${tool}' does not exist; the contract cannot be reproduced`,
        },
      ],
    };
  }

  const temporaryRoot = join(repoRoot, TEMPORARY_ROOT);
  rmSync(temporaryRoot, { recursive: true, force: true });
  const filesChecked = [];

  try {
    for (const input of INPUT_PATHS) {
      const source = join(repoRoot, input);
      if (!existsSync(source)) {
        throw new Error(`materialization input '${input}' does not exist`);
      }
      const destination = join(temporaryRoot, input);
      mkdirSync(dirname(destination), { recursive: true });
      cpSync(source, destination, { recursive: true });
    }

    execFileSync(process.execPath, [toolPath, ...redirectRoot(args, TEMPORARY_ROOT)], {
      cwd: repoRoot,
      stdio: "pipe",
      encoding: "utf8",
    });

    for (const name of GENERATED_FILES) {
      const committedPath = join(repoRoot, CONTRACT_DIRECTORY, name);
      const regeneratedPath = join(temporaryRoot, CONTRACT_DIRECTORY, name);
      filesChecked.push(name);
      if (!existsSync(committedPath)) {
        failures.push({
          file: `${CONTRACT_DIRECTORY}/${name}`,
          reason: "missing-generated-file",
          message: `the generator emits ${name} but ${CONTRACT_DIRECTORY}/${name} is absent`,
        });
        continue;
      }
      if (!existsSync(regeneratedPath)) {
        failures.push({
          file: `${CONTRACT_DIRECTORY}/${name}`,
          reason: "unexpected-generator-output",
          message: `the registered command did not emit ${name}`,
        });
        continue;
      }
      const committed = readFileSync(committedPath, "utf8");
      const regenerated = readFileSync(regeneratedPath, "utf8");
      if (normaliseText(committed) === normaliseText(regenerated)) {
        continue;
      }
      const difference = firstDifferingLine(committed, regenerated);
      failures.push({
        file: `${CONTRACT_DIRECTORY}/${name}`,
        reason: "non-reproducible-contract",
        message:
          `re-running 'pnpm run ${MATERIALIZE_SCRIPT}' produces a different ${name}` +
          (difference
            ? `; first difference at line ${difference.line}: committed '${difference.expected}' vs regenerated '${difference.actual}'`
            : ""),
      });
    }
  } catch (error) {
    failures.push({
      file: CONTRACT_DIRECTORY,
      reason: "materialization-failed",
      message: `the registered command could not be re-run: ${error.message.trim().split("\n").at(-1)}`,
    });
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    command,
    temporaryRoot: relative(repoRoot, temporaryRoot).split(sep).join("/"),
    filesChecked,
    failures,
  };
}

export function formatDatabaseContractReproducibilityReport(assessment) {
  if (assessment.ok) {
    return (
      `Database contract reproducibility: re-running the registered command reproduces ` +
      `${assessment.filesChecked.length} committed artifact(s) byte for byte\n`
    );
  }
  const lines = [
    `Database contract reproducibility: ${assessment.failures.length} problem(s); ` +
      `re-running the registered command produced ${assessment.filesChecked.length} artifact(s)`,
  ];
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.file}: ${failure.message}`);
  }
  lines.push(
    `Regenerate with 'pnpm run ${MATERIALIZE_SCRIPT}' and commit the result, or correct the ` +
      "registered command if it no longer describes how the contract is produced.",
  );
  return `${lines.join("\n")}\n`;
}

export function parseDatabaseContractReproducibilityArgs(argv) {
  const options = { json: false, root: repositoryRoot };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
    } else if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("--root requires a directory");
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
    const options = parseDatabaseContractReproducibilityArgs(process.argv.slice(2));
    const assessment = assessDatabaseContractReproducibility({ repoRoot: options.root });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatDatabaseContractReproducibilityReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox database contract reproducibility check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
