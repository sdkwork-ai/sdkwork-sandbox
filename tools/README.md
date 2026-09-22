# Tools

Purpose: reusable repository validators, generators, migration utilities, and operator tooling.

Owner: SDKWork Sandbox engineering maintainers.

Allowed: reusable tool implementations with tests and documented inputs. Forbidden: application runtime code, thin shell wrappers, generated SDK output, and credentials.

Related specs: `../../sdkwork-specs/CODE_STYLE_SPEC.md`, `../../sdkwork-specs/TEST_SPEC.md`.

## Cargo Path Dependency Gate

`check-sandbox-cargo-path-dependencies.mjs` resolves every `path = "..."` dependency in every owned `Cargo.toml` and fails when the target does not exist, is not a Cargo package, or resolves outside the multi-repository workspace root. It is the only static gate that covers this invariant: a single surplus `..` retargets a dependency silently, and because `cargo metadata` then fails before any cargo command runs, the documentation, naming, layout, port, layering, path-portability, manifest-standard, dependency-completeness and backend-composition validators all stay green while `cargo fmt`, `cargo check`, `cargo clippy` and `cargo test` are unrunnable for the whole workspace.

```bash
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-cargo-path-dependencies.mjs --json
node --test tests/contract/cargo-path-dependency-tool.contract.test.mjs
```

Run it before any cargo command. It reports the offending manifest, line, declared path and resolved path instead of an opaque `cargo metadata` failure.

## Cargo Workspace Dependency Inheritance Gate

`check-sandbox-workspace-dependency-inheritance.mjs` enforces both directions of the dependency-declaration contract: `RUST_CODE_SPEC.md` section 14 (`Dependencies MUST be declared at the workspace root and inherited; member crates MUST NOT invent divergent third-party versions`) and `NAMING_SPEC.md` section 3.2 rule 6 (a `workspace = true` key MUST exist in the root `[workspace.dependencies]` table). It covers `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, their `[target.<triple>.*]` forms and the `[dependencies.<crate>]` section form, and deliberately excludes `[patch.*]` and `[package]` so that `edition.workspace` / `rust-version.workspace` are never mistaken for dependencies.

`check-rust-manifest-standard.mjs` does not cover this invariant: it inspects only `[package]` inheritance and `[lints]` wiring, so a member-local `axum = "0.8"` reports PASS there.

```bash
node tools/check-sandbox-workspace-dependency-inheritance.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs --json
node --test tests/contract/workspace-dependency-inheritance-tool.contract.test.mjs
```

## Documentation Integrity Gate

`check-sandbox-doc-integrity.mjs` enforces two invariants that `check-repository-docs-standard.mjs` does not cover: it validates document *structure* and path ownership, not link resolvability or command executability.

1. **Links.** Every relative markdown link must resolve from its document's own directory. On 2026-09-22 the repository carried fourteen dead links, including three `docs/guides/<role>/README.md` files reaching `../../../sdkwork-specs/...` one `..` short and `docs/releases/RELEASE-v0.1.0.md` using repository-root-relative paths.
2. **Commands**, inside the fenced code blocks of live documentation only: `cargo fmt --all` in any spelling is rejected (it formats local path dependencies, so it reports formatting diffs owned by sibling repositories this repository must not edit); a `node <path>.mjs` target must resolve from the document directory, the repository root or the multi-repository checkout root; a shell entry point with a directory component (`bin/doctor.sh`, `./bin/backup.sh`, `sdkwork-specs/tools/x.sh`) must exist, because the runbooks and `bin/` entry points go stale exactly like the `node` tools; and a `pnpm run <script>` target must exist in this repository's `package.json`. The developer guide previously prescribed four `scripts/*-checker.mjs` entrypoints that do not exist; `scripts/` holds only a README. A bare basename, an absolute path and a variable-prefixed path are not probed for any command family: they name no location inside this checkout, so treating them as repository-relative would produce false positives. The document's *parent* directory is deliberately not a candidate: a `../`-prefixed target that resolves only one level above the document was written for a directory the document does not occupy. `crates/sdkwork-intelligence-sandbox-repository-sqlx/README.md` prescribed `node ../../sdkwork-specs/tools/check-database-framework-standard.mjs --root ../..`, which resolved only from `crates/`; there `--root ../..` named the checkout root rather than this repository, so the tool printed `Database framework standard skipped (no database/ directory)` and exited 0. A verification command that silently checks nothing is worse than one that fails. Measured when the candidate was removed: of 203 `node` prescriptions in live documents exactly one resolved only from the document's parent, and it was this one.

Point-in-time evidence records under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and `docs/archive/` are exempt from the command rules, because a recorded command is a fact about the past and may legitimately name a command that has since been replaced. Their links are still checked, because navigation must work everywhere.

```bash
node tools/check-sandbox-doc-integrity.mjs
node tools/check-sandbox-doc-integrity.mjs --json
node tools/check-sandbox-doc-integrity.mjs --root <dir>
node --test tests/contract/doc-integrity-tool.contract.test.mjs
```

`--root` audits another tree, which is how the contract test proves the gate can go red.

## Component Contract Alignment Gate

`check-sandbox-component-contract-alignment.mjs` derives its rules from `COMPONENT_SPEC.md` prose instead of copying a spec list, and checks every `component.spec.json` in this repository against what the component actually is:

1. **`component.languages` must be backed by authored source.** A declared language with no source in that language and no matching language spec in `canonicalSpecs` is a phantom declaration.
2. **A language with authored source must reference its language spec.** For `rust` that is `RUST_CODE_SPEC.md`; `TYPESCRIPT_CODE_SPEC.md` and `SHELL_SCRIPT_SPEC.md` are covered on the same rule so a future TypeScript or shell surface cannot be added without its standard.
3. **Components with authored source must reference `CODE_STYLE_SPEC.md`.**
4. **A `rust-api-assembly` must reference every spec named by its MUST sentence in `COMPONENT_SPEC.md` section 4**, including `WEB_BACKEND_SPEC.md`, and must declare `component.surface: "api-assembly"`.

The gate deliberately omits the section 4 table's `APP_COMPOSITION_SPEC.md` entry for `rust-api-assembly`: that spec governs client application composition and is referenced by no component in the whole multi-repository workspace, so the table row is a spec-side artefact. `check-rust-manifest-standard.mjs`, `validate-api-assembly.mjs` and `verify-repo.mjs` do not cover any of these four invariants.

```bash
node tools/check-sandbox-component-contract-alignment.mjs
node tools/check-sandbox-component-contract-alignment.mjs --json
node --test tests/contract/component-contract-alignment-tool.contract.test.mjs
```

## Database Contract Reproducibility Gate

`check-sandbox-database-contract-reproducibility.mjs` re-runs the command this repository registers as `db:materialize:contract` against a throwaway copy of its inputs, then compares the three generated artifacts (`database/contract/schema.yaml`, `prefix-registry.json`, `table-registry.json`) with the committed ones. It reads the argument list from `package.json` and redirects only the `--root` value, so the gate cannot drift away from the command operators actually run.

`DATABASE_FRAMEWORK_SPEC.md` section 6.2 makes those three files generated artifacts, and a generated artifact its own generator cannot reproduce is unmaintainable: every regeneration produces a spurious diff, and because regeneration *deletes* unknown fields rather than adding them, an operator who runs the registered command silently loses authored data. On 2026-09-22 the committed `table-registry.json` carried `contractVersion` and `moduleId` that the generator never emits, and `prefix-registry.json` carried a hand-compacted `forbidden_aliases` array the generator reflows. Both were removed by regenerating; 68 of 70 workspace repositories already had the generator's shape, `schema.yaml` and `database.manifest.json` already carry both values, and no consumer reads either field from the registry.

`check-database-framework-standard.mjs` and `verify-database-initialization-state.mjs` validate the contract's shape and its relationship to the baseline, but neither re-runs the generator, so neither can detect a non-reproducible artifact.

```bash
node tools/check-sandbox-database-contract-reproducibility.mjs
node tools/check-sandbox-database-contract-reproducibility.mjs --json
node tools/check-sandbox-database-contract-reproducibility.mjs --root <dir>
node --test tests/contract/database-contract-reproducibility-tool.contract.test.mjs
```

The gate writes nothing outside the git-ignored `target/contract-reproducibility` tree, which it removes before returning, including when the artifacts turn out not to be reproducible. Without `--root` it inspects its own repository, resolved from its own location rather than the working directory.

## Requirement Traceability Gate

`check-sandbox-requirement-traceability.mjs` enforces the chain `PRD -> requirement -> architecture decision -> implementation` that `DOCUMENTATION_SPEC.md` section 28 requires lifecycle documentation to preserve, and the `REQUIREMENTS_SPEC.md` section 6 rule that a requirement id referenced by docs, code comments, ADRs or tests resolves to a real requirement record. No validator under `../../sdkwork-specs/tools/` and none of this repository's other gates covered either invariant.

1. **Id resolution.** Every `REQ-####-####` and `ADR-########-<slug>` token in a live document must resolve to a record of this repository, or be written inside a sibling-repository path (`sdkwork-agents/.../REQ-....md`). A `REQ-*` token that is immediately preceded by another repository's owner label (`BirdCoder`, `Agents`, `Kernel`, `Drive`, `IAM`, `Commerce`) must be written inside that path, because such a token otherwise resolves to *this* repository's identically numbered record. The delivery plan cited `sdkwork-kernel/docs/product/requirements/REQ-2026-0002-distributed-execution-placement-control-plane.md` and `sdkwork-birdcoder/docs/product/requirements/REQ-2026-0006-hybrid-local-cloud-agent-execution.md` as bare `REQ-2026-0002` and `REQ-2026-0006`, and both numbers also name this repository's own lifecycle-core and key-rotation records, so the references looked resolvable while pointing at the wrong records until 2026-09-22. It is the enclosing path that qualifies a reference, not a sibling path mentioned elsewhere on the line: an earlier build of this gate tested the line prefix and went blind on exactly the prose that explains the rule. An abbreviated decision id is accepted when the same line anchors it to an existing decision file, which is how the repository has always written `[ADR-20260729: ...](../../architecture/decisions/ADR-20260729-<full-slug>.md)`.
2. **No orphan authority.** Every requirement and decision record must be cited by at least one live document other than itself.
3. **Capability matrix classification.** Every row of `PRD-capabilities.md` section 11 cites at least one `REQ-*`, or carries the section's own `无` no-carrier marker, or inherits the row above with `同上`; row numbers must be contiguous from 1. The same run prints the capability census — how many rows cite a requirement, how many are marked `无`, how many inherit — so this repository's answer to `PRD.md` section 6's "能力对齐完整性" metric is visible without reading two documents and diffing two tables by hand. The census states only what the matrix literally says and never derives a "carried / not carried" total: that is a product judgement, and `PRD.md` section 6 reserves the completion claim for capabilities that have both a carrier and verification evidence.

A general "does nearby prose name another repository" heuristic was measured while building this gate and rejected: it flagged 93 lines and every one was a false positive, because text about BirdCoder routinely discusses this repository's requirements. Requiring the owner label to directly prefix the id flagged 5 lines, all genuine.

```bash
node tools/check-sandbox-requirement-traceability.mjs
node tools/check-sandbox-requirement-traceability.mjs --json
node tools/check-sandbox-requirement-traceability.mjs --root <dir>
node --test tests/contract/requirement-traceability-tool.contract.test.mjs
```

`--root` audits another tree, which is how the contract test proves every rule family can go red. Point-in-time evidence records under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and `docs/archive/` are exempt from the resolution rules: a review packet written in July may cite the id that was current in July.

## E2B Parity Matrix Gate

`check-sandbox-e2b-parity-matrix.mjs` keeps `docs/architecture/tech/TECH-e2b-capability-parity.md` — the repository's field-level answer to "is our capability set aligned with the mature microVM agent runtime baseline" — internally consistent and resolvable. No other gate reads it: `check-sandbox-doc-integrity.mjs` checks that links and prescribed commands resolve but never that a table foots, and `check-sandbox-requirement-traceability.mjs` reads only the 34-row `PRD-capabilities.md` census. A census that does not add up is quoted in review and nobody adds it by hand. Eight rule families:

1. **Vocabulary.** The document must declare exactly the four status markers (`✅ 完整对齐`, `🟡 部分 / 形态不同`, `❌ 未实现`, `⛔ 刻意不做`) in its status vocabulary section. A fifth marker, or a count other than four, is rejected.
2. **Numbering and shape.** Matrix rows are numbered `1..N`, ascending, each exactly once, and carry all five cells. Gaps break the cross-reference from a review comment to a row.
3. **Status cell.** Every row's status cell must start with one of the four declared markers. A synonym such as `部分`, or an empty cell, is rejected, because an unparseable status reads exactly like a deliberate one.
4. **Category alignment.** The census categories must correspond, in order, to the matrix subsection headings, so a new subsection cannot be added without a census row. Comparison ignores whitespace, because the two tables legitimately differ in spacing around `/` and `与`.
5. **Census arithmetic.** Per-category counts are recomputed from the matrix, the category rows must sum to the declared total row, and the total row must equal the recomputed whole-document totals and be a partition of `N`. Markdown emphasis is stripped before parsing: with `| **合计** | **78** | ... |` left unstripped the total row parsed as `null`, every total-row assertion was skipped, and the gate printed the *recomputed* sum while appearing to have verified the declared one. A census row with a numeric row count but a non-numeric state count is reported rather than dropped. A census with no total row is itself a failure.
6. **Citation resolution and registration.** Every `REQ-####-####` and `ADR-########-<slug>` token must resolve to a record of this repository or be written inside a sibling-repository path, restated here so the parity document cannot cite a record that does not exist; and the document must be linked from `TECH_ARCHITECTURE.md` and the tech README and registered in `docs/INDEX.yaml`.
7. **Residual gaps.** The section 3.2 coverage-gap table must be typed: each row declares `优先级`, `空档`, `性质`, `取证` and `说明`, the priorities run `1..N`, the kind is one of `治理阻塞` / `缺门禁` / `缺产物`, and the evidence cell names at least one artifact. The gate then checks the *direction* the kind asserts — a `缺产物` row's paths must all be absent, a `缺门禁` row's paths must all exist, and a `治理阻塞` row must name a `REQ-*` that both exists and is still short of `ready`. An empty gap table is a failure too, because it asserts the audit is complete. This rule exists because the repository carried the failure it catches: the table claimed no benchmark suite existed while `tools/bench-sandbox-lifecycle.mjs` and a published two-platform baseline were sitting in the tree.

8. **Zero-requirement claims.** The document asserts in a dozen places that some capability has no requirement behind it. Section 3.4 registers each such claim with the keywords that stand for it, and the gate re-derives ownership from every requirement record's id, slug and title: a keyword that now matches a record turns the row red, naming the record. The claim may only be asserted from the registry, so every such phrasing elsewhere must cite its row (`〔§3.4/N〕`) and every row must be cited at least once -- the same two-way accounting rule 7 of the field gate applies to OpenAPI operations. The reading is deliberately narrow and says so: id, slug and title, not the body, because the bodies mention these words in other senses (`suspended` spawn, a key rotation's `Pause/Resume`, a stream's `resume cursor`) and reading them would redden the whole table for the wrong reason. This is the rule that would have caught the false gap row: register "the performance baseline is unowned" with `allocation` and it fails against `REQ-2026-0006-sandbox-provider-allocation-key-rotation` and `REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation`.
```bash
node tools/check-sandbox-e2b-parity-matrix.mjs
node tools/check-sandbox-e2b-parity-matrix.mjs --json
node tools/check-sandbox-e2b-parity-matrix.mjs --root <dir>
node --test tests/contract/e2b-parity-matrix-tool.contract.test.mjs
```

Current reading: 17 categories, 78 rows, census `✅ 0 | 🟡 16 | ❌ 60 | ⛔ 2`, 6 residual gaps, 7 registered zero-requirement claims.

## E2B Field Parity Gate

`check-sandbox-e2b-field-parity.mjs` keeps the *baseline* the parity matrix rests on. The matrix gate proves `docs/architecture/tech/TECH-e2b-capability-parity.md` is internally consistent; it cannot prove the document was ever read, and for a long time it had not been: 23 of the 78 rows carried `基准仅索引`, judged from E2B documentation index titles with no field name verified against the page that defines it. A census that foots while resting on page titles is the failure mode a parity audit is least able to detect about itself. `specs/sandbox-e2b-capability-baseline.json` replaces the recollection with a captured inventory, and this gate keeps the two in step.

Ten rule families:

1. **Baseline shape.** The artifact declares kind `sdkwork.sandbox.e2b-capability-baseline`, a supported `schemaVersion`, and non-empty `sources`, `categories` and `rows`.
2. **Provenance.** Every source carries a unique id, a unique absolute https url, one of the declared kinds, a positive byte count and a 64-hex `sha256`. A source without provenance is an assertion, not evidence. The hashes are provenance statements recorded at capture time, not offline-verifiable digests; `recaptureCommand` records how to re-fetch and compare them.
3. **Row evidence.** Rows are numbered `1..N`, ascending, each exactly once; every row names a source that resolves and carries at least one extracted surface item — an API operationId or schema field, a CLI command form, or a page section heading. Documentary capabilities (region, compliance, BYOC) legitimately evidence themselves with headings; a row with no evidence of any kind is rejected.
4. **Category alignment.** The category index covers every row exactly once, each declared `rowCount` equals the rows it owns, and the categories sum to the row total.
5. **Document join.** The document's matrix rows, per-category subsection counts and declared census must equal the baseline's, so a row cannot be re-scoped in prose without the baseline moving with it.
6. **Ratchet.** Rows still marked baseline-incomplete must not exceed `ratchet.maxIndexOnlyRows`, and the ceiling must equal the recorded list, so an improvement is recorded rather than silently taken and a regression fails.
7. **Operation coverage.** Every operation in the captured OpenAPI document is accounted for exactly once: cited by a row as `[operationId]`, or recorded in `operationCoverage.unjudged` with the reason no row judges it. Row evidence may not cite an operation the document does not define. Two measurement mistakes sit behind the current reading, and both are the reason this rule exists. A first pass with a `[A-Za-z0-9_]+` scan reported 36 unjudged operations, because `filesystem.Filesystem.Stat` and `process.Process.Start` contain dots and were dropped silently. A second pass reported 19, of which 18 were accounting errors rather than gaps: those operations were the evidence for rows that already judge the capability (E2B's whole Templates REST lifecycle to row 26, alias to row 29, `GET /envs` to row 1, `GET /metrics` to row 65). Only `GET /health` genuinely has no home — a control-plane liveness probe is not a sandbox capability, so it stays recorded with that reason.
8. **Registration.** The baseline must be linked from the audit document and from `specs/README.md`, so a reader arrives at the evidence instead of a claim.
9. **Test inventory.** The counts the audit document states about the test suite must be true. The contract suite is recomputed from `tests/contract/*.test.mjs` itself; the Rust reading cannot be derived statically, so it is compared against the measurement `baseline.testInventory.rustWorkspace` records next to the command that produced it. The coverage section said `406 pass / 0 fail` and `63 passed / 1 ignored` long after the suite held 515 and `cargo test --workspace` reported 67 — a falsehood about the only part of this audit that executes.
10. **Self-description.** Every surface describing this gate — its own header, `tools/README.md`, root `README.md` and the Gate 0 view — must declare the same number of rule families the gate implements. The root README said “seven” for as long as operation coverage had existed, because the paragraph was never revisited when that family was added. The count is derived from the rule-family registry, not typed.

```bash
node tools/check-sandbox-e2b-field-parity.mjs
node tools/check-sandbox-e2b-field-parity.mjs --json
node tools/check-sandbox-e2b-field-parity.mjs --root <dir>
node --test tests/contract/sandbox-e2b-field-parity-tool.contract.test.mjs
```

Current reading: 78 rows fully evidenced across 17 categories from 101 captured sources, 0 rows index-only (ceiling 0); 71 E2B OpenAPI operations, 70 judged by a row, 1 recorded unjudged; the audit document's quoted suite readings (515 contract tests recomputed from `tests/contract/*.test.mjs`, 67 Rust tests measured by `cargo test --workspace`) are checked against the files and the recording, and ten rule families are declared consistently in all four surfaces that describe this gate.

## Platform Support Gate

`check-sandbox-platform-code.mjs` keeps `docs/architecture/tech/TECH-platform-support.md` — the repository's answer to "on which platforms can the control plane and the execution environment actually run, and on what evidence" — honest and machine-checked. It covers the invariant the two upstream portability gates leave open: `check-workspace-path-portability.mjs` checks machine-specific absolute paths and `check-shell-portability.mjs` checks shell syntax, but neither inspects `#[cfg(windows)]`, `std::process::Command`, `libc::` or `std::path::MAIN_SEPARATOR`, so a runtime backend could become Windows-only with every existing gate still green. It also closed a wiring gap: before 2026-09-22 both upstream gates passed against this repository but were not in `package.json` at all, so `DEPENDENCY_MANAGEMENT_SPEC.md` section 1's cross-platform requirement had no executor.

1. **Document shape and vocabulary.** The platform vocabulary and the support matrix must both be present, the vocabulary must be exactly the tool's fixed `PLATFORM_IDS`, and every status cell must come from `verified | partial | unsupported | unmeasured`. A platform outside the fixed list is rejected rather than silently accepted as a new row.
2. **Citation anchoring.** Every matrix row must cite at least one repository-relative path or host-capability id. Paths must resolve from the repository root and capability ids must exist in the probe vocabulary of `tools/testing/sandbox-host-capability-evidence.mjs`, which is imported rather than restated so a renamed capability cannot keep passing. Absolute paths such as `/etc/os-release` are host observations and stay free-form, and a `verified` row must additionally cite a requirement, a decision or a document path.
3. **Platform-conditional code.** Every marker in `crates/*/src` must be declared in the document with its platform and a reason, every declaration must still match source, and the declared census count must equal the number of markers found. Test code counts: a platform-conditional test is still a platform claim, and excluding it would let the runtime become single-platform behind a green suite.
4. **Gate wiring.** Every gate the document names must exist and be declared in `package.json` invoking that same script. A portability gate that never runs is exactly the failure this gate exists to prevent.
5. **Registration.** The document must be linked from `TECH_ARCHITECTURE.md` and the tech README and registered in `docs/INDEX.yaml`.

Two parser defects were found by running the gate against the real document rather than a fixture, and both are pinned by contract tests. A `|` inside an inline-code span (`namespace.mount|pid|uts`) split the row, so the evidence cell ended at the first such span and every citation after it — including a path that did not exist — went unchecked. And the `| （无） | | | |` placeholder row of an empty declaration table was read as a declaration, reporting three failures for a census of zero.

```bash
node tools/check-sandbox-platform-code.mjs
node tools/check-sandbox-platform-code.mjs --json
node tools/check-sandbox-platform-code.mjs --root <dir>
node --test tests/contract/sandbox-platform-code-tool.contract.test.mjs
```

Current reading: 5 platforms (`windows-x64` unsupported, `linux-x64-wsl2` partial, `linux-x64-native` and `linux-aarch64` unmeasured, `macos-arm64` unsupported), 0 platform-conditional markers under `crates/*/src`.

## Commercial Readiness Gate

`check-sandbox-commercial-readiness.mjs` evaluates the repository-level commercial readiness contract and verifies that all local evidence references resolve. The default audit command reports the current decision without converting the expected Gate 0 `NO-GO` into a test failure. Release preflight must use `--require-go`, which exits unsuccessfully until every required slice, cross-repository authority, and missing contract is closed.

```bash
node tools/check-sandbox-commercial-readiness.mjs
node tools/check-sandbox-commercial-readiness.mjs --json
node tools/check-sandbox-commercial-readiness.mjs --require-go
node --test tests/contract/sandbox-commercial-readiness.contract.test.mjs
```

## Human Review Sign-Off Gate

`check-sandbox-human-review-signoff.mjs` parses every `docs/engineering/reviews/REVIEW-*.md` packet and every `specs/*.contract.json` review demand, then fails when a contract names a packet that does not exist, when a required reviewer role is not asked to sign its packet, when a contract requires roles from a packet with no sign-off table, when a packet still marked `pending-human-review` coexists with an `Approved` reviewer outcome or a `ready`/`accepted` requirement or an `accepted` decision, when any contract sets `implementationAuthorized` before every packet it names is signed off, when a review status leaves the vocabulary, or when the index at `docs/engineering/human-review-signoff-backlog.md` stops listing exactly the contract-required packets. It also prints the live pending backlog with each packet's reviewer roles and gating contracts. A consistent result means the review record agrees with itself; it never means a review was approved.

```bash
node tools/check-sandbox-human-review-signoff.mjs
node tools/check-sandbox-human-review-signoff.mjs --json
node --test tests/contract/human-review-signoff.contract.test.mjs
```

## Real-Evidence Traceability Gate

`check-sandbox-evidence-traceability.mjs` recomputes the evidence ids the `specs/*.contract.json` contracts require and compares them with `specs/sandbox-real-evidence-registry.json`. It fails when a contract requires an id the registry does not snapshot, when the registry snapshots an id no contract requires any more, when a contract starts or stops declaring requirements without a registry update, when an acknowledged count drifts, when the registry claims host-precondition coverage for an id its contract does not require, or when a claimed capability witness is not declared by `HOST_CAPABILITY_IDS` or not actually emitted by the probe. Passing it is a traceability statement, never evidence.

```bash
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-evidence-traceability.mjs --json
node --test tests/contract/real-evidence-traceability.contract.test.mjs
```

## Host Capability Evidence Runner

`testing/sandbox-host-capability-evidence.mjs` probes a target host read-only and classifies every capability as `verified` / `unsupported` / `denied` / `unverifiable`, so Gate 0 review packets can cite observed host facts instead of assumptions. It runs on the local shell or through a named WSL distribution, never escalates privilege, and its opt-in `--probe-write` probes are self-cleaning inside a user namespace and a dedicated cgroup. It is a diagnostic: it creates no Provider, isolation policy, API route, or deployment profile.

```bash
node tools/testing/sandbox-host-capability-evidence.mjs
node tools/testing/sandbox-host-capability-evidence.mjs --target wsl:Ubuntu-22.04 --probe-write
node tools/testing/sandbox-host-capability-evidence.mjs --json --out target/host-capability-evidence.json
node tools/testing/sandbox-host-capability-evidence.mjs --require namespace.mount-userns,cgroup.v2-mounted
node --test tests/contract/host-capability-evidence-tool.contract.test.mjs
```

`--require <ids>` exits unsuccessfully when any listed capability is not `verified`, so a host can be gated in automation without reading the report. A `denied` capability is a fact about the probing identity on that host, not a Provider limitation decision, and a `verified` capability is not conformance evidence.

## PostgreSQL Evidence Runner

`testing/sandbox-postgres-evidence.mjs` runs the accepted lifecycle persistence evidence against an internally named disposable PostgreSQL 16 or 17 container. It binds PostgreSQL only to a Docker-selected loopback port, provisions a canonical `sdkwork_ai_test_<run_id>` database/schema, uses `sdkwork-database-cli`, runs the ignored Repository test, verifies custom-format backup/restore and plaintext absence, and removes only the container it successfully created.

```bash
node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 16
node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 17
node --test tests/contract/postgres-evidence-tool.contract.test.mjs
```

The runner requires Docker Engine, Cargo, the sibling `../sdkwork-database` checkout, and the cached or pullable official PostgreSQL image. It creates no deployment profile, application manifest, credential file, host dump, or persistent volume.
