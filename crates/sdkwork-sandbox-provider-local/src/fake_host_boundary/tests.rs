// WORKSPACE-PATH:allow-fixture: every path literal in this file is hostile input
// the boundary must reject, so the fixture has to spell the machine path it
// guards against.
use std::collections::BTreeMap;

use super::{SandboxFakeHostBoundary, SandboxFakeHostBoundaryError};

#[test]
fn sandbox_fake_host_boundary_preserves_typed_arguments_without_shell_parsing() {
    let sandbox_fake_host_boundary = SandboxFakeHostBoundary::for_test(&["cargo"], &[]);

    let sandbox_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &["test", "--", "name;$(not-a-shell)"],
        "workspace/src",
        &BTreeMap::new(),
    );

    assert_eq!(sandbox_result, Ok(()));
}

#[test]
fn sandbox_fake_host_boundary_rejects_path_escape_and_windows_path_hazards() {
    let sandbox_fake_host_boundary = SandboxFakeHostBoundary::for_test(&["cargo"], &[]);

    for sandbox_working_directory in [
        "",
        ".",
        "..",
        "../outside",
        "workspace/../outside",
        "/host/root",
        "C:\\host\\root",
        "\\\\server\\share",
        "workspace/NUL.txt",
        "workspace/file:stream",
        "workspace/trailing. ",
    ] {
        let sandbox_result = sandbox_fake_host_boundary.validate_sandbox_command(
            "cargo",
            &[],
            sandbox_working_directory,
            &BTreeMap::new(),
        );

        assert_eq!(
            sandbox_result,
            Err(SandboxFakeHostBoundaryError::WorkingDirectoryInvalid),
            "sandbox path should be rejected: {sandbox_working_directory:?}"
        );
    }
}

#[test]
fn sandbox_fake_host_boundary_denies_command_strings_and_ambient_credentials() {
    let sandbox_fake_host_boundary = SandboxFakeHostBoundary::for_test(
        &["cargo", "cargo test", "../cargo"],
        &["PATH", "AWS_SECRET_ACCESS_KEY"],
    );

    let sandbox_command_string_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo test",
        &[],
        "workspace",
        &BTreeMap::new(),
    );
    assert_eq!(
        sandbox_command_string_result,
        Err(SandboxFakeHostBoundaryError::ExecutableInvalid)
    );

    let sandbox_path_executable_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "../cargo",
        &[],
        "workspace",
        &BTreeMap::new(),
    );
    assert_eq!(
        sandbox_path_executable_result,
        Err(SandboxFakeHostBoundaryError::ExecutableInvalid)
    );

    let sandbox_path_environment_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[],
        "workspace",
        &BTreeMap::from([("PATH", "workspace/bin")]),
    );
    assert_eq!(
        sandbox_path_environment_result,
        Err(SandboxFakeHostBoundaryError::EnvironmentProtected)
    );

    let sandbox_credential_environment =
        BTreeMap::from([("AWS_SECRET_ACCESS_KEY", "not-a-real-secret")]);
    let sandbox_environment_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[],
        "workspace",
        &sandbox_credential_environment,
    );
    assert_eq!(
        sandbox_environment_result,
        Err(SandboxFakeHostBoundaryError::EnvironmentSensitive)
    );
}

#[test]
fn sandbox_fake_host_boundary_enforces_argument_and_environment_bounds() {
    let sandbox_fake_host_boundary =
        SandboxFakeHostBoundary::for_test(&["cargo"], &["SANDBOX_MODE"]);
    let sandbox_contract: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apis/commands/sandbox-command-contract.json"
    )))
    .expect("sandbox command contract must be valid JSON");
    let sandbox_bounds = &sandbox_contract["bounds"];
    let sandbox_request_schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apis/commands/sandbox-command-execution-request.schema.json"
    )))
    .expect("sandbox command request schema must be valid JSON");
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_executable_bytes,
        sandbox_bounds["maxExecutableBytes"]
            .as_u64()
            .expect("maxExecutableBytes must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_arguments,
        sandbox_bounds["maxArgumentCount"]
            .as_u64()
            .expect("maxArgumentCount must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_argument_bytes,
        sandbox_bounds["maxArgumentBytes"]
            .as_u64()
            .expect("maxArgumentBytes must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_working_directory_bytes,
        sandbox_bounds["maxWorkingDirectoryBytes"]
            .as_u64()
            .expect("maxWorkingDirectoryBytes must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_environment_entries,
        sandbox_bounds["maxEnvironmentEntries"]
            .as_u64()
            .expect("maxEnvironmentEntries must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_environment_name_bytes,
        sandbox_bounds["maxEnvironmentNameBytes"]
            .as_u64()
            .expect("maxEnvironmentNameBytes must be an unsigned integer") as usize
    );
    assert_eq!(
        sandbox_fake_host_boundary.sandbox_max_environment_value_bytes,
        sandbox_bounds["maxEnvironmentValueBytes"]
            .as_u64()
            .expect("maxEnvironmentValueBytes must be an unsigned integer") as usize
    );

    let sandbox_arguments = vec!["argument"; 129];

    let sandbox_argument_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &sandbox_arguments,
        "workspace",
        &BTreeMap::new(),
    );
    assert_eq!(
        sandbox_argument_result,
        Err(SandboxFakeHostBoundaryError::ArgumentCountExceeded)
    );

    let sandbox_invalid_environment = BTreeMap::from([("Path", "sandbox-toolchain")]);
    let sandbox_environment_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[],
        "workspace",
        &sandbox_invalid_environment,
    );
    assert_eq!(
        sandbox_environment_result,
        Err(SandboxFakeHostBoundaryError::EnvironmentNameInvalid)
    );

    let sandbox_long_argument = "a".repeat(4_097);
    let sandbox_argument_bytes_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[sandbox_long_argument.as_str()],
        "workspace",
        &BTreeMap::new(),
    );
    assert_eq!(
        sandbox_argument_bytes_result,
        Err(SandboxFakeHostBoundaryError::ArgumentInvalid)
    );

    for sandbox_argument in ["line\rreturn", "line\nfeed"] {
        let sandbox_argument_control_result = sandbox_fake_host_boundary.validate_sandbox_command(
            "cargo",
            &[sandbox_argument],
            "workspace",
            &BTreeMap::new(),
        );
        assert_eq!(
            sandbox_argument_control_result,
            Err(SandboxFakeHostBoundaryError::ArgumentInvalid)
        );
    }

    let sandbox_environment_value_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[],
        "workspace",
        &BTreeMap::from([("SANDBOX_MODE", "invalid\0value")]),
    );
    assert_eq!(
        sandbox_environment_value_result,
        Err(SandboxFakeHostBoundaryError::EnvironmentValueInvalid)
    );

    for sandbox_environment_value in ["line\rreturn", "line\nfeed"] {
        let sandbox_environment_control_result = sandbox_fake_host_boundary
            .validate_sandbox_command(
                "cargo",
                &[],
                "workspace",
                &BTreeMap::from([("SANDBOX_MODE", sandbox_environment_value)]),
            );
        assert_eq!(
            sandbox_environment_control_result,
            Err(SandboxFakeHostBoundaryError::EnvironmentValueInvalid)
        );
    }

    let sandbox_protected_environment_names = sandbox_request_schema["properties"]
        ["sandboxEnvironment"]["propertyNames"]["allOf"][1]["not"]["enum"]
        .as_array()
        .expect("protected environment names must be an array");
    for sandbox_protected_environment_name in sandbox_protected_environment_names {
        let sandbox_protected_environment_name = sandbox_protected_environment_name
            .as_str()
            .expect("protected environment names must be strings");
        let sandbox_protected_environment_result =
            SandboxFakeHostBoundary::for_test(&["cargo"], &[sandbox_protected_environment_name])
                .validate_sandbox_command(
                    "cargo",
                    &[],
                    "workspace",
                    &BTreeMap::from([(sandbox_protected_environment_name, "caller-value")]),
                );
        assert_eq!(
            sandbox_protected_environment_result,
            Err(SandboxFakeHostBoundaryError::EnvironmentProtected)
        );
    }

    let sandbox_sensitive_environment_segments = sandbox_contract["environmentPolicy"]
        ["sensitiveRequestNameSegments"]
        .as_array()
        .expect("sensitive environment name segments must be an array");
    for sandbox_sensitive_environment_segment in sandbox_sensitive_environment_segments {
        let sandbox_sensitive_environment_name = format!(
            "SANDBOX_{}_VALUE",
            sandbox_sensitive_environment_segment
                .as_str()
                .expect("sensitive environment name segments must be strings")
        );
        let sandbox_sensitive_environment_result = SandboxFakeHostBoundary::for_test(
            &["cargo"],
            &[sandbox_sensitive_environment_name.as_str()],
        )
        .validate_sandbox_command(
            "cargo",
            &[],
            "workspace",
            &BTreeMap::from([(sandbox_sensitive_environment_name.as_str(), "caller-value")]),
        );
        assert_eq!(
            sandbox_sensitive_environment_result,
            Err(SandboxFakeHostBoundaryError::EnvironmentSensitive)
        );
    }
}

#[test]
fn sandbox_fake_host_boundary_enforces_environment_entry_bound() {
    let sandbox_environment_names: Vec<String> = (0..65)
        .map(|sandbox_index| format!("SANDBOX_{sandbox_index}"))
        .collect();
    let sandbox_environment_name_refs: Vec<&str> = sandbox_environment_names
        .iter()
        .map(String::as_str)
        .collect();
    let sandbox_fake_host_boundary =
        SandboxFakeHostBoundary::for_test(&["cargo"], &sandbox_environment_name_refs);
    let sandbox_environment = sandbox_environment_name_refs
        .iter()
        .map(|sandbox_environment_name| (*sandbox_environment_name, "1"))
        .collect();

    let sandbox_result = sandbox_fake_host_boundary.validate_sandbox_command(
        "cargo",
        &[],
        "workspace",
        &sandbox_environment,
    );
    assert_eq!(
        sandbox_result,
        Err(SandboxFakeHostBoundaryError::EnvironmentCountExceeded)
    );
}
