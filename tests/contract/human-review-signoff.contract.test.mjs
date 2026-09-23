import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  assessHumanReviewSignoff,
  parseHeaderField,
  parseReviewPacket,
  readContractHumanReview,
  readExitPackage,
  readReviewPackets,
  readSignoffIndex,
  resolveHeaderField,
} from "../../tools/check-sandbox-human-review-signoff.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const reviewsDirectory = path.join(repoRoot, "docs/engineering/reviews");

const packet = (name) => readFileSync(path.join(reviewsDirectory, `${name}.md`), "utf8");

const live = () => ({
  packets: readReviewPackets(),
  demands: readContractHumanReview(),
  indexDocument: readSignoffIndex(),
  exitPackageDocument: readExitPackage(),
});

const assess = (overrides = {}) => assessHumanReviewSignoff({ ...live(), ...overrides });

const rowPattern = (reviewId) => new RegExp(`^\\|\\s*\\d+\\s*\\|\\s*${reviewId}\\s*\\|.*$`, "mu");

test("the live repository human-review sign-off state is coherent", () => {
  const assessment = assess();

  assert.equal(assessment.ok, true, assessment.failures.join("\n"));
  assert.equal(assessment.summary.sandbox_named_by_contracts, 14);
  // Since 2026-09-24 three of the fourteen contract-gated packets are signed off (local provider,
  // command execution, firecracker provider) and their contracts' shared delivery gate stays closed
  // on the remaining eleven; the API/SDK authority packet gates no contract.
  assert.equal(assessment.summary.sandbox_named_by_contracts_and_pending, 11);
  assert.ok(assessment.summary.sandbox_pending_human_review > 0);
  // The backlog only ever holds pending packets, so the count is the filter itself.
  assert.equal(
    assessment.backlog.filter((item) => item.gatingContracts.length > 0).length,
    11,
    "the eleven unapproved contract-gated packets must still be pending",
  );
});

test("the sign-off index lists exactly the contract-required packets and nothing else", () => {
  const { packets, demands, indexDocument } = live();
  const assessment = assessHumanReviewSignoff({ packets, demands, indexDocument });

  assert.equal(assessment.ok, true, assessment.failures.join("\n"));
  const mentioned = new Set(indexDocument.match(/REVIEW-[0-9]{8}-[a-z0-9-]+/gu));
  assert.deepEqual([...mentioned].sort(), assessment.declaredReviewIds);
});

test("the shared provider conformance packet is named by more than one gating contract", () => {
  const assessment = assess();
  // The local-provider packet signed off on 2026-09-24 and left the backlog; the network-isolation
  // one is the pending packet now named by two gating contracts.
  const shared = assessment.backlog.find(
    (item) => item.reviewId === "REVIEW-20260729-sandbox-firecracker-network-isolation",
  );

  assert.deepEqual(shared.gatingContracts, [
    "sandbox-firecracker-network-isolation.contract.json",
    "sandbox-provider-delivery-gates.contract.json",
  ]);
});

test("a contract naming a review packet that does not exist fails", () => {
  const assessment = assess({
    demands: [
      ...readContractHumanReview(),
      {
        contractFile: "sandbox-fabricated.contract.json",
        implementationAuthorized: false,
        approvedOutcomeRequired: true,
        packetIds: ["REVIEW-20260101-does-not-exist"],
        requiredRoles: [],
      },
    ],
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("REVIEW-20260101-does-not-exist")),
    assessment.failures.join("\n"),
  );
});

test("a required reviewer role the packet never asks to sign fails", () => {
  const demands = readContractHumanReview().map((demand) =>
    demand.contractFile === "sandbox-local-provider-host-boundary.contract.json"
      ? { ...demand, requiredRoles: [...demand.requiredRoles, "data-protection-owner"] }
      : demand,
  );

  const assessment = assess({ demands });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("data-protection-owner")),
    assessment.failures.join("\n"),
  );
});

test("a contract requiring roles from a packet with no sign-off table fails", () => {
  const packets = readReviewPackets().map((entry) =>
    entry.file === "REVIEW-20260729-local-provider-architecture-security.md"
      ? { ...entry, hasReviewerTable: false, reviewerRows: [] }
      : entry,
  );

  const assessment = assess({ packets });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("must carry a reviewer sign-off table")),
    assessment.failures.join("\n"),
  );
});

test("a pending packet cannot coexist with an Approved reviewer outcome", () => {
  // Still-pending packet after the 2026-09-24 transition: the firecracker network-isolation one.
  const packets = readReviewPackets().map((entry) =>
    entry.file === "REVIEW-20260729-sandbox-firecracker-network-isolation.md"
      ? {
          ...entry,
          reviewerRows: entry.reviewerRows.map((row, index) =>
            index === 0 ? { ...row, outcome: "Approved" } : row,
          ),
        }
      : entry,
  );

  const assessment = assess({ packets });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("already read Approved")),
    assessment.failures.join("\n"),
  );
});

test("a defect on a packet named by two contracts is reported once, not once per contract", () => {
  // The local-provider packet signed off on 2026-09-24, so the shared packet under test is the
  // firecracker network-isolation one: still pending, and named by its own contract plus the
  // shared provider delivery gate.
  const shared = "REVIEW-20260729-sandbox-firecracker-network-isolation";
  const packets = readReviewPackets().map((entry) =>
    entry.file === `${shared}.md`
      ? {
          ...entry,
          reviewerRows: entry.reviewerRows.map((row, index) =>
            index === 0 ? { ...row, outcome: "Approved" } : row,
          ),
        }
      : entry,
  );
  const demands = readContractHumanReview();
  const naming = demands.filter((demand) => demand.packetIds.includes(shared));

  assert.ok(naming.length > 1, "this packet must be named by more than one contract for the test to mean anything");

  const assessment = assess({ packets, demands });
  const matching = assessment.failures.filter((line) => line.includes("already read Approved"));

  assert.equal(matching.length, 1, assessment.failures.join("\n"));
});

test("a pending packet cannot coexist with a ready requirement or an accepted decision", () => {
  const pending = "REVIEW-20260729-sandbox-firecracker-network-isolation.md";
  const readyRequirement = readReviewPackets().map((entry) =>
    entry.file === pending
      ? { ...entry, resolvedRequirement: { ...entry.resolvedRequirement, status: "ready" } }
      : entry,
  );
  const acceptedDecision = readReviewPackets().map((entry) =>
    entry.file === pending
      ? { ...entry, resolvedDecision: { ...entry.resolvedDecision, status: "accepted" } }
      : entry,
  );

  for (const packets of [readyRequirement, acceptedDecision]) {
    const assessment = assess({ packets });
    assert.equal(assessment.ok, false);
  }
});

test("an authorized contract fails while any packet it names is still pending", () => {
  // The local-provider host-boundary contract flipped to authorized on 2026-09-24 with every
  // packet it names signed off; the shared delivery gate still names pending packets, so it is
  // the one whose authorization must keep failing.
  const demands = readContractHumanReview().map((demand) =>
    demand.contractFile === "sandbox-provider-delivery-gates.contract.json"
      ? { ...demand, implementationAuthorized: true }
      : demand,
  );

  const assessment = assess({ demands });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("implementationAuthorized")),
    assessment.failures.join("\n"),
  );
});

test("a review status outside the vocabulary fails", () => {
  const packets = readReviewPackets().map((entry) =>
    entry.file === "REVIEW-20260729-local-provider-architecture-security.md"
      ? { ...entry, status: "looks-fine-to-me" }
      : entry,
  );

  const assessment = assess({ packets });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("unsupported review status")),
    assessment.failures.join("\n"),
  );
});

test("the sign-off index must list every contract-required packet", () => {
  const assessment = assess({
    indexDocument: readSignoffIndex().replaceAll(
      "REVIEW-20260729-local-provider-architecture-security",
      "REVIEW-20260729-local-provider-architecture-securit",
    ),
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("sign-off index omits")),
    assessment.failures.join("\n"),
  );
});

test("the sign-off index must not list a packet no contract requires", () => {
  const assessment = assess({
    indexDocument: `${readSignoffIndex()}\n\nsee REVIEW-20260301-imaginary-packet\n`,
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("no contract requires")),
    assessment.failures.join("\n"),
  );
});

test("requirement and decision headers resolve in both real header shapes", () => {
  const plain = parseHeaderField("Decision: ADR-20260728-runtime-boundary-and-rust-workspace", /\bADR-[0-9]{8}(?:-[a-z0-9]+)*\b/u);
  const linked = parseHeaderField(
    "Decision: [ADR-20260729: Sandbox Observability, Event, Audit And Outbox Boundary](../../architecture/decisions/ADR-20260729-sandbox-observability-event-audit-outbox-boundary.md)",
    /\bADR-[0-9]{8}(?:-[a-z0-9]+)*\b/u,
  );
  const linkedShortRequirement = parseHeaderField(
    "Requirement: [REQ-2026-0005](../../product/requirements/REQ-2026-0005-durable-sandbox-session-repository-and-reconciliation.md)",
    /\bREQ-[0-9]{4}-[0-9]{4}\b/u,
  );

  assert.deepEqual(plain, {
    id: "ADR-20260728-runtime-boundary-and-rust-workspace",
    href: null,
    raw: "ADR-20260728-runtime-boundary-and-rust-workspace",
  });
  // The link text carries the date-only id and a human title; the href is what disambiguates the file.
  assert.equal(linked.id, "ADR-20260729");
  assert.match(linked.href, /ADR-20260729-sandbox-observability-event-audit-outbox-boundary\.md$/u);
  assert.equal(linkedShortRequirement.id, "REQ-2026-0005");
  assert.equal(parseHeaderField(null, /\bREQ-[0-9]{4}-[0-9]{4}\b/u), null);
});

test("a date-only decision id without an href is left unresolved rather than guessed", () => {
  const ambiguous = resolveHeaderField(
    parseHeaderField("Decision: ADR-20260729", /\bADR-[0-9]{8}(?:-[a-z0-9]+)*\b/u),
  );

  assert.equal(ambiguous.exists, false, "several ADRs share the ADR-20260729 prefix, so it must not resolve");
  assert.equal(ambiguous.ambiguousCandidates > 1, true);

  const unique = resolveHeaderField(
    parseHeaderField("Decision: ADR-20260728-sandbox-provider-allocation-key-rotation-and-reencryption", /\bADR-[0-9]{8}(?:-[a-z0-9]+)*\b/u),
  );
  assert.equal(unique.exists, true);
  assert.equal(unique.status, "proposed");
});

test("every review packet parses a status and the two accepted verification headers", () => {
  for (const entry of readReviewPackets()) {
    const name = entry.file.replace(/\.md$/u, "");
    assert.ok(entry.status.length > 0, `${name} must declare a status`);
    if (entry.status !== "active") {
      assert.ok(entry.requirement, `${name} must declare a Requirement header`);
      assert.ok(entry.decision, `${name} must declare a Decision header`);
    }
  }
});

test("a packet without a Status header is rejected", () => {
  assert.throws(() => parseReviewPacket("# REVIEW\n\nOwner: nobody\n"), /has no Status header/u);
});

test("the two verification reviews that previously dropped their Decision header stay linked", () => {
  for (const [name, decisionId] of [
    [
      "REVIEW-20260728-sandbox-postgresql-persistence-verification",
      "ADR-20260728-postgresql-sandbox-lifecycle-persistence-and-reconciliation",
    ],
    [
      "REVIEW-20260729-sandbox-provider-allocation-key-rotation-verification",
      "ADR-20260728-sandbox-provider-allocation-key-rotation-and-reencryption",
    ],
  ]) {
    const parsed = parseReviewPacket(packet(name), { file: `${name}.md` });
    assert.equal(parsed.decision.id, decisionId);
  }
});

test("the exit-readiness package lists every pending packet and projects each packet's own risk", () => {
  const { packets, exitPackageDocument } = live();
  const assessment = assess();
  assert.equal(assessment.ok, true, assessment.failures.join("\n"));

  const pending = packets.filter((entry) => entry.status === "pending-human-review");
  assert.ok(pending.length > 0, "the projection is vacuous when nothing is pending");
  for (const entry of pending) {
    const reviewId = entry.file.replace(/\.md$/u, "");
    assert.ok(
      rowPattern(reviewId).test(exitPackageDocument),
      `the exit-readiness package must carry a row for the pending packet ${reviewId}`,
    );
  }
});

test("the exit-readiness package omitting a pending packet fails", () => {
  const { packets, exitPackageDocument } = live();
  const victim = packets.find((entry) => entry.status === "pending-human-review").file.replace(
    /\.md$/u,
    "",
  );

  const assessment = assess({
    exitPackageDocument: exitPackageDocument.replace(rowPattern(victim), ""),
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("omits") && line.includes(victim)),
    assessment.failures.join("\n"),
  );
});

test("the exit-readiness package under-reporting a packet's own risk fails", () => {
  const { packets, exitPackageDocument } = live();
  const victim = packets.find(
    (entry) => entry.status === "pending-human-review" && entry.risk === "critical",
  );
  assert.ok(victim, "a packet declaring critical risk must exist for this test to mean anything");
  const reviewId = victim.file.replace(/\.md$/u, "");

  const assessment = assess({
    exitPackageDocument: exitPackageDocument.replace(
      rowPattern(reviewId),
      (line) => line.replace(/\|\s*critical\s*\|/u, "| high |"),
    ),
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes(reviewId) && line.includes("risk")),
    assessment.failures.join("\n"),
  );
});

test("the exit-readiness package declaring a stale pending count fails", () => {
  const { exitPackageDocument } = live();
  const stale = exitPackageDocument.replace(
    /当前全部\s*\*{0,2}\d+\*{0,2}\s*个相关\s*Review Packet/u,
    "当前全部 **17** 个相关 Review Packet",
  );
  assert.notEqual(stale, exitPackageDocument, "the count sentence must exist for this test to bite");

  const assessment = assess({ exitPackageDocument: stale });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("declares 17 pending review packet")),
    assessment.failures.join("\n"),
  );
});
