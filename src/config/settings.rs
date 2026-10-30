//! codemap's tunable data: limits, word lists, folder names, kinds, bridges, the agents `setup` knows
//! and the texts the agent is shown. The defaults are `defaults/settings.toml` and `defaults/prompts/`,
//! built into the binary. The user's `codemap/config.json` and then a repository's `.codemap.json`
//! override them under `settings` (and `prompts`): tables merge key by key, any other value replaces.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::agent::setup::Format;
use crate::config::BridgeRule;

const DEFAULTS: &str = include_str!("../../defaults/settings.toml");

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub limits: Limits,
    pub show: Show,
    pub logs: Logs,
    pub search: Search,
    pub scope: Scope,
    pub kinds: Kinds,
    pub parse: Parse,
    pub bridges: Vec<BridgeRule>,
    pub language_families: BTreeMap<String, Vec<String>>,
    pub shell: Shell,
    pub tools: Tools,
    pub agents: Vec<Agent>,
    pub prompts: Prompts,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub threads: usize,
    pub answer_lines: usize,
    pub nearest_names: usize,
    pub index_file_bytes: usize,
    pub text_bytes: usize,
    pub find_line_chars: usize,
    pub start_budget_seconds: f64,
    pub cache_days: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Show {
    pub definitions: usize,
    pub lines: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Logs {
    pub width: usize,
    pub detail_kept: usize,
    pub detail_shown: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Search {
    pub per_repo: usize,
    pub rrf_k: f64,
    pub weight_original: f64,
    pub weight_related: f64,
    pub weight_identifiers: f64,
    pub weight_paths: f64,
    pub semantic_files: usize,
    pub identifiers_per_word: usize,
    pub identifier_min_length: usize,
    /// The Snowball stemmer for words, by language name ("English"); none when absent.
    #[serde(default)]
    pub stemmer: Option<rust_stemmers::Algorithm>,
    /// The share of the best fuzzy score a name needs to be listed next to it.
    pub fuzzy_keep: f64,
    /// The lowest Damerau-Levenshtein similarity a name needs to count as a typo of another.
    pub typo_similarity: f64,
    pub stopwords: Vec<String>,
    pub synonyms: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub vendored_dirs: Vec<String>,
    pub generated_dirs: Vec<String>,
    pub test_dirs: Vec<String>,
    pub test_files: Vec<String>,
    #[serde(skip)]
    sets: OnceLock<[GlobSet; 4]>,
}

fn glob_set(globs: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        match Glob::new(glob) {
            Ok(g) => {
                builder.add(g);
            }
            Err(e) => eprintln!("codemap: ignoring scope glob {glob}: {e}"),
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

impl Scope {
    /// `vendoredDirs`, `generatedDirs`, `testDirs` and `testFiles` compiled, in that order; a glob
    /// that does not compile matches nothing.
    pub fn globs(&self) -> &[GlobSet; 4] {
        self.sets.get_or_init(|| {
            [
                glob_set(&self.vendored_dirs),
                glob_set(&self.generated_dirs),
                glob_set(&self.test_dirs),
                glob_set(&self.test_files),
            ]
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kinds {
    pub facades: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parse {
    pub declaration_keywords: Vec<String>,
    pub prose_extensions: Vec<String>,
    #[serde(skip)]
    declaration: OnceLock<Option<Regex>>,
}

impl Parse {
    /// A line that starts with only word-like tokens before a declaration keyword. Comment markers,
    /// quotes and operators are not word-like, so commented and quoted keywords never match. `None`
    /// without keywords.
    pub fn declaration_regex(&self) -> Option<&Regex> {
        self.declaration
            .get_or_init(|| {
                if self.declaration_keywords.is_empty() {
                    return None;
                }
                let keywords: Vec<String> = self
                    .declaration_keywords
                    .iter()
                    .map(|w| regex::escape(w))
                    .collect();
                Some(
                    Regex::new(&format!(
                        r"^\s*(?:[A-Za-z_@][\w@]*\s+)*?({})\s+([A-Za-z_]\w*)",
                        keywords.join("|")
                    ))
                    .expect("escaped keywords always compile"),
                )
            })
            .as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shell {
    pub separators: Vec<String>,
    pub glob_options: Vec<String>,
    pub write_commands: Vec<String>,
    pub read_only_filters: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tools {
    pub grep: Vec<String>,
    pub ide_grep: Vec<String>,
    pub ide_symbol: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Agent {
    pub name: String,
    pub format: Format,
    pub detect: String,
    pub global: Option<String>,
    pub project: Option<String>,
}

/// `~/x` under `home`, `{config}/x` under `config`, anything else as written.
pub fn expand(path: &str, home: &Path, config: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if let Some(rest) = path.strip_prefix("{config}/") {
        config.join(rest)
    } else {
        PathBuf::from(path)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prompts {
    /// Told to the agent at the start of a session.
    pub session: String,
    /// The MCP server's instructions.
    pub mcp: String,
}

fn builtin_value() -> Value {
    let table: toml::Table = DEFAULTS
        .parse()
        .expect("defaults/settings.toml is valid TOML");
    let mut value = serde_json::to_value(table).expect("TOML converts to JSON");
    value["prompts"] = json!({
        "session": include_str!("../../defaults/prompts/session.md").trim(),
        "mcp": include_str!("../../defaults/prompts/mcp.md").trim(),
    });
    value
}

/// The defaults, parsed once.
pub fn builtin() -> &'static Settings {
    static BUILTIN: OnceLock<Settings> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        serde_json::from_value(builtin_value()).expect("defaults match the Settings struct")
    })
}

impl Default for Settings {
    fn default() -> Self {
        builtin().clone()
    }
}

fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                merge(base.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (base, over) => *base = over.clone(),
    }
}

/// The defaults with each layer's `settings` and `prompts` applied in order (later layers win). A
/// layer is the whole JSON of a config file.
pub fn resolve(layers: &[&Value]) -> Result<Settings, String> {
    let mut value = builtin_value();
    for layer in layers {
        if let Some(settings) = layer.get("settings") {
            merge(&mut value, settings);
        }
        if let Some(prompts) = layer.get("prompts") {
            merge(&mut value["prompts"], prompts);
        }
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

impl Settings {
    /// What the stored parses depend on; a store made under another fingerprint is parsed again.
    pub fn index_fingerprint(&self) -> String {
        // The version and the grammar queries too: an updated binary parses a known file differently.
        let parts = json!([
            env!("CARGO_PKG_VERSION"),
            env!("CODEMAP_GRAMMARS"),
            (self.search.semantic_files > 0).then(crate::index::embed::identity),
            self.parse,
            self.limits.index_file_bytes,
            self.limits.text_bytes
        ]);
        sha1_smol::Sha1::from(parts.to_string())
            .digest()
            .to_string()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/config/settings.rs"]
mod tests;
