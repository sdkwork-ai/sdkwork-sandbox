import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function readStatus(relativePath) {
  const source = readFileSync(path.join(repoRoot, relativePath), "utf8");
  const match = source.match(/^(?:status|Status):\s*(\S+)\s*$/mu);
  assert.ok(match, `${relativePath} must declare status`);
  return match[1];
}

const contract = readJson("specs/sandbox-template-authority.contract.json");

test("Template authority requirement and contract stay draft and implementation-gated", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.template-authority-contract");
  // 2026-09-29: REQ-2026-0029 registered as the Template capability carrier; the
  // naming/review packet is still pending, so nothing may implement behind it.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0029-sandbox-template-authority.md"),
    "draft",
  );
});

test("Template definition fields are declared, sandbox-prefixed and immutable after publication", () => {
  const definition = contract.templateDefinition;
  assert.equal(definition.type, "SandboxTemplateDefinition");
  assert.equal(definition.unknownFieldsRejected, true);
  assert.equal(definition.immutableAfterPublication, true);
  assert.equal(definition.allSandboxOwnedFieldsRequirePrefix, "sandbox_");
  for (const field of definition.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  // The declarative model carries base environment, file layers, envs and the
  // start command; publication immutability is what makes a template a fixed
  // allocation input for Pool/PreparedSlot work rather than a mutable image.
  assert.deepEqual(definition.requiredFields, [
    "sandbox_template_definition_id",
    "sandbox_template_definition_version",
    "sandbox_base_environment_ref",
    "sandbox_file_layers",
    "sandbox_set_envs",
    "sandbox_set_start_cmd",
    "sandbox_published_at",
  ]);
});

test("Template versioning carries tags and versioned aliases bound to one artifact tuple ref", () => {
  const versioning = contract.versioning;
  assert.equal(versioning.type, "SandboxTemplateVersion");
  assert.equal(versioning.aliasesAreVersionedNames, true);
  assert.equal(versioning.tagsAreImmutableAfterPublication, true);
  for (const field of versioning.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  assert.ok(versioning.requiredFields.includes("sandbox_artifact_tuple_ref"));
});

test("Docker is a build input format only and never a runtime isolation boundary", () => {
  const buildInput = contract.buildInput;
  assert.equal(buildInput.type, "SandboxTemplateBuildInput");
  assert.deepEqual(buildInput.allowedFormats, [
    "sandbox_dockerfile_ref",
    "sandbox_build_script_ref",
  ]);
  assert.equal(buildInput.dockerfileIsBuildInputOnly, true);
  assert.equal(buildInput.dockerRuntimeBoundaryAllowed, false);
});

test("Build inputs stay opaque: no host path, download URL or embedded signature material", () => {
  const buildInput = contract.buildInput;
  assert.equal(buildInput.hostPathAllowed, false);
  assert.equal(buildInput.downloadUrlAllowed, false);
  assert.equal(buildInput.embeddedSignatureOrKeyMaterialAllowed, false);
});

test("Cache semantics stay a three-layer authority and authorize no implementation or backend", () => {
  const cachePolicy = contract.cachePolicy;
  assert.equal(cachePolicy.type, "SandboxTemplateCachePolicy");
  assert.deepEqual(cachePolicy.layers, ["sandbox_hot", "sandbox_warm", "sandbox_cold"]);
  assert.equal(cachePolicy.evictionPolicyExplicit, true);
  assert.equal(cachePolicy.crossTemplateLayerReuseRequiresExactDigestMatch, true);
  assert.equal(cachePolicy.cacheImplementationAuthorized, false);
  assert.equal(cachePolicy.cacheStorageBackendInScope, false);
});

test("The artifact boundary stays layered on REQ-2026-0012 with a single supply-chain authority", () => {
  const boundary = contract.artifactBoundary;
  assert.equal(boundary.artifactAuthority, "REQ-2026-0012");
  assert.equal(boundary.templateReferencesArtifactTuple, true);
  assert.equal(boundary.templateOwnsEvidenceOrSignature, false);
  assert.equal(boundary.secondSupplyChainAuthorityAllowed, false);
  const artifactContract = readJson(
    "specs/sandbox-firecracker-artifact-compatibility.contract.json",
  );
  assert.ok(artifactContract, "REQ-2026-0012 machine contract must exist");
});

test("Builder, registry, pipeline, CLI, public API/SDK and deployment stay out of scope", () => {
  assert.deepEqual(contract.forbidden, {
    templateBuilderRuntime: true,
    templateRegistryService: true,
    buildPipelineOrBuildArtifactStorage: true,
    cliSurface: true,
    publicApiOrSdkSurface: true,
    deploymentProfile: true,
  });
});
