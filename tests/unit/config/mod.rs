use super::*;
use std::fs;

#[test]
fn config_falls_back_to_dot_claude_then_default() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(load(root).siblings.is_empty(), "no file: default");

    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::write(
        root.join(".claude/.codemap.json"),
        r#"{"siblings": ["../b"], "literals": ["**/*.bru"]}"#,
    )
    .unwrap();
    let cfg = load(root);
    assert_eq!(cfg.siblings, [std::path::PathBuf::from("../b")]);
    assert_eq!(cfg.literals, vec!["**/*.bru"]);

    fs::write(
        root.join(".codemap.json"),
        r#"{"siblings": ["../c"], "roles": {"Port": ["*Port"]}}"#,
    )
    .unwrap();
    let cfg = load(root);
    assert_eq!(
        cfg.siblings,
        [std::path::PathBuf::from("../c")],
        "root wins"
    );
    assert_eq!(
        cfg.roles.get("Port").map(Vec::as_slice),
        Some(&["*Port".to_string()][..])
    );
    assert!(cfg.search_tools.is_empty());
}

#[test]
fn ignore_file_is_found_at_root_or_in_dot_claude() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(ignore(root).matched("vendor/x.kt", false).is_none());
    fs::create_dir_all(root.join(".claude")).unwrap();
    fs::write(root.join(".claude/.codemapignore"), "vendor/\n").unwrap();
    assert!(
        ignore(root)
            .matched_path_or_any_parents("vendor/x.kt", false)
            .is_ignore()
    );
}

#[test]
fn a_repository_cannot_change_what_the_hook_approves() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".codemap.json"),
        r#"{"settings": {"shell": {"writeCommands": [], "readOnlyFilters": ["sh"]}, "limits": {"answerLines": 3}}}"#,
    )
    .unwrap();
    let loaded = load(dir.path()).settings;
    assert_eq!(loaded.limits.answer_lines, 3, "other settings still apply");
    assert!(
        loaded
            .shell
            .write_commands
            .iter()
            .any(|c| c == "self-update")
    );
    assert!(loaded.shell.write_commands.iter().any(|c| c == "logs"));
    assert!(!loaded.shell.read_only_filters.iter().any(|f| f == "sh"));
}
