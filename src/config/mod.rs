//! Optional per-repository configuration: `.codemap.json` and `.codemapignore`, looked up at the
//! repository root and then in its `.claude/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod settings;

use settings::Settings;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Sibling repositories searched and resolved with this one, relative to the repository root;
    /// none by default.
    pub siblings: Vec<PathBuf>,
    /// Globs of contract files (API collections, OpenAPI documents) scanned for link literals.
    pub literals: Vec<String>,
    pub bridges: Vec<BridgeRule>,
    /// Role name to supertypes or name patterns, for `outline` grouping.
    pub roles: BTreeMap<String, Vec<String>>,
    /// Tools besides Bash whose searches the hook answers, for example `Grep`; none by default.
    pub search_tools: Vec<String>,
    /// The defaults with the user's and the repository's `settings` and `prompts` applied.
    #[serde(skip)]
    pub settings: Settings,
}

/// The user's config file: `settings` and `prompts` for every repository.
pub fn global_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("codemap/config.json"))
}

fn read(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            eprintln!("codemap: ignoring {}: {e}", path.display());
            None
        }
    }
}

fn config_of(value: &Value) -> Option<Config> {
    serde_json::from_value(value.clone())
        .map_err(|e| eprintln!("codemap: ignoring .codemap.json: {e}"))
        .ok()
}

/// How a definition in one language is named in another; `rename` is a template such as `{file}Kt.{name}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRule {
    pub name: String,
    pub from_language: String,
    pub to_language: String,
    pub rename: String,
    /// What separates the parts of an import path in the target language (`.` for Python).
    #[serde(default = "default_import_separator")]
    pub import_separator: String,
}

fn default_import_separator() -> String {
    "/".into()
}

pub(crate) fn find(root: &Path, file: &str) -> Option<PathBuf> {
    [root.join(file), root.join(".claude").join(file)]
        .into_iter()
        .find(|p| p.is_file())
}

/// The configuration of a repository; the default when there is none. A file that does not parse is
/// reported on stderr and treated as absent, so a typo never blocks a session.
pub fn load(root: &Path) -> Config {
    let repo = find(root, ".codemap.json").and_then(|p| read(&p));
    let global = global_path().and_then(|p| read(&p));
    let mut config = repo.as_ref().and_then(config_of).unwrap_or_default();
    let layers: Vec<&Value> = [global.as_ref(), repo.as_ref()]
        .into_iter()
        .flatten()
        .collect();
    config.settings = settings::resolve(&layers).unwrap_or_else(|e| {
        eprintln!("codemap: ignoring the settings overrides: {e}");
        Settings::default()
    });
    // What the hook approves without asking is not a repository's to decide: a cloned `.codemap.json`
    // could otherwise empty `writeCommands` or add `sh` to the filters.
    let own: Vec<&Value> = global.iter().collect();
    let trusted = settings::resolve(&own).unwrap_or_default().shell;
    config.settings.shell.write_commands = trusted.write_commands;
    config.settings.shell.read_only_filters = trusted.read_only_filters;
    config
}

/// The `.codemapignore` rules of a repository, empty when there is no file.
pub fn ignore(root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    if let Some(path) = find(root, ".codemapignore")
        && let Some(e) = builder.add(path)
    {
        eprintln!("codemap: ignoring .codemapignore: {e}");
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

#[cfg(test)]
#[path = "../../tests/unit/config/mod.rs"]
mod tests;
