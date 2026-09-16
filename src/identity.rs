use sha2::{Digest, Sha256};
use std::path::Path;

fn slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut in_invalid_run = false;
    for ch in value.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            normalized.push(ch);
            in_invalid_run = false;
        } else if !in_invalid_run {
            normalized.push('-');
            in_invalid_run = true;
        }
    }
    let trimmed = normalized.trim_matches('-');
    if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

pub fn lane_id(primary_root: &Path, name: &str) -> String {
    let basename = primary_root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let repository = truncate(&slug(&basename), 20);
    let lane = truncate(&slug(name), 24);
    let mut hasher = Sha256::new();
    hasher.update(primary_root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let digest_hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>().chars().take(10).collect();
    format!("aw-{repository}-{lane}-{digest_hex}")
}
