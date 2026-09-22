#!/usr/bin/env node
/**
 * Static gate: the product traceability chain `PRD -> requirement -> architecture decision ->
 * implementation/verification` must be resolvable from the documents that assert it.
 *
 * Why this exists. `DOCUMENTATION_SPEC.md` section 28 makes lifecycle documentation traceable
 * through that chain, and `REQUIREMENTS_SPEC.md` section 6 requires that a requirement id
 * referenced by docs, code comments, ADRs or tests resolves to a real requirement record. No
 * validator in `../sdkwork-specs/tools/` or in this repository checked either invariant, so on
 * 2026-09-22 the repository carried references that resolve to nothing or to the wrong record:
 *
 *   - `docs/migrations/MIG-2026-0003-postgresql-quota-capacity-tables.md` cited
 *     `ADR-20260729-quota-persistence`, an id that matches no decision record. The same document
 *     already carried the authoritative id in its `Adr:` header, so the body silently disagreed
 *     with its own header.
 *   - `docs/engineering/plans/PLAN-2026-0002-...md` wrote `BirdCoder REQ-2026-0006` and
 *     `Kernel REQ-2026-0002` as bare ids. `cargo fmt`-style link checking did not catch them
 *     because they are plain text, and worse, both numbers are also *this* repository's own
 *     requirement ids, so a reader, a tool, or a future agent resolves them to the Sandbox
 *     records for key rotation and lifecycle core instead of the BirdCoder and Kernel records
 *     the sentence actually means. `check-sandbox-doc-integrity.mjs` cannot see this: it checks
 *     link *targets* and prescribed *commands*, never id semantics.
 *
 * Six rule families, all deterministic:
 *
 *   1. ID RESOLUTION. Every `REQ-####-####` and `ADR-########-<slug>` token in a live document
 *      must resolve:
 *      1a. A `REQ-*` token resolves when it is one of this repository's requirement records, or
 *          when it is written inside a sibling-repository path
 *          (`sdkwork-<name>/.../REQ-....md`). A cross-repository requirement is located by path,
 *          not by number, and the path must be the one the id lives in rather than one mentioned
 *          elsewhere on the line.
 *      1b. An `ADR-*` token resolves when it is one of this repository's decision records, or
 *          when the same line anchors it to an existing decision file (`ADR-....md`). The
 *          repository legitimately writes abbreviated link text such as
 *          `[ADR-20260729: ...](../../architecture/decisions/ADR-20260729-full-slug.md)`, so the
 *          anchor, not the abbreviation, is what must resolve; bare abbreviations that match
 *          many records are exactly the ambiguous case this rejects.
 *
 *   2. NO SHADOWED CROSS-REPOSITORY ID. A `REQ-*` token immediately preceded by another
 *      repository's owner label (`BirdCoder`, `Agents`, `Kernel`, `Drive`, `IAM`, `Commerce`)
 *      names that repository's requirement, so it must be written inside a sibling-repository
 *      path. Without the path the sentence is indistinguishable from a statement about this
 *      repository's identically numbered record.
 *      A general "does any nearby word name another repository" heuristic was measured and
 *      rejected: it produced 93 hits of which all were false positives, because prose about
 *      BirdCoder routinely discusses *this* repository's requirements. Requiring the owner label
 *      to directly prefix the id produced 5 hits, all genuine.
 *
 *   3. NO ORPHAN AUTHORITY. Every requirement record and decision record must be referenced by at
 *      least one live document other than itself. An authority that nothing cites is either dead
 *      or is missing from the product canon index; either way the chain has a break.
 *
 *   4. CAPABILITY MATRIX CLASSIFICATION. In `docs/product/prd/PRD-capabilities.md` section 11 every
 *      capability row must be classified, using the section's own vocabulary: it cites at least one
 *      `REQ-*`, or it carries the no-carrier marker `无` that the section header defines, or it
 *      inherits the row above with `同上`. A row that does none of the three leaves the reader
 *      unable to tell whether the capability is authorized. Row numbers must also be contiguous
 *      from 1, so a dropped or appended row cannot pass unnoticed.
 *      The census printed on every run states only what the document literally says, and its buckets
 *      form a partition: every row is counted as citing a requirement, as carrying the no-carrier
 *      marker with no requirement, as inheriting the row above, or as unattributed — exactly one of
 *      the four. A row that both cites a requirement and adds a `无` caveat for a narrower scope
 *      (`Snapshot`, `Egress Policy`) is counted once, under "citing", and additionally reported in
 *      the caveat sub-count, because that caveat is the easiest part of the row to read past. An
 *      invariant fails if the four buckets do not sum to the matrix row count.
 *      It deliberately does not publish a derived "carried / not carried" total: deciding whether a
 *      row such as `Snapshot` (which states it has no product-level capability while pointing at two
 *      Gate 0 requirements) is "carried" is a product judgement, and `PRD.md` section 6 reserves the
 *      product-completion claim for capabilities that have both a carrier *and* verification
 *      evidence. A capability without a requirement needs an owner decision to split one, so this
 *      gate reports the gap and never invents a requirement to close it.
 *
 *   5. PRODUCT STATE MACHINE JOIN. The state machine in `docs/product/prd/PRD-capabilities.md`
 *      section 3 and the `SandboxSessionState` enum in
 *      `crates/sdkwork-intelligence-sandbox-service/src/model.rs` are two copies of one contract,
 *      and until 2026-09-22 nothing read them together: the PRD diagram carried `Pausing`/`Paused`/
 *      `Recovering` while the implementation enum held eight states, and the PRD did not say so.
 *      The join is exact, in both directions: every state in the diagram either exists in the
 *      implementation enum or is declared on a `目标态标记` line as a target state the enum does not
 *      have yet; the declared target set must equal the diagram-minus-enum difference exactly, so a
 *      target that reaches the implementation without the marker shrinking is as red as a diagram
 *      state that is neither implemented nor declared; every implementation state must appear in
 *      the diagram, so the product canon cannot silently drop one.
 *
 *   6. OBSERVABILITY FAMILY JOIN. The metric families in `docs/product/prd/PRD-sandbox-surfaces.md`
 *      section 13 and the `metrics.productFamilies` mapping in
 *      `apis/async/sandbox-observability-catalog.json` are the product promise and its machine
 *      mapping; before this family existed the PRD declared fifteen families (several named
 *      against the OBSERVABILITY_SPEC metric-naming rules) and nothing compared the two lists —
 *      the namesets were fully disjoint and nobody could tell. The join is bidirectional: every
 *      family declared in the PRD has exactly one mapping entry and vice versa; every mapping
 *      entry names a valid plane, a `kind` whose suffix the name actually carries
 *      (`duration` -> `_duration_seconds`, `counter` -> `_total`, gauge names carry neither), and
 *      `catalogMetrics` ids that resolve to real `metrics.catalog` entries; an entry that maps to
 *      nothing must say in its note which requirement or PRD row owns the gap, so a runtime-plane
 *      family cannot silently pretend a contract exists.
 *
 * Live documentation means every markdown/json/yaml document except the point-in-time evidence
 * records under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and
 * `docs/archive/`, which record what was true when they were written.
 *
 * Usage:
 *   node tools/check-sandbox-requirement-traceability.mjs [--json] [--root <dir>]
 *
 * `--root` audits another tree, which is how the contract test proves this gate can go red.
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const SKIPPED_DIRECTORIES = new Set([".git", "node_modules", "target", ".workbuddy"]);

/**
 * Point-in-time evidence records. Their references are historical facts: a review packet written
 * in July may cite the id that was current in July.
 */
const HISTORICAL_EVIDENCE_DIRECTORIES = [
  "docs/archive/",
  "docs/changelogs/",
  "docs/engineering/reviews/",
  "docs/releases/",
];

const REQUIREMENTS_DIRECTORY = join("docs", "product", "requirements");
const DECISIONS_DIRECTORY = join("docs", "architecture", "decisions");
const CAPABILITY_MATRIX_FILE = join("docs", "product", "prd", "PRD-capabilities.md");
const CAPABILITY_MATRIX_HEADING = "## 11. ";
const CAPABILITY_MATRIX_END_HEADING = "## 12. ";

/**
 * Product state machine (PRD section 3) and the implementation enum it must stay joined with. The
 * enum file is read as text, not compiled: the gate is static and must go red on a fixture tree
 * that carries a different enum body.
 */
const STATE_MACHINE_HEADING = "## 3. ";
const IMPLEMENTATION_STATE_FILE = join(
  "crates",
  "sdkwork-intelligence-sandbox-service",
  "src",
  "model.rs",
);
const IMPLEMENTATION_STATE_ENUM = "SandboxSessionState";
/** The line label the PRD uses to declare states the implementation enum does not have yet. */
const TARGET_STATE_MARKER = "目标态标记";

/**
 * Observability surface (PRD-sandbox-surfaces section 13) and the machine contract that owns the
 * concrete metric inventory the product families map into.
 */
const OBSERVABILITY_SURFACES_FILE = join("docs", "product", "prd", "PRD-sandbox-surfaces.md");
const OBSERVABILITY_HEADING = "## 13. ";
const OBSERVABILITY_END_HEADING = "## 14. ";
const OBSERVABILITY_CATALOG_FILE = join("apis", "async", "sandbox-observability-catalog.json");
const PRODUCT_FAMILY_PLANES = Object.freeze(["control-plane", "runtime-plane"]);
const PRODUCT_FAMILY_KINDS = Object.freeze(["duration", "counter", "gauge"]);

const REQUIREMENT_ID = /REQ-\d{4}-\d{4}/gu;
const DECISION_ID = /ADR-\d{8}-[a-z0-9-]+/gu;
const DECISION_FILE_ANCHOR = /ADR-\d{8}-[a-z0-9-]+\.md/gu;
/** Owner labels of the repositories that share this requirement numbering space. */
const CROSS_REPOSITORY_OWNER_LABEL = /\b(?:BirdCoder|Agents|Kernel|Drive|IAM|Commerce)\s+$/u;
/** The capability matrix's own marker for "this capability has no carrier requirement yet". */
const NO_CARRIER_MARKER = "无";
/**
 * The capability matrix's own way of inheriting the row above, which is how `Resume` takes the
 * gate of `Pause` and `Auto Resume` takes the gate of `Auto Pause`.
 */
const BACK_REFERENCE_MARKER = "同上";
const DOCUMENT_EXTENSIONS = [".md", ".json", ".yaml", ".yml"];

function toPosix(value) {
  return value.split(sep).join("/");
}

function isHistoricalEvidence(relativePath) {
  return HISTORICAL_EVIDENCE_DIRECTORIES.some((prefix) => relativePath.startsWith(prefix));
}

/** Discover every document this gate reads: live documents plus the authority records. */
export function discoverRepositoryDocuments(root) {
  const documents = [];
  function walk(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (SKIPPED_DIRECTORIES.has(entry.name)) {
        continue;
      }
      const absolute = join(directory, entry.name);
      if (entry.isDirectory()) {
        walk(absolute);
        continue;
      }
      if (DOCUMENT_EXTENSIONS.some((extension) => entry.name.endsWith(extension))) {
        documents.push(absolute);
      }
    }
  }
  walk(root);
  return documents.sort();
}

/**
 * The authority records this repository owns. A record whose file name does not carry its own id
 * cannot be resolved by id at all, so it is reported instead of being silently skipped.
 */
export function readLocalAuthorities({ repoRoot }) {
  const requirements = new Map();
  const decisions = new Map();
  const failures = [];

  const requirementsDirectory = join(repoRoot, REQUIREMENTS_DIRECTORY);
  const decisionsDirectory = join(repoRoot, DECISIONS_DIRECTORY);
  // Record paths are reported and compared in posix form. Mixing the platform separator in here
  // silently broke the "cited by another document" comparison on Windows: `docs\product\...`
  // never equals the `docs/product/...` that reference collection produces, so every record
  // looked cited by itself and rule 3 could not fail.
  const requirementsPrefix = toPosix(REQUIREMENTS_DIRECTORY);
  const decisionsPrefix = toPosix(DECISIONS_DIRECTORY);

  if (existsSync(requirementsDirectory)) {
    for (const name of readdirSync(requirementsDirectory).sort()) {
      if (!name.endsWith(".md")) continue;
      const id = REQUIREMENT_ID.exec(name)?.[0];
      REQUIREMENT_ID.lastIndex = 0;
      if (!id) {
        if (/^REQ-/u.test(name)) {
          failures.push({
            reason: "malformed-authority-filename",
            document: `${requirementsPrefix}/${name}`,
            line: 1,
            message: "a requirement record file name must carry its REQ-####-#### id",
          });
        }
        continue;
      }
      requirements.set(id, `${requirementsPrefix}/${name}`);
    }
  }

  if (existsSync(decisionsDirectory)) {
    for (const name of readdirSync(decisionsDirectory).sort()) {
      if (!name.endsWith(".md") || name === "README.md") continue;
      const id = DECISION_ID.exec(name)?.[0];
      DECISION_ID.lastIndex = 0;
      if (!id) {
        if (/^ADR-/u.test(name)) {
          failures.push({
            reason: "malformed-authority-filename",
            document: `${decisionsPrefix}/${name}`,
            line: 1,
            message: "a decision record file name must carry its ADR-########-<slug> id",
          });
        }
        continue;
      }
      decisions.set(id, `${decisionsPrefix}/${name}`);
    }
  }

  return { requirements, decisions, failures };
}

/**
 * Whether the text immediately before `index` qualifies the id that follows with the location of
 * a sibling repository, which is what makes a cross-repository reference unambiguous.
 *
 * The qualifier must be the path the id lives in, not merely a sibling path mentioned somewhere
 * earlier on the line. An earlier build tested the whole prefix and went silently blind on long
 * prose: a line that first explains "qualify it with a sibling path such as `sdkwork-agents/...`"
 * and then writes a bare `Kernel REQ-2026-0002` passed, because an unrelated clause supplied the
 * qualifier. The enclosing whitespace-delimited token is what carries the reference, so that is
 * what is inspected.
 */
function isInsideSiblingRepositoryPath(line, index) {
  const isBoundary = (character) => /[\s"'`()<>[\],;]/u.test(character);
  let start = index;
  while (start > 0 && !isBoundary(line[start - 1])) start -= 1;
  let end = index;
  while (end < line.length && !isBoundary(line[end])) end += 1;
  const token = line.slice(start, end);
  return token.includes("/") && /(?:\.\.\/)?sdkwork-[a-z0-9-]+\//u.test(token);
}

/** Whether the line anchors an abbreviated decision id to a decision file that exists. */
function lineAnchorsDecisionFile(line, decisions) {
  DECISION_FILE_ANCHOR.lastIndex = 0;
  for (const match of line.matchAll(DECISION_FILE_ANCHOR)) {
    const anchorId = match[0].slice(0, -".md".length);
    if (decisions.has(anchorId)) {
      return true;
    }
  }
  return false;
}

/**
 * Classify every capability row of `PRD-capabilities.md` section 11 and check that the section's
 * own contract with its reader holds: each row is either carried by a requirement or marked.
 */
export function readCapabilityMatrix({ repoRoot }) {
  const file = join(repoRoot, CAPABILITY_MATRIX_FILE);
  const relativeFile = toPosix(CAPABILITY_MATRIX_FILE);
  if (!existsSync(file)) {
    return {
      rows: [],
      failures: [
        {
          reason: "missing-capability-matrix",
          document: relativeFile,
          line: 1,
          message: "the capability alignment matrix document must exist",
        },
      ],
    };
  }

  const lines = readFileSync(file, "utf8").split(/\r?\n/u);
  const start = lines.findIndex((line) => line.startsWith(CAPABILITY_MATRIX_HEADING));
  const end = lines.findIndex(
    (line, index) => index > start && line.startsWith(CAPABILITY_MATRIX_END_HEADING),
  );
  const failures = [];
  if (start === -1) {
    failures.push({
      reason: "missing-capability-matrix",
      document: relativeFile,
      line: 1,
      message: `the capability alignment matrix heading \`${CAPABILITY_MATRIX_HEADING}\` must exist`,
    });
    return { rows: [], failures };
  }

  const rows = [];
  const limit = end === -1 ? lines.length : end;
  for (let index = start + 1; index < limit; index += 1) {
    const line = lines[index];
    const match = /^\|\s*(\d+)\s*\|([^|]*)\|(.*)\|\s*$/u.exec(line);
    if (!match) continue;
    const carrier = match[3];
    rows.push({
      number: Number.parseInt(match[1], 10),
      line: index + 1,
      capability: match[2].trim(),
      requirements: [...carrier.matchAll(REQUIREMENT_ID)].map((entry) => entry[0]),
      markedNoCarrier: carrier.includes(NO_CARRIER_MARKER),
      backReference: carrier.includes(BACK_REFERENCE_MARKER),
    });
  }

  if (rows.length === 0) {
    failures.push({
      reason: "missing-capability-matrix",
      document: relativeFile,
      line: start + 1,
      message: "the capability alignment matrix must declare at least one capability row",
    });
    return { rows, failures };
  }

  const expected = rows.map((_row, index) => index + 1);
  const actual = rows.map((row) => row.number);
  if (actual.join(",") !== expected.join(",")) {
    failures.push({
      reason: "capability-row-numbering",
      document: relativeFile,
      line: rows[0].line,
      message: `capability rows must be numbered contiguously from 1; found ${actual.join(", ")}`,
    });
  }

  for (const row of rows) {
    if (row.requirements.length === 0 && !row.markedNoCarrier && !row.backReference) {
      failures.push({
        reason: "unclassified-capability-row",
        document: relativeFile,
        line: row.line,
        message:
          `capability ${row.number} (\`${row.capability}\`) cites no \`REQ-*\`, carries no ` +
          `\`${NO_CARRIER_MARKER}\` marker and does not inherit the row above with ` +
          `\`${BACK_REFERENCE_MARKER}\`, so a reader cannot tell whether it is authorized`,
      });
    }
  }

  return { rows, failures, document: relativeFile };
}

/**
 * The product state machine in PRD section 3: the states the mermaid diagram names, and the
 * target states the section declares the implementation does not have yet.
 *
 * States are read from inside the ```mermaid fence only. The section's prose bullets also name
 * states in backticks (`Running`, `Destroyed`), and reading prose would let a bullet mention make
 * a missing implementation look declared — the diagram is the contract, the bullets explain it.
 */
export function readStateMachine({ repoRoot }) {
  const file = join(repoRoot, CAPABILITY_MATRIX_FILE);
  const relativeFile = toPosix(CAPABILITY_MATRIX_FILE);
  if (!existsSync(file)) {
    return {
      states: [],
      declaredTargetStates: [],
      failures: [
        {
          reason: "missing-state-machine",
          document: relativeFile,
          line: 1,
          message: "the state machine document must exist",
        },
      ],
    };
  }

  const lines = readFileSync(file, "utf8").split(/\r?\n/u);
  const start = lines.findIndex((line) => line.startsWith(STATE_MACHINE_HEADING));
  if (start === -1) {
    return {
      states: [],
      declaredTargetStates: [],
      failures: [
        {
          reason: "missing-state-machine",
          document: relativeFile,
          line: 1,
          message: `the state machine heading \`${STATE_MACHINE_HEADING}\` must exist`,
        },
      ],
    };
  }
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^##\s/u.test(lines[index])) {
      end = index;
      break;
    }
  }
  const section = lines.slice(start, end);

  const failures = [];
  const states = new Set();
  let fenceSeen = false;
  for (const line of section) {
    if (/^\s*```mermaid/u.test(line)) {
      fenceSeen = true;
      continue;
    }
    if (fenceSeen) {
      if (/^\s*```/u.test(line)) {
        fenceSeen = false;
        continue;
      }
      for (const match of line.matchAll(/([A-Za-z][A-Za-z0-9_]*)\s*-->/gu)) states.add(match[1]);
      for (const match of line.matchAll(/-->\s*(?:\[\*\]|([A-Za-z][A-Za-z0-9_]*))/gu)) {
        if (match[1]) states.add(match[1]);
      }
    }
  }
  if (states.size === 0) {
    failures.push({
      reason: "missing-state-machine",
      document: relativeFile,
      line: start + 1,
      message: "the state machine section must carry a ```mermaid diagram naming at least one state",
    });
  }

  const declaredTargetStates = new Set();
  section.forEach((line, index) => {
    // The marker reads naturally as a list item ("- 目标态标记：…"), so a leading bullet is
    // stripped before matching; a marker buried mid-sentence is not the declaration.
    const content = line.trimStart().replace(/^[-*]\s+/u, "");
    if (!content.startsWith(TARGET_STATE_MARKER)) return;
    for (const match of line.matchAll(/`([A-Za-z][A-Za-z0-9_]*)`/gu)) {
      declaredTargetStates.add(match[1]);
    }
    if (declaredTargetStates.size === 0) {
      failures.push({
        reason: "malformed-target-state-marker",
        document: relativeFile,
        line: start + index + 1,
        message: `a \`${TARGET_STATE_MARKER}\` line must name the target states in backticks`,
      });
    }
  });

  return { states: [...states], declaredTargetStates: [...declaredTargetStates], failures, document: relativeFile };
}

/**
 * The `SandboxSessionState` variants the implementation actually declares. Read as text so the
 * rule works on any checkout and goes red, not green-with-exceptions, when the file is gone.
 */
export function readImplementedStates({ repoRoot }) {
  const file = join(repoRoot, IMPLEMENTATION_STATE_FILE);
  const relativeFile = toPosix(IMPLEMENTATION_STATE_FILE);
  if (!existsSync(file)) {
    return {
      states: [],
      failures: [
        {
          reason: "missing-implementation-state-file",
          document: relativeFile,
          line: 1,
          message: `the implementation state file \`${relativeFile}\` must exist`,
        },
      ],
    };
  }
  const text = readFileSync(file, "utf8");
  const body = text.match(
    new RegExp(`pub\\s+enum\\s+${IMPLEMENTATION_STATE_ENUM}\\s*\\{([^}]*)\\}`, "su"),
  );
  if (!body) {
    return {
      states: [],
      failures: [
        {
          reason: "missing-implementation-state-enum",
          document: relativeFile,
          line: 1,
          message: `\`${relativeFile}\` must declare \`pub enum ${IMPLEMENTATION_STATE_ENUM}\``,
        },
      ],
    };
  }
  const states = [...body[1].matchAll(/([A-Za-z][A-Za-z0-9_]*)/gu)]
    .map((match) => match[1])
    // Attribute-less variants only: this enum is a plain state set, and if it ever grows data
    // carriers the parser must be taught the syntax rather than silently misreading it.
    .filter((token) => !["pub", "enum"].includes(token));
  return { states: [...new Set(states)], failures: [], document: relativeFile };
}

/**
 * The observability join: the metric families PRD-sandbox-surfaces section 13 declares, and the
 * `metrics.productFamilies` mapping the catalog contract carries for them.
 */
export function readObservabilityJoin({ repoRoot }) {
  const failures = [];
  const surfacesFile = join(repoRoot, OBSERVABILITY_SURFACES_FILE);
  const catalogFile = join(repoRoot, OBSERVABILITY_CATALOG_FILE);

  let declaredFamilies = [];
  if (!existsSync(surfacesFile)) {
    failures.push({
      reason: "missing-observability-families",
      document: toPosix(OBSERVABILITY_SURFACES_FILE),
      line: 1,
      message: "the observability surfaces document must exist",
    });
  } else {
    const lines = readFileSync(surfacesFile, "utf8").split(/\r?\n/u);
    const start = lines.findIndex((line) => line.startsWith(OBSERVABILITY_HEADING));
    if (start === -1) {
      failures.push({
        reason: "missing-observability-families",
        document: toPosix(OBSERVABILITY_SURFACES_FILE),
        line: 1,
        message: `the observability heading \`${OBSERVABILITY_HEADING}\` must exist`,
      });
    } else {
      let end = lines.length;
      for (let index = start + 1; index < lines.length; index += 1) {
        if (lines[index].startsWith(OBSERVABILITY_END_HEADING)) {
          end = index;
          break;
        }
      }
      let fenceSeen = false;
      for (let index = start; index < end; index += 1) {
        const line = lines[index];
        if (!fenceSeen && /^\s*```(?:text)?\s*$/u.test(line)) {
          fenceSeen = true;
          continue;
        }
        if (fenceSeen) {
          if (/^\s*```/u.test(line)) {
            fenceSeen = false;
            continue;
          }
          const name = line.trim();
          if (name !== "") declaredFamilies.push({ name, line: index + 1 });
        }
      }
      if (declaredFamilies.length === 0) {
        failures.push({
          reason: "missing-observability-families",
          document: toPosix(OBSERVABILITY_SURFACES_FILE),
          line: start + 1,
          message: "the observability section must declare at least one metric family in a fenced block",
        });
      }
    }
  }

  let catalogNames = [];
  let productFamilies = null;
  if (!existsSync(catalogFile)) {
    failures.push({
      reason: "missing-observability-catalog",
      document: toPosix(OBSERVABILITY_CATALOG_FILE),
      line: 1,
      message: `the observability catalog \`${toPosix(OBSERVABILITY_CATALOG_FILE)}\` must exist`,
    });
  } else {
    let catalog;
    try {
      catalog = JSON.parse(readFileSync(catalogFile, "utf8"));
    } catch (error) {
      failures.push({
        reason: "missing-observability-catalog",
        document: toPosix(OBSERVABILITY_CATALOG_FILE),
        line: 1,
        message: `the observability catalog is not valid JSON: ${error.message}`,
      });
      catalog = null;
    }
    if (catalog) {
      catalogNames = (catalog.metrics?.catalog ?? [])
        .map((entry) => entry?.name)
        .filter((name) => typeof name === "string");
      productFamilies = catalog.metrics?.productFamilies ?? null;
      if (!productFamilies || !Array.isArray(productFamilies.families)) {
        failures.push({
          reason: "missing-observability-catalog",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: "the observability catalog must carry metrics.productFamilies.families",
        });
        productFamilies = null;
      }
    }
  }

  return { declaredFamilies, productFamilies, catalogNames, failures };
}

/**
 * The two joins of families 5 and 6, as findings. Kept in one place so the failure reasons and the
 * contract tests that redden them read side by side.
 */
function assessProductJoins({ repoRoot }) {
  const failures = [];

  const prd = readStateMachine({ repoRoot });
  const impl = readImplementedStates({ repoRoot });
  failures.push(...prd.failures, ...impl.failures);
  if (prd.states.length > 0 && impl.states.length > 0) {
    const prdStates = new Set(prd.states);
    const implStates = new Set(impl.states);
    const declared = new Set(prd.declaredTargetStates);
    for (const state of prd.states) {
      if (implStates.has(state)) continue;
      if (declared.has(state)) continue;
      failures.push({
        reason: "unmarked-target-state",
        document: prd.document,
        line: 1,
        message:
          `state \`${state}\` is in the PRD diagram but neither implemented in ` +
          `\`${toPosix(IMPLEMENTATION_STATE_FILE)}\` nor declared on a \`${TARGET_STATE_MARKER}\` line`,
      });
    }
    for (const state of prd.declaredTargetStates) {
      if (!prdStates.has(state)) {
        failures.push({
          reason: "unknown-target-state",
          document: prd.document,
          line: 1,
          message: `\`${TARGET_STATE_MARKER}\` names \`${state}\`, which the diagram does not declare`,
        });
      } else if (implStates.has(state)) {
        failures.push({
          reason: "stale-target-state",
          document: prd.document,
          line: 1,
          message:
            `\`${TARGET_STATE_MARKER}\` still declares \`${state}\` as unimplemented, but the ` +
            `implementation enum contains it; shrink the marker`,
        });
      }
    }
    for (const state of impl.states) {
      if (!prdStates.has(state)) {
        failures.push({
          reason: "unlisted-implementation-state",
          document: impl.document,
          line: 1,
          message: `implementation state \`${state}\` appears in no PRD diagram state`,
        });
      }
    }
  }

  const observability = readObservabilityJoin({ repoRoot });
  failures.push(...observability.failures);
  if (observability.productFamilies && observability.declaredFamilies.length > 0) {
    const declared = observability.declaredFamilies;
    const declaredNames = declared.map((family) => family.name);
    const duplicateDeclared = declaredNames.filter(
      (name, index) => declaredNames.indexOf(name) !== index,
    );
    for (const name of new Set(duplicateDeclared)) {
      failures.push({
        reason: "observability-family-join",
        document: toPosix(OBSERVABILITY_SURFACES_FILE),
        line: declared.find((family) => family.name === name).line,
        message: `metric family \`${name}\` is declared more than once in the observability section`,
      });
    }
    const mapping = observability.productFamilies.families;
    const mappingNames = mapping.map((family) => family?.name);
    const mappedNameSet = new Set(mappingNames);
    const duplicateMapped = mappingNames.filter(
      (name, index) => mappingNames.indexOf(name) !== index,
    );
    for (const name of new Set(duplicateMapped)) {
      failures.push({
        reason: "observability-family-join",
        document: toPosix(OBSERVABILITY_CATALOG_FILE),
        line: 1,
        message: `productFamilies maps \`${name}\` more than once`,
      });
    }
    for (const name of declaredNames) {
      if (!mappedNameSet.has(name)) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `declared metric family \`${name}\` has no metrics.productFamilies entry`,
        });
      }
    }
    for (const name of new Set(mappingNames)) {
      if (!declaredNames.includes(name)) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `metrics.productFamilies maps \`${name}\`, which the observability section does not declare`,
        });
      }
    }
    const catalogNames = new Set(observability.catalogNames);
    for (const entry of mapping) {
      const name = entry?.name ?? "<no name>";
      if (!PRODUCT_FAMILY_PLANES.includes(entry?.plane)) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `productFamilies entry \`${name}\` carries unknown plane "${entry?.plane ?? ""}"`,
        });
      }
      if (!PRODUCT_FAMILY_KINDS.includes(entry?.kind)) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `productFamilies entry \`${name}\` carries unknown kind "${entry?.kind ?? ""}"`,
        });
      }
      if (typeof entry?.name === "string" && !/^[a-z][a-z0-9_]*$/u.test(entry.name)) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `productFamilies entry \`${name}\` is not lowercase snake case`,
        });
      }
      if (entry?.kind === "duration" && !entry.name?.endsWith("_duration_seconds")) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `duration family \`${name}\` must end in \`_duration_seconds\` (OBSERVABILITY_SPEC metric naming)`,
        });
      }
      if (entry?.kind === "counter" && !entry.name?.endsWith("_total")) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `counter family \`${name}\` must end in \`_total\` (OBSERVABILITY_SPEC metric naming)`,
        });
      }
      if (
        entry?.kind === "gauge" &&
        (entry.name?.endsWith("_total") || entry.name?.endsWith("_duration_seconds"))
      ) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message: `gauge family \`${name}\` carries a counter/duration suffix`,
        });
      }
      for (const metric of entry?.catalogMetrics ?? []) {
        if (!catalogNames.has(metric)) {
          failures.push({
            reason: "observability-family-join",
            document: toPosix(OBSERVABILITY_CATALOG_FILE),
            line: 1,
            message: `productFamilies entry \`${name}\` maps \`${metric}\`, which metrics.catalog does not define`,
          });
        }
      }
      const note = entry?.note ?? "";
      if (
        (entry?.catalogMetrics ?? []).length === 0 &&
        (!(typeof note === "string") || note.trim().length < 10 || !/(?:REQ-|PRD-|第 \d+ 节)/u.test(note))
      ) {
        failures.push({
          reason: "observability-family-join",
          document: toPosix(OBSERVABILITY_CATALOG_FILE),
          line: 1,
          message:
            `productFamilies entry \`${name}\` maps to no catalog metric, so its note must name the ` +
            `requirement or PRD row that owns the gap`,
        });
      }
    }
  }

  return {
    failures,
    prdStates: prd.states,
    declaredTargetStates: prd.declaredTargetStates,
    implementedStates: impl.states,
    declaredFamilies: observability.declaredFamilies.map((family) => family.name),
    catalogMetricCount: observability.catalogNames.length,
    runtimePlaneFamilies: (observability.productFamilies?.families ?? [])
      .filter((family) => family?.plane === "runtime-plane")
      .map((family) => family?.name),
  };
}

export function assessRequirementTraceability({ repoRoot = repositoryRoot } = {}) {
  const authority = readLocalAuthorities({ repoRoot });
  const failures = [...authority.failures];
  const matrix = readCapabilityMatrix({ repoRoot });
  failures.push(...matrix.failures);
  const joins = assessProductJoins({ repoRoot });
  failures.push(...joins.failures);

  const documents = discoverRepositoryDocuments(repoRoot);
  const liveDocuments = [];
  const references = new Map([
    ["REQ", new Map()],
    ["ADR", new Map()],
  ]);
  let requirementsReferenced = 0;
  let decisionsReferenced = 0;

  const recordReference = (kind, id, relativePath) => {
    const bucket = references.get(kind);
    if (!bucket.has(id)) bucket.set(id, new Set());
    bucket.get(id).add(relativePath);
    if (kind === "REQ") requirementsReferenced += 1;
    else decisionsReferenced += 1;
  };

  for (const document of documents) {
    const relativePath = toPosix(relative(repoRoot, document));
    if (isHistoricalEvidence(relativePath)) continue;
    liveDocuments.push(relativePath);

    let text;
    try {
      text = readFileSync(document, "utf8");
    } catch {
      continue;
    }
    const lines = text.split(/\r?\n/u);
    lines.forEach((line, index) => {
      const lineNumber = index + 1;

      for (const match of line.matchAll(REQUIREMENT_ID)) {
        const id = match[0];
        recordReference("REQ", id, relativePath);
        const before = line.slice(0, match.index);

        // The owner-label rule is checked before local resolution on purpose. `Kernel REQ-2026-0002`
        // and `BirdCoder REQ-2026-0006` both resolve to this repository's own records, which is
        // exactly the silent misresolution the rule exists to prevent, so short-circuiting on a
        // local match first would make the rule unreachable for every colliding number.
        const qualified = isInsideSiblingRepositoryPath(line, match.index);

        if (CROSS_REPOSITORY_OWNER_LABEL.test(before)) {
          if (qualified) continue;
          failures.push({
            reason: "unqualified-cross-repository-id",
            document: relativePath,
            line: lineNumber,
            message:
              `\`${id}\` is attributed to another repository by the owner label before it, but ` +
              `no sibling-repository path qualifies it; write the owning record's path so the ` +
              `reference cannot be resolved to this repository's identically numbered record`,
          });
          continue;
        }

        if (authority.requirements.has(id)) continue;
        if (qualified) continue;

        failures.push({
          reason: "dangling-id",
          document: relativePath,
          line: lineNumber,
          message:
            `\`${id}\` resolves to no requirement record in this repository and carries no ` +
            `sibling-repository path qualifier`,
        });
      }

      for (const match of line.matchAll(DECISION_ID)) {
        const id = match[0];
        recordReference("ADR", id, relativePath);
        if (authority.decisions.has(id)) continue;
        if (lineAnchorsDecisionFile(line, authority.decisions)) continue;
        failures.push({
          reason: "dangling-id",
          document: relativePath,
          line: lineNumber,
          message:
            `\`${id}\` resolves to no decision record in this repository and the line anchors ` +
            `it to no existing decision file`,
        });
      }
    });
  }

  const unreferencedRequirements = [];
  const unreferencedDecisions = [];
  for (const [id, recordPath] of authority.requirements) {
    const cited = [...(references.get("REQ").get(id) ?? [])].filter(
      (relativePath) => relativePath !== recordPath,
    );
    if (cited.length === 0) unreferencedRequirements.push({ id, record: recordPath });
  }
  for (const [id, recordPath] of authority.decisions) {
    const cited = [...(references.get("ADR").get(id) ?? [])].filter(
      (relativePath) => relativePath !== recordPath,
    );
    if (cited.length === 0) unreferencedDecisions.push({ id, record: recordPath });
  }
  for (const orphan of unreferencedRequirements) {
    failures.push({
      reason: "orphan-authority",
      document: orphan.record,
      line: 1,
      message: `${orphan.id} is referenced by no live document other than itself`,
    });
  }
  for (const orphan of unreferencedDecisions) {
    failures.push({
      reason: "orphan-authority",
      document: orphan.record,
      line: 1,
      message: `${orphan.id} is referenced by no live document other than itself`,
    });
  }

  // The four buckets are a PARTITION of the matrix rows: every row lands in exactly one.
  // They were previously computed independently, which double-counted rows that both cite a
  // requirement and carry a scoped `无` caveat (`Snapshot` says "无产品级能力" while citing
  // REQ-2026-0021/0008; `Egress Policy` cites REQ-2026-0014 while noting that `shared` mode has
  // none). The census line then summed to more rows than the matrix has. The partition is
  // enforced by the `capability-census-partition` invariant below.
  const rowsCitingRequirement = matrix.rows.filter((row) => row.requirements.length > 0);
  const rowsNoCarrier = matrix.rows.filter(
    (row) => row.requirements.length === 0 && row.markedNoCarrier,
  );
  const rowsBackReference = matrix.rows.filter(
    (row) => row.requirements.length === 0 && row.backReference,
  );
  const rowsUnattributed = matrix.rows.filter(
    (row) =>
      row.requirements.length === 0 && !row.markedNoCarrier && !row.backReference,
  );

  // Not part of the partition: rows that DO cite a requirement but qualify it with a `无` caveat
  // about a narrower scope. Reported because such a caveat is the part of the row most easily
  // read past, and the partition above would otherwise hide it.
  const rowsCitingWithScopeCaveat = rowsCitingRequirement.filter((row) => row.markedNoCarrier);

  const partitionSum =
    rowsCitingRequirement.length +
    rowsNoCarrier.length +
    rowsBackReference.length +
    rowsUnattributed.length;
  if (partitionSum !== matrix.rows.length) {
    failures.push({
      reason: "capability-census-partition",
      document: matrix.document,
      line: matrix.rows[0]?.line ?? 1,
      message:
        `the capability census buckets must partition the ${matrix.rows.length} matrix row(s), ` +
        `but they sum to ${partitionSum}; a row is being classified twice or not at all`,
    });
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    liveDocumentsChecked: liveDocuments.length,
    documentsScanned: documents.length,
    requirementsOnRecord: authority.requirements.size,
    decisionsOnRecord: authority.decisions.size,
    requirementReferencesChecked: requirementsReferenced,
    decisionReferencesChecked: decisionsReferenced,
    capabilityRows: matrix.rows.length,
    capabilityRowsCitingRequirement: rowsCitingRequirement.length,
    capabilityRowsCitingWithScopeCaveat: rowsCitingWithScopeCaveat.length,
    capabilityRowsCitingWithScopeCaveatDetail: rowsCitingWithScopeCaveat.map((row) => ({
      number: row.number,
      capability: row.capability,
      document: matrix.document,
      line: row.line,
    })),
    capabilityRowsNoCarrier: rowsNoCarrier.length,
    capabilityRowsBackReference: rowsBackReference.length,
    capabilityRowsUnattributed: rowsUnattributed.length,
    capabilityRowsUnattributedDetail: rowsUnattributed.map((row) => ({
      number: row.number,
      capability: row.capability,
      document: matrix.document,
      line: row.line,
    })),
    capabilityMatrixDocument: matrix.document,
    stateMachineStates: joins.prdStates.length,
    declaredTargetStates: joins.declaredTargetStates,
    implementedStates: joins.implementedStates,
    observabilityFamilies: joins.declaredFamilies.length,
    catalogMetrics: joins.catalogMetricCount,
    runtimePlaneFamilies: joins.runtimePlaneFamilies,
    failures,
  };
}

export function formatRequirementTraceabilityReport(assessment) {
  const chain =
    `Requirement traceability: ${assessment.requirementReferencesChecked} requirement and ` +
    `${assessment.decisionReferencesChecked} decision reference(s) across ` +
    `${assessment.liveDocumentsChecked} live document(s) resolve to ` +
    `${assessment.requirementsOnRecord} requirement and ${assessment.decisionsOnRecord} ` +
    `decision record(s), and every record is cited by another live document\n`;

  const census =
    `Capability alignment census: of ${assessment.capabilityRows} capability row(s) in ` +
    `${assessment.capabilityMatrixDocument}, ` +
    `${assessment.capabilityRowsCitingRequirement} cite a \`REQ-*\`, ` +
    `${assessment.capabilityRowsNoCarrier} are marked \`${NO_CARRIER_MARKER}\` ` +
    `(no carrier requirement), ${assessment.capabilityRowsBackReference} inherit the row above ` +
    `with \`${BACK_REFERENCE_MARKER}\`, and ${assessment.capabilityRowsUnattributed} are ` +
    `unattributed; ` +
    `${assessment.capabilityRowsCitingWithScopeCaveat} of the citing row(s) qualify the carrier ` +
    `with a scoped \`${NO_CARRIER_MARKER}\` caveat` +
      (assessment.capabilityRowsCitingWithScopeCaveatDetail.length > 0
        ? ` (${assessment.capabilityRowsCitingWithScopeCaveatDetail
            .map((row) => `${row.number} \`${row.capability}\``)
            .join(", ")})`
        : "") +
    `\n`;

  const joins =
    `Product joins: the PRD state machine declares ${assessment.stateMachineStates} state(s) ` +
    `(${assessment.declaredTargetStates.length} on the \`目标态标记\` line), the implementation ` +
    `enum carries ${assessment.implementedStates.length}; the observability section declares ` +
    `${assessment.observabilityFamilies} metric famil${assessment.observabilityFamilies === 1 ? "y" : "ies"} ` +
    `(${assessment.runtimePlaneFamilies.length} runtime-plane) against ` +
    `${assessment.catalogMetrics} catalog metric(s)\n`;

  if (assessment.ok) {
    return `${chain}${census}${joins}`;
  }

  const lines = [
    `Requirement traceability failed: ${assessment.failures.length} problem(s) across ` +
      `${assessment.liveDocumentsChecked} live document(s)`,
  ];
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.document}:${failure.line} ${failure.message}`);
  }
  lines.push(census.trimEnd());
  lines.push(joins.trimEnd());
  return `${lines.join("\n")}\n`;
}

export function parseRequirementTraceabilityArgs(argv) {
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
    const options = parseRequirementTraceabilityArgs(process.argv.slice(2));
    const assessment = assessRequirementTraceability({ repoRoot: options.root });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatRequirementTraceabilityReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox requirement traceability check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
