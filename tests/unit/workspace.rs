use super::*;
use std::path::Path;
use std::process::Command;

fn init(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    let out = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn neighbouring_repositories_are_not_siblings_unless_listed() {
    let parent = tempfile::tempdir().unwrap();
    for name in ["app", "server", "web"] {
        init(&parent.path().join(name));
    }
    let ws = discover(&parent.path().join("app")).unwrap();
    assert_eq!(ws.project.name, "app");
    assert!(ws.siblings.is_empty(), "{:?}", ws.siblings);
}

#[test]
fn siblings_come_from_the_config() {
    let parent = tempfile::tempdir().unwrap();
    for name in ["app", "server", "web"] {
        init(&parent.path().join(name));
    }
    std::fs::write(
        parent.path().join("app/.codemap.json"),
        r#"{"siblings": ["../web"]}"#,
    )
    .unwrap();
    let ws = discover(&parent.path().join("app")).unwrap();
    let names: Vec<&str> = ws.siblings.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["../web"], "named by the configured path");
    assert_eq!(
        ws.prompt("one call searches {repositories}."),
        "one call searches this repository and ../web."
    );
}

#[test]
fn above_repositories_the_error_says_which_ones_to_run_in() {
    let parent = tempfile::tempdir().unwrap();
    for name in ["web", "app"] {
        init(&parent.path().join(name));
    }
    std::fs::create_dir_all(parent.path().join("notes")).unwrap();
    let err = discover(parent.path()).unwrap_err().to_string();
    assert!(
        err.ends_with("run codemap inside one of the repositories here: app, web"),
        "{err}"
    );
}

#[test]
fn outside_a_repository_is_an_error_with_the_exact_message() {
    let dir = tempfile::tempdir().unwrap();
    let err = discover(dir.path()).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("not a git repository: {}", dir.path().display())
    );
}

#[test]
fn a_sibling_is_a_git_repository_next_to_the_project() {
    let parent = tempfile::tempdir().unwrap();
    for name in ["group/app", "group/web"] {
        init(&parent.path().join(name));
    }
    init(&parent.path().join("elsewhere"));
    std::fs::create_dir_all(parent.path().join("group/notes")).unwrap();
    std::fs::write(
        parent.path().join("group/app/.codemap.json"),
        r#"{"siblings": ["../web", "../notes", "../../elsewhere", "/", "."]}"#,
    )
    .unwrap();
    let ws = discover(&parent.path().join("group/app")).unwrap();
    let names: Vec<&str> = ws.siblings.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["../web"], "only the repository beside it");
}
