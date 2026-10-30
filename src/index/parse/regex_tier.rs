//! The fallback parser for files without a grammar: one declaration regex over the keywords of
//! `settings.parse` (without keywords it finds nothing), and the headings of prose.

use std::sync::OnceLock;

use regex::Regex;

use crate::config::settings::Parse;
use crate::model::{Role, Symbol};

pub fn parse(source: &str, rules: &Parse) -> Vec<Symbol> {
    let Some(pattern) = rules.declaration_regex() else {
        return Vec::new();
    };
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let caps = pattern.captures(line)?;
            Some(Symbol {
                name: caps[2].to_string(),
                role: Role::Definition,
                kind: caps[1].to_string(),
                line: i as u32 + 1,
                start_line: i as u32 + 1,
                end_line: i as u32 + 1,
                flags: Vec::new(),
                supertypes: Vec::new(),
            })
        })
        .collect()
}

/// The headings of a prose file: a line underlined with a row of `=`, `-`, `~` or `^` at least as long
/// (reStructuredText, plain text), or an AsciiDoc title (`== Title`).
pub fn headings(source: &str) -> Vec<Symbol> {
    let lines: Vec<&str> = source.lines().collect();
    let underline = |line: &str| {
        let line = line.trim_end();
        line.len() >= 3
            && line.starts_with(['=', '-', '~', '^'])
            && line.chars().all(|c| line.starts_with(c))
    };
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let title = line.trim_end();
        let underlined = !title.is_empty()
            && !title.starts_with(char::is_whitespace)
            && !underline(title)
            && lines
                .get(i + 1)
                .is_some_and(|next| underline(next) && next.trim_end().len() >= title.len());
        let asciidoc = title
            .strip_prefix('=')
            .map(|rest| rest.trim_start_matches('='))
            .and_then(|rest| rest.strip_prefix(' '));
        let name = match (underlined, asciidoc) {
            (true, _) => title,
            (false, Some(name)) if !name.trim().is_empty() => name.trim(),
            _ => continue,
        };
        found.push(Symbol {
            name: name.to_string(),
            role: Role::Definition,
            kind: "heading".into(),
            line: i as u32 + 1,
            start_line: i as u32 + 1,
            end_line: i as u32 + 1,
            flags: Vec::new(),
            supertypes: Vec::new(),
        });
    }
    found
}

/// Link targets written as `[text](target)` or `<https://...>`, as literal symbols.
pub fn link_literals(source: &str) -> Vec<Symbol> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"\]\(([^)\s]+)\)|<(https?://[^>\s]+)>").expect("link regex"));
    source
        .lines()
        .enumerate()
        .flat_map(|(i, line)| {
            re.captures_iter(line).filter_map(move |caps| {
                let target = caps.get(1).or_else(|| caps.get(2))?.as_str();
                super::grammar::looks_like_link(target).then(|| Symbol {
                    name: target.to_string(),
                    role: Role::Literal,
                    kind: "literal".into(),
                    line: i as u32 + 1,
                    start_line: i as u32 + 1,
                    end_line: i as u32 + 1,
                    flags: Vec::new(),
                    supertypes: Vec::new(),
                })
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/unit/index/parse/regex_tier.rs"]
mod tests;
