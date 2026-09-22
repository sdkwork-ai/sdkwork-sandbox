import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readText(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function parseIndexEntries() {
  const entries = [];
  let currentEntry;

  for (const line of readText("docs/INDEX.yaml").split(/\r?\n/u)) {
    // `domains:` and `canon:` are top-level sections whose children also carry a four-space
    // `path:` key (`    path: docs/product/prd/`). Without resetting the cursor on a top-level
    // key, the first `path:` after `entries:` ends silently overwrote the LAST entry, which made
    // the "every working document is indexed" assertion structurally unable to see a working
    // document registered in the last position.
    if (/^[^\s]/u.test(line)) {
      currentEntry = undefined;
      continue;
    }

    const idMatch = line.match(/^  - id:\s*(\S+)\s*$/u);
    if (idMatch) {
      currentEntry = { id: idMatch[1] };
      entries.push(currentEntry);
      continue;
    }
    if (!currentEntry) {
      continue;
    }

    const pathMatch = line.match(/^    path:\s*(\S+)\s*$/u);
    if (pathMatch) {
      currentEntry.path = pathMatch[1];
      continue;
    }

    const statusMatch = line.match(/^    status:\s*(\S+)\s*$/u);
    if (statusMatch) {
      currentEntry.status = statusMatch[1];
    }
  }

  return entries;
}

function readDocumentStatus(relativePath) {
  const statusMatch = readText(relativePath).match(/^(?:status|Status):\s*(\S+)\s*$/mu);
  assert.ok(statusMatch, `${relativePath} must declare a status`);
  return statusMatch[1];
}

const workingDirectories = [
  ["docs/product/requirements", /^REQ-.*\.md$/u],
  ["docs/architecture/decisions", /^ADR-.*\.md$/u],
  ["docs/engineering/plans", /^PLAN-.*\.md$/u],
  ["docs/engineering/reviews", /^REVIEW-.*\.md$/u],
];

test("documentation index status matches every registered working document", () => {
  const workingEntries = parseIndexEntries().filter((entry) =>
    workingDirectories.some(([directory]) => entry.path?.startsWith(`${directory}/`)),
  );

  assert.ok(workingEntries.length > 0);
  const mismatches = [];
  for (const entry of workingEntries) {
    assert.ok(entry.path, `${entry.id} must declare a path`);
    assert.ok(entry.status, `${entry.id} must declare a status`);
    const documentStatus = readDocumentStatus(entry.path);
    if (entry.status !== documentStatus) {
      mismatches.push({
        id: entry.id,
        path: entry.path,
        indexStatus: entry.status,
        documentStatus,
      });
    }
  }
  assert.deepEqual(mismatches, [], "documentation index contains stale working-document status");
});

test("every REQ ADR PLAN and REVIEW working document is indexed", () => {
  const indexedPaths = new Set(parseIndexEntries().map((entry) => entry.path));

  for (const [directory, filePattern] of workingDirectories) {
    for (const fileName of readdirSync(path.join(repoRoot, directory))) {
      if (!filePattern.test(fileName)) {
        continue;
      }
      const relativePath = `${directory}/${fileName}`;
      assert.ok(indexedPaths.has(relativePath), `${relativePath} must be registered in docs/INDEX.yaml`);
    }
  }
});

test("requirement prose does not retain an obsolete self-declared status", () => {
  const requirementEntries = parseIndexEntries().filter((entry) =>
    entry.path?.startsWith("docs/product/requirements/REQ-"),
  );
  const selfStatusPattern =
    /(?:本需求|this requirement)[^\r\n]{0,320}?(?:保持|remains?)\s+`(draft|in-progress|ready|accepted|rejected|superseded)`/giu;
  const mismatches = [];

  for (const entry of requirementEntries) {
    const documentText = readText(entry.path);
    for (const match of documentText.matchAll(selfStatusPattern)) {
      const proseStatus = match[1].toLowerCase();
      if (proseStatus !== entry.status) {
        mismatches.push({
          id: entry.id,
          path: entry.path,
          documentStatus: entry.status,
          proseStatus,
        });
      }
    }
  }

  assert.deepEqual(mismatches, [], "requirement prose contains a stale self-declared status");
});
