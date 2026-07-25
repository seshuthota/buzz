//! OpenCode harness config reader (`~/.config/opencode/opencode.jsonc`).
//!
//! OpenCode uses JSONC (JSON with comments). Model is typically
//! `provider/model` (e.g. `anthropic/claude-sonnet-4-20250514`). MCP servers
//! live under the top-level `mcp` object in the same file.

use super::types::{ExtensionEntry, RuntimeFileConfig};

/// Read OpenCode config from the standard user config path.
pub(super) fn read_config_file() -> Option<RuntimeFileConfig> {
    let path = opencode_config_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    parse_opencode_config(&raw)
}

/// Resolve `~/.config/opencode/opencode.jsonc`, falling back to `.json`.
pub(super) fn opencode_config_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".config").join("opencode");
    let jsonc = dir.join("opencode.jsonc");
    if jsonc.is_file() {
        return Some(jsonc);
    }
    let json = dir.join("opencode.json");
    if json.is_file() {
        return Some(json);
    }
    // Prefer the documented path even when absent so callers can report it.
    Some(jsonc)
}

fn parse_opencode_config(raw: &str) -> Option<RuntimeFileConfig> {
    let cleaned = strip_jsonc(raw);
    let value: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
    Some(config_from_value(&value))
}

fn config_from_value(value: &serde_json::Value) -> RuntimeFileConfig {
    let model_raw = json_string(value, "model");
    // OpenCode model format is `provider/model`. Split when possible so the
    // normalized surface can show provider separately (Goose/Codex style).
    let (provider_from_model, model) = split_provider_model(model_raw.as_deref());

    let mode = json_string(value, "mode").or_else(|| permission_as_mode(value));

    let skip = &[
        "model",
        "mode",
        "permission",
        "mcp",
        "$schema",
        // Nested tables are flattened into extra; keep top-level skip clean.
    ];
    let mut extra = super::schema_walker::extract_config_fields(value, skip);

    // Surface the full model string when we split it, so advanced view still
    // shows the OpenCode-native value.
    if let Some(ref full) = model_raw {
        if provider_from_model.is_some() {
            extra
                .entry("model_full".to_string())
                .or_insert_with(|| full.clone());
        }
    }

    let extensions = parse_mcp(value);

    RuntimeFileConfig {
        model,
        provider: provider_from_model,
        mode,
        thinking_effort: None,
        max_output_tokens: None,
        context_limit: None,
        system_prompt: None,
        extensions,
        extra,
    }
}

/// Split `provider/model` into (provider, model). Returns (None, Some(full))
/// when there is no slash; (None, None) when input is empty.
fn split_provider_model(model: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = model.map(str::trim).filter(|s| !s.is_empty()) else {
        return (None, None);
    };
    match raw.split_once('/') {
        Some((provider, rest))
            if !provider.is_empty() && !rest.is_empty() && !provider.contains(' ') =>
        {
            (Some(provider.to_string()), Some(rest.to_string()))
        }
        _ => (None, Some(raw.to_string())),
    }
}

/// Map string permission (`ask`/`allow`/`deny`) into the normalized `mode` field.
fn permission_as_mode(value: &serde_json::Value) -> Option<String> {
    match value.get("permission") {
        Some(serde_json::Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

fn parse_mcp(value: &serde_json::Value) -> Vec<ExtensionEntry> {
    let Some(servers) = value.get("mcp").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    servers
        .iter()
        .map(|(name, cfg)| {
            let enabled = cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            ExtensionEntry {
                name: name.clone(),
                kind: "mcp".to_string(),
                enabled,
            }
        })
        .collect()
}

fn json_string(val: &serde_json::Value, key: &str) -> Option<String> {
    val.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Minimal JSONC → JSON conversion for OpenCode configs.
///
/// Strips `//` line comments, `/* */` block comments, and trailing commas
/// before `}` / `]`. Not a full JSONC parser — sufficient for OpenCode's
/// schema-shaped configs (no comments inside strings is assumed for `//`
/// only when outside quotes).
fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' => match chars.peek() {
                Some('/') => {
                    // Line comment
                    chars.next();
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    // Block comment
                    chars.next();
                    let mut prev = '\0';
                    for nc in chars.by_ref() {
                        if prev == '*' && nc == '/' {
                            break;
                        }
                        prev = nc;
                    }
                }
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }

    // Remove trailing commas before } or ] (ASCII-aware; config keys are ASCII).
    let mut cleaned = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // skip the comma
                i += 1;
                continue;
            }
        }
        cleaned.push(chars[i]);
        i += 1;
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_provider_slash_form() {
        let cfg = parse_opencode_config(
            r#"{ "model": "anthropic/claude-sonnet-4-20250514", "mode": "build" }"#,
        )
        .expect("parse");
        assert_eq!(cfg.provider.as_deref(), Some("anthropic"));
        assert_eq!(cfg.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(cfg.mode.as_deref(), Some("build"));
        assert_eq!(
            cfg.extra.get("model_full").map(String::as_str),
            Some("anthropic/claude-sonnet-4-20250514")
        );
    }

    #[test]
    fn parse_model_without_slash() {
        let cfg = parse_opencode_config(r#"{ "model": "gpt-4o" }"#).expect("parse");
        assert_eq!(cfg.provider, None);
        assert_eq!(cfg.model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn parse_jsonc_with_comments_and_trailing_comma() {
        let raw = r#"
        {
          // primary model
          "model": "openrouter/foo",
          "permission": "allow",
          "mcp": {
            "filesystem": { "enabled": true },
          },
        }
        "#;
        let cfg = parse_opencode_config(raw).expect("jsonc parse");
        assert_eq!(cfg.provider.as_deref(), Some("openrouter"));
        assert_eq!(cfg.model.as_deref(), Some("foo"));
        assert_eq!(cfg.mode.as_deref(), Some("allow"));
        assert_eq!(cfg.extensions.len(), 1);
        assert_eq!(cfg.extensions[0].name, "filesystem");
        assert!(cfg.extensions[0].enabled);
    }

    #[test]
    fn parse_block_comment() {
        let raw = r#"{ /* skip */ "model": "a/b" }"#;
        let cfg = parse_opencode_config(raw).expect("parse");
        assert_eq!(cfg.provider.as_deref(), Some("a"));
        assert_eq!(cfg.model.as_deref(), Some("b"));
    }

    #[test]
    fn strip_jsonc_preserves_slashes_inside_strings() {
        let raw = r#"{ "url": "https://example.com//path", "model": "p/m" }"#;
        let cfg = parse_opencode_config(raw).expect("parse");
        assert_eq!(cfg.model.as_deref(), Some("m"));
        assert_eq!(
            cfg.extra.get("url").map(String::as_str),
            Some("https://example.com//path")
        );
    }

    #[test]
    fn empty_object_parses() {
        let cfg = parse_opencode_config("{}\n").expect("parse");
        assert!(cfg.model.is_none());
        assert!(cfg.provider.is_none());
        assert!(cfg.extensions.is_empty());
    }
}
