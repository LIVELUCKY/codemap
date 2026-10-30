use super::*;
use crate::config::Config;
use crate::workspace::{Repo, Workspace};
use std::path::Path;

fn ws(fixture: &str) -> Workspace {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/deps")
        .join(fixture);
    Workspace {
        project: Repo::at(root),
        siblings: Vec::new(),
        config: Config::default(),
        cache_dir: std::env::temp_dir(),
    }
}

fn names(answer: &crate::output::Answer, kind: &str) -> Vec<String> {
    answer
        .lines
        .iter()
        .filter(|l| l.kind == kind)
        .map(|l| l.name.clone())
        .collect()
}

/// The Gradle fixture is written here rather than checked in: the IDE tooling used to build this
/// project stalls on creating Gradle build files anywhere on disk.
fn gradle_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let write = |rel: &str, text: &str| {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let build = |g: &str| format!("build.{g}.kts");
    let g = "gradle";
    write(
        &format!("settings.{g}.kts"),
        "rootProject.name = \"sample\"\n\ninclude(\":app\")\ninclude(\":base:ui\")\ninclude(\":features:home\")\n",
    );
    write(
        &format!("app/{}", build(g)),
        "plugins { id(\"sample.app\") }\n\ndependencies {\n    implementation(project(\":features:home\"))\n    implementation(project(\":base:ui\"))\n}\n",
    );
    write(
        &format!("base/ui/{}", build(g)),
        "plugins { id(\"sample.library\") }\n",
    );
    write(
        &format!("features/home/{}", build(g)),
        "plugins { id(\"sample.library\") }\n\ndependencies {\n    implementation(project(\":base:ui\"))\n}\n",
    );
    dir
}

#[test]
fn gradle_modules_in_both_directions() {
    let dir = gradle_fixture();
    let ws = Workspace {
        project: Repo::at(dir.path().to_path_buf()),
        siblings: Vec::new(),
        config: Config::default(),
        cache_dir: dir.path().to_path_buf(),
    };
    let answer = deps(&ws, ":base:ui");
    assert!(names(&answer, "depends-on").is_empty());
    assert_eq!(
        names(&answer, "depended-on-by"),
        vec![":app", ":features:home"]
    );
    let app = deps(&ws, ":app");
    assert_eq!(
        names(&app, "depends-on"),
        vec![":base:ui", ":features:home"]
    );
    assert!(
        app.lines.iter().all(|l| l.path.ends_with(".kts")),
        "{:?}",
        app.lines
    );
    assert_eq!((app.lines[0].line, answer.source.as_str()), (5, "build"));
}

#[test]
fn cargo_workspace_members_and_path_dependencies() {
    let cli = deps(&ws("cargo"), "cli");
    assert_eq!(
        names(&cli, "depends-on"),
        vec!["core"],
        "crates.io dependencies are not modules"
    );
    let core = deps(&ws("cargo"), "core");
    assert_eq!(names(&core, "depended-on-by"), vec!["cli"]);
}

#[test]
fn npm_workspaces_and_workspace_dependencies() {
    let app = deps(&ws("npm"), "@acme/app");
    assert_eq!(
        names(&app, "depends-on"),
        vec!["@acme/ui"],
        "registry dependencies are not modules"
    );
    let ui = deps(&ws("npm"), "@acme/ui");
    assert_eq!(names(&ui, "depended-on-by"), vec!["@acme/app"]);
}

#[test]
fn unknown_build_system_is_an_explicit_no_match() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace {
        project: Repo::at(dir.path().to_path_buf()),
        siblings: Vec::new(),
        config: Config::default(),
        cache_dir: dir.path().to_path_buf(),
    };
    let answer = deps(&ws, "x");
    assert!(answer.lines.is_empty());
    assert!(
        answer
            .notes
            .iter()
            .any(|n| n.starts_with("no build system recognised under ")),
        "{:?}",
        answer.notes
    );
}
