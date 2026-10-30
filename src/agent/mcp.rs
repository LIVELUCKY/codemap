//! `codemap mcp`: a Model Context Protocol server over stdio, so agents without Claude-style hooks
//! (Codex, Gemini CLI, Antigravity, Cursor and any other MCP client) get the same index as tools.
//!
//! The protocol is rmcp's, the official Rust SDK; this file only lists the tools and answers them.
//! The list is short on purpose: every extra tool is one more choice a small model has to get right.
//! Every call discovers its workspace afresh, so answers follow the working tree like every other command.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};

use crate::output::{Answer, render};
use crate::query::find::{FindOptions, find_with};
use crate::query::{cap, impl_, uses};
use crate::workspace::{Workspace, discover};

/// Finds the workspace of a directory; a parameter so tests can point it at a fixture cache.
pub type Open<'a> = &'a dyn Fn(&Path) -> anyhow::Result<Workspace>;

fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
) -> Tool {
    let mut properties = properties;
    properties["directory"] = json!({"type": "string", "description": "A folder inside the repository to work in; defaults to the server's working directory."});
    properties["all"] = json!({"type": "boolean", "description": "Every line of an answer that ended with `... N more (--all)`."});
    let schema = json!({"type": "object", "properties": properties, "required": required});
    let Value::Object(schema) = schema else {
        unreachable!()
    };
    Tool::new(name, description, Arc::new(schema))
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

struct Server {
    cwd: PathBuf,
    instructions: String,
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(self.instructions.clone())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let (text, is_error) = call(&request.name, &arguments, &self.cwd, &discover);
        let content = vec![ContentBlock::text(text)];
        Ok(if is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        }
        .into())
    }
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
    let server = Server {
        cwd: cwd.to_path_buf(),
        instructions,
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            server
                .serve(rmcp::transport::stdio())
                .await?
                .waiting()
                .await?;
            anyhow::Ok(())
        })
}
