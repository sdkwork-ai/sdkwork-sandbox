#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Human-review sign-off coherence gate.
 *
 * Every Provider-adjacent contract in `specs/` sets `implementationAuthorized: false` and requires an
 * approved human review before implementation. The review packets that record those decisions already
 * carry a `Status:` header, a Requirement/Decision header, a reviewer sign-off table and a close-out
 * checklist. What was missing was any check that this state is self-consistent:
 *
 *   - nothing proved a contract-named review packet exists on disk;
 *   - nothing proved a reviewer role the contract requires is actually asked to sign the packet;
 *   - nothing proved a review still marked pending cannot coexist with a `ready`/`accepted` requirement,
 *     an `accepted` decision, an `Approved` reviewer outcome, or an authorised contract.
 *
 * This gate closes that hole. It fails when the packet set, the role set, the review status, the
 * requirement status, the decision status, the reviewer outcomes and `implementationAuthorized` stop
 * agreeing with each other. It also prints the live sign-off backlog so a reviewer can see, in one place,
 * everything waiting on a human decision.
 *
 * Passing this gate means "the review record is internally consistent". It never means "the review was
 * approved" and it never authorizes a Provider, Port, API route, SDK, scheduler, isolation policy,
 * deployable profile or runtime dependency change.
 */

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const specsDirectory = resolve(repositoryRoot, "specs");
const reviewsDirectory = resolve(repositoryRoot, "docs/engineering/reviews");
const signoffIndexPath = resolve(repositoryRoot, "docs/engineering/human-review-signoff-backlog.md");
const requirementsDirectory = resolve(repositoryRoot, "docs/product/requirements");
const decisionsDirectory = resolve(repositoryRoot, "docs/architecture/decisions");

const REVIEW_FILE = /^REVIEW-[0-9]{8}-[a-z0-9-]+\.md$/u;
const REVIEW_ID = /^REVIEW-[0-9]{8}-[a-z0-9-]+$/u;
const REVIEW_STATUSES = new Set([
  "pending-human-review",
  "changes-requested",
  "rejected",
  "conditional-pass",
  "accepted",
  "active",
]);
const SIGNED_OFF_STATUSES = new Set(["accepted", "conditional-pass"]);
const ASSESSMENT_STATUSES = new Set(["active"]);
const APPROVED_OUTCOME = "approved";
const REQUIREMENT_IDS_STATISFIED = new Set(["ready", "accepted", "in-progress"]);

function fail(message) {
  throw new Error(message);
}

function normaliseRole(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9]/gu, "");
}

export function parseSignoffArgs(argv) {
  const options = { json: false };
  for (const argument of argv) {
    if (argument === "--json") {
      options.json = true;
    } else {
      fail(`unsupported argument: ${argument}`);
    }
  }
  return options;
}

const REQUIREMENT_ID = /\bREQ-[0-9]{4}-[0-9]{4}\b/u;
const DECISION_ID = /\bADR-[0-9]{8}(?:-[a-z0-9]+)*\b/u;

/**
 * Review packets in this repository write the Requirement/Decision header in two real shapes: a plain
 * id (`Decision: ADR-20260728-runtime-boundary-and-rust-workspace`) and a markdown link whose text is
 * sometimes just the id and sometimes the id plus a title
 * (`[ADR-20260729: Sandbox Observability, Event, Audit And Outbox Boundary](…)`).
 * Extract the id token from either shape instead of trusting the whole text.
 */
export function parseHeaderField(line, idPattern) {
  if (!line) {
    return null;
  }
  const value = line.replace(/^[A-Za-z][A-Za-z-]*:\s*/u, "").trim();
  const link = value.match(/\[([^\]]+)\]\(([^)]+)\)/u);
  const idMatch = (link ? link[1] : value).match(idPattern);
  return { id: idMatch ? idMatch[0] : null, href: link ? link[2].trim() : null, raw: value };
}

export function parseReviewPacket(markdown, { file = null } = {}) {
  const lines = String(markdown).split("\n");
  const statusLine = lines.find((line) => /^Status:/u.test(line));
  const requirementLine = lines.find((line) => /^Requirement:/u.test(line));
  const decisionLine = lines.find((line) => /^Decision:/u.test(line));
  if (!statusLine) {
    fail(`${file ?? "review packet"} has no Status header`);
  }

  const reviewerRows = [];
  const headerIndex = lines.findIndex((line) => /^\|\s*Reviewer role\s*\|/iu.test(line));
  if (headerIndex >= 0) {
    for (let index = headerIndex + 2; index < lines.length; index += 1) {
      const line = lines[index];
      if (!line.startsWith("|")) {
        break;
      }
      const cells = line
        .split("|")
        .slice(1, -1)
        .map((cell) => cell.trim());
      if (cells.length < 3) {
        break;
      }
      reviewerRows.push({ role: cells[0], reviewer: cells[1], outcome: cells[2], date: cells[3] ?? "" });
    }
  }

  return {
    file,
    status: statusLine.replace(/^Status:\s*/u, "").trim(),
    requirement: parseHeaderField(requirementLine, REQUIREMENT_ID),
    decision: parseHeaderField(decisionLine, DECISION_ID),
    hasReviewerTable: headerIndex >= 0,
    hasOutcome: /^Outcome:/mu.test(String(markdown)),
    reviewerRows,
    checklist: {
      unchecked: (String(markdown).match(/^- \[ \]/gmu) ?? []).length,
      checked: (String(markdown).match(/^- \[x\]/gmu) ?? []).length,
    },
  };
}

export function readReviewPackets({
  directory = reviewsDirectory,
  requirements = requirementsDirectory,
  decisions = decisionsDirectory,
} = {}) {
  const packets = [];
  for (const file of readdirSync(directory).filter((name) => REVIEW_FILE.test(name)).sort()) {
    const packet = parseReviewPacket(readFileSync(join(directory, file), "utf8"), { file });
    packet.resolvedRequirement = resolveHeaderField(packet.requirement, {
      fromDirectory: directory,
      searchDirectory: requirements,
    });
    packet.resolvedDecision = resolveHeaderField(packet.decision, {
      fromDirectory: directory,
      searchDirectory: decisions,
    });
    packets.push(packet);
  }
  return packets;
}

export function resolveHeaderField(
  link,
  { fromDirectory = reviewsDirectory, searchDirectory = decisionsDirectory } = {},
) {
  if (!link) {
    return { id: null, exists: false, status: null };
  }
  if (link.href) {
    const direct = resolve(fromDirectory, link.href);
    if (existsSync(direct)) {
      return { id: link.id, exists: true, status: readStatusField(direct), path: direct };
    }
  }
  if (link.id) {
    const matches = readdirSync(searchDirectory).filter(
      (name) => name.replace(/\.md$/u, "") === link.id || name.startsWith(`${link.id}-`),
    );
    if (matches.length === 1) {
      const path = join(searchDirectory, matches[0]);
      return { id: link.id, exists: true, status: readStatusField(path), path };
    }
    if (matches.length > 1) {
      // A date-only id (`ADR-20260729`) is only usable when the packet also carries an href.
      return { id: link.id, exists: false, status: null, ambiguousCandidates: matches.length };
    }
  }
  return { id: link.id, exists: false, status: null };
}

function readStatusField(path) {
  const match = readFileSync(path, "utf8").match(/^status:\s*(.+)$/imu);
  return match ? match[1].trim().toLowerCase() : null;
}

/**
 * Collect the review packets every contract demands, plus the reviewer roles each contract requires.
 * The collector must descend into arrays: several contracts nest `humanReview` inside `providers[]`, and
 * an array-skipping walk silently loses an entire provider subtree.
 */
export function readContractHumanReview({ directory = specsDirectory } = {}) {
  const demands = [];
  for (const file of readdirSync(directory).filter((name) => name.endsWith(".json")).sort()) {
    const value = JSON.parse(readFileSync(join(directory, file), "utf8"));
    if (value?.kind === "sdkwork.sandbox.real-evidence-producer-registry") {
      continue;
    }
    const collect = (node) => {
      if (Array.isArray(node)) {
        node.forEach(collect);
        return;
      }
      if (!node || typeof node !== "object") {
        return;
      }
      const declared = node.reviewPackets ?? (node.reviewPacket ? [node.reviewPacket] : null);
      if (Array.isArray(declared)) {
        demands.push({
          contractFile: file,
          implementationAuthorized: value.implementationAuthorized === true,
          approvedOutcomeRequired: node.approvedOutcomeRequiredBeforeImplementation === true,
          packetIds: declared.filter((id) => REVIEW_ID.test(id)),
          requiredRoles: Array.isArray(node.requiredRoles) ? node.requiredRoles : [],
        });
      }
      Object.values(node).forEach(collect);
    };
    collect(value);
  }
  return demands;
}

export function readSignoffIndex({ file = signoffIndexPath } = {}) {
  return readFileSync(file, "utf8");
}

export function assessHumanReviewSignoff({
  packets,
  demands,
  indexDocument = null,
  reviewsDirectoryPath = reviewsDirectory,
} = {}) {
  if (!Array.isArray(packets) || packets.length === 0) {
    fail("sign-off assessment requires at least one review packet");
  }
  const failures = [];
  const byId = new Map();
  for (const packet of packets) {
    const id = packet.file.replace(/\.md$/u, "");
    byId.set(id, packet);
    if (!REVIEW_STATUSES.has(packet.status)) {
      failures.push(`${id}: unsupported review status '${packet.status}'`);
    }
    if (ASSESSMENT_STATUSES.has(packet.status)) {
      // A continuing assessment spans the whole product rather than one requirement, so it declares an
      // Outcome instead of a Requirement/Decision pair.
      if (!packet.hasOutcome) {
        failures.push(`${id}: a continuing assessment must declare an Outcome header`);
      }
    } else {
      for (const [label, resolved] of [
        ["Requirement", packet.resolvedRequirement],
        ["Decision", packet.resolvedDecision],
      ]) {
        if (!resolved.exists) {
          failures.push(`${id}: ${label} header does not resolve to a recipient record`);
        }
      }
    }
  }

  // Merge every contract demand per packet. Several contracts name the same shared conformance packet,
  // and checking it once per demand would report the same defect twice and hide how many distinct
  // problems actually remain.
  const demandByPacket = new Map();
  for (const demand of demands) {
    for (const packetId of demand.packetIds) {
      if (!demandByPacket.has(packetId)) {
        demandByPacket.set(packetId, {
          requiredRoles: new Set(),
          gatingContracts: new Set(),
          implementationAuthorized: false,
          approvedOutcomeRequired: false,
        });
      }
      const merged = demandByPacket.get(packetId);
      for (const role of demand.requiredRoles) {
        merged.requiredRoles.add(role);
      }
      merged.gatingContracts.add(demand.contractFile);
      merged.implementationAuthorized = merged.implementationAuthorized || demand.implementationAuthorized;
      merged.approvedOutcomeRequired = merged.approvedOutcomeRequired || demand.approvedOutcomeRequired;
    }
  }
  const declaredIds = new Set(demandByPacket.keys());

  for (const packetId of [...demandByPacket.keys()].sort()) {
    const demand = demandByPacket.get(packetId);
    const gateway = [...demand.gatingContracts].sort().join(", ");
    const packet = byId.get(packetId);
    if (!packet) {
      failures.push(
        `${gateway} requires review packet ${packetId}, which does not exist in docs/engineering/reviews`,
      );
      continue;
    }
    const declaredRoles = new Set(packet.reviewerRows.map((row) => normaliseRole(row.role)));
    if (demand.requiredRoles.size > 0 && !packet.hasReviewerTable) {
      failures.push(
        `${packetId} must carry a reviewer sign-off table because ${gateway} requires reviewer roles`,
      );
    }
    for (const role of demand.requiredRoles) {
      if (!declaredRoles.has(normaliseRole(role))) {
        failures.push(
          `${gateway} requires reviewer role '${role}' on ${packetId}, but the packet does not ask that role to sign`,
        );
      }
    }
    if (packet.status === "pending-human-review") {
      const approved = packet.reviewerRows.filter(
        (row) => normaliseRole(row.outcome) === APPROVED_OUTCOME,
      );
      if (approved.length > 0) {
        failures.push(
          `${packetId} is pending-human-review but ${approved.length} reviewer outcome(s) already read Approved`,
        );
      }
      const requirementStatus = packet.resolvedRequirement.status;
      if (requirementStatus && REQUIREMENT_IDS_STATISFIED.has(requirementStatus)) {
        failures.push(
          `${packetId} is pending-human-review but its requirement status is '${requirementStatus}'`,
        );
      }
      const decisionStatus = packet.resolvedDecision.status;
      if (decisionStatus === "accepted") {
        failures.push(`${packetId} is pending-human-review but its decision is already accepted`);
      }
      if (demand.approvedOutcomeRequired && demand.implementationAuthorized) {
        failures.push(
          `${gateway} sets implementationAuthorized while ${packetId} is still pending-human-review`,
        );
      }
    }
    if (demand.implementationAuthorized) {
      if (!SIGNED_OFF_STATUSES.has(packet.status)) {
        failures.push(
          `${gateway} sets implementationAuthorized while ${packetId} status is '${packet.status}'`,
        );
      }
      const notApproved = packet.reviewerRows.filter(
        (row) => normaliseRole(row.outcome) !== APPROVED_OUTCOME,
      );
      if (notApproved.length > 0) {
        failures.push(
          `${gateway} sets implementationAuthorized while ${packetId} has ${notApproved.length} reviewer outcome(s) that are not Approved`,
        );
      }
    }
  }

  if (indexDocument !== null) {
    const mentioned = [...new Set(String(indexDocument).match(/\bREVIEW-[0-9]{8}-[a-z0-9-]+\b/gu) ?? [])].sort();
    const declared = [...declaredIds].sort();
    const omitted = declared.filter((id) => !mentioned.includes(id));
    const unexpected = mentioned.filter((id) => !declared.includes(id));
    if (omitted.length > 0) {
      failures.push(
        `sign-off index omits ${omitted.length} contract-required review packet(s): ${omitted.join(", ")}`,
      );
    }
    if (unexpected.length > 0) {
      failures.push(
        `sign-off index lists ${unexpected.length} review packet(s) no contract requires: ${unexpected.join(", ")}`,
      );
    }
    for (const id of [...mentioned, ...declared]) {
      if (!byId.has(id)) {
        failures.push(`sign-off index references ${id}, which has no review packet file`);
      }
    }
  }

  const backlog = packets
    .filter((packet) => packet.status === "pending-human-review")
    .map((packet) => {
      const id = packet.file.replace(/\.md$/u, "");
      return {
        reviewId: id,
        requirement: packet.resolvedRequirement.id,
        decision: packet.resolvedDecision.id,
        reviewerRoles: packet.reviewerRows.map((row) => row.role),
        gatingContracts: [...(demandByPacket.get(id)?.gatingContracts ?? [])].sort(),
        closeOutItemsRemaining: packet.checklist.unchecked,
      };
    })
    .sort((left, right) => left.reviewId.localeCompare(right.reviewId));

  return {
    ok: failures.length === 0,
    failures,
    summary: {
      sandbox_review_packets: packets.length,
      sandbox_pending_human_review: backlog.length,
      sandbox_named_by_contracts: declaredIds.size,
      sandbox_named_by_contracts_and_pending: backlog.filter((item) => item.gatingContracts.length > 0)
        .length,
    },
    backlog,
    declaredReviewIds: [...declaredIds].sort(),
    reviewsDirectoryPath,
  };
}

export function formatSignoffBacklog(assessment) {
  const lines = [
    "SDKWork Sandbox human-review sign-off coherence",
    `review packets on record: ${assessment.summary.sandbox_review_packets}`,
    `pending human review: ${assessment.summary.sandbox_pending_human_review}`,
    `named as required by a contract: ${assessment.summary.sandbox_named_by_contracts} (${assessment.summary.sandbox_named_by_contracts_and_pending} still pending)`,
    "",
  ];
  if (assessment.ok) {
    lines.push(
      "coherence: consistent (consistent is not approved; no review outcome has been accepted by this check)",
    );
  } else {
    lines.push(`coherence: FAILED (${assessment.failures.length})`);
    for (const failure of assessment.failures) {
      lines.push(`- ${failure}`);
    }
  }
  lines.push("", "sign-off backlog (pending human review):");
  lines.push("reviewId | requirement | decision | reviewer roles | gating contracts");
  for (const item of assessment.backlog) {
    lines.push(
      `${item.reviewId} | ${item.requirement ?? "-"} | ${item.decision ?? "-"} | ${item.reviewerRoles.join(", ") || "-"} | ${item.gatingContracts.join(", ") || "-"}`,
    );
  }
  return `${lines.join("\n")}\n`;
}

function isMainModule() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  try {
    const options = parseSignoffArgs(process.argv.slice(2));
    const assessment = assessHumanReviewSignoff({
      packets: readReviewPackets(),
      demands: readContractHumanReview(),
      indexDocument: existsSync(signoffIndexPath) ? readSignoffIndex() : null,
    });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatSignoffBacklog(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox human-review sign-off check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
