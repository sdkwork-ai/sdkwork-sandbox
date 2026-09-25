use std::collections::BTreeSet;

use sdkwork_intelligence_sandbox_service::{
    SandboxOperationOutcome, SandboxSessionFailure, SandboxSessionOperationKind,
    SandboxSessionRepositoryError, SandboxSessionRepositoryResult, SandboxSessionState,
};
use sdkwork_sandbox_provider_spi::{IsolationAssurance, RuntimeCapability};
use serde_json::Value;

pub(crate) fn sandbox_session_state_value(
    sandbox_session_state: SandboxSessionState,
) -> &'static str {
    match sandbox_session_state {
        SandboxSessionState::Created => "created",
        SandboxSessionState::Starting => "starting",
        SandboxSessionState::Running => "running",
        SandboxSessionState::Stopping => "stopping",
        SandboxSessionState::Stopped => "stopped",
        SandboxSessionState::Failed => "failed",
        SandboxSessionState::Destroying => "destroying",
        SandboxSessionState::Destroyed => "destroyed",
    }
}

pub(crate) fn parse_sandbox_session_state(
    value: &str,
) -> SandboxSessionRepositoryResult<SandboxSessionState> {
    match value {
        "created" => Ok(SandboxSessionState::Created),
        "starting" => Ok(SandboxSessionState::Starting),
        "running" => Ok(SandboxSessionState::Running),
        "stopping" => Ok(SandboxSessionState::Stopping),
        "stopped" => Ok(SandboxSessionState::Stopped),
        "failed" => Ok(SandboxSessionState::Failed),
        "destroying" => Ok(SandboxSessionState::Destroying),
        "destroyed" => Ok(SandboxSessionState::Destroyed),
        _ => Err(SandboxSessionRepositoryError::InvalidStoredData),
    }
}

fn sandbox_runtime_capability_value(sandbox_runtime_capability: RuntimeCapability) -> &'static str {
    // Delegates to the owning vocabulary so the wire spelling and the stored
    // spelling cannot diverge.
    sandbox_runtime_capability.as_str()
}

fn parse_sandbox_runtime_capability(
    value: &str,
) -> SandboxSessionRepositoryResult<RuntimeCapability> {
    RuntimeCapability::parse(value).ok_or(SandboxSessionRepositoryError::InvalidStoredData)
}

pub(crate) fn sandbox_runtime_capabilities_value(
    sandbox_runtime_capabilities: &BTreeSet<RuntimeCapability>,
) -> Value {
    Value::Array(
        sandbox_runtime_capabilities
            .iter()
            .map(|sandbox_runtime_capability| {
                Value::String(
                    sandbox_runtime_capability_value(*sandbox_runtime_capability).to_string(),
                )
            })
            .collect(),
    )
}

pub(crate) fn parse_sandbox_runtime_capabilities(
    value: Value,
) -> SandboxSessionRepositoryResult<BTreeSet<RuntimeCapability>> {
    let Value::Array(values) = value else {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    };
    let mut sandbox_runtime_capabilities = BTreeSet::new();
    for value in values {
        let Value::String(value) = value else {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        };
        if !sandbox_runtime_capabilities.insert(parse_sandbox_runtime_capability(&value)?) {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }
    }
    Ok(sandbox_runtime_capabilities)
}

pub(crate) fn sandbox_isolation_assurance_value(
    sandbox_isolation_assurance: IsolationAssurance,
) -> &'static str {
    // Delegates to the owning vocabulary; see `sandbox_runtime_capability_value`.
    sandbox_isolation_assurance.as_str()
}

pub(crate) fn parse_sandbox_isolation_assurance(
    value: &str,
) -> SandboxSessionRepositoryResult<IsolationAssurance> {
    IsolationAssurance::parse(value).ok_or(SandboxSessionRepositoryError::InvalidStoredData)
}

pub(crate) fn sandbox_session_failure_value(
    sandbox_session_failure: SandboxSessionFailure,
) -> &'static str {
    match sandbox_session_failure {
        SandboxSessionFailure::Provider => "provider",
        SandboxSessionFailure::Readiness => "readiness",
        SandboxSessionFailure::Cleanup => "cleanup",
    }
}

pub(crate) fn parse_sandbox_session_failure(
    value: &str,
) -> SandboxSessionRepositoryResult<SandboxSessionFailure> {
    match value {
        "provider" => Ok(SandboxSessionFailure::Provider),
        "readiness" => Ok(SandboxSessionFailure::Readiness),
        "cleanup" => Ok(SandboxSessionFailure::Cleanup),
        _ => Err(SandboxSessionRepositoryError::InvalidStoredData),
    }
}

pub(crate) fn sandbox_operation_kind_value(
    sandbox_operation_kind: SandboxSessionOperationKind,
) -> &'static str {
    match sandbox_operation_kind {
        SandboxSessionOperationKind::Create => "create",
        SandboxSessionOperationKind::Start => "start",
        SandboxSessionOperationKind::Stop => "stop",
        SandboxSessionOperationKind::Destroy => "destroy",
    }
}

pub(crate) fn parse_sandbox_operation_kind(
    value: &str,
) -> SandboxSessionRepositoryResult<SandboxSessionOperationKind> {
    match value {
        "create" => Ok(SandboxSessionOperationKind::Create),
        "start" => Ok(SandboxSessionOperationKind::Start),
        "stop" => Ok(SandboxSessionOperationKind::Stop),
        "destroy" => Ok(SandboxSessionOperationKind::Destroy),
        _ => Err(SandboxSessionRepositoryError::InvalidStoredData),
    }
}

pub(crate) fn sandbox_operation_outcome_values(
    sandbox_operation_outcome: SandboxOperationOutcome,
) -> (&'static str, Option<&'static str>) {
    match sandbox_operation_outcome {
        SandboxOperationOutcome::InProgress => ("in_progress", None),
        SandboxOperationOutcome::Succeeded => ("succeeded", None),
        SandboxOperationOutcome::Failed(sandbox_session_failure) => (
            "failed",
            Some(sandbox_session_failure_value(sandbox_session_failure)),
        ),
    }
}

pub(crate) fn parse_sandbox_operation_outcome(
    sandbox_operation_outcome: &str,
    sandbox_session_failure: Option<&str>,
) -> SandboxSessionRepositoryResult<SandboxOperationOutcome> {
    match (sandbox_operation_outcome, sandbox_session_failure) {
        ("in_progress", None) => Ok(SandboxOperationOutcome::InProgress),
        ("succeeded", None) => Ok(SandboxOperationOutcome::Succeeded),
        ("failed", Some(sandbox_session_failure)) => Ok(SandboxOperationOutcome::Failed(
            parse_sandbox_session_failure(sandbox_session_failure)?,
        )),
        _ => Err(SandboxSessionRepositoryError::InvalidStoredData),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sdkwork_intelligence_sandbox_service::{
        SandboxOperationOutcome, SandboxSessionFailure, SandboxSessionOperationKind,
        SandboxSessionRepositoryError, SandboxSessionState,
    };
    use sdkwork_sandbox_provider_spi::RuntimeCapability;
    use serde_json::json;

    use super::{
        parse_sandbox_isolation_assurance, parse_sandbox_operation_kind,
        parse_sandbox_operation_outcome, parse_sandbox_runtime_capabilities,
        parse_sandbox_session_failure, parse_sandbox_session_state,
        sandbox_isolation_assurance_value, sandbox_operation_kind_value,
        sandbox_operation_outcome_values, sandbox_runtime_capabilities_value,
        sandbox_session_failure_value, sandbox_session_state_value,
    };

    #[test]
    fn sandbox_capability_codec_is_canonical_and_rejects_duplicates() {
        let sandbox_capabilities =
            BTreeSet::from([RuntimeCapability::Filesystem, RuntimeCapability::Terminal]);
        let sandbox_value = sandbox_runtime_capabilities_value(&sandbox_capabilities);
        assert_eq!(
            parse_sandbox_runtime_capabilities(sandbox_value),
            Ok(sandbox_capabilities)
        );
        assert_eq!(
            parse_sandbox_runtime_capabilities(json!(["filesystem", "filesystem"])),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_state_codec_round_trips_every_state_and_rejects_unknown_values() {
        for sandbox_session_state in [
            SandboxSessionState::Created,
            SandboxSessionState::Starting,
            SandboxSessionState::Running,
            SandboxSessionState::Stopping,
            SandboxSessionState::Stopped,
            SandboxSessionState::Failed,
            SandboxSessionState::Destroying,
            SandboxSessionState::Destroyed,
        ] {
            assert_eq!(
                parse_sandbox_session_state(sandbox_session_state_value(sandbox_session_state)),
                Ok(sandbox_session_state)
            );
        }
        assert_eq!(
            parse_sandbox_session_state("unknown"),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_operation_kind_codec_round_trips_every_kind_and_rejects_unknown_values() {
        for sandbox_operation_kind in [
            SandboxSessionOperationKind::Create,
            SandboxSessionOperationKind::Start,
            SandboxSessionOperationKind::Stop,
            SandboxSessionOperationKind::Destroy,
        ] {
            assert_eq!(
                parse_sandbox_operation_kind(sandbox_operation_kind_value(sandbox_operation_kind)),
                Ok(sandbox_operation_kind)
            );
        }
        assert_eq!(
            parse_sandbox_operation_kind("unknown"),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_operation_outcome_codec_round_trips_every_outcome_and_rejects_mismatches() {
        for sandbox_operation_outcome in [
            SandboxOperationOutcome::InProgress,
            SandboxOperationOutcome::Succeeded,
            SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider),
            SandboxOperationOutcome::Failed(SandboxSessionFailure::Readiness),
            SandboxOperationOutcome::Failed(SandboxSessionFailure::Cleanup),
        ] {
            let (sandbox_operation_outcome_value, sandbox_session_failure_value) =
                sandbox_operation_outcome_values(sandbox_operation_outcome);
            assert_eq!(
                parse_sandbox_operation_outcome(
                    sandbox_operation_outcome_value,
                    sandbox_session_failure_value,
                ),
                Ok(sandbox_operation_outcome)
            );
        }
        // A failed outcome without a failure reason is rejected.
        assert_eq!(
            parse_sandbox_operation_outcome("failed", None),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        // A non-failed outcome carrying a failure reason is rejected.
        assert_eq!(
            parse_sandbox_operation_outcome(
                "succeeded",
                Some(sandbox_session_failure_value(
                    SandboxSessionFailure::Provider
                )),
            ),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_session_failure_and_isolation_assurance_codecs_reject_unknown_values() {
        for sandbox_session_failure in [
            SandboxSessionFailure::Provider,
            SandboxSessionFailure::Readiness,
            SandboxSessionFailure::Cleanup,
        ] {
            assert_eq!(
                parse_sandbox_session_failure(sandbox_session_failure_value(
                    sandbox_session_failure
                )),
                Ok(sandbox_session_failure)
            );
        }
        assert_eq!(
            parse_sandbox_session_failure("unknown"),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        for sandbox_isolation_assurance in [
            sdkwork_sandbox_provider_spi::IsolationAssurance::HostUser,
            sdkwork_sandbox_provider_spi::IsolationAssurance::Container,
            sdkwork_sandbox_provider_spi::IsolationAssurance::UserSpaceKernel,
            sdkwork_sandbox_provider_spi::IsolationAssurance::MicroVm,
            sdkwork_sandbox_provider_spi::IsolationAssurance::DedicatedVm,
        ] {
            assert_eq!(
                parse_sandbox_isolation_assurance(sandbox_isolation_assurance_value(
                    sandbox_isolation_assurance
                )),
                Ok(sandbox_isolation_assurance)
            );
        }
        assert_eq!(
            parse_sandbox_isolation_assurance("unknown"),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }
}
