//! PreToolUse hook: reproduce a search, answer it from the workspace in the same step, or let it pass.
//!
//! The hook reads a Claude Code tool event (`{"tool_name", "tool_input", "cwd"}`). A grep, egrep, rg or
//! git grep in Bash becomes the same search: the same regex semantics over the same files, gitignored
//! files excluded, ranked and capped, with the lines that start a definition marked. The answer is
//! returned in the same step, so no shell ever parses the command. Anything the hook cannot reproduce
//! exactly (an unknown option, PCRE, context or count output, a read of one file or of stdin) passes
//! through untouched, as does anything that is not a search.

mod classify;
mod shell;

use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{Value, json};

pub use classify::{Search, classify};

use crate::config::settings::Shell;
use crate::output::render;
use crate::query::find::{FindOptions, find, find_with};
use crate::query::{cap, def, uses};
use crate::workspace::{Workspace, discover};

/// Whether a Bash command is nothing but codemap queries (`codemap` from PATH, or this binary by its
/// absolute path): any codemap command except the ones that
/// write (`settings.shell.writeCommands`), so the bare `codemap <query>` and every alias count. They
/// may be joined by `;` or `&&` with `echo` separators, merge stderr (`2>&1`) and pipe into read-only
/// filters (`settings.shell.readOnlyFilters`, `| head -40`). Any other shell syntax (other redirects,
/// substitutions) disqualifies it.
pub fn codemap_query(command: &str, sh: &Shell) -> bool {
    let command = command.replace("2>&1", "");
    if command.contains(['>', '<', '$', '`', '(', ')', '\n', '\\']) {
        return false;
    }
    // The bare name, which the shell finds on PATH, or this very binary by its path. Any other path
    // is not ours: a repository can ship its own `./codemap`.
    let is_codemap = |exe: &str| {
        exe == env!("CARGO_PKG_NAME")
            || std::env::current_exe().is_ok_and(|own| {
                let asked = Path::new(exe);
                asked.is_absolute()
                    && asked.canonicalize().ok() == own.canonicalize().ok()
                    && asked.exists()
            })
    };
    let mut any = false;
    for part in command.split("&&").flat_map(|p| p.split(';')) {
        let mut stages = part.split('|');
        let Some(words) = stages.next().and_then(shlex::split) else {
            return false;
        };
        let filtered = stages.all(|stage| {
            shlex::split(stage)
                .and_then(|w| w.into_iter().next())
                .is_some_and(|filter| sh.read_only_filters.contains(&filter))
        });
        if !filtered {
            return false;
        }
        match words.as_slice() {
            [] => {}
            [echo, ..] if echo == "echo" => {}
            [exe, rest @ ..]
                if is_codemap(exe) && rest.first().is_none_or(|sub| !writes(sub, sh)) =>
            {
                any = true;
            }
            _ => return false,
        }
    }
    any
}

/// Whether a subcommand word is one of `writeCommands`, by the command it names: an alias (`log` for
/// `logs`) is the same command.
fn writes(sub: &str, sh: &Shell) -> bool {
    use clap::CommandFactory;
    let cli = crate::cli::Cli::command();
    let name = cli.find_subcommand(sub).map_or(sub, |c| c.get_name());
    sh.write_commands.iter().any(|w| w == name)
}

fn deny(reason: &str) -> String {
    json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}}).to_string()
}

fn symbol_answer(ws: &Workspace, name: &str) -> String {
    let mut definitions = def(ws, name);
    let mut references = uses(ws, name, false);
    let limit = ws.config.settings.limits.answer_lines;
    cap(&mut definitions, false, limit);
    cap(&mut references, false, limit);
    if definitions.lines.is_empty() && references.lines.is_empty() {
        let mut fallback = find(ws, &format!(r"\b{}\b", regex::escape(name)), None, false);
        fallback
            .notes
            .insert(0, "not in the index; text matches:".into());
        return render(&fallback);
    }
    let mut out = String::new();
    if !definitions.lines.is_empty() {
        definitions.source = "index:definitions".into();
        out.push_str(&render(&definitions));
    }
    if !references.lines.is_empty() {
        references.source = "index:uses".into();
        out.push_str(&render(&references));
    }
    out
}

/// A grep's matches; a cut says how many lines are not shown and how to see them with the same tool.
fn grep_answer(ws: &Workspace, options: &FindOptions) -> String {
    let mut answer = find_with(ws, options);
    let cut = std::mem::take(&mut answer.cut);
    let mut out = render(&answer);
    if cut > 0 {
        out.push_str(&format!(
            "... {cut} more lines; a narrower path or pattern shows them\n"
        ));
    }
    out
}

/// The scope relative to the project root (empty for the root itself); `None` when the folder is
/// outside the project.
fn relative_scope(ws: &Workspace, scope: &Path) -> Option<PathBuf> {
    let full = scope.canonicalize().ok()?;
    Some(full.strip_prefix(&ws.project.root).ok()?.to_path_buf())
}

/// The JSON a hook prints for `search`, or `None` when the search passes. `None` as workspace means the
/// working directory is not inside a repository. Always a `deny`; see [`respond_for`] for the form that
/// reaches the agent as ordinary output.
pub fn respond(search: &Search, ws: Option<&Workspace>) -> Option<String> {
    if let Search::Reject(message) = search {
        return Some(deny(message));
    }
    Some(deny(&answer_body(search, ws)?))
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

/// Like [`respond`], but for a Bash call the command is rewritten to print the answer. A `deny` reaches
/// the agent labelled `PreToolUse:Bash hook error` and invites a retry with another tool; a rewritten
/// command runs and its output arrives as the agent's own search result. The replacement is `exe emit
/// <base64>`: it prints text and does nothing else, so approving it is safe, and the original command
/// never runs. A refusal, and every other tool (a `Grep` call cannot become a different tool), stays a
/// `deny`.
pub fn respond_for(
    tool: &str,
    input: &Value,
    search: &Search,
    ws: Option<&Workspace>,
    exe: &Path,
) -> Option<String> {
    if tool != "Bash" || matches!(search, Search::Reject(_)) {
        return respond(search, ws);
    }
    let body = answer_body(search, ws)?;
    let mut updated = input.clone();
    updated["command"] = json!(format!(
        "{} emit {}",
        shell_quote(exe),
        base64::engine::general_purpose::STANDARD.encode(body)
    ));
    Some(
        json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": "answered by codemap",
            "updatedInput": updated
        }})
        .to_string(),
    )
}

/// The text answering `search`, `None` when it passes or needs a repository there is not.
fn answer_body(search: &Search, ws: Option<&Workspace>) -> Option<String> {
    let ws = ws?;
    let body = match search {
        Search::Pass | Search::Reject(_) => return None,
        Search::Symbol(name) => symbol_answer(ws, name),
        Search::Grep(options) => {
            let mut options = options.clone();
            options.scope = match &options.scope {
                Some(dir) => Some(relative_scope(ws, dir)?),
                None => None,
            };
            grep_answer(ws, &options)
        }
    };
    Some(body)
}

/// The whole hook: a tool event on stdin text, the response to print. Never fails; anything that is
/// not understood passes.
pub fn handle(stdin: &str, fallback_cwd: &Path) -> Option<String> {
    let event: Value = serde_json::from_str(stdin).ok()?;
    let tool = event.get("tool_name")?.as_str()?;
    let input = event.get("tool_input").cloned().unwrap_or(Value::Null);
    let cwd = event
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| fallback_cwd.to_path_buf());
    // A refusal needs no repository, so classification runs with the default configuration outside one.
    let ws = discover(&cwd).ok();
    let config = ws.as_ref().map(|w| w.config.clone()).unwrap_or_default();
    // Without this, every `codemap def` the session prompt asks for stops at an approval prompt, and
    // a headless session gives up on it.
    if tool == "Bash"
        && input["command"]
            .as_str()
            .is_some_and(|c| codemap_query(c, &config.settings.shell))
    {
        return Some(
            json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "permissionDecisionReason": "read-only codemap query"
            }})
            .to_string(),
        );
    }
    // codemap is optional: only Bash searches are answered (the agent still gets a search result);
    // a dedicated tool is taken over only when the repository lists it in `searchTools`.
    if tool != "Bash" && !config.search_tools.iter().any(|t| t == tool) {
        return None;
    }
    let search = classify(tool, &input, &config, &cwd);
    let exe = std::env::current_exe().ok()?;
    respond_for(tool, &input, &search, ws.as_ref(), &exe)
}
