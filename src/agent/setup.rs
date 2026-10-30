//! `codemap setup`: registers the codemap MCP server with coding agents, for every project (global) or
//! for one repository (project). Claude Code gets the plugin instead, which adds the search hook.
//!
//! Only the `codemap` entry of a config file is touched: other keys keep their order, an entry the user
//! already has is left alone, the previous file is kept as `<file>.codemap.bak`, and a file that is not
//! plain JSON (Zed allows comments) is not rewritten; the entry is printed to add by hand instead.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::config::settings::{Agent, expand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Scope {
    /// Every project of this user.
    Global,
    /// The current repository; the files can be committed for the team.
    Project,
}

/// How an agent's config file holds an MCP server; the agents themselves are `settings.agents`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// Claude Code: the plugin, installed with the `claude` CLI, instead of a file.
    ClaudePlugin,
    /// `{"mcpServers": {"codemap": {"command", "args"}}}`.
    McpServers,
    /// Copilot CLI: `mcpServers` with `"type": "local"` and a tool list.
    CopilotCli,
    /// VS Code: `servers` with `"type": "stdio"`.
    Vscode,
    /// Zed: `context_servers` in its settings.
    Zed,
    /// Codex: a `[mcp_servers.codemap]` table in TOML.
    CodexToml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub agent: String,
    pub path: PathBuf,
    format: Format,
}

/// Where `agent` keeps its MCP servers for `scope`; `None` for the Claude plugin and for a scope the
/// agent does not have.
pub fn target(
    agent: &Agent,
    scope: Scope,
    home: &Path,
    config: &Path,
    project: &Path,
) -> Option<Target> {
    if agent.format == Format::ClaudePlugin {
        return None;
    }
    let path = match scope {
        Scope::Global => expand(agent.global.as_deref()?, home, config),
        Scope::Project => project.join(agent.project.as_deref()?),
    };
    Some(Target {
        agent: agent.name.clone(),
        path,
        format: agent.format,
    })
}

/// Whether `agent` looks installed: its `detect` folder exists.
pub fn detected(agent: &Agent, home: &Path, config: &Path) -> bool {
    expand(&agent.detect, home, config).is_dir()
}

/// `owner/name` of the repository codemap is published from, as `Cargo.toml` names it.
pub fn repository() -> &'static str {
    env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/")
}

/// `<plugin>@<marketplace>`, from the marketplace file the repository ships.
pub fn plugin_id() -> String {
    let marketplace: Value =
        serde_json::from_str(include_str!("../../.claude-plugin/marketplace.json"))
            .unwrap_or_default();
    format!(
        "{}@{}",
        env!("CARGO_PKG_NAME"),
        marketplace["name"]
            .as_str()
            .unwrap_or(env!("CARGO_PKG_NAME"))
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Added,
    /// A `codemap` entry was already there and was left as it is.
    Present,
    /// The file could not be edited safely; add this by hand.
    Manual(String),
}

fn entry(format: Format, command: &str) -> Value {
    match format {
        Format::CopilotCli => {
            json!({"type": "local", "command": command, "args": ["mcp"], "tools": ["*"]})
        }
        Format::Vscode => json!({"type": "stdio", "command": command, "args": ["mcp"]}),
        Format::Zed => json!({"command": command, "args": ["mcp"], "env": {}}),
        Format::McpServers | Format::CodexToml | Format::ClaudePlugin => {
            json!({"command": command, "args": ["mcp"]})
        }
    }
}

fn key(format: Format) -> &'static str {
    match format {
        Format::Vscode => "servers",
        Format::Zed => "context_servers",
        _ => "mcpServers",
    }
}

fn codex_table(command: &str) -> String {
    format!(
        "[mcp_servers.codemap]\ncommand = {}\nargs = [\"mcp\"]\n",
        toml::Value::String(command.to_string())
    )
}

fn write_with_backup(path: &Path, text: &str) -> Result<()> {
    if path.exists() {
        let backup = PathBuf::from(format!("{}.codemap.bak", path.display()));
        fs::copy(path, &backup).with_context(|| format!("backing up {}", path.display()))?;
    } else if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Adds the codemap server to `target`, started as `command mcp`.
pub fn apply(target: &Target, command: &str, dry_run: bool) -> Result<Outcome> {
    let existing = match fs::read_to_string(&target.path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e).with_context(|| format!("reading {}", target.path.display())),
    };
    if target.format == Format::CodexToml {
        let text = existing.unwrap_or_default();
        if text.contains("[mcp_servers.codemap]") {
            return Ok(Outcome::Present);
        }
        let separator = if text.is_empty() || text.ends_with("\n\n") {
            ""
        } else if text.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        if !dry_run {
            write_with_backup(
                &target.path,
                &format!("{text}{separator}{}", codex_table(command)),
            )?;
        }
        return Ok(Outcome::Added);
    }
    let server = entry(target.format, command);
    let manual = || {
        Outcome::Manual(
            serde_json::to_string_pretty(&json!({key(target.format): {"codemap": server}}))
                .unwrap_or_default(),
        )
    };
    let mut root: Value = match existing.as_deref().map(str::trim) {
        None | Some("") => Value::Object(Map::new()),
        Some(text) => match serde_json::from_str(text) {
            Ok(value) => value,
            Err(_) => return Ok(manual()),
        },
    };
    let Some(object) = root.as_object_mut() else {
        return Ok(manual());
    };
    let servers = object
        .entry(key(target.format))
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else {
        return Ok(manual());
    };
    if servers.contains_key("codemap") {
        return Ok(Outcome::Present);
    }
    servers.insert("codemap".into(), server.clone());
    if !dry_run {
        write_with_backup(
            &target.path,
            &format!("{}\n", serde_json::to_string_pretty(&root)?),
        )?;
    }
    Ok(Outcome::Added)
}

/// What `codemap setup` was asked to do.
pub struct Options {
    /// Agent names, `all`, or empty for the detected ones.
    pub agents: Vec<String>,
    pub scope: Scope,
    /// The user's login shell (`$SHELL`), for Tab completion.
    pub shell: Option<String>,
    pub dry_run: bool,
}

/// Where `setup` writes; parameters so tests can use a temporary home.
pub struct Places {
    pub home: PathBuf,
    pub config: PathBuf,
    /// The repository root, required for the project scope.
    pub project: Option<PathBuf>,
    /// This binary, used as the server command in global configs. Project configs say `codemap` so
    /// they work on every machine of the team.
    pub exe: PathBuf,
    /// The user's codemap config file.
    pub global_config: PathBuf,
    /// The agents `setup` knows (`settings.agents`).
    pub agents: Vec<Agent>,
}

fn claude_steps(scope: Scope) -> [Vec<String>; 2] {
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "project",
    };
    let id = plugin_id();
    [
        ["claude", "plugin", "marketplace", "add", repository()]
            .map(String::from)
            .to_vec(),
        ["claude", "plugin", "install", &id, "-s", scope]
            .map(String::from)
            .to_vec(),
    ]
}

fn claude(scope: Scope, dry_run: bool) -> String {
    let steps = claude_steps(scope);
    let shown: Vec<String> = steps.iter().map(|s| s.join(" ")).collect();
    if dry_run {
        return format!("claude: would run {}", shown.join(" && "));
    }
    let run = |step: &Vec<String>| {
        Command::new(&step[0])
            .args(&step[1..])
            .output()
            .is_ok_and(|o| o.status.success())
    };
    // Adding the marketplace is refused when Claude Code already knows one of that name from
    // another source (a local clone, for example). The plugin installs from the known one, so only
    // a failed install is reported, with the first command that did not run.
    let added = run(&steps[0]);
    if run(&steps[1]) {
        return "claude: plugin installed".into();
    }
    let failed = if added { &steps[1] } else { &steps[0] };
    format!(
        "claude: could not run `{}`; run it yourself",
        failed.join(" ")
    )
}

/// Runs the setup and reports one line per action. Nothing here fails the whole run: an agent that
/// cannot be configured is reported and the next one is tried.
pub fn run(options: &Options, places: &Places) -> Vec<String> {
    let mut report = Vec::new();
    let project = match (options.scope, &places.project) {
        (Scope::Project, None) => {
            return vec![
                "the project scope needs a git repository; run it inside one or use --scope global"
                    .into(),
            ];
        }
        (_, project) => project.clone().unwrap_or_default(),
    };
    let command = match options.scope {
        Scope::Global => places.exe.to_string_lossy().into_owned(),
        Scope::Project => "codemap".into(),
    };
    let known: Vec<&str> = places.agents.iter().map(|a| a.name.as_str()).collect();
    let wanted: Vec<&str> = if options.agents.iter().any(|a| a == "all") {
        known.clone()
    } else if options.agents.is_empty() {
        places
            .agents
            .iter()
            .filter(|a| detected(a, &places.home, &places.config))
            .map(|a| a.name.as_str())
            .collect()
    } else {
        options.agents.iter().map(String::as_str).collect()
    };
    if wanted.is_empty() {
        report.push("no agent detected; name them with --agent (see --help)".into());
    }
    for name in wanted {
        let Some(agent) = places.agents.iter().find(|a| a.name == name) else {
            report.push(format!("{name}: unknown; known: {}", known.join(", ")));
            continue;
        };
        if agent.format == Format::ClaudePlugin {
            report.push(claude(options.scope, options.dry_run));
            continue;
        }
        let Some(target) = target(agent, options.scope, &places.home, &places.config, &project)
        else {
            report.push(format!("{name}: has no project config; use --scope global"));
            continue;
        };
        let path = target.path.display();
        let added = if options.dry_run {
            "would add to"
        } else {
            "added to"
        };
        report.push(match apply(&target, &command, options.dry_run) {
            Ok(Outcome::Added) => format!("{name}: {added} {path}"),
            Ok(Outcome::Present) => format!("{name}: already in {path}"),
            Ok(Outcome::Manual(snippet)) => {
                format!("{name}: {path} is not plain JSON; add this by hand:\n{snippet}")
            }
            Err(e) => format!("{name}: {e:#}"),
        });
    }
    if options.scope == Scope::Global {
        report.push(completions(
            options.shell.as_deref(),
            &places.home,
            &places.exe,
            options.dry_run,
        ));
    }
    if options.dry_run {
        report.push("dry run: nothing was written".into());
    }
    report
}

/// Tab completion where the shell loads it by itself: a one-line file that asks this binary for the
/// script, so it never goes stale after an update. zsh has no such folder by default, so it gets the
/// line for `~/.zshrc` instead of an edit there.
fn completions(shell: Option<&str>, home: &Path, exe: &Path, dry_run: bool) -> String {
    let name = shell
        .and_then(|s| Path::new(s).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let exe = exe.to_string_lossy().into_owned();
    let exe = shlex::try_quote(&exe)
        .map(|q| q.into_owned())
        .unwrap_or(exe);
    let (path, stub) = match name {
        "fish" => (
            home.join(".config/fish/completions/codemap.fish"),
            format!("{exe} completions fish | source\n"),
        ),
        "bash" => (
            home.join(".local/share/bash-completion/completions/codemap"),
            format!("source <({exe} completions bash)\n"),
        ),
        "zsh" => {
            return "completions: add `eval \"$(codemap completions zsh)\"` to ~/.zshrc".into();
        }
        _ => return "completions: `codemap completions <shell>` prints the script".into(),
    };
    if dry_run {
        return format!("completions: would write {}", path.display());
    }
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&path, stub));
    match written {
        Ok(()) => format!("completions: {} ({name})", path.display()),
        Err(e) => format!("completions: {}: {e}", path.display()),
    }
}
