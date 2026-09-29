//! Command admission for the Firecracker provider: the fail-closed sequence a
//! request passes before the guest channel may carry it. Pure data in,
//! admission decision out - no host I/O and no guest traffic, per the shared
//! command contract (`REQ-2026-0007`) and the delivery-gates Firecracker
//! capability policy.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sdkwork_sandbox_provider_spi::{
    sandbox_verify_request_fingerprint, SandboxCommandExecutionRequest, SandboxCommandLimitsError,
};

use crate::guest_boundary::{
    SandboxFirecrackerGuestBoundary, SandboxFirecrackerGuestBoundaryError,
};

/// Why the Firecracker provider refused to admit a command request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerCommandAdmissionError {
    /// The request's declared limits exceed the contract maxima.
    LimitsOverBound(SandboxCommandLimitsError),
    /// The request failed the pure-data guest boundary rules.
    BoundaryDenied(SandboxFirecrackerGuestBoundaryError),
    /// The caller-supplied fingerprint does not match the recomputed canonical
    /// fingerprint.
    FingerprintMismatch,
}

impl fmt::Display for SandboxFirecrackerCommandAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitsOverBound(sandbox_error) => {
                write!(f, "sandbox command limits rejected: {sandbox_error}")
            }
            Self::BoundaryDenied(sandbox_error) => {
                write!(
                    f,
                    "sandbox command denied by guest boundary: {sandbox_error}"
                )
            }
            Self::FingerprintMismatch => {
                f.write_str("sandbox command fingerprint does not match the recomputed value")
            }
        }
    }
}

impl Error for SandboxFirecrackerCommandAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LimitsOverBound(sandbox_error) => Some(sandbox_error),
            Self::BoundaryDenied(sandbox_error) => Some(sandbox_error),
            Self::FingerprintMismatch => None,
        }
    }
}

/// The Firecracker admission sequence: contract limits, then the pure-data
/// guest boundary, then - when the caller declared a fingerprint - the
/// recomputed comparison. Admission authorizes nothing by itself: it is the
/// precondition the executor must pass before registering a live execution.
///
/// # Errors
///
/// Returns the typed admission error for the first failed rule.
pub fn admit_sandbox_command(
    sandbox_boundary: &SandboxFirecrackerGuestBoundary,
    sandbox_request: &SandboxCommandExecutionRequest,
    sandbox_declared_fingerprint: Option<&str>,
) -> Result<(), SandboxFirecrackerCommandAdmissionError> {
    if let Err(sandbox_limits_error) = sandbox_request.sandbox_command_limits.validate() {
        return Err(SandboxFirecrackerCommandAdmissionError::LimitsOverBound(
            sandbox_limits_error,
        ));
    }
    let sandbox_environment: BTreeMap<&str, &str> = sandbox_request
        .sandbox_environment
        .iter()
        .map(|(sandbox_name, sandbox_value)| (sandbox_name.as_str(), sandbox_value.as_str()))
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
        .map_err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied)?;
    if let Some(sandbox_declared) = sandbox_declared_fingerprint {
        if !sandbox_verify_request_fingerprint(sandbox_request, sandbox_declared) {
            return Err(SandboxFirecrackerCommandAdmissionError::FingerprintMismatch);
        }
    }
    Ok(())
}
