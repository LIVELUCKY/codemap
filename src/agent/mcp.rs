//! `codemap mcp`: a Model Context Protocol server over stdio, so agents without Claude-style hooks
//! (Codex, Gemini CLI, Antigravity, Cursor and any other MCP client) get the same index as tools.
//!
//! The protocol is JSON-RPC, one message per line; a server that only offers tools needs four
//! methods of it, which are answered here. The list is short on purpose: every extra tool is one more choice a small model has to get right.
//! Every call discovers its workspace afresh, so answers follow the working tree like every other command.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use crate::output::{Answer, render};
use crate::query::find::{FindOptions, find_with};
use crate::query::{cap, impl_, uses};
use crate::workspace::{Workspace, discover};

/// Finds the workspace of a directory; a parameter so tests can point it at a fixture cache.
pub type Open<'a> = &'a dyn Fn(&Path) -> anyhow::Result<Workspace>;

/// The protocol version answered when the client asks for one this server does not know.
const PROTOCOL: &str = "2025-06-18";

/// One tool as `tools/list` describes it.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
) -> Tool {
    let mut properties = properties;
    properties["directory"] = json!({"type": "string", "description": "A folder inside the repository to work in; defaults to the server's working directory."});
    properties["all"] = json!({"type": "boolean", "description": "Every line of an answer that ended with `... N more (--all)`."});
    Tool {
        name,
        description,
        input_schema: json!({"type": "object", "properties": properties, "required": required}),
    }
}

pub fn tools() -> Vec<Tool> {
    let name = |what: &str| json!({"name": {"type": "string", "description": what}});
    vec![
        tool(
            "codemap_lookup",
            "Find code: where something is defined, with its code when it has one definition. Takes a name, part of a name, a file name, a route or URL, or a few words describing it.",
            json!({"query": {"type": "string", "description": "For example BookRoute, bookrout, build.gradle.kts, /api/v1/books, or token refresh."}}),
            &["query"],
        ),
        tool(
            "codemap_uses",
            "Who uses a name: calls and references, from every language.",
            name("The name."),
            &["name"],
        ),
        tool(
            "codemap_impl",
            "What extends or implements a class or interface.",
            json!({"type": {"type": "string", "description": "The interface or base class."}}),
            &["type"],
        ),
        tool(
            "codemap_grep",
            "Search every file for exact text or a regular expression.",
            json!({
                "pattern": {"type": "string", "description": "A regular expression."},
                "path": {"type": "string", "description": "Limit to this folder of the repository."},
                "ignore_case": {"type": "boolean"},
                "files_only": {"type": "boolean", "description": "One line per file."},
                "include": {"type": "array", "items": {"type": "string"}, "description": "Globs a file must match, for example *.kt."},
                "exclude": {"type": "array", "items": {"type": "string"}, "description": "Globs to leave out."}
            }),
            &["pattern"],
        ),
    ]
}

fn capped(mut answer: Answer, ws: &Workspace, all: bool) -> String {
    cap(&mut answer, all, ws.config.settings.limits.answer_lines);
    render(&answer)
}

fn string(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("missing argument `{key}`"))
}

fn strings(arguments: &Value, key: &str) -> Vec<String> {
    arguments
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn flag(arguments: &Value, key: &str) -> bool {
    arguments.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn run_tool(name: &str, arguments: &Value, ws: &Workspace) -> Result<String, String> {
    let all = flag(arguments, "all");
    Ok(match name {
        "codemap_lookup" => {
            let query = string(arguments, "query")?;
            crate::query::show::lookup(ws, &if all { query + " --all" } else { query })
        }
        "codemap_uses" => capped(uses(ws, &string(arguments, "name")?, false), ws, all),
        "codemap_impl" => capped(impl_(ws, &string(arguments, "type")?), ws, all),
        "codemap_grep" => render(&find_with(
            ws,
            &FindOptions {
                pattern: string(arguments, "pattern")?,
                scope: arguments
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from),
                ignore_case: flag(arguments, "ignore_case"),
                files_only: flag(arguments, "files_only"),
                include: strings(arguments, "include"),
                exclude: strings(arguments, "exclude"),
                all,
                ..FindOptions::default()
            },
        )),
        other => return Err(format!("unknown tool `{other}`")),
    })
}

/// The text of one tool call and whether it is an error. A `directory` argument picks the workspace.
pub fn call(name: &str, arguments: &Value, cwd: &Path, open: Open) -> (String, bool) {
    let directory = arguments
        .get("directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.to_path_buf());
    let ws = match open(&directory) {
        Ok(ws) => ws,
        Err(e) => return (e.to_string(), true),
    };
    match run_tool(name, arguments, &ws) {
        Ok(text) => (text, false),
        Err(message) => (message, true),
    }
}

/// The reply to one message, `None` for a notification: `initialize`, `ping`, `tools/list` and
/// `tools/call` are answered, any other request is an unknown method.
pub fn respond(message: &Value, cwd: &Path, instructions: &str, open: Open) -> Option<Value> {
    let id = message.get("id").filter(|id| !id.is_null())?.clone();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            json!({
                "protocolVersion": asked.unwrap_or(PROTOCOL),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
                "instructions": instructions
            })
        }
        Some("ping") => json!({}),
        Some("tools/list") => json!({"tools": tools().iter().map(|t| json!({
            "name": t.name, "description": t.description, "inputSchema": t.input_schema
        })).collect::<Vec<_>>()}),
        Some("tools/call") => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let (text, is_error) = call(name, &arguments, cwd, open);
            json!({"content": [{"type": "text", "text": text}], "isError": is_error})
        }
        _ => {
            return Some(json!({"jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": "method not found"}}));
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Indexes the workspace inside the start budget, then serves until stdin closes.
pub fn serve(cwd: &Path) -> anyhow::Result<()> {
    let ws = discover(cwd).ok();
    let settings = ws
        .as_ref()
        .map(|w| &w.config.settings)
        .unwrap_or(crate::config::settings::builtin());
    if let Some(ws) = &ws {
        let budget = Duration::from_secs_f64(settings.limits.start_budget_seconds);
        let _ = crate::index::build(ws, false, Some(budget));
    }
    let instructions = match &ws {
        Some(ws) => ws.prompt(&settings.prompts.mcp),
        None => settings
            .prompts
            .mcp
            .replace("{repositories}", "the repository you work in"),
    };
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => respond(&message, cwd, &instructions, &discover),
            Err(_) => Some(json!({"jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": "parse error"}})),
        };
        if let Some(reply) = reply {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
