use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

use crate::devcontainer::find_devcontainer_config;
use crate::errors::{AgentWorktreeError, Result};

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file: String,
    pub message: String,
}

fn strip_json_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn parse_jsonc(input: &str) -> serde_json::Result<JsonValue> {
    let without_comments = strip_json_comments(input);
    let without_trailing_commas = strip_trailing_commas(&without_comments);
    serde_json::from_str(&without_trailing_commas)
}

fn as_object(value: &JsonValue) -> serde_json::Map<String, JsonValue> {
    value.as_object().cloned().unwrap_or_default()
}

fn strings(value: Option<&JsonValue>) -> Vec<String> {
    match value {
        Some(JsonValue::String(single)) => vec![single.clone()],
        Some(JsonValue::Array(items)) => {
            if items.iter().all(|item| item.is_string()) {
                items.iter().map(|item| item.as_str().unwrap().to_string()).collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn yaml_as_json(value: &YamlValue) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

fn inspect_compose(compose: &JsonValue, filepath: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let services = compose.get("services").map(as_object).unwrap_or_default();
    for (service_name, service) in &services {
        let Some(service) = service.as_object() else { continue };
        if service.get("container_name").and_then(JsonValue::as_str).is_some() {
            diagnostics.push(Diagnostic {
                file: filepath.to_string(),
                message: format!("service {service_name} fixes container_name globally"),
            });
        }
        if service.get("network_mode").and_then(JsonValue::as_str) == Some("host") {
            diagnostics.push(Diagnostic {
                file: filepath.to_string(),
                message: format!("service {service_name} uses the host network"),
            });
        }
        let volumes = service.get("volumes").and_then(JsonValue::as_array).cloned().unwrap_or_default();
        for volume in &volumes {
            let serialized = volume.as_str().map(str::to_string).unwrap_or_else(|| volume.to_string());
            if serialized.contains("/var/run/docker.sock") {
                diagnostics.push(Diagnostic {
                    file: filepath.to_string(),
                    message: format!("service {service_name} mounts the Docker socket"),
                });
            }
        }
        let ports = service.get("ports").and_then(JsonValue::as_array).cloned().unwrap_or_default();
        for port in &ports {
            let fixed_short_port = port.as_str().map(is_fixed_short_port).unwrap_or(false);
            let fixed_long_port = port
                .as_object()
                .map(|obj| matches!(obj.get("published"), Some(JsonValue::Number(_)) | Some(JsonValue::String(_))))
                .unwrap_or(false);
            if fixed_short_port || fixed_long_port {
                diagnostics.push(Diagnostic {
                    file: filepath.to_string(),
                    message: format!("service {service_name} publishes a fixed host port"),
                });
            }
        }
    }

    for section_name in ["volumes", "networks"] {
        let section = compose.get(section_name).map(as_object).unwrap_or_default();
        for (resource_name, resource) in &section {
            let Some(resource) = resource.as_object() else { continue };
            if resource.get("external") == Some(&JsonValue::Bool(true)) {
                diagnostics.push(Diagnostic {
                    file: filepath.to_string(),
                    message: format!("{section_name}.{resource_name} is external and may be shared"),
                });
            }
            if resource.get("name").and_then(JsonValue::as_str).is_some() {
                diagnostics.push(Diagnostic {
                    file: filepath.to_string(),
                    message: format!("{section_name}.{resource_name} has a fixed global name"),
                });
            }
        }
    }
    diagnostics
}

fn is_fixed_short_port(value: &str) -> bool {
    let (main, has_protocol) = match value.rsplit_once('/') {
        Some((prefix, suffix)) if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => (prefix, true),
        _ => (value, false),
    };
    let _ = has_protocol;
    let parts: Vec<&str> = main.split(':').collect();
    let is_digits = |value: &str| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit());
    match parts.len() {
        2 => is_digits(parts[0]) && is_digits(parts[1]),
        3 => !parts[0].is_empty() && !parts[0].contains(':') && is_digits(parts[1]) && is_digits(parts[2]),
        _ => false,
    }
}

pub fn diagnose(worktree_path: &Path) -> Result<Vec<Diagnostic>> {
    let config_path = find_devcontainer_config(worktree_path)?;
    let config_text = fs::read_to_string(&config_path)?;
    let parsed = parse_jsonc(&config_text)
        .map_err(|error| AgentWorktreeError::new(format!("{}: {error}", config_path.display())))?;
    if !parsed.is_object() {
        return Err(AgentWorktreeError::new(format!("{} does not contain a JSON object", config_path.display())));
    }

    let mut diagnostics = Vec::new();
    if parsed.get("workspaceMount").and_then(JsonValue::as_str).is_some() {
        diagnostics.push(Diagnostic {
            file: config_path.to_string_lossy().to_string(),
            message: "custom workspaceMount can prevent automatic Git common-directory mounting for worktrees"
                .to_string(),
        });
    }

    for compose_file in strings(parsed.get("dockerComposeFile")) {
        let filepath: PathBuf = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&compose_file);
        let compose_text = fs::read_to_string(&filepath)?;
        let compose_yaml: YamlValue = serde_yaml::from_str(&compose_text)
            .map_err(|error| AgentWorktreeError::new(format!("{}: {error}", filepath.display())))?;
        let compose = yaml_as_json(&compose_yaml);
        if !compose.is_object() {
            return Err(AgentWorktreeError::new(format!("{} does not contain a Compose object", filepath.display())));
        }
        diagnostics.extend(inspect_compose(&compose, &filepath.to_string_lossy()));
    }
    Ok(diagnostics)
}
