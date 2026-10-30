//! The benchmark harness, against a fake `claude` (tests/fixtures/bench/fake-claude).

use std::fs;
use std::path::{Path, PathBuf};

use codemap::bench::{Cfg, Options, correct, materialise, run, table, update_readme};

fn fake_claude() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bench/fake-claude")
        .to_string_lossy()
        .into_owned()
}

/// A question file in a temp dir whose workspace is a fresh git repository.
fn question_file(expect: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("app");
    fs::create_dir_all(&repo).unwrap();
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap();
    let file = dir.path().join("questions.json");
    let body = serde_json::json!({
        "workspace": "app",
        "questions": [{"q": "where is Book defined", "expect": expect}],
    });
    fs::write(&file, body.to_string()).unwrap();
    (dir, file)
}

fn options(cfgs: Vec<Cfg>, runs: u32) -> Options {
    Options {
        cfgs,
        runs,
        claude: fake_claude(),
        plugin_dir: None,
        model: None,
    }
}

#[test]
fn median_of_three_runs_per_configuration() {
    let (_dir, file) = question_file(&["src/app/Book.kt:12"]);
    let runs = run(&file, &options(vec![Cfg::A, Cfg::C], 3)).unwrap();
    assert_eq!(runs.len(), 6);
    let a: Vec<_> = runs.iter().filter(|r| r.cfg == Cfg::A).collect();
    let c: Vec<_> = runs.iter().filter(|r| r.cfg == Cfg::C).collect();
    assert!(
        a.iter().all(|r| !r.correct && r.turns == 8),
        "line 40 is outside the tolerance of 12"
    );
    assert!(c.iter().all(|r| r.correct && r.turns == 4));
    assert_eq!(
        c[0].tokens, 1160,
        "input + output + cache creation + cache read"
    );
    assert!((c[0].seconds - 9.0).abs() < 1e-9);
}

#[test]
fn correctness_requires_every_expected_path_and_line_tolerance_two() {
    let answer = "Defined at src/app/Book.kt:12, served by server/Routes.kt line 4";
    assert!(correct(answer, &["src/app/Book.kt:12".into()]));
    assert!(
        correct(answer, &["src/app/Book.kt:14".into()]),
        "within two lines"
    );
    assert!(
        !correct(answer, &["src/app/Book.kt:15".into()]),
        "three lines off"
    );
    assert!(
        correct(answer, &["server/Routes.kt:4".into()]),
        "`line 4` counts as the line"
    );
    assert!(
        correct(answer, &["server/Routes.kt".into()]),
        "a path without a line needs only the path"
    );
    assert!(
        !correct(answer, &["src/app/Book.kt:12".into(), "web/api.ts".into()]),
        "every expectation"
    );
    assert!(
        !correct("Book.kt:12", &["src/app/Book.kt:12".into()]),
        "the path as written, not a suffix of it"
    );
}

#[test]
fn table_has_factors_against_baseline() {
    let (_dir, file) = question_file(&["src/app/Book.kt:12"]);
    let runs = run(&file, &options(vec![Cfg::A, Cfg::C], 3)).unwrap();
    let text = table(&runs, "header line");
    assert!(text.starts_with("header line\n"), "{text}");
    let a = text.lines().find(|l| l.starts_with("| A ")).expect("row A");
    let c = text.lines().find(|l| l.starts_with("| C ")).expect("row C");
    assert!(a.contains("| 0/3 |") && a.contains("| 8 |"), "{a}");
    assert!(c.contains("| 3/3 |") && c.contains("| 4 (x0.50) |"), "{c}");
    assert!(c.contains("$0.150 (x1.25)"), "{c}");
}

#[test]
fn update_readme_replaces_only_the_marked_block() {
    let dir = tempfile::tempdir().unwrap();
    let readme = dir.path().join("README.md");
    fs::write(&readme, "intro\n<!-- bench:fixture:start -->\nold\n<!-- bench:fixture:end -->\noutro\n<!-- bench:real:start -->\nkeep\n<!-- bench:real:end -->\n").unwrap();
    update_readme(&readme, "new table\n", "fixture").unwrap();
    let text = fs::read_to_string(&readme).unwrap();
    assert_eq!(
        text,
        "intro\n<!-- bench:fixture:start -->\nnew table\n<!-- bench:fixture:end -->\noutro\n<!-- bench:real:start -->\nkeep\n<!-- bench:real:end -->\n"
    );
    assert!(
        update_readme(&readme, "x", "missing").is_err(),
        "an unknown section is an error, not a silent append"
    );
}

#[test]
fn fixture_workspace_is_materialised_as_git_repositories() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace");
    let target = tempfile::tempdir().unwrap();
    materialise(&source, target.path()).unwrap();
    for repo in ["app", "server", "web"] {
        let root = target.path().join(repo);
        assert!(root.join(".git").is_dir(), "{repo} is a git repository");
        let head = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(head.status.success(), "{repo} has a commit");
    }
}

#[test]
fn the_model_option_reaches_claude() {
    let args = codemap::bench::claude_args(Cfg::C, "q", None, Some("haiku"));
    let at = args
        .iter()
        .position(|a| a == "--model")
        .expect("--model passed");
    assert_eq!(args[at + 1], "haiku");
    assert!(!codemap::bench::claude_args(Cfg::C, "q", None, None).contains(&"--model".to_string()));
}

#[test]
fn pins_resolve_against_the_real_parent_of_a_relative_workspace() {
    // A question file kept inside the project (`.claude/bench/`) names the project as `../..`; the
    // sibling repositories of a pin must be looked up next to the project, not next to `.claude`.
    let dir = tempfile::tempdir().unwrap();
    for repo in ["app", "server"] {
        let root = dir.path().join(repo);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("f"), repo).unwrap();
        for args in [
            &["init", "-q"][..],
            &["add", "-A"],
            &[
                "-c",
                "user.email=t@e.com",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "x",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    let head = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(dir.path().join("server"))
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let bench_dir = dir.path().join("app/.claude/bench");
    fs::create_dir_all(&bench_dir).unwrap();
    let file = bench_dir.join("questions.json");
    let body = serde_json::json!({
        "workspace": "../..",
        "pins": {"server": head.trim()},
        "questions": [{"q": "q", "expect": ["src/app/Book.kt:12"]}],
    });
    fs::write(&file, body.to_string()).unwrap();
    let runs = run(&file, &options(vec![Cfg::C], 1)).unwrap();
    assert!(runs[0].correct);
}

#[test]
fn a_pin_that_does_not_match_head_is_an_error() {
    let (dir, file) = question_file(&["x"]);
    let body = serde_json::json!({
        "workspace": "app",
        "pins": {"app": "0000000"},
        "questions": [{"q": "q", "expect": ["x"]}],
    });
    fs::write(&file, body.to_string()).unwrap();
    let err = run(&file, &options(vec![Cfg::C], 1))
        .unwrap_err()
        .to_string();
    assert!(err.contains("app") && err.contains("pinned"), "{err}");
    drop(dir);
}
