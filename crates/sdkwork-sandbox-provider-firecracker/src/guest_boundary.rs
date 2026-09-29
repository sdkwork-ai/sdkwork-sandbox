//! Pure-data guest command boundary for the Firecracker provider.
//!
//! Commands execute inside the guest, but admission is provider-owned on the
//! host side: the same Executable/Argv/WorkingDirectory/Environment rule
//! family the Local boundary enforces (`REQ-2026-0007` shared contract), so a
//! shell string or a traversal path is refused before it ever reaches the
//! guest channel. Resolution inside the guest is provider-owned (the guest
//! agent resolves an allowlisted bare name inside the pinned rootfs, never a
//! path search), which is why an allowlisted bare-name executable and a
//! logical relative working directory are the only shapes admitted. No host
//! I/O: every check is a pure function over the request data.

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

/// Why a sandbox command request failed the Firecracker guest boundary.
///
/// The variants are the contract's fail-closed decision points; a caller may
/// only retry with new request data, never relax the boundary that produced
/// one of these values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerGuestBoundaryError {
    /// The executable is empty, over-long, or not a bare alphanumeric name
    /// with `.`, `_`, `+`, `-`.
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
    /// An environment name is protected: the guest rootfs owns it, the request
    /// may not set it.
    EnvironmentProtected,
    /// An environment name is sensitive by segment (`TOKEN`, `SECRET`, ...).
    EnvironmentSensitive,
    /// An environment name is well-formed but not in the boundary's allowlist.
    EnvironmentDenied,
    /// An environment value is over-long or contains a forbidden byte.
    EnvironmentValueInvalid,
}

impl fmt::Display for SandboxFirecrackerGuestBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sandbox_message = match self {
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
                "sandbox environment name is guest-protected and may not be set"
            }
            Self::EnvironmentSensitive => {
                "sandbox environment name carries a sensitive segment and is denied"
            }
            Self::EnvironmentDenied => "sandbox environment name is not allowlisted",
            Self::EnvironmentValueInvalid => {
                "sandbox environment value is over-long or contains a forbidden byte"
            }
        };
        f.write_str(sandbox_message)
    }
}

impl Error for SandboxFirecrackerGuestBoundaryError {}

/// The hard bounds the guest boundary enforces on one command request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerGuestBoundaryLimits {
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

impl Default for SandboxFirecrackerGuestBoundaryLimits {
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

/// The fail-closed guest boundary: allowlisted bare-name executables (resolved
/// inside the pinned rootfs by the guest agent, without path search),
/// allowlisted environment names, and hard request bounds, checked entirely on
/// the request data before the guest channel may carry anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerGuestBoundary {
    sandbox_allowed_executables: BTreeSet<String>,
    sandbox_allowed_environment: BTreeSet<String>,
    sandbox_limits: SandboxFirecrackerGuestBoundaryLimits,
}

impl SandboxFirecrackerGuestBoundary {
    /// Builds a boundary from the allowlisted executable names and environment
    /// names. The default limits apply; use
    /// [`with_limits`](Self::with_limits) to override them.
    #[must_use]
    pub fn new(
        sandbox_allowed_executables: BTreeSet<String>,
        sandbox_allowed_environment: BTreeSet<String>,
    ) -> Self {
        Self {
            sandbox_allowed_executables,
            sandbox_allowed_environment,
            sandbox_limits: SandboxFirecrackerGuestBoundaryLimits::default(),
        }
    }

    /// Replaces the hard bounds on this boundary.
    #[must_use]
    pub fn with_limits(mut self, sandbox_limits: SandboxFirecrackerGuestBoundaryLimits) -> Self {
        self.sandbox_limits = sandbox_limits;
        self
    }

    /// Validates one command request against the allowlists and the hard
    /// bounds. A successful return authorizes nothing by itself and performs
    /// no host or guest action.
    ///
    /// # Errors
    ///
    /// Returns the first failed rule in the boundary's fixed order:
    /// executable, arguments, working directory, then environment entries.
    pub fn validate_sandbox_command(
        &self,
        sandbox_executable: &str,
        sandbox_arguments: &[&str],
        sandbox_working_directory: &str,
        sandbox_environment: &BTreeMap<&str, &str>,
    ) -> Result<(), SandboxFirecrackerGuestBoundaryError> {
        if !is_valid_sandbox_executable(
            sandbox_executable,
            self.sandbox_limits.sandbox_max_executable_bytes,
        ) {
            return Err(SandboxFirecrackerGuestBoundaryError::ExecutableInvalid);
        }
        if !self
            .sandbox_allowed_executables
            .contains(sandbox_executable)
        {
            return Err(SandboxFirecrackerGuestBoundaryError::ExecutableDenied);
        }

        if sandbox_arguments.len() > self.sandbox_limits.sandbox_max_arguments {
            return Err(SandboxFirecrackerGuestBoundaryError::ArgumentCountExceeded);
        }
        if sandbox_arguments.iter().any(|sandbox_argument| {
            contains_forbidden_sandbox_string_byte(sandbox_argument)
                || sandbox_argument.len() > self.sandbox_limits.sandbox_max_argument_bytes
        }) {
            return Err(SandboxFirecrackerGuestBoundaryError::ArgumentInvalid);
        }

        if !is_valid_sandbox_logical_relative_path(
            sandbox_working_directory,
            self.sandbox_limits.sandbox_max_working_directory_bytes,
        ) {
            return Err(SandboxFirecrackerGuestBoundaryError::WorkingDirectoryInvalid);
        }

        if sandbox_environment.len() > self.sandbox_limits.sandbox_max_environment_entries {
            return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentCountExceeded);
        }
        for (sandbox_environment_name, sandbox_environment_value) in sandbox_environment {
            if !is_valid_sandbox_environment_name(
                sandbox_environment_name,
                self.sandbox_limits.sandbox_max_environment_name_bytes,
            ) {
                return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentNameInvalid);
            }
            if is_protected_sandbox_environment_name(sandbox_environment_name) {
                return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentProtected);
            }
            if is_sensitive_sandbox_environment_name(sandbox_environment_name) {
                return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentSensitive);
            }
            if !self
                .sandbox_allowed_environment
                .contains(*sandbox_environment_name)
            {
                return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentDenied);
            }
            if contains_forbidden_sandbox_string_byte(sandbox_environment_value)
                || sandbox_environment_value.len()
                    > self.sandbox_limits.sandbox_max_environment_value_bytes
            {
                return Err(SandboxFirecrackerGuestBoundaryError::EnvironmentValueInvalid);
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

fn is_valid_sandbox_logical_relative_path(
    sandbox_path: &str,
    sandbox_max_path_bytes: usize,
) -> bool {
    if sandbox_path.is_empty()
        || sandbox_path.len() > sandbox_max_path_bytes
        || sandbox_path.as_bytes().contains(&0)
        || sandbox_path.starts_with('/')
        || sandbox_path.starts_with('\\')
    {
        return false;
    }

    // The contract's workspace-root token: a command may address the Workspace
    // root itself as exactly `.`. Any other use of a `.` segment (embedded or
    // as `..`) stays rejected, so traversal forms gain nothing.
    if sandbox_path == "." {
        return true;
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
    !is_reserved_sandbox_device_name(sandbox_base_name)
}

fn is_reserved_sandbox_device_name(sandbox_base_name: &str) -> bool {
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
            // Dynamic-loader injection vectors: a request that could plant
            // code into every guest-spawned child must never pass.
            | "LD_PRELOAD"
            | "LD_LIBRARY_PATH"
            | "DYLD_INSERT_LIBRARIES"
            | "DYLD_LIBRARY_PATH"
            | "DYLD_FRAMEWORK_PATH"
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
                    // `API_KEY`/`GPG_KEY`-style names carry secrets exactly
                    // like tokens do; the segment filter is the last line of
                    // defense if an allowlist is mis-composed.
                    | "KEY"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::SandboxFirecrackerGuestBoundaryError;
    use crate::guest_boundary::SandboxFirecrackerGuestBoundary;
    use std::collections::BTreeMap;

    fn sandbox_boundary() -> SandboxFirecrackerGuestBoundary {
        SandboxFirecrackerGuestBoundary::new(
            ["toybox", "sh"].into_iter().map(str::to_owned).collect(),
            ["LANG".to_owned(), "LC_ALL".to_owned()]
                .into_iter()
                .collect(),
        )
    }

    #[test]
    fn admits_an_allowlisted_bare_name_with_a_logical_directory() {
        let sandbox_environment: BTreeMap<&str, &str> = BTreeMap::from([("LANG", "c")]);
        assert_eq!(
            Ok(()),
            sandbox_boundary().validate_sandbox_command(
                "toybox",
                &["echo", "hello"],
                "workspace/out",
                &sandbox_environment
            )
        );
        assert_eq!(
            Ok(()),
            sandbox_boundary().validate_sandbox_command("toybox", &[], ".", &BTreeMap::new())
        );
    }

    #[test]
    fn rejects_shell_strings_and_unallowlisted_executables() {
        assert_eq!(
            Err(SandboxFirecrackerGuestBoundaryError::ExecutableInvalid),
            sandbox_boundary().validate_sandbox_command("sh -c 'id'", &[], ".", &BTreeMap::new())
        );
        assert_eq!(
            Err(SandboxFirecrackerGuestBoundaryError::ExecutableDenied),
            sandbox_boundary().validate_sandbox_command("python", &[], ".", &BTreeMap::new())
        );
    }

    #[test]
    fn rejects_traversal_and_absolute_working_directories() {
        for sandbox_directory in ["/tmp", "../escape", "..\\escape", "C:\\Windows", "a/../b"] {
            assert_eq!(
                Err(SandboxFirecrackerGuestBoundaryError::WorkingDirectoryInvalid),
                sandbox_boundary().validate_sandbox_command(
                    "toybox",
                    &[],
                    sandbox_directory,
                    &BTreeMap::new()
                ),
                "directory {sandbox_directory} must be rejected"
            );
        }
    }

    #[test]
    fn denies_the_environment_by_default_and_protects_guest_owned_names() {
        let sandbox_boundary = sandbox_boundary();
        assert_eq!(
            Err(SandboxFirecrackerGuestBoundaryError::EnvironmentDenied),
            sandbox_boundary.validate_sandbox_command(
                "toybox",
                &[],
                ".",
                &BTreeMap::from([("NOT_ON_THE_ALLOWLIST", "value")])
            )
        );
        for sandbox_protected in ["PATH", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES"] {
            assert_eq!(
                Err(SandboxFirecrackerGuestBoundaryError::EnvironmentProtected),
                sandbox_boundary.validate_sandbox_command(
                    "toybox",
                    &[],
                    ".",
                    &BTreeMap::from([(sandbox_protected, "override")])
                ),
                "protected name {sandbox_protected} must be denied"
            );
        }
        assert_eq!(
            Err(SandboxFirecrackerGuestBoundaryError::EnvironmentSensitive),
            sandbox_boundary.validate_sandbox_command(
                "toybox",
                &[],
                ".",
                &BTreeMap::from([("GPG_KEY", "secret-value")])
            )
        );
    }
}
