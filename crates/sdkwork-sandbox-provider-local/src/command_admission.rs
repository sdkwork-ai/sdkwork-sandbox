//! Command admission for the local Sandbox provider: the fail-closed sequence a request passes
//! before any execution slice may run. Pure data in, admission decision out - no host I/O and no
//! process creation, per `specs/sandbox-local-provider-host-boundary.contract.json`.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sdkwork_sandbox_provider_spi::{
    sandbox_verify_request_fingerprint, SandboxCommandExecutionRequest, SandboxCommandLimitsError,
};

use crate::host_boundary::{SandboxLocalHostBoundary, SandboxLocalHostBoundaryError};

/// Why the local provider refused to admit a command request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxLocalCommandAdmissionError {
    /// The request's declared limits exceed the contract maxima.
    LimitsOverBound(SandboxCommandLimitsError),
    /// The request failed the pure-data host boundary rules.
    BoundaryDenied(SandboxLocalHostBoundaryError),
    /// The caller-supplied fingerprint does not match the recomputed canonical fingerprint.
    FingerprintMismatch,
}

impl fmt::Display for SandboxLocalCommandAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitsOverBound(error) => write!(f, "sandbox command limits rejected: {error}"),
            Self::BoundaryDenied(error) => {
                write!(f, "sandbox command denied by host boundary: {error}")
            }
            Self::FingerprintMismatch => {
                f.write_str("sandbox command fingerprint does not match the recomputed value")
            }
        }
    }
}

impl Error for SandboxLocalCommandAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LimitsOverBound(error) => Some(error),
            Self::BoundaryDenied(error) => Some(error),
            Self::FingerprintMismatch => None,
        }
    }
}

/// The local admission sequence: contract limits, then the pure-data host boundary, then - when
/// the caller declared a fingerprint - the recomputed comparison. Admission authorizes nothing by
/// itself - it is the precondition every execution slice must pass first.
pub fn admit_sandbox_command(
    sandbox_boundary: &SandboxLocalHostBoundary,
    sandbox_request: &SandboxCommandExecutionRequest,
    sandbox_declared_fingerprint: Option<&str>,
) -> Result<(), SandboxLocalCommandAdmissionError> {
    if let Err(limits_error) = sandbox_request.sandbox_command_limits.validate() {
        return Err(SandboxLocalCommandAdmissionError::LimitsOverBound(
            limits_error,
        ));
    }
    let sandbox_environment: BTreeMap<&str, &str> = sandbox_request
        .sandbox_environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let sandbox_arguments: Vec<&str> = sandbox_request
        .sandbox_arguments
        .iter()
        .map(String::as_str)
        .collect();
    sandbox_boundary
        .validate_sandbox_command(
            &sandbox_request.sandbox_executable,
            &sandbox_arguments,
            &sandbox_request.sandbox_working_directory,
            &sandbox_environment,
        )
        .map_err(SandboxLocalCommandAdmissionError::BoundaryDenied)?;
    if let Some(declared) = sandbox_declared_fingerprint {
        if !sandbox_verify_request_fingerprint(sandbox_request, declared) {
            return Err(SandboxLocalCommandAdmissionError::FingerprintMismatch);
        }
    }
    Ok(())
}
