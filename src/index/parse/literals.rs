//! String literals that look like links (paths, URLs), with the definition they sit in.

use std::sync::OnceLock;

use regex::Regex;

use super::Language;
use super::grammar::looks_like_link;
use crate::model::{Role, Symbol};

/// Every quoted string in `source` whose content is a path or URL, as a literal symbol flagged with
/// `in:<definition>`. The definition is the nearest one at or above the line, except on an annotation
/// line (`@Resource("/x")`, `#[route("/x")]`, `[Route("/x")]`), which belongs to the declaration
/// below it. `lang` is accepted for a later per-language quoting rule; every language uses the same
/// two quote styles today.
pub fn literals(_lang: &Language, source: &str, defs: &[Symbol]) -> Vec<Symbol> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#""([^"\\\n]*)"|'([^'\\\n]*)'"#).expect("string regex"));
    let mut defs: Vec<&Symbol> = defs.iter().filter(|d| d.role == Role::Definition).collect();
    defs.sort_by_key(|d| d.line);
    let enclosing = |line: u32, text: &str| -> &str {
        let annotation =
            text.trim_start().starts_with(['@', '[']) || text.trim_start().starts_with("#[");
        let below = || defs.iter().find(|d| d.line > line);
        let above = || defs.iter().rev().find(|d| d.line <= line);
        let found = if annotation {
            below().or_else(above)
        } else {
            above().or_else(below)
        };
        found.map(|d| d.name.as_str()).unwrap_or("")
    };
    source
        .lines()
        .enumerate()
        .flat_map(|(i, text)| {
            let line = i as u32 + 1;
            re.captures_iter(text).filter_map(move |caps| {
                let value = caps.get(1).or_else(|| caps.get(2))?.as_str();
                looks_like_link(value).then(|| Symbol {
                    name: value.to_string(),
                    role: Role::Literal,
                    kind: "literal".into(),
                    line,
                    start_line: line,
                    end_line: line,
                    flags: vec![format!("in:{}", enclosing(line, text))],
                    supertypes: Vec::new(),
                })
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/index/parse/literals.rs"]
mod tests;
