//! `codemap show <name>`: the source of each definition of a name, with line numbers, so an agent that
//! has to change it gets exactly the block to edit in one step. The block runs from the definition's
//! line to the end of the declaration node the grammar captured at index time; a definition without a
//! grammar is its one line.

use std::fs;
use std::path::PathBuf;

use crate::query::{Located, definitions, fuzzy};
use crate::workspace::Workspace;

fn root_of(ws: &Workspace, repo: &Option<String>) -> PathBuf {
    match repo {
        None => ws.project.root.clone(),
        Some(name) => ws
            .siblings
            .iter()
            .find(|s| &s.name == name)
            .map(|s| s.root.clone())
            .unwrap_or_else(|| ws.project.root.clone()),
    }
}

/// What `codemap <anything>` answers: a path or URL gets the route answer, a file name the files, a
/// name `def_with_code`, words that include a defined compound identifier (`platformName expect and
/// actuals`) that name, and other words a word search. `--all` among the words lifts the cap, as the cut of an answer says.
pub fn lookup(ws: &Workspace, query: &str) -> String {
    let all = query.split_whitespace().any(|w| w == "--all");
    let words: Vec<&str> = query.split_whitespace().filter(|w| *w != "--all").collect();
    let query = words.join(" ");
    let query = query.as_str();
    let limit = ws.config.settings.limits.answer_lines;
    let capped = |mut answer: crate::output::Answer| {
        crate::query::cap(&mut answer, all, limit);
        crate::output::render(&answer)
    };
    if let [word] = words.as_slice() {
        if word.contains('/') {
            let routes = crate::query::link(ws, word);
            if !routes.lines.is_empty() {
                return capped(routes);
            }
        }
        // `test.yml` or `ci/test.yml` is a file by its shape; `Dockerfile` only when no definition
        // has that name, and then only by its whole name.
        let file_shaped = word.contains(['.', '/']);
        if file_shaped || definitions(ws, word).is_empty() {
            let files = crate::query::files_named(ws, word, file_shaped);
            if !files.lines.is_empty() {
                return capped(files);
            }
        }
        return def_with_code(ws, query, all);
    }
    // Only a compound identifier counts: a plain word like `base` names too many things.
    let named = words.iter().find(|w| {
        crate::index::text::split_identifier(w).len() > 1 && !definitions(ws, w).is_empty()
    });
    match named {
        Some(name) => def_with_code(ws, name, all),
        None => crate::output::render(&crate::query::search::search(ws, query, all)),
    }
}

/// `def`, with the code when there is exactly one definition: the read that almost always follows
/// is done in the same call.
pub fn def_with_code(ws: &Workspace, name: &str, all: bool) -> String {
    if definitions(ws, name).len() == 1 {
        return show(ws, name, all);
    }
    let mut answer = crate::query::def(ws, name);
    crate::query::cap(&mut answer, all, ws.config.settings.limits.answer_lines);
    crate::output::render(&answer)
}

pub fn show(ws: &Workspace, name: &str, all: bool) -> String {
    let settings = &ws.config.settings;
    let limits = &settings.show;
    let found = definitions(ws, name);
    if found.is_empty() {
        let nearest = fuzzy(ws, name, settings.limits.nearest_names);
        let hint = if nearest.is_empty() {
            String::new()
        } else {
            format!("; nearest: {}", nearest.join(", "))
        };
        return format!("# show {name}\nno match{hint}\n");
    }
    let shown: Vec<&Located> = found.iter().take(limits.definitions).collect();
    let mut out = format!(
        "# show {name} ({} definition{})\n",
        found.len(),
        if found.len() == 1 { "" } else { "s" }
    );
    for (repo, hit) in shown {
        // A path of the index is one git listed; anything else (absolute, `..`) is not read.
        if std::path::Path::new(&hit.path)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            continue;
        }
        let file = root_of(ws, repo).join(&hit.path);
        // Nor a link, as in the indexer and grep.
        if fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink()) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        let start = (hit.symbol.start_line as usize).clamp(1, hit.symbol.line as usize);
        let end = (hit.symbol.end_line as usize).max(hit.symbol.line as usize);
        let prefix = repo.as_deref().map(|r| format!("{r}/")).unwrap_or_default();
        let scope = if hit.scope == "main" {
            String::new()
        } else {
            format!(" [{}]", hit.scope)
        };
        out.push_str(&format!(
            "{prefix}{}:{start}-{end} {} {}{scope}\n",
            hit.path, hit.symbol.kind, hit.symbol.name
        ));
        let total = end - start + 1;
        let printed = if all { total } else { total.min(limits.lines) };
        for (offset, text) in source.lines().skip(start - 1).take(printed).enumerate() {
            out.push_str(&format!("{:>4} | {text}\n", start + offset));
        }
        if printed < total {
            out.push_str(&format!(
                "     ... {} more lines; read {prefix}{}:{}-{end} (or --all)\n",
                total - printed,
                hit.path,
                start + printed
            ));
        }
    }
    if found.len() > limits.definitions {
        out.push_str(&format!(
            "... {} more definitions: codemap def {name} --all\n",
            found.len() - limits.definitions
        ));
    }
    out
}
