use std::fs;

use agent_worktree::doctor::diagnose;

#[test]
fn reports_compose_settings_that_leak_across_lanes() {
    let root = std::env::temp_dir().join(format!(
        "agent-worktree-doctor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let config_directory = root.join(".devcontainer");
    fs::create_dir_all(&config_directory).unwrap();
    fs::write(
        config_directory.join("devcontainer.json"),
        r#"{
      // JSONC is supported by devcontainers.
      "dockerComposeFile": "compose.yaml",
      "service": "app",
      "workspaceMount": "source=.,target=/workspace,type=bind",
    }"#,
    )
    .unwrap();
    fs::write(
        config_directory.join("compose.yaml"),
        r#"services:
  app:
    container_name: shared-app
    network_mode: host
    ports:
      - "3000:3000"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
volumes:
  db:
    name: shared-db
networks:
  backend:
    external: true
"#,
    )
    .unwrap();

    let diagnostics = diagnose(&root).unwrap();
    let messages: Vec<String> = diagnostics.iter().map(|diagnostic| diagnostic.message.clone()).collect();

    assert_eq!(messages.len(), 7);
    assert!(messages.iter().any(|message| message.contains("custom workspaceMount")));
    assert!(messages.iter().any(|message| message.contains("container_name")));
    assert!(messages.iter().any(|message| message.contains("host network")));
    assert!(messages.iter().any(|message| message.contains("fixed host port")));
    assert!(messages.iter().any(|message| message.contains("Docker socket")));
    assert!(messages.iter().any(|message| message.contains("fixed global name")));
    assert!(messages.iter().any(|message| message.contains("external and may be shared")));

    fs::remove_dir_all(&root).unwrap();
}
