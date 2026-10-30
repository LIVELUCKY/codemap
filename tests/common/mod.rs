//! The fixture workspace (`tests/fixtures/workspace`) materialised as three git repositories in a temp
//! dir: `app` (Kotlin Multiplatform plus a Swift shell), `server` (Kotlin), `web` (TypeScript). The
//! project is `app`; its `.codemap.json` lists the other two as siblings.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;

use codemap::index;
use codemap::output::Answer;
use codemap::workspace::{Workspace, discover};

pub fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub struct Fixture {
    pub parent: tempfile::TempDir,
    pub cache: tempfile::TempDir,
    pub ws: Workspace,
}

/// The project's `.codemap.json`: `config` plus what the fixture declares unless `config` says
/// otherwise: its siblings, and its test folders (a repository names its own; nothing is guessed).
fn with_siblings(config: Option<&str>) -> String {
    let mut value: serde_json::Value = config
        .map(|c| serde_json::from_str(c).expect("test config is JSON"))
        .unwrap_or_else(|| serde_json::json!({}));
    if value.get("siblings").is_none() {
        value["siblings"] = serde_json::json!(["../server", "../web"]);
    }
    if value.pointer("/settings/scope/testDirs").is_none() {
        value["settings"]["scope"]["testDirs"] = serde_json::json!(["commonTest", "test"]);
    }
    value.to_string()
}

/// `config` becomes the project's `.codemap.json`, `ignore` its `.codemapignore`; both optional.
pub fn workspace_with(config: Option<&str>, ignore: Option<&str>) -> Fixture {
    let parent = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace");
    for repo in ["app", "server", "web"] {
        let root = parent.path().join(repo);
        fs::create_dir_all(&root).unwrap();
        copy_tree(&source.join(repo), &root);
        if repo == "app" {
            fs::write(root.join(".codemap.json"), with_siblings(config)).unwrap();
            if let Some(ignore) = ignore {
                fs::write(root.join(".codemapignore"), ignore).unwrap();
            }
        }
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "t@example.com"]);
        git(&root, &["config", "user.name", "t"]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "fixture"]);
    }
    let mut ws = discover(&parent.path().join("app")).unwrap();
    ws.cache_dir = cache.path().to_path_buf();
    index::build(&ws, false, None).unwrap();
    Fixture { parent, cache, ws }
}

pub fn workspace(config: Option<&str>) -> Fixture {
    workspace_with(config, None)
}

/// `[repo:]path:line kind name [scope]` per answer line.
pub fn lines(answer: &Answer) -> Vec<String> {
    answer
        .lines
        .iter()
        .map(|l| {
            format!(
                "{}{}:{} {} {} [{}]",
                l.repo
                    .as_deref()
                    .map(|r| format!("{r}/"))
                    .unwrap_or_default(),
                l.path,
                l.line,
                l.kind,
                l.name,
                l.scope
            )
        })
        .collect()
}
