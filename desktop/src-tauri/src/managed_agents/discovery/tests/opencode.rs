//! OpenCode runtime catalog and arg-normalization coverage.

use super::super::{known_acp_runtime, known_acp_runtime_exact, normalize_agent_args};

#[test]
fn normalizes_opencode_and_goose_args_to_acp() {
    assert_eq!(normalize_agent_args("opencode", Vec::new()), vec!["acp"]);
    assert_eq!(
        normalize_agent_args("opencode", vec!["".into()]),
        vec!["acp"]
    );
    assert_eq!(
        normalize_agent_args("/usr/local/bin/opencode", Vec::new()),
        vec!["acp"]
    );
    assert_eq!(normalize_agent_args("goose", Vec::new()), vec!["acp"]);
}

#[test]
fn opencode_runtime_catalog_metadata() {
    let runtime = known_acp_runtime_exact("opencode").expect("opencode catalog entry");
    assert_eq!(runtime.label, "OpenCode");
    assert_eq!(runtime.commands, &["opencode"]);
    assert!(runtime.adapter_install_commands.is_empty());
    assert_eq!(runtime.underlying_cli, Some("opencode"));
    assert!(known_acp_runtime("opencode").is_some());
    assert!(known_acp_runtime("/opt/bin/opencode").is_some());
    assert!(
        !runtime.cli_install_commands.is_empty(),
        "opencode must have CLI install commands"
    );
    assert!(
        !runtime.cli_install_commands_for_os().is_empty(),
        "opencode must have install commands on every platform"
    );
    assert_eq!(runtime.skill_dir, Some(".opencode/skills"));
    assert_eq!(
        runtime.config_file_path,
        Some("~/.config/opencode/opencode.jsonc")
    );
    assert_eq!(
        runtime.cli_install_instructions_url,
        "https://opencode.ai/docs/"
    );
    assert!(runtime.adapter_install_instructions_url.is_empty());
    assert!(runtime.cli_install_hint.contains("desktop app alone"));
}
