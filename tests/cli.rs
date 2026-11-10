//! The binary end to end, against the fixture workspace. `CODEMAP_CACHE_DIR` keeps the index out of
//! the real cache.

mod common;

use std::path::Path;
use std::process::Command;

use common::workspace;

fn run(dir: &Path, cache: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .args(args)
        .current_dir(dir)
        .env("CODEMAP_CACHE_DIR", cache)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn version_prints_crate_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .starts_with(concat!("codemap ", env!("CARGO_PKG_VERSION")))
    );
}

#[test]
fn completion_scripts_know_the_commands_and_parse_in_their_shell() {
    for (shell, checker) in [("bash", "bash"), ("zsh", "zsh"), ("fish", "fish")] {
        let out = Command::new(env!("CARGO_BIN_EXE_codemap"))
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(out.status.success(), "{shell}");
        let script = String::from_utf8_lossy(&out.stdout);
        // fish names long flags without their dashes (`-l dry-run`).
        for word in ["show", "setup", "dry-run"] {
            assert!(script.contains(word), "{shell} script lacks {word}");
        }
        for hidden in ["session-start", "emit", "Hidden"] {
            assert!(!script.contains(hidden), "{shell} script offers {hidden}");
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("completion");
        std::fs::write(&file, script.as_bytes()).unwrap();
        // Parse without running; a shell that is not installed is skipped.
        if let Ok(check) = Command::new(checker).arg("-n").arg(&file).output() {
            assert!(
                check.status.success(),
                "{shell}: {}",
                String::from_utf8_lossy(&check.stderr)
            );
        }
    }
}

#[test]
fn emit_prints_its_payload_and_nothing_else() {
    use base64::Engine;
    let payload = base64::engine::general_purpose::STANDARD.encode("line one\n$HOME `x`\n");
    let out = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .args(["emit", &payload])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "line one\n$HOME `x`\n"
    );
    let bad = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .args(["emit", "!!not base64!!"])
        .output()
        .unwrap();
    assert!(
        bad.status.success() && bad.stdout.is_empty(),
        "a bad payload prints nothing"
    );
}

#[test]
fn session_start_builds_and_tells_the_agent_to_use_codemap_first() {
    let f = workspace(None);
    let (code, stdout, _) = run(&f.ws.project.root, f.cache.path(), &["session-start"]);
    assert_eq!(code, 0);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let output = &value["hookSpecificOutput"];
    assert_eq!(output["hookEventName"], "SessionStart");
    let context = output["additionalContext"].as_str().unwrap();
    for needle in [
        "IMPORTANT",
        "searches this repository, ../server and ../web at once",
        "codemap <name, part of a name, file name, or a few words>",
        "codemap uses",
        "codemap impl",
        "codemap grep '<regex>'",
    ] {
        assert!(context.contains(needle), "{needle} missing in {context}");
    }
    let (_, listed, _) = run(&f.ws.project.root, f.cache.path(), &["def", "BookRoute"]);
    assert!(listed.contains("BookRoute.kt:12"), "the index was built");
    let outside = tempfile::tempdir().unwrap();
    let (code, stdout, _) = run(outside.path(), f.cache.path(), &["session-start"]);
    assert_eq!(
        (code, stdout.as_str()),
        (0, ""),
        "outside a repository it stays silent"
    );
}

#[test]
fn self_update_dry_run_prints_the_installer_command_and_runs_nothing() {
    let out = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .args(["self-update", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(
            "curl -fsSL https://raw.githubusercontent.com/LIVELUCKY/codemap/main/install.sh | sh -s -- --no-setup"
        ),
        "an update does not redo the agent setup: {stdout}"
    );
}

#[test]
fn def_prints_header_and_lines() {
    let f = workspace(None);
    let (code, stdout, _) = run(&f.ws.project.root, f.cache.path(), &["def", "BookRoute"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("# index (grammar)\n"), "{stdout}");
    assert!(
        stdout.contains("src/commonMain/kotlin/app/BookRoute.kt:12 class BookRoute @Resource\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("../server/src/main/kotlin/server/Routes.kt:4 class BookRoute @Resource\n"),
        "{stdout}"
    );
    assert!(!stdout.contains("next:"), "facts only: {stdout}");
}

#[test]
fn a_query_that_starts_with_a_command_word_is_still_a_query() {
    let f = workspace(None);
    for words in [
        &["find", "where", "is", "the", "book", "route"][..],
        &["show", "me", "the", "book", "route"][..],
        &["BookRuote"][..],
    ] {
        let (code, stdout, stderr) = run(&f.ws.project.root, f.cache.path(), words);
        assert_eq!(code, 0, "{words:?}: {stderr}");
        assert!(stdout.contains("BookRoute.kt"), "{words:?}: {stdout}");
    }
    let (code, _, stderr) = run(&f.ws.project.root, f.cache.path(), &["def", "--nope"]);
    assert_eq!(code, 2, "a wrong flag keeps clap's error: {stderr}");
}

#[test]
fn outside_repo_exits_2_with_message() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let (code, _, stderr) = run(dir.path(), cache.path(), &["def", "X"]);
    assert_eq!(code, 2);
    // the process sees its working directory in canonical form (/private/var on macOS)
    let expected = format!(
        "not a git repository: {}",
        dir.path().canonicalize().unwrap().display()
    );
    assert!(stderr.contains(&expected), "{stderr}");
}

#[test]
fn build_prints_stats_per_repository() {
    let f = workspace(None);
    let (code, stdout, _) = run(&f.ws.project.root, f.cache.path(), &["build"]);
    assert_eq!(code, 0, "{stdout}");
    for repo in ["app", "../server", "../web"] {
        assert!(
            stdout
                .lines()
                .any(|l| l.starts_with(&format!("{repo}: parsed "))
                    && l.contains(" reused ")
                    && l.contains(" ms: git ")
                    && l.contains(", parse ")
                    && l.contains(", write ")),
            "{stdout}"
        );
    }
    let (_, full, _) = run(&f.ws.project.root, f.cache.path(), &["build", "--full"]);
    assert!(
        full.lines()
            .next()
            .is_some_and(|l| l.starts_with("app: parsed ") && !l.starts_with("app: parsed 0 ")),
        "{full}"
    );
}

#[test]
fn update_reparses_one_file() {
    let f = workspace(None);
    run(&f.ws.project.root, f.cache.path(), &["build"]);
    let file =
        f.ws.project
            .root
            .join("src/commonMain/kotlin/app/Platform.kt");
    std::fs::write(
        &file,
        "package app\n\nexpect fun platformName(): String\nexpect fun other(): Int\n",
    )
    .unwrap();
    let (code, stdout, _) = run(
        &f.ws.project.root,
        f.cache.path(),
        &["update", "src/commonMain/kotlin/app/Platform.kt"],
    );
    assert_eq!(code, 0);
    assert!(stdout.starts_with("app: parsed 1 "), "{stdout}");
    let (_, unchanged, _) = run(
        &f.ws.project.root,
        f.cache.path(),
        &["update", "src/commonMain/kotlin/app/Platform.kt"],
    );
    assert!(unchanged.starts_with("app: parsed 0 "), "{unchanged}");
}

#[test]
fn all_lifts_the_cap() {
    let f = workspace(None);
    let (_, capped, _) = run(&f.ws.project.root, f.cache.path(), &["def", "same"]);
    assert!(capped.contains("... 5 more (--all)\n"), "{capped}");
    let (_, all, _) = run(
        &f.ws.project.root,
        f.cache.path(),
        &["def", "same", "--all"],
    );
    assert!(!all.contains(" more (--all)"), "{all}");
    // The bare lookup prints the same hint, so it takes the same option.
    let (_, bare, _) = run(&f.ws.project.root, f.cache.path(), &["same", "--all"]);
    assert_eq!(bare, all);
    assert_eq!(
        all.lines()
            .filter(|l| l.ends_with(" function same"))
            .count(),
        30
    );
}

#[test]
fn every_query_command_answers() {
    let f = workspace(None);
    let cases: &[(&[&str], &str)] = &[
        (&["uses", "BookPort"], "# index"),
        (&["impl", "BookPort"], "# index"),
        (&["variants", "platformName"], "# index"),
        (&["link", "/api/v1/books/{id}"], "# index"),
        (&["outline", "src/commonMain"], "# index"),
        (&["find", "BOOK_URL"], "# grep"),
        (&["search", "book", "route"], "# fts"),
        (&["deps", ":app"], "# build"),
    ];
    for (args, header) in cases {
        let (code, stdout, stderr) = run(&f.ws.project.root, f.cache.path(), args);
        assert_eq!(code, 0, "{args:?}: {stderr}");
        assert!(stdout.starts_with(header), "{args:?}: {stdout}");
    }
}
