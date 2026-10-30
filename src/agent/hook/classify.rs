//! What a tool call is searching for, read from the input of each tool shape. A search is taken as
//! written: a grep becomes the same grep, and anything that cannot be reproduced exactly passes.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::shell::{bash_search, glob_rejection, parse_grep, unquoted_glob_words};
use crate::config::Config;
use crate::config::settings::Shell;
use crate::query::find::{FindOptions, type_globs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Search {
    /// A grep with the matches the real tool prints, gitignored files excluded. The scope is the
    /// absolute folder searched.
    Grep(FindOptions),
    /// A name an IDE symbol tool asks about: its definitions and uses from the index.
    Symbol(String),
    /// A command that would fail in the shell, refused with the reason and the fix.
    Reject(String),
    /// Not ours to answer.
    Pass,
}

/// The folder a search is aimed at, `cwd` without a path; `None` for a file (the caller reads it) or
/// a path that does not exist (the tool reports the error).
fn folder(path: Option<&str>, cwd: &Path) -> Option<PathBuf> {
    let Some(path) = path.filter(|p| !p.is_empty()) else {
        return Some(cwd.to_path_buf());
    };
    let full = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };
    full.is_dir().then_some(full)
}

fn int(input: &Value, key: &str) -> bool {
    input
        .get(key)
        .and_then(Value::as_i64)
        .is_some_and(|n| n > 0)
}

fn text(input: &Value, key: &str) -> Option<String> {
    input.get(key).and_then(Value::as_str).map(str::to_string)
}

fn truthy(input: &Value, key: &str) -> bool {
    input.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Claude's Grep is ripgrep: its pattern is a Rust regex as written.
fn grep_tool(input: &Value, cwd: &Path) -> Search {
    let Some(pattern) = text(input, "pattern") else {
        return Search::Pass;
    };
    if ["-A", "-B", "-C", "context", "offset"]
        .iter()
        .any(|k| int(input, k))
        || truthy(input, "multiline")
    {
        return Search::Pass;
    }
    let files_only = match input.get("output_mode").and_then(Value::as_str) {
        None | Some("files_with_matches") => true,
        Some("content") => false,
        Some(_) => return Search::Pass,
    };
    let Some(scope) = folder(input.get("path").and_then(Value::as_str), cwd) else {
        return Search::Pass;
    };
    let mut options = FindOptions {
        pattern,
        scope: Some(scope),
        ignore_case: truthy(input, "-i"),
        files_only,
        ..FindOptions::default()
    };
    if let Some(glob) = text(input, "glob") {
        match glob.strip_prefix('!') {
            Some(negated) => options.exclude.push(negated.to_string()),
            None => options.include.push(glob),
        }
    }
    if let Some(kind) = text(input, "type") {
        match type_globs(&kind) {
            Some(globs) => options.include.extend(globs),
            None => return Search::Pass,
        }
    }
    Search::Grep(options)
}

fn studio_grep(input: &Value, cwd: &Path) -> Search {
    let Some(pattern) = text(input, "pattern") else {
        return Search::Pass;
    };
    if ["after", "before", "context"].iter().any(|k| int(input, k)) {
        return Search::Pass;
    }
    let path = ["path", "absolutePath"]
        .iter()
        .find_map(|k| input.get(*k).and_then(Value::as_str));
    let Some(scope) = folder(path, cwd) else {
        return Search::Pass;
    };
    let mut options = FindOptions {
        pattern,
        scope: Some(scope),
        ..FindOptions::default()
    };
    for key in ["include", "fileFilter"] {
        options.include.extend(text(input, key));
    }
    options.exclude.extend(text(input, "exclude"));
    Search::Grep(options)
}

fn studio_symbol(input: &Value) -> Search {
    let Some(symbol) = text(input, "symbol") else {
        return Search::Pass;
    };
    let has_context = ["contextFile", "contextSnippet"]
        .iter()
        .any(|k| input.get(*k).is_some_and(|v| !v.is_null()))
        || ["after", "before", "context"].iter().any(|k| int(input, k));
    if has_context || symbol.starts_with("R.") {
        return Search::Pass;
    }
    let name = symbol.rsplit(['#', '.']).next().unwrap_or(&symbol);
    if name.is_empty() {
        Search::Pass
    } else {
        Search::Symbol(name.to_string())
    }
}

fn bash_tool(input: &Value, cwd: &Path, sh: &Shell) -> Search {
    let Some(command) = text(input, "command") else {
        return Search::Pass;
    };
    let search = bash_grep(&command, cwd, sh).unwrap_or(Search::Pass);
    if search == Search::Pass {
        let globs = unquoted_glob_words(&command, sh);
        if !globs.is_empty() {
            return Search::Reject(glob_rejection(&globs));
        }
    }
    search
}

/// A Bash grep, egrep, rg or git grep as the same search; `None` when it reads input or a file, or
/// carries anything the hook does not reproduce.
fn bash_grep(command: &str, cwd: &Path, sh: &Shell) -> Option<Search> {
    let (program, args) = bash_search(command, sh)?;
    let grep = parse_grep(&program, &args)?;
    if !grep.recursive {
        return None;
    }
    let pattern = grep.regex()?;
    let scope = match grep.paths.as_slice() {
        [] => folder(None, cwd)?,
        [one] => folder(Some(one), cwd)?,
        _ => return None,
    };
    Some(Search::Grep(FindOptions {
        pattern,
        scope: Some(scope),
        ..grep.options
    }))
}

/// What a tool call is searching for, by the shape of its input (`settings.tools` names the tools of
/// each shape). `cwd` resolves relative paths.
pub fn classify(tool_name: &str, input: &Value, cfg: &Config, cwd: &Path) -> Search {
    let sh = &cfg.settings.shell;
    let tools = &cfg.settings.tools;
    let is = |names: &[String]| names.iter().any(|n| n == tool_name);
    if tool_name == "Bash" {
        bash_tool(input, cwd, sh)
    } else if is(&tools.ide_grep) {
        studio_grep(input, cwd)
    } else if is(&tools.ide_symbol) {
        studio_symbol(input)
    } else if is(&tools.grep) || is(&cfg.search_tools) {
        grep_tool(input, cwd)
    } else {
        Search::Pass
    }
}
