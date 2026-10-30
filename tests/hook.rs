//! The PreToolUse hook: a search is reproduced exactly and answered in the same step, or passes.

mod common;

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use codemap::agent::hook::{Search, classify, respond};
use codemap::config::Config;
use codemap::query::find::FindOptions;
use common::workspace;
use serde_json::{Value, json};

fn classified(tool: &str, input: Value, cwd: &Path) -> Search {
    classify(tool, &input, &Config::default(), cwd)
}

fn bash(command: &str, cwd: &Path) -> Search {
    classified("Bash", json!({ "command": command }), cwd)
}

/// The search a Bash command becomes; panics when it passes.
fn grep_of(command: &str, cwd: &Path) -> FindOptions {
    match bash(command, cwd) {
        Search::Grep(options) => options,
        other => panic!("{command}: expected a grep, got {other:?}"),
    }
}

fn deny_reason(answer: Option<String>) -> String {
    let value: Value = serde_json::from_str(&answer.expect("an answer")).unwrap();
    value["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .to_string()
}

fn answer(command: &str, f: &common::Fixture) -> String {
    deny_reason(respond(&bash(command, &f.ws.project.root), Some(&f.ws)))
}

#[test]
fn a_grep_stays_the_same_grep_whatever_its_pattern_looks_like() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    for (command, pattern) in [
        ("grep -rn BookRoute .", "BookRoute"),
        ("rg 'class BookRoute'", "class BookRoute"),
        (
            "grep -r 'delete endpoint journal' .",
            "delete endpoint journal",
        ),
        (r"grep -rn 'foo\|bar' src", "foo|bar"),
        ("grep -rnE 'foo|bar' src", "foo|bar"),
        ("rg 'foo|bar' src", "foo|bar"),
        ("rg -F 'a.b' src", r"a\.b"),
        ("git grep -n BookRoute", "BookRoute"),
    ] {
        assert_eq!(grep_of(command, root).pattern, pattern, "{command}");
    }
}

#[test]
fn options_and_folders_are_carried_over() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    let options = grep_of(
        "grep -rIl -i \"okhttp\" --include=*.kt --include=*.pro . 2>/dev/null",
        root,
    );
    assert!(options.ignore_case && options.files_only);
    assert_eq!(options.include, ["*.kt", "*.pro"]);
    assert_eq!(options.scope.as_deref(), Some(root.join(".").as_path()));
    let rg = grep_of("rg -S -g '!*Test.kt' -t kotlin foo src", root);
    assert!(rg.smart_case && !rg.ignore_case);
    assert_eq!(rg.exclude, ["*Test.kt"]);
    assert_eq!(rg.include, ["*.kt", "*.kts"]);
    assert_eq!(rg.scope.as_deref(), Some(root.join("src").as_path()));
    assert!(!rg.hidden, "rg skips hidden files unless told");
    assert!(
        grep_of("grep -r foo src", root).hidden,
        "grep -r reads them"
    );
    assert!(grep_of("git grep foo", root).tracked);
}

#[test]
fn what_cannot_be_reproduced_exactly_passes() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    for command in [
        "grep foo",                                           // reads stdin
        "cat x | grep foo",                                   // filters a pipe
        "grep -n foo src/commonMain/kotlin/app/BookRoute.kt", // reads one file
        "grep -rc foo .",                                     // counts
        "grep -rn -C2 foo .",                                 // context
        "grep -rv foo .",                                     // inverted
        "grep -rP '\\d+' .",                                  // PCRE
        "grep -r --no-such-flag foo .",                       // unknown option
        "rg --pcre2 foo",
        "rg -t nosuchtype foo",
        "grep -r foo src ios", // several folders
        "grep -r foo no/such/dir",
        "cargo build",
    ] {
        assert_eq!(bash(command, root), Search::Pass, "{command}");
    }
}

#[test]
fn redirections_are_not_paths() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    let options = grep_of("grep -rn foo src 2>/dev/null", root);
    assert_eq!(options.scope.as_deref(), Some(root.join("src").as_path()));
}

#[test]
fn the_answer_has_every_match_in_grep_shape() {
    let f = workspace(None);
    let reason = answer("grep -rnE 'Resource|class BookRoute' src", &f);
    assert!(reason.starts_with("# grep"), "{reason}");
    assert!(
        reason.contains("src/commonMain/kotlin/app/\n"),
        "the shared folder is named once: {reason}"
    );
    assert!(
        reason.contains("\n  BookRoute.kt:12: class BookRoute("),
        "{reason}"
    );
    assert!(
        reason.contains("\n  BookRoute.kt:11: @Resource"),
        "any other match is a plain match, not left out: {reason}"
    );
    assert!(reason.contains("gitignored excluded"), "{reason}");
    assert!(!reason.contains("next:"), "facts only: {reason}");
}

#[test]
fn files_with_matches_lists_files() {
    let f = workspace(None);
    let reason = answer("grep -rIl -i okhttp --include=*.pro --include=*.toml .", &f);
    assert!(reason.contains("proguard-rules.pro:1 file"), "{reason}");
    assert!(reason.contains("libs.versions.toml:1 file"), "{reason}");
}

fn matches_of(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|l| {
            let mut parts = l.splitn(3, ':');
            let path = parts.next()?.trim_start_matches("./");
            let line = parts.next()?.split(' ').next()?;
            line.parse::<u32>().ok().map(|n| format!("{path}:{n}"))
        })
        .collect()
}

#[test]
fn the_same_matches_as_the_real_tool() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    for (program, command) in [
        (
            "grep",
            r"grep -rn --exclude-dir=.git 'Book\(Route\|Port\)' .",
        ),
        ("grep", "grep -rnE --exclude-dir=.git 'load|Route' ."),
        ("rg", "rg -n 'class \\w+' ."),
    ] {
        let Ok(real) = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(root)
            .output()
        else {
            continue;
        };
        if Command::new(program).arg("--version").output().is_err() {
            continue;
        }
        let expected = matches_of(&String::from_utf8_lossy(&real.stdout));
        assert!(!expected.is_empty(), "{command} found something");
        let mut options = grep_of(command, root);
        options.all = true;
        options.scope = Some(Default::default());
        let answer = codemap::query::find::find_with(&f.ws, &options);
        let got: BTreeSet<String> = answer
            .lines
            .iter()
            .map(|l| format!("{}:{}", l.path, l.line))
            .collect();
        assert_eq!(got, expected, "{command}");
    }
}

fn rejection(search: Search) -> String {
    match search {
        Search::Reject(message) => message,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn an_unquoted_glob_in_an_option_value_is_refused_with_the_fix() {
    // These would reach a shell: zsh expands the glob first and aborts with `no matches found`.
    let f = workspace(None);
    let root = &f.ws.project.root;
    let message = rejection(bash("rg -C2 foo --glob=*.kt src", root));
    assert!(
        message.contains("--glob=*.kt") && message.contains("no matches found"),
        "{message}"
    );
    assert!(message.contains("--glob='*.kt'"), "{message}");
    assert!(rejection(bash("find . -name *.kt -print", root)).contains("*.kt"));
    assert!(rejection(bash("cat x | rg -g *.kt foo", root)).contains("*.kt"));
}

#[test]
fn quoted_globs_and_intended_expansions_are_left_alone() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    for command in [
        "find . -name '*.kt' -print",
        "rg -C2 foo --glob='*.kt' src",
        "ls *.kt",
        "rm -f build/*.log",
        "echo '--include=*.kt'",
    ] {
        assert_eq!(bash(command, root), Search::Pass, "{command}");
    }
}

#[test]
fn a_rejection_needs_no_repository_and_nothing_else_answers_outside_one() {
    let f = workspace(None);
    let search = Search::Reject("quote the glob".into());
    assert_eq!(deny_reason(respond(&search, Some(&f.ws))), "quote the glob");
    assert!(respond(&search, None).is_some());
    assert!(respond(&Search::Pass, Some(&f.ws)).is_none());
    assert!(respond(&Search::Symbol("BookRoute".into()), None).is_none());
}

#[test]
fn ide_tools_follow_what_they_ask_for() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    let cfg = Config::default();
    let ide = |tool: &str, input: Value| classify(tool, &input, &cfg, root);
    match ide(
        "mcp__ide-tools__studio_grep",
        json!({"pattern": "foo.*bar", "path": root.join("src")}),
    ) {
        Search::Grep(o) => assert_eq!(o.scope.as_deref(), Some(root.join("src").as_path())),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        ide(
            "mcp__ide-tools__studio_find_declaration",
            json!({"symbol": "app.BookRoute"})
        ),
        Search::Symbol("BookRoute".into()),
        "a declaration tool asks for definitions"
    );
    assert_eq!(
        ide(
            "mcp__ide-tools__studio_code_search",
            json!({"terms": ["BookRoute"]})
        ),
        Search::Pass
    );
}

fn hook(stdin: &str, cwd: &Path, cache: &Path) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .arg("hook")
        .current_dir(cwd)
        .env("CODEMAP_CACHE_DIR", cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn hook_output(event: &Value, f: &common::Fixture) -> Value {
    let (code, stdout) = hook(&event.to_string(), &f.ws.project.root, f.cache.path());
    assert_eq!(code, 0);
    serde_json::from_str::<Value>(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout:?}"))
        ["hookSpecificOutput"]
        .clone()
}

#[test]
fn a_bash_search_is_rewritten_so_its_answer_arrives_as_ordinary_output() {
    // A deny reaches the agent labelled `PreToolUse:Bash hook error`. Rewriting the command makes the tool
    // run a harmless printer instead, so the answer looks like what the agent's own grep printed.
    let f = workspace(None);
    let event = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "grep -rn BookRoute src", "description": "find uses", "timeout": 5000},
        "cwd": f.ws.project.root,
    });
    let out = hook_output(&event, &f);
    assert_eq!(out["permissionDecision"], "allow");
    let updated = &out["updatedInput"];
    assert_eq!(updated["description"], "find uses");
    assert_eq!(updated["timeout"], 5000);
    let command = updated["command"].as_str().unwrap();
    assert!(!command.contains("grep -rn"), "{command}");
    let ran = Command::new("sh").arg("-c").arg(command).output().unwrap();
    let printed = String::from_utf8_lossy(&ran.stdout);
    assert!(ran.status.success());
    assert!(
        printed.contains("src/commonMain/kotlin/app/\n")
            && printed.contains("  BookRoute.kt:12: class BookRoute("),
        "{printed}"
    );
}

#[test]
fn the_rewritten_command_survives_quotes_and_odd_characters() {
    let f = workspace(None);
    let event = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "grep -rn \"it's $HOME `x` \\\" ; rm -rf /\" src"},
        "cwd": f.ws.project.root,
    });
    let out = hook_output(&event, &f);
    let command = out["updatedInput"]["command"].as_str().unwrap();
    let ran = Command::new("sh").arg("-c").arg(command).output().unwrap();
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(String::from_utf8_lossy(&ran.stdout).starts_with("# grep"));
}

#[test]
fn other_tools_run_untouched_unless_listed_and_then_get_a_deny() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    let refused = hook_output(
        &json!({"tool_name": "Bash", "tool_input": {"command": "find . -name *.kt"}, "cwd": root}),
        &f,
    );
    assert_eq!(refused["permissionDecision"], "deny");
    let grep = json!({"tool_name": "Grep", "tool_input": {"pattern": "BookRoute"}, "cwd": root});
    let ide = json!({"tool_name": "mcp__ide-tools__studio_grep", "tool_input": {"pattern": "BookRoute"}, "cwd": root});
    for event in [&grep, &ide] {
        assert_eq!(
            hook(&event.to_string(), root, f.cache.path()),
            (0, String::new()),
            "codemap is optional: {event}"
        );
    }
    let opted_in = workspace(Some(r#"{"searchTools": ["Grep"]}"#));
    let grep = json!({"tool_name": "Grep", "tool_input": {"pattern": "BookRoute", "output_mode": "content"}, "cwd": opted_in.ws.project.root});
    let out = hook_output(&grep, &opted_in);
    assert_eq!(out["permissionDecision"], "deny");
    assert!(
        out["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("BookRoute.kt:12"),
        "{out}"
    );
}

#[test]
fn hook_binary_exits_zero_on_malformed_stdin_and_outside_a_repository() {
    let f = workspace(None);
    for bad in ["", "not json", "{}", "[1,2]", "{\"tool_name\": 7}"] {
        assert_eq!(
            hook(bad, &f.ws.project.root, f.cache.path()),
            (0, String::new()),
            "stdin {bad:?}"
        );
    }
    let elsewhere = tempfile::tempdir().unwrap();
    let event = json!({"tool_name": "Bash", "tool_input": {"command": "grep -rn foo ."}, "cwd": elsewhere.path()});
    assert_eq!(
        hook(&event.to_string(), elsewhere.path(), f.cache.path()),
        (0, String::new())
    );
}

fn shell() -> &'static codemap::config::settings::Shell {
    &codemap::config::settings::builtin().shell
}

#[test]
fn read_only_codemap_queries_are_approved_and_nothing_else() {
    for command in [
        "codemap def BookRoute",
        "codemap uses BookRoute --lines",
        "codemap link '/api/v1/books/{id}'",
        "codemap def platformName; echo ---; codemap uses platformName",
        "codemap show A && codemap impl B",
        "codemap BookRoute",
        "codemap okhttp dependency",
        "codemap \"api/v1/books/{id}\" 2>&1 | head -80",
        "codemap definition BookRoute | grep -v test | wc -l",
    ] {
        assert!(
            codemap::agent::hook::codemap_query(command, shell()),
            "{command}"
        );
    }
    for command in [
        "codemap setup --agent all",
        "codemap self-update",
        "codemap def X | sh",
        "codemap def X | xargs rm",
        "codemap def X || rm y",
        "codemap update src/A.kt",
        "./codemap def X",
        "/tmp/elsewhere/codemap def X",
        "bin/codemap def X",
        "codemap logs /etc/passwd",
        "codemap log /etc/passwd",
        "codemap logcat /etc/passwd",
        "codemap summarize /etc/passwd",
        "codemap def X > out.txt",
        "codemap def $(rm -rf ~)",
        "codemap def X; rm -rf build",
        "echo hi",
        "notcodemap def X",
        "codemap def X\nrm y",
    ] {
        assert!(
            !codemap::agent::hook::codemap_query(command, shell()),
            "{command}"
        );
    }
    let f = workspace(None);
    let event = json!({"tool_name": "Bash", "tool_input": {"command": "codemap def BookRoute"}, "cwd": f.ws.project.root});
    let (_, stdout) = hook(&event.to_string(), &f.ws.project.root, f.cache.path());
    assert!(
        stdout.contains(r#""permissionDecision":"allow""#),
        "{stdout}"
    );
    assert!(!stdout.contains("updatedInput"), "{stdout}");
}
