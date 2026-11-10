//! `codemap mcp`: the MCP server other agents (Codex, Gemini, Antigravity, Cursor) talk to.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use codemap::workspace::discover;
use common::{Fixture, workspace};
use serde_json::{Value, json};

/// `codemap::agent::mcp::call` with workspaces opened against the fixture's cache.
fn call_in(f: &Fixture, cwd: &Path, name: &str, arguments: Value) -> (String, bool) {
    let open = |dir: &Path| {
        let mut ws = discover(dir)?;
        ws.cache_dir = f.cache.path().to_path_buf();
        Ok(ws)
    };
    codemap::agent::mcp::call(name, &arguments, cwd, &open)
}

fn call(f: &Fixture, name: &str, arguments: Value) -> String {
    let (text, is_error) = call_in(f, &f.ws.project.root, name, arguments);
    assert!(!is_error, "{text}");
    text
}

#[test]
fn the_tool_list_is_short_and_every_tool_has_a_schema() {
    let tools = codemap::agent::mcp::tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    assert_eq!(
        names,
        [
            "codemap_lookup",
            "codemap_uses",
            "codemap_impl",
            "codemap_grep"
        ]
    );
    for tool in &tools {
        assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
        assert!(tool.description.len() > 20, "{}", tool.name);
    }
}

#[test]
fn tool_calls_answer_from_the_index() {
    let f = workspace(None);
    let found = call(&f, "codemap_lookup", json!({"query": "BookRoute"}));
    assert!(
        found.contains("src/commonMain/kotlin/app/BookRoute.kt:12"),
        "{found}"
    );
    let grep = call(
        &f,
        "codemap_grep",
        json!({"pattern": "okhttp", "ignore_case": true, "files_only": true, "include": ["*.pro"]}),
    );
    assert!(
        grep.contains("proguard-rules.pro:1 file 2 matches"),
        "{grep}"
    );
    let words = call(
        &f,
        "codemap_lookup",
        json!({"query": "where is the book route defined"}),
    );
    assert!(words.contains("BookRoute.kt"), "{words}");
}

#[test]
fn all_lifts_the_cap_the_answer_names() {
    let f = workspace(None);
    for (tool, arguments) in [
        ("codemap_lookup", json!({"query": "same"})),
        ("codemap_grep", json!({"pattern": "fun same"})),
    ] {
        let capped = call(&f, tool, arguments.clone());
        assert!(capped.contains(" more (--all)"), "{tool}: {capped}");
        let mut arguments = arguments;
        arguments["all"] = json!(true);
        let all = call(&f, tool, arguments);
        assert!(!all.contains(" more (--all)"), "{tool}: {all}");
    }
}

#[test]
fn bad_calls_are_tool_errors_not_crashes() {
    let f = workspace(None);
    let root = f.ws.project.root.clone();
    assert!(call_in(&f, &root, "codemap_nope", json!({})).1);
    let (missing, is_error) = call_in(&f, &root, "codemap_lookup", json!({}));
    assert!(is_error);
    assert!(missing.contains("query"), "{missing}");
    let outside = tempfile::tempdir().unwrap();
    let (text, is_error) = call_in(&f, outside.path(), "codemap_lookup", json!({"query": "X"}));
    assert!(is_error);
    assert!(text.contains("not a git repository"), "{text}");
}

#[test]
fn a_directory_argument_picks_the_workspace() {
    let f = workspace(None);
    let elsewhere = tempfile::tempdir().unwrap();
    let (text, _) = call_in(
        &f,
        elsewhere.path(),
        "codemap_lookup",
        json!({"query": "BookRoute", "directory": f.ws.project.root}),
    );
    assert!(text.contains("BookRoute.kt:12"), "{text}");
}

#[test]
fn the_binary_speaks_mcp_over_stdio() {
    let f = workspace(None);
    let mut child = Command::new(env!("CARGO_BIN_EXE_codemap"))
        .arg("mcp")
        .current_dir(&f.ws.project.root)
        .env("CODEMAP_CACHE_DIR", f.cache.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut send = |message: Value| writeln!(stdin, "{message}").unwrap();
    let mut reply = || {
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}),
    );
    let init = reply();
    assert_eq!(init["result"]["serverInfo"]["name"], "codemap", "{init}");
    assert!(
        init["result"]["instructions"]
            .as_str()
            .is_some_and(|i| i.contains("codemap_lookup")),
        "{init}"
    );
    send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    send(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "codemap_lookup", "arguments": {"query": "BookRoute"}}}),
    );
    let answer = reply();
    assert_eq!(answer["id"], 2, "{answer}");
    assert!(
        answer["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("BookRoute.kt:12")),
        "{answer}"
    );
    send(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}));
    let listed = reply();
    assert_eq!(
        listed["result"]["tools"][0]["name"], "codemap_lookup",
        "{listed}"
    );
    assert_eq!(
        listed["result"]["tools"][0]["inputSchema"]["type"], "object",
        "{listed}"
    );
    send(json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list"}));
    assert_eq!(
        reply()["error"]["code"],
        -32601,
        "an unknown method is said to be one"
    );
    drop(send);
    child.kill().ok();
    child.wait().ok();
}
