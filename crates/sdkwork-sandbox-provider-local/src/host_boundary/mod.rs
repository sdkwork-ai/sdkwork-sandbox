//! Pure-data host boundary rules for the local Sandbox provider.
//!
//! This module is the production form of the Executable/Path/Argv/Environment rules that the
//! Phase 0 fake host boundary validated from tests. It performs no host I/O and spawns no
//! process: every function is a pure check over the request data, so the fail-closed boundary
//! decisions are unit-testable on every platform the crate compiles for. Process creation,
//! supervision, and cleanup stay behind the provider execution slice and its real-platform
//! evidence gates (`specs/sandbox-local-provider-host-boundary.contract.json`,
//! `requiredRealEvidence`).

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// Upper bound for the executable's byte length.
pub const SANDBOX_MAX_EXECUTABLE_BYTES: usize = 128;
/// Upper bound for the argument vector's length.
pub const SANDBOX_MAX_ARGUMENTS: usize = 128;
/// Upper bound for a single argument's byte length.
pub const SANDBOX_MAX_ARGUMENT_BYTES: usize = 4_096;
/// Upper bound for the working directory's byte length.
pub const SANDBOX_MAX_WORKING_DIRECTORY_BYTES: usize = 512;
/// Upper bound for the environment map's entry count.
pub const SANDBOX_MAX_ENVIRONMENT_ENTRIES: usize = 64;
/// Upper bound for an environment name's byte length.
pub const SANDBOX_MAX_ENVIRONMENT_NAME_BYTES: usize = 64;
/// Upper bound for an environment value's byte length.
pub const SANDBOX_MAX_ENVIRONMENT_VALUE_BYTES: usize = 1_024;

/// Why a sandbox command request failed the local host boundary.
///
/// The variants are the contract's fail-closed decision points; a caller may only retry with new
/// request data, never relax the boundary that produced one of these values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxLocalHostBoundaryError {
    /// The executable is empty, over-long, or not a bare alphanumeric name with `.`, `_`, `+`, `-`.
    ExecutableInvalid,
    /// The executable is well-formed but not in the boundary's allowlist.
    ExecutableDenied,
    /// The argument vector exceeds its count bound.
    ArgumentCountExceeded,
    /// An argument is over-long or contains a forbidden byte (`NUL`, `CR`, `LF`).
    ArgumentInvalid,
    /// The working directory is not a valid logical relative path.
    WorkingDirectoryInvalid,
    /// The environment exceeds its entry-count bound.
    EnvironmentCountExceeded,
    /// An environment name is empty, over-long, or outside `[A-Z_][A-Z0-9_]*`.
    EnvironmentNameInvalid,
    /// An environment name is protected: the host owns it, the request may not set it.
    EnvironmentProtected,
    /// An environment name is sensitive by segment (`TOKEN`, `SECRET`, `PASSWORD`, ...).
    EnvironmentSensitive,
    /// An environment name is well-formed but not in the boundary's allowlist.
    EnvironmentDenied,
    /// An environment value is over-long or contains a forbidden byte.
    EnvironmentValueInvalid,
}

impl fmt::Display for SandboxLocalHostBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ExecutableInvalid => "sandbox executable is empty, over-long, or not a bare name",
            Self::ExecutableDenied => "sandbox executable is not allowlisted",
            Self::ArgumentCountExceeded => "sandbox argument count exceeds the boundary bound",
            Self::ArgumentInvalid => "sandbox argument is over-long or contains a forbidden byte",
            Self::WorkingDirectoryInvalid => {
                "sandbox working directory is not a logical relative path"
            }
            Self::EnvironmentCountExceeded => {
                "sandbox environment entry count exceeds the boundary bound"
            }
            Self::EnvironmentNameInvalid => {
                "sandbox environment name is empty, over-long, or not `[_A-Z][_A-Z0-9]*`"
            }
            Self::EnvironmentProtected => {
                "sandbox environment name is host-protected and may not be set"
            }
            Self::EnvironmentSensitive => {
                "sandbox environment name carries a sensitive segment and is denied"
            }
            Self::EnvironmentDenied => "sandbox environment name is not allowlisted",
            Self::EnvironmentValueInvalid => {
                "sandbox environment value is over-long or contains a forbidden byte"
            }
        };
        f.write_str(message)
    }
}

impl Error for SandboxLocalHostBoundaryError {}

/// The hard bounds a local host boundary enforces on one command request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxLocalHostBoundaryLimits {
    /// Maximum executable byte length.
    pub sandbox_max_executable_bytes: usize,
    /// Maximum argument count.
    pub sandbox_max_arguments: usize,
    /// Maximum per-argument byte length.
    pub sandbox_max_argument_bytes: usize,
    /// Maximum working-directory byte length.
    pub sandbox_max_working_directory_bytes: usize,
    /// Maximum environment entry count.
    pub sandbox_max_environment_entries: usize,
    /// Maximum environment-name byte length.
    pub sandbox_max_environment_name_bytes: usize,
    /// Maximum environment-value byte length.
    pub sandbox_max_environment_value_bytes: usize,
}

impl Default for SandboxLocalHostBoundaryLimits {
    fn default() -> Self {
        Self {
            sandbox_max_executable_bytes: SANDBOX_MAX_EXECUTABLE_BYTES,
            sandbox_max_arguments: SANDBOX_MAX_ARGUMENTS,
            sandbox_max_argument_bytes: SANDBOX_MAX_ARGUMENT_BYTES,
            sandbox_max_working_directory_bytes: SANDBOX_MAX_WORKING_DIRECTORY_BYTES,
            sandbox_max_environment_entries: SANDBOX_MAX_ENVIRONMENT_ENTRIES,
            sandbox_max_environment_name_bytes: SANDBOX_MAX_ENVIRONMENT_NAME_BYTES,
            sandbox_max_environment_value_bytes: SANDBOX_MAX_ENVIRONMENT_VALUE_BYTES,
        }
    }
}

/// The fail-closed local host boundary: allowlisted executables and environment names plus hard
/// request bounds, checked entirely on the request data before any provider execution slice may
/// run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxLocalHostBoundary {
    sandbox_allowed_executables: BTreeSet<String>,
    sandbox_allowed_environment: BTreeSet<String>,
    sandbox_limits: SandboxLocalHostBoundaryLimits,
}

impl SandboxLocalHostBoundary {
    /// Builds a boundary from the allowlisted executable names and environment names. The default
    /// limits apply; use [`with_limits`](Self::with_limits) to override them.
    #[must_use]
    pub fn new(
        sandbox_allowed_executables: BTreeSet<String>,
        sandbox_allowed_environment: BTreeSet<String>,
    ) -> Self {
        Self {
            sandbox_allowed_executables,
            sandbox_allowed_environment,
            sandbox_limits: SandboxLocalHostBoundaryLimits::default(),
        }
    }

    /// Replaces the hard bounds on this boundary.
    #[must_use]
    pub fn with_limits(mut self, sandbox_limits: SandboxLocalHostBoundaryLimits) -> Self {
        self.sandbox_limits = sandbox_limits;
        self
    }

    /// Validates one command request against the allowlists and the hard bounds. Every rule is
    /// checked on the request data alone; a successful return authorizes nothing by itself and
    /// performs no host action.
    pub fn validate_sandbox_command(
        &self,
        sandbox_executable: &str,
        sandbox_arguments: &[&str],
        sandbox_working_directory: &str,
        sandbox_environment: &BTreeMap<&str, &str>,
    ) -> Result<(), SandboxLocalHostBoundaryError> {
        if !is_valid_sandbox_executable(
            sandbox_executable,
            self.sandbox_limits.sandbox_max_executable_bytes,
        ) {
            return Err(SandboxLocalHostBoundaryError::ExecutableInvalid);
        }
        if !self.sandbox_allowed_executables.contains(sandbox_executable) {
            return Err(SandboxLocalHostBoundaryError::ExecutableDenied);
        }

        if sandbox_arguments.len() > self.sandbox_limits.sandbox_max_arguments {
            return Err(SandboxLocalHostBoundaryError::ArgumentCountExceeded);
        }
        if sandbox_arguments.iter().any(|sandbox_argument| {
            contains_forbidden_sandbox_string_byte(sandbox_argument)
                || sandbox_argument.len() > self.sandbox_limits.sandbox_max_argument_bytes
        }) {
            return Err(SandboxLocalHostBoundaryError::ArgumentInvalid);
        }

        if !is_valid_sandbox_logical_relative_path(
            sandbox_working_directory,
            self.sandbox_limits.sandbox_max_working_directory_bytes,
        ) {
            return Err(SandboxLocalHostBoundaryError::WorkingDirectoryInvalid);
        }

        if sandbox_environment.len() > self.sandbox_limits.sandbox_max_environment_entries {
            return Err(SandboxLocalHostBoundaryError::EnvironmentCountExceeded);
        }
        for (sandbox_environment_name, sandbox_environment_value) in sandbox_environment {
            if !is_valid_sandbox_environment_name(
                sandbox_environment_name,
                self.sandbox_limits.sandbox_max_environment_name_bytes,
            ) {
                return Err(SandboxLocalHostBoundaryError::EnvironmentNameInvalid);
            }
            if is_protected_sandbox_environment_name(sandbox_environment_name) {
                return Err(SandboxLocalHostBoundaryError::EnvironmentProtected);
            }
            if is_sensitive_sandbox_environment_name(sandbox_environment_name) {
                return Err(SandboxLocalHostBoundaryError::EnvironmentSensitive);
            }
            if !self.sandbox_allowed_environment.contains(*sandbox_environment_name) {
                return Err(SandboxLocalHostBoundaryError::EnvironmentDenied);
            }
            if contains_forbidden_sandbox_string_byte(sandbox_environment_value)
                || sandbox_environment_value.len()
                    > self.sandbox_limits.sandbox_max_environment_value_bytes
            {
                return Err(SandboxLocalHostBoundaryError::EnvironmentValueInvalid);
            }
        }

        Ok(())
    }
}

fn contains_forbidden_sandbox_string_byte(sandbox_value: &str) -> bool {
    sandbox_value
        .bytes()
        .any(|sandbox_byte| matches!(sandbox_byte, 0 | b'\r' | b'\n'))
}

fn is_valid_sandbox_executable(
    sandbox_executable: &str,
    sandbox_max_executable_bytes: usize,
) -> bool {
    let mut sandbox_executable_bytes = sandbox_executable.bytes();
    let Some(sandbox_first_byte) = sandbox_executable_bytes.next() else {
        return false;
    };

    sandbox_executable.len() <= sandbox_max_executable_bytes
        && sandbox_first_byte.is_ascii_alphanumeric()
        && sandbox_executable_bytes.all(|sandbox_byte| {
            sandbox_byte.is_ascii_alphanumeric()
                || matches!(sandbox_byte, b'.' | b'_' | b'+' | b'-')
        })
}

fn is_valid_sandbox_logical_relative_path(sandbox_path: &str, sandbox_max_path_bytes: usize) -> bool {
    if sandbox_path.is_empty()
        || sandbox_path.len() > sandbox_max_path_bytes
        || sandbox_path.as_bytes().contains(&0)
        || sandbox_path.starts_with('/')
        || sandbox_path.starts_with('\\')
    {
        return false;
    }

    let sandbox_path_bytes = sandbox_path.as_bytes();
    if sandbox_path_bytes.len() >= 2
        && sandbox_path_bytes[0].is_ascii_alphabetic()
        && sandbox_path_bytes[1] == b':'
    {
        return false;
    }

    sandbox_path
        .split(['/', '\\'])
        .all(is_valid_sandbox_logical_path_segment)
}

fn is_valid_sandbox_logical_path_segment(sandbox_path_segment: &str) -> bool {
    if sandbox_path_segment.is_empty()
        || matches!(sandbox_path_segment, "." | "..")
        || sandbox_path_segment.ends_with(['.', ' '])
        || sandbox_path_segment.contains(':')
        || sandbox_path_segment.chars().any(char::is_control)
    {
        return false;
    }

    let sandbox_base_name = sandbox_path_segment
        .split_once('.')
        .map_or(sandbox_path_segment, |(sandbox_base_name, _)| {
            sandbox_base_name
        });
    !is_reserved_sandbox_windows_device_name(sandbox_base_name)
}

fn is_reserved_sandbox_windows_device_name(sandbox_base_name: &str) -> bool {
    let sandbox_uppercase_name = sandbox_base_name.to_ascii_uppercase();
    matches!(
        sandbox_uppercase_name.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || (sandbox_uppercase_name.len() == 4
        && (sandbox_uppercase_name.starts_with("COM") || sandbox_uppercase_name.starts_with("LPT"))
        && matches!(sandbox_uppercase_name.as_bytes()[3], b'1'..=b'9'))
}

fn is_valid_sandbox_environment_name(
    sandbox_environment_name: &str,
    sandbox_max_environment_name_bytes: usize,
) -> bool {
    let mut sandbox_environment_name_bytes = sandbox_environment_name.bytes();
    let Some(sandbox_first_byte) = sandbox_environment_name_bytes.next() else {
        return false;
    };

    sandbox_environment_name.len() <= sandbox_max_environment_name_bytes
        && (sandbox_first_byte.is_ascii_uppercase() || sandbox_first_byte == b'_')
        && sandbox_environment_name_bytes.all(|sandbox_byte| {
            sandbox_byte.is_ascii_uppercase()
                || sandbox_byte.is_ascii_digit()
                || sandbox_byte == b'_'
        })
}

fn is_protected_sandbox_environment_name(sandbox_environment_name: &str) -> bool {
    matches!(
        sandbox_environment_name,
        "PATH"
            | "PATHEXT"
            | "COMSPEC"
            | "SYSTEMROOT"
            | "WINDIR"
            | "HOME"
            | "USERPROFILE"
            | "TMP"
            | "TEMP"
    )
}

fn is_sensitive_sandbox_environment_name(sandbox_environment_name: &str) -> bool {
    sandbox_environment_name
        .split('_')
        .any(|sandbox_name_segment| {
            matches!(
                sandbox_name_segment,
                "TOKEN"
                    | "SECRET"
                    | "PASSWORD"
                    | "CREDENTIAL"
                    | "PRIVATE"
                    | "SSH"
                    | "DOCKER"
                    | "AWS"
                    | "AZURE"
                    | "GOOGLE"
                    | "PROXY"
            )
        })
}

#[cfg(test)]
mod tests;
