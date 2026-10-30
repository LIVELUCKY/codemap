//! `codemap setup` against a temporary home: every agent's file, merging, and the project scope.

use std::fs;
use std::path::Path;

use codemap::agent::setup::{Options, Outcome, Places, Scope, apply, run, target};
use codemap::config::settings::{Agent, builtin};
use serde_json::Value;

fn agent(name: &str) -> &'static Agent {
    builtin().agents.iter().find(|a| a.name == name).unwrap()
}

fn places(dir: &Path, project: bool) -> Places {
    Places {
        home: dir.join("home"),
        config: dir.join("config"),
        project: project.then(|| dir.join("repo")),
        exe: "/opt/bin/codemap".into(),
        global_config: dir.join("config/codemap/config.json"),
        agents: builtin().agents.clone(),
    }
}

fn options(agents: &[&str], scope: Scope) -> Options {
    Options {
        agents: agents.iter().map(|a| a.to_string()).collect(),
        scope,
        shell: None,
        dry_run: false,
    }
}

#[test]
fn global_setup_installs_tab_completion_for_the_login_shell() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), false);
    let mut o = options(&["none-of-them"], Scope::Global);
    o.shell = Some("/opt/homebrew/bin/fish".into());
    run(&o, &p);
    assert_eq!(
        fs::read_to_string(p.home.join(".config/fish/completions/codemap.fish")).unwrap(),
        "/opt/bin/codemap completions fish | source\n",
        "a stub that asks the binary, so it follows updates"
    );
    o.shell = Some("/bin/bash".into());
    run(&o, &p);
    assert!(
        p.home
            .join(".local/share/bash-completion/completions/codemap")
            .exists()
    );
    o.shell = Some("/bin/zsh".into());
    let report = run(&o, &p);
    assert!(report.iter().any(|l| l.contains("~/.zshrc")), "{report:?}");
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn global_setup_writes_each_agents_own_format() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), false);
    let agents: Vec<&str> = builtin()
        .agents
        .iter()
        .map(|a| a.name.as_str())
        .filter(|a| *a != "claude")
        .collect();
    let report = run(&options(&agents, Scope::Global), &p);
    assert!(
        report
            .iter()
            .filter(|l| !l.starts_with("completions:"))
            .all(|l| l.contains(": added to ")),
        "{report:?}"
    );
    let home = &p.home;
    assert_eq!(
        json(&home.join(".cursor/mcp.json"))["mcpServers"]["codemap"]["command"],
        "/opt/bin/codemap"
    );
    assert_eq!(
        json(&home.join(".cursor/mcp.json"))["mcpServers"]["codemap"]["args"][0],
        "mcp"
    );
    assert_eq!(
        json(&home.join(".copilot/mcp-config.json"))["mcpServers"]["codemap"]["type"],
        "local"
    );
    assert_eq!(
        json(&p.config.join("Code/User/mcp.json"))["servers"]["codemap"]["type"],
        "stdio"
    );
    assert!(
        json(&home.join(".config/zed/settings.json"))["context_servers"]["codemap"].is_object()
    );
    for file in [
        ".codeium/windsurf/mcp_config.json",
        ".gemini/settings.json",
        ".gemini/config/mcp_config.json",
        ".junie/mcp/mcp.json",
    ] {
        assert!(
            json(&home.join(file))["mcpServers"]["codemap"].is_object(),
            "{file}"
        );
    }
    let codex = fs::read_to_string(home.join(".codex/config.toml")).unwrap();
    let parsed: toml::Table = codex.parse().unwrap();
    assert_eq!(
        parsed["mcp_servers"]["codemap"]["command"].as_str(),
        Some("/opt/bin/codemap")
    );
}

#[test]
fn existing_settings_are_kept_in_order_and_backed_up_and_a_second_run_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), false);
    let file = p.home.join(".gemini/settings.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let before = r#"{"theme": "dark", "mcpServers": {"other": {"command": "x"}}, "zzz": 1}"#;
    fs::write(&file, before).unwrap();
    run(&options(&["gemini"], Scope::Global), &p);
    let after = fs::read_to_string(&file).unwrap();
    let keys: Vec<String> = json(&file).as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["theme", "mcpServers", "zzz"], "{after}");
    assert!(json(&file)["mcpServers"]["other"].is_object());
    assert_eq!(
        fs::read_to_string(format!("{}.codemap.bak", file.display())).unwrap(),
        before
    );
    let again = run(&options(&["gemini"], Scope::Global), &p);
    assert!(again[0].contains("already in"), "{again:?}");
    assert_eq!(fs::read_to_string(&file).unwrap(), after);

    let codex = p.home.join(".codex/config.toml");
    fs::create_dir_all(codex.parent().unwrap()).unwrap();
    fs::write(&codex, "# mine\nmodel = \"o4\"\n").unwrap();
    run(&options(&["codex"], Scope::Global), &p);
    run(&options(&["codex"], Scope::Global), &p);
    let text = fs::read_to_string(&codex).unwrap();
    assert!(
        text.starts_with("# mine\nmodel = \"o4\"\n\n[mcp_servers.codemap]"),
        "{text}"
    );
    assert_eq!(text.matches("[mcp_servers.codemap]").count(), 1);
}

#[test]
fn a_file_with_comments_is_left_alone_and_the_entry_printed() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), false);
    let file = p.home.join(".config/zed/settings.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "// my settings\n{\"theme\": \"One\"}\n").unwrap();
    let t = target(
        agent("zed"),
        Scope::Global,
        &p.home,
        &p.config,
        Path::new("/"),
    )
    .unwrap();
    match apply(&t, "codemap", false).unwrap() {
        Outcome::Manual(snippet) => assert!(snippet.contains("context_servers"), "{snippet}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "// my settings\n{\"theme\": \"One\"}\n"
    );
}

#[test]
fn project_scope_writes_into_the_repository_with_a_portable_command() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), true);
    let repo = dir.path().join("repo");
    let report = run(
        &options(&["cursor", "vscode", "windsurf", "zed"], Scope::Project),
        &p,
    );
    assert_eq!(
        json(&repo.join(".cursor/mcp.json"))["mcpServers"]["codemap"]["command"],
        "codemap"
    );
    assert!(json(&repo.join(".vscode/mcp.json"))["servers"]["codemap"].is_object());
    assert!(
        report[2].starts_with("windsurf: has no project config"),
        "{report:?}"
    );
    assert!(
        report[3].starts_with("zed: has no project config"),
        "{report:?}"
    );
    let outside = places(dir.path(), false);
    assert!(
        run(&options(&["cursor"], Scope::Project), &outside)[0].contains("needs a git repository")
    );
}

#[test]
fn dry_run_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = places(dir.path(), false);
    let mut o = options(&["cursor", "claude"], Scope::Global);
    o.dry_run = true;
    let report = run(&o, &p);
    assert!(!p.home.exists(), "{report:?}");
    assert!(
        report[1].contains("claude plugin install codemap@codemap -s user"),
        "{report:?}"
    );
    assert_eq!(report.last().unwrap(), "dry run: nothing was written");
}
