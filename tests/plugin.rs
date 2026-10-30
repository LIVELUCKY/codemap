//! The plugin files: manifests, hooks and the launcher.

use std::fs;
use std::path::Path;

use serde_json::Value;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}")),
    )
    .unwrap_or_else(|e| panic!("{path} is not JSON: {e}"))
}

#[test]
fn manifests_are_valid_json() {
    assert_eq!(json(".claude-plugin/plugin.json")["name"], "codemap");
    let market = json(".claude-plugin/marketplace.json");
    assert_eq!(market["plugins"][0]["name"], "codemap");
    assert_eq!(market["plugins"][0]["source"], "./");
}

#[test]
fn plugin_version_equals_crate_version() {
    assert_eq!(
        json(".claude-plugin/plugin.json")["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn hooks_reference_the_launcher_and_the_three_events() {
    let hooks = json("hooks/hooks.json");
    let commands: Vec<(String, String, String)> = ["SessionStart", "PreToolUse", "PostToolUse"]
        .iter()
        .map(|event| {
            let entry = &hooks["hooks"][*event][0];
            (
                event.to_string(),
                entry["matcher"].as_str().unwrap_or("").to_string(),
                entry["hooks"][0]["command"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect();
    for (event, _, command) in &commands {
        assert!(
            command.contains("${CLAUDE_PLUGIN_ROOT}/bin/codemap"),
            "{event}: {command}"
        );
    }
    assert!(
        commands[0].2.contains("codemap\" session-start") && commands[0].2.ends_with("exit 0"),
        "{:?}",
        commands[0]
    );
    assert_eq!(commands[1].1, "Grep|Bash|mcp__.*");
    assert!(commands[1].2.ends_with(" hook"), "{:?}", commands[1]);
    assert_eq!(commands[2].1, "Edit|Write|MultiEdit");
    assert!(
        commands[2].2.contains("update --from-event"),
        "{:?}",
        commands[2]
    );
}

#[test]
fn launcher_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(root().join("bin/codemap"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0, "bin/codemap must be executable");
}
