use std::collections::{BTreeMap, BTreeSet};

use crate::command_admission::{admit_sandbox_command, SandboxLocalCommandAdmissionError};
use crate::host_boundary::SandboxLocalHostBoundary;
use sdkwork_sandbox_provider_spi::{
    sandbox_command_execution_fingerprint, SandboxCommandExecutionRequest, SandboxCommandLimits,
    SandboxCommandLimitsError,
};

fn sandbox_request() -> SandboxCommandExecutionRequest {
    SandboxCommandExecutionRequest {
        sandbox_tenant_id: "tenant-1".to_owned(),
        sandbox_provider_id: "provider-local".to_owned(),
        sandbox_workspace_id: "workspace-1".to_owned(),
        sandbox_session_id: "session-1".to_owned(),
        sandbox_id: "sandbox-1".to_owned(),
        sandbox_runtime_binding_id: "binding-1".to_owned(),
        sandbox_fencing_token: 7,
        sandbox_command_operation_id: "operation-1".to_owned(),
        sandbox_executable: "toybox".to_owned(),
        sandbox_arguments: vec!["echo".to_owned(), "hello".to_owned()],
        sandbox_working_directory: "workspace/out".to_owned(),
        sandbox_environment: BTreeMap::from([("SANDBOX_MODE".to_owned(), "strict".to_owned())]),
        sandbox_command_limits: SandboxCommandLimits {
            sandbox_timeout_ms: 5_000,
            sandbox_stdout_byte_limit: 1_024,
            sandbox_stderr_byte_limit: 1_024,
            sandbox_cleanup_timeout_ms: 1_000,
            sandbox_max_process_count: 1,
        },
    }
}

fn sandbox_boundary() -> SandboxLocalHostBoundary {
    SandboxLocalHostBoundary::new(
        BTreeSet::from(["toybox".to_owned()]),
        BTreeSet::from(["SANDBOX_MODE".to_owned()]),
    )
}

#[test]
fn admits_a_request_whose_declared_fingerprint_matches() {
    let request = sandbox_request();
    let fingerprint = sandbox_command_execution_fingerprint(&request);
    assert_eq!(
        admit_sandbox_command(&sandbox_boundary(), &request, &fingerprint),
        Ok(())
    );
}

#[test]
fn rejects_a_request_whose_declared_fingerprint_was_tampered_with() {
    let request = sandbox_request();
    assert_eq!(
        admit_sandbox_command(&sandbox_boundary(), &request, "deadbeef"),
        Err(SandboxLocalCommandAdmissionError::FingerprintMismatch)
    );
}

#[test]
fn rejects_a_request_over_the_contract_limits_before_the_boundary() {
    let mut request = sandbox_request();
    request.sandbox_command_limits.sandbox_timeout_ms = 86_400_001;
    let fingerprint = sandbox_command_execution_fingerprint(&request);
    assert_eq!(
        admit_sandbox_command(&sandbox_boundary(), &request, &fingerprint),
        Err(SandboxLocalCommandAdmissionError::LimitsOverBound(
            SandboxCommandLimitsError::FieldOverBound
        ))
    );
}

#[test]
fn rejects_a_request_the_host_boundary_denies() {
    let mut request = sandbox_request();
    request.sandbox_executable = "curl".to_owned();
    let fingerprint = sandbox_command_execution_fingerprint(&request);
    assert!(matches!(
        admit_sandbox_command(&sandbox_boundary(), &request, &fingerprint),
        Err(SandboxLocalCommandAdmissionError::BoundaryDenied(_))
    ));
}
