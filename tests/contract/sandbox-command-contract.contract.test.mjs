// WORKSPACE-PATH:allow-fixture: fixtures name a foreign checkout root, drive, or home directory to exercise path handling, so the literal is the value under assertion rather than a binding this build resolves
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const commandRoot = path.join(root, 'apis', 'commands');
const loadJson = (name) => JSON.parse(fs.readFileSync(path.join(commandRoot, name), 'utf8'));

const catalog = loadJson('sandbox-command-contract.json');
const request = loadJson('sandbox-command-execution-request.schema.json');
const cancellationRequest = loadJson('sandbox-command-cancellation-request.schema.json');
const result = loadJson('sandbox-command-execution-result.schema.json');
const error = loadJson('sandbox-command-execution-error.schema.json');

const forbiddenKeys = [
  'secret',
  'token',
  'password',
  'credential',
  'privatekey',
  'hostpath',
  'hostpid',
  'allocationreference',
  'signedurl',
  'sql',
  'rawoutput',
  'argv'
];

function walkKeys(value, keys = []) {
  if (Array.isArray(value)) {
    for (const item of value) walkKeys(item, keys);
    return keys;
  }
  if (!value || typeof value !== 'object') return keys;
  for (const [key, child] of Object.entries(value)) {
    keys.push(key.toLowerCase().replaceAll('_', ''));
    walkKeys(child, keys);
  }
  return keys;
}

function matchesStringSchema(schema, value) {
  if (schema.const !== undefined) return value === schema.const;
  if (schema.type !== 'string' || typeof value !== 'string') return false;
  if (schema.maxLength !== undefined && value.length > schema.maxLength) return false;
  if (schema.pattern !== undefined && !new RegExp(schema.pattern, 'u').test(value)) return false;
  for (const condition of schema.allOf ?? []) {
    if (condition.not?.pattern && new RegExp(condition.not.pattern, 'u').test(value)) return false;
  }
  return true;
}

function matchesWorkingDirectory(value) {
  return request.properties.sandboxWorkingDirectory.oneOf.some((schema) =>
    matchesStringSchema(schema, value)
  );
}

test('Sandbox command contract is a draft, provider-neutral machine authority', () => {
  assert.equal(catalog.kind, 'sdkwork.sandbox.command-contract');
  assert.equal(catalog.status, 'draft');
  assert.equal(catalog.implementationAuthorized, false);
  assert.equal(catalog.requirementId, 'REQ-2026-0007');
  assert.equal(request['x-sdkwork-requirement-id'], 'REQ-2026-0007');
  assert.equal(cancellationRequest['x-sdkwork-requirement-id'], 'REQ-2026-0007');
  assert.equal(result['x-sdkwork-requirement-id'], 'REQ-2026-0007');
  assert.equal(error['x-sdkwork-requirement-id'], 'REQ-2026-0007');
  assert.equal(catalog['x-sdkwork-no-host-io'], true);
  assert.equal(catalog['x-sdkwork-require-human-review'], true);
  assert.equal(catalog.cancellationRequestSchema, './sandbox-command-cancellation-request.schema.json');
});

test('Sandbox command request preserves identity, fencing, idempotency, and bounded Argv', () => {
  assert.equal(request.additionalProperties, false);
  assert.deepEqual(request.required, [
    'traceId',
    'tenantId',
    'sandboxProviderId',
    'sandboxWorkspaceId',
    'sandboxSessionId',
    'sandboxId',
    'sandboxRuntimeBindingId',
    'sandboxFencingToken',
    'sandboxCommandOperationId',
    'sandboxRequestFingerprint',
    'sandboxExecutable',
    'sandboxArguments',
    'sandboxWorkingDirectory',
    'sandboxEnvironment',
    'sandboxCommandLimits'
  ]);
  assert.equal(request.properties.sandboxExecutable['x-sdkwork-shell'], false);
  assert.equal(
    request.properties.sandboxExecutable['x-sdkwork-identifier-kind'],
    'provider-neutral-logical-executable'
  );
  assert.equal(
    request.properties.sandboxExecutable['x-sdkwork-path-resolution'],
    'provider-owned-binding-policy-only'
  );
  assert.equal(request.properties.sandboxArguments.maxItems, 128);
  assert.equal(request.properties.sandboxWorkingDirectory['x-sdkwork-path-kind'], 'workspace-logical-relative');
  assert.equal(request.properties.sandboxEnvironment['x-sdkwork-secret-policy'], 'deny-by-default');
  assert.equal(
    request.properties.sandboxEnvironment['x-sdkwork-policy-authority'],
    'provider-execution-policy-snapshot'
  );
  assert.equal(
    request.properties.sandboxEnvironment['x-sdkwork-protected-values'],
    'provider-fixed-after-request-validation'
  );
  assert.equal(request.properties.sandboxEnvironment.additionalProperties['x-sdkwork-max-utf8-bytes'], 1024);
  assert.equal(request.properties.sandboxCommandLimits.additionalProperties, false);
  assert.equal(request.properties.sandboxFencingToken.maximum, 9223372036854775807);
  assert.equal(request.properties.sandboxRequestFingerprint.pattern, '^sha256:[0-9a-f]{64}$');
  assert.equal(request.properties.sandboxRequestFingerprint['x-sdkwork-authority'], 'sandbox-service-derived');
  assert.equal(request.properties.sandboxRequestFingerprint['x-sdkwork-executor-verification'], 'required');
  assert.equal(request.properties.traceId['x-sdkwork-authority'], 'server-owned-command-trace');
  assert.equal(request['x-sdkwork-max-canonical-request-bytes'], 614400);
});

test('Sandbox command working directory has one cross-platform root and rejects path hazards', () => {
  assert.equal(request.properties.sandboxWorkingDirectory['x-sdkwork-workspace-root'], '.');
  assert.equal(request.properties.sandboxWorkingDirectory['x-sdkwork-path-separator'], '/');
  for (const sandboxWorkingDirectory of [
    '.',
    'workspace',
    'workspace/src',
    'workspace/src-code',
    'workspace/.cargo',
    'workspace/src.v2.generated',
    'workspace/source files'
  ]) {
    assert.equal(matchesWorkingDirectory(sandboxWorkingDirectory), true, sandboxWorkingDirectory);
  }
  for (const sandboxWorkingDirectory of [
    '',
    '..',
    '../outside',
    'workspace/../outside',
    '/host/root',
    'C:/host/root',
    'workspace\\src',
    'workspace/NUL.txt',
    'workspace/con.log',
    'workspace/COM1.cache',
    'workspace/lpt9.data',
    'workspace/CLOCK$.cache',
    'workspace/conin$.log',
    'workspace/CONOUT$.data',
    'workspace/invalid?name',
    'workspace/ leading-space',
    'workspace/trailing.'
  ]) {
    assert.equal(matchesWorkingDirectory(sandboxWorkingDirectory), false, sandboxWorkingDirectory);
  }
  assert.equal(request.properties.sandboxEnvironment.propertyNames.pattern, '^[A-Z_][A-Z0-9_]{0,63}$');
  assert.equal(new RegExp(request.properties.sandboxEnvironment.propertyNames.pattern, 'u').test('_PATH'), true);
});

test('Sandbox cancellation is independently idempotent, scoped, fenced, and reason-free', () => {
  assert.equal(cancellationRequest.additionalProperties, false);
  assert.deepEqual(cancellationRequest.required, [
    'traceId',
    'tenantId',
    'sandboxProviderId',
    'sandboxWorkspaceId',
    'sandboxSessionId',
    'sandboxId',
    'sandboxRuntimeBindingId',
    'sandboxFencingToken',
    'sandboxCommandOperationId',
    'sandboxCancellationOperationId',
    'sandboxCancellationRequestFingerprint'
  ]);
  assert.equal(cancellationRequest.properties.sandboxFencingToken.minimum, 1);
  assert.equal(cancellationRequest.properties.sandboxFencingToken.maximum, 9223372036854775807);
  assert.equal(
    cancellationRequest.properties.sandboxCancellationRequestFingerprint['x-sdkwork-executor-verification'],
    'required'
  );
  assert.equal(cancellationRequest['x-sdkwork-no-arbitrary-reason'], true);
  assert.equal('sandboxCancellationReason' in cancellationRequest.properties, false);
});

test('Sandbox command result is binary-safe, bounded, and does not expose host details', () => {
  assert.equal(result.additionalProperties, false);
  assert.deepEqual(result.properties.sandboxOutcome.enum, [
    'succeeded',
    'failed',
    'timed-out',
    'cancelled',
    'output-limit',
    'resource-exhausted',
    'fencing-lost'
  ]);
  assert.equal(result.properties.sandboxStdoutBase64.contentEncoding, 'base64');
  assert.equal(result.properties.sandboxStderrBase64.contentEncoding, 'base64');
  assert.equal(result.required.includes('sandboxRequestFingerprint'), true);
  assert.equal(result.required.includes('sandboxCommandResultReplayed'), true);
  assert.equal('sandboxReplayed' in result.properties, false);
  assert.equal(result.properties.sandboxExitStatus.oneOf.length, 3);
  assert.deepEqual(result.properties.sandboxExitStatus.oneOf[1].required, [
    'sandboxExitKind',
    'sandboxExitCode'
  ]);
  assert.deepEqual(result.properties.sandboxExitStatus.oneOf[2].required, [
    'sandboxExitKind',
    'sandboxSignalName'
  ]);
  assert.equal(result.properties.sandboxResourceUsage.additionalProperties, false);
  assert.equal(
    result.properties.sandboxResourceUsage.required.includes('sandboxStdoutCapturedBytes'),
    true
  );
  assert.equal(
    result.properties.sandboxResourceUsage.required.includes('sandboxStderrCapturedBytes'),
    true
  );
  assert.equal(result.properties.sandboxResourceUsage.properties.sandboxProcessCount.minimum, 1);
  assert.equal(result.required.includes('sandboxCleanupStatus'), true);
  assert.equal(result.required.includes('sandboxCleanupDurationMs'), true);
  assert.deepEqual(result.properties.sandboxCleanupStatus.enum, ['not-required', 'completed', 'failed']);
  assert.equal(result.properties.sandboxCleanupDurationMs.maximum, catalog.bounds.maxCleanupTimeoutMs);
  assert.equal(result.allOf.length, 5);
  assert.equal(result['x-sdkwork-output-encoding'], 'base64-binary');
  assert.equal(result['x-sdkwork-terminal-outcome-authority'], true);
  assert.equal(result['x-sdkwork-terminal-arbitration'], 'executor-durable-first-terminal-cas');
  assert.equal(result['x-sdkwork-cleanup-failure-policy'], 'explicit-result-and-binding-quarantine');
});

test('Sandbox command errors use a closed safe taxonomy with explicit retryability', () => {
  assert.equal(error.additionalProperties, false);
  assert.deepEqual(error.properties.sandboxErrorCode.enum, catalog.errorCodes.map((item) => item.code));
  assert.equal(error.properties.sandboxSafeMessage.maxLength, 256);
  assert.equal(error['x-sdkwork-detail-policy'], 'safe-message-only');
  assert.equal(error['x-sdkwork-pre-start-or-result-unavailable-only'], true);
  assert.equal(catalog.errorCodes.length, 10);
  assert.equal(new Set(catalog.errorCodes.map((item) => item.code)).size, 10);
  assert.ok(catalog.errorCodes.some((item) => item.code === 'idempotency-conflict' && !item.retryable));
  assert.ok(catalog.errorCodes.some((item) => item.code === 'operation-in-progress' && item.retryable));
  assert.ok(catalog.errorCodes.some((item) =>
    item.code === 'result-unavailable'
      && item.retryable
      && item.retryPrerequisite === 'same-operation-and-fingerprint'
  ));
  assert.ok(catalog.errorCodes.some((item) => item.code === 'command-not-found' && !item.retryable));
  assert.equal(error.allOf.length, 2);
  assert.deepEqual(
    error.allOf[0].if.properties.sandboxErrorCode.enum,
    catalog.errorCodes.filter((item) => item.retryable).map((item) => item.code)
  );
  assert.equal(error['x-sdkwork-retry-policy'], 'code-bound-prerequisites');
  assert.deepEqual(
    error['x-sdkwork-retry-prerequisites'],
    Object.fromEntries(
      catalog.errorCodes
        .filter((item) => item.retryable)
        .map((item) => [item.code, item.retryPrerequisite])
    )
  );
  for (const terminalOutcome of catalog.terminalOutcomes) {
    assert.equal(error.properties.sandboxErrorCode.enum.includes(terminalOutcome), false);
  }
});

test('Sandbox command fingerprint, idempotency, binding policy, and completion prevent unsafe retry', () => {
  assert.equal(catalog.fingerprint.algorithm, 'sha256');
  assert.equal(catalog.fingerprint.authority, 'sandbox-service-derived');
  assert.equal(catalog.fingerprint.executorRecomputationRequired, true);
  assert.equal(catalog.fingerprint.callerOverrideAllowed, false);
  assert.equal(catalog.fingerprint.traceIdIncluded, false);
  assert.ok(catalog.fingerprint.executionFields.includes('sandboxCommandLimits'));
  assert.ok(catalog.fingerprint.cancellationFields.includes('sandboxCancellationOperationId'));
  assert.deepEqual(catalog.idempotency.executionKey, [
    'tenantId',
    'sandboxProviderId',
    'sandboxCommandOperationId'
  ]);
  assert.equal(catalog.idempotency.differentFingerprint, 'idempotency-conflict');
  assert.equal(catalog.idempotency.automaticNewOperationRetryAllowed, false);
  assert.equal(catalog.completion.acceptedExecutionReturnsTerminalResult, true);
  assert.equal(catalog.completion.timeoutCancellationOutputAndResourceLimitsAreTerminalResults, true);
  assert.equal(catalog.completion.blindRetryAfterTerminalOutcomeAllowed, false);
  assert.equal(catalog.completion.cancellationReturnsTargetTerminalExecutionResult, true);
  assert.equal(catalog.completion.interruptedOutcomeRequiresCleanupAttempt, true);
  assert.equal(catalog.completion.cleanupFailureIsExplicitAndNotBlindlyRetryable, true);
  assert.equal(catalog.terminalArbitration.firstPersistedPrimaryTerminalFactWins, true);
  assert.equal(catalog.terminalArbitration.laterTerminalSignalsCannotRewritePrimaryOutcome, true);
  assert.equal(catalog.terminalArbitration.cleanupFailureDoesNotHideOrRewritePrimaryOutcome, true);
  assert.equal(catalog.terminalArbitration.cleanupFailureRequiresBindingQuarantineAndProviderUnavailability, true);
  assert.deepEqual(catalog.terminalOutcomes, result.properties.sandboxOutcome.enum);

  assert.equal(catalog.executableResolution.logicalIdentifierField, 'sandboxExecutable');
  assert.equal(catalog.executableResolution.callerSuppliedPathAllowed, false);
  assert.equal(catalog.executableResolution.operatingSystemPathSearchAllowed, false);
  assert.equal(catalog.executableResolution.workingDirectoryLookupAllowed, false);
  assert.equal(catalog.executableResolution.providerOwnedRegistryRequired, true);
  assert.equal(catalog.executableResolution.registrySnapshotBoundToRuntimeBinding, true);
  assert.equal(catalog.executableResolution.registrySnapshotImmutableForBindingLifetime, true);
  assert.equal(catalog.executableResolution.replayResolvedExecutableIdentityMustMatchOriginal, true);
  assert.equal(
    catalog.executableResolution.changedOrUnavailableReplayOutcome,
    'result-unavailable-no-reexecution'
  );

  assert.equal(catalog.environmentPolicy.finalEnvironmentStartsEmpty, true);
  assert.equal(catalog.environmentPolicy.requestMayExtendNameAllowlist, false);
  assert.equal(catalog.environmentPolicy.requestValuesRequirePerNameValidation, true);
  assert.deepEqual(catalog.environmentPolicy.protectedRequestNames, [
    'PATH',
    'PATHEXT',
    'COMSPEC',
    'SYSTEMROOT',
    'WINDIR',
    'HOME',
    'USERPROFILE',
    'TMP',
    'TEMP'
  ]);
  assert.deepEqual(
    request.properties.sandboxEnvironment.propertyNames.allOf[1].not.enum,
    catalog.environmentPolicy.protectedRequestNames
  );
  assert.deepEqual(catalog.environmentPolicy.sensitiveRequestNameSegments, [
    'TOKEN',
    'SECRET',
    'PASSWORD',
    'CREDENTIAL',
    'PRIVATE',
    'SSH',
    'DOCKER',
    'AWS',
    'AZURE',
    'GOOGLE',
    'PROXY'
  ]);
  assert.equal(
    request.properties.sandboxEnvironment.propertyNames.allOf[0].not.pattern,
    `(^|_)(${catalog.environmentPolicy.sensitiveRequestNameSegments.join('|')})(_|$)`
  );
  assert.equal(catalog.environmentPolicy.providerFixedValuesInjectedAfterRequestValidation, true);
  assert.equal(catalog.environmentPolicy.callerMayOverrideProviderFixedValues, false);
  assert.equal(catalog.environmentPolicy.policySnapshotBoundToRuntimeBinding, true);
  assert.equal(catalog.environmentPolicy.policySnapshotImmutableForBindingLifetime, true);
  assert.equal(catalog.environmentPolicy.ambientEnvironmentInheritanceAllowed, false);
});

test('Sandbox command contract forbids shell, private metadata, and unbounded execution', () => {
  assert.deepEqual(catalog.executionModes, ['executable-argv']);
  assert.ok(catalog.forbiddenExecutionModes.includes('shell-string'));
  assert.ok(catalog.forbiddenExecutionModes.includes('implicit-shell'));
  assert.ok(catalog.forbiddenExecutionModes.includes('secret-injection'));
  assert.equal(catalog.bounds.maxStdoutBytes, 67108864);
  assert.equal(catalog.bounds.maxStderrBytes, 67108864);
  assert.equal(catalog.bounds.maxTimeoutMs, 86400000);
  assert.equal(catalog.bounds.maxEnvironmentValueBytes, 1024);
  assert.equal(catalog.bounds.maxCanonicalRequestBytes, 614400);
  const keys = walkKeys({ catalog, request, cancellationRequest, result, error });
  for (const forbiddenKey of forbiddenKeys) {
    assert.ok(!keys.includes(forbiddenKey), `forbidden command contract key: ${forbiddenKey}`);
  }
});

test('Sandbox command catalog includes the required common conformance scenarios', () => {
  for (const scenario of [
    'typed-executable-and-argv-preservation',
    'shell-string-rejection',
    'logical-working-directory-escape-rejection',
    'deny-by-default-environment',
    'timeout-and-descendant-cleanup',
    'cancellation-and-descendant-cleanup',
    'stdout-and-stderr-hard-bounds',
    'stale-fencing-fail-closed',
    'derived-fingerprint-recomputation-and-mismatch-rejection',
    'same-operation-same-fingerprint-replay',
    'same-operation-different-fingerprint-conflict',
    'in-progress-operation-does-not-spawn-duplicate',
    'fenced-idempotent-cancellation-request',
    'accepted-execution-terminal-result-error-partition',
    'terminal-race-single-winner-and-replay',
    'cleanup-failure-visible-and-binding-quarantined',
    'safe-error-and-private-metadata-redaction',
    'provider-owned-executable-resolution-without-path-search',
    'protected-environment-override-rejection',
    'runtime-binding-policy-snapshot-immutability'
  ]) {
    assert.ok(catalog.conformanceScenarios.includes(scenario), `missing scenario: ${scenario}`);
  }
});
