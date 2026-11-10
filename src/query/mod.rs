//! The index queries: definitions, uses, implementations, variants, links, outline. Every query
//! reconciles the project first and reads the siblings' databases as they are.

use std::collections::BTreeMap;

#[cfg(test)]
#[path = "../../tests/unit/query/mod.rs"]
mod tests;

pub mod bridge;
pub mod deps;
pub mod find;
pub mod search;
pub mod show;

use crate::config;
use crate::index::parse::{Language, Tier, tier};
use crate::index::scope::class_of;
use crate::index::store::Store;
use crate::index::{Depth, reconcile};
use crate::model::{Hit, Role};
use crate::output::{Answer, Line};
use crate::workspace::Workspace;

/// A hit with the repository it came from (`None` for the project).
pub(crate) type Located = (Option<String>, Hit);

/// The first hit of a file, how many hits the file has, and the bridge labels seen on them.
type PerFile = BTreeMap<(Option<String>, String), (Located, usize, Vec<String>)>;

/// Opens every database of the workspace, reconciling the project so the answer never trails an edit.
pub(crate) fn stores(ws: &Workspace) -> Vec<(Option<String>, Store)> {
    let mut out = Vec::new();
    if let Ok(mut store) = Store::open(&ws.cache_dir, &ws.project.root) {
        let _ = reconcile(
            &ws.project,
            &mut store,
            &config::ignore(&ws.project.root),
            Depth::Full,
            &ws.config.settings,
        );
        out.push((None, store));
    }
    for sibling in &ws.siblings {
        if let Ok(store) = Store::open(&ws.cache_dir, &sibling.root) {
            out.push((Some(sibling.name.clone()), store));
        }
    }
    out
}

fn collect(ws: &Workspace, query: impl Fn(&Store) -> anyhow::Result<Vec<Hit>>) -> Vec<Located> {
    let mut hits = Vec::new();
    for (repo, store) in stores(ws) {
        if let Ok(found) = query(&store) {
            hits.extend(found.into_iter().map(|h| (repo.clone(), h)));
        }
    }
    hits
}

fn role_order(role: Role) -> u8 {
    match role {
        Role::Definition => 0,
        Role::Reference => 1,
        Role::Literal => 2,
    }
}

/// Project before siblings, definitions before references, main before test before generated and
/// vendored; stable by path and line within a group.
pub fn rank(hits: &mut [Located]) {
    hits.sort_by(|(ra, a), (rb, b)| {
        (
            ra.is_some(),
            ra,
            role_order(a.symbol.role),
            class_of(&a.scope),
            &a.path,
            a.symbol.line,
        )
            .cmp(&(
                rb.is_some(),
                rb,
                role_order(b.symbol.role),
                class_of(&b.scope),
                &b.path,
                b.symbol.line,
            ))
    });
}

fn line(repo: &Option<String>, hit: &Hit) -> Line {
    Line {
        repo: repo.clone(),
        path: hit.path.clone(),
        line: hit.symbol.line,
        kind: hit.symbol.kind.clone(),
        name: hit.symbol.name.clone(),
        scope: hit.scope.clone(),
        flags: hit.symbol.flags.join(" "),
    }
}

fn tier_of(hits: &[Located]) -> String {
    let mut grammar = false;
    let mut regex = false;
    for (_, hit) in hits {
        match Language::from_name(&hit.language).map(|l| tier(&l)) {
            Some(Tier::Grammar) => grammar = true,
            _ => regex = true,
        }
    }
    match (grammar, regex) {
        (true, false) => "grammar",
        (false, true) => "regex",
        (true, true) => "mixed",
        (false, false) => "-",
    }
    .into()
}

fn answer(mut hits: Vec<Located>) -> Answer {
    rank(&mut hits);
    let mut answer = Answer::new("index");
    answer.tier = tier_of(&hits);
    answer.lines = hits.iter().map(|(repo, hit)| line(repo, hit)).collect();
    answer
}

/// Cuts the lines to `limit` (`settings.limits.answerLines`) unless `all`, recording how many were cut.
pub fn cap(answer: &mut Answer, all: bool, limit: usize) {
    if !all && answer.lines.len() > limit {
        answer.cut = answer.lines.len() - limit;
        answer.lines.truncate(limit);
    }
}

/// The definitions of `name` across the workspace, ranked.
pub(crate) fn definitions(ws: &Workspace, name: &str) -> Vec<Located> {
    let mut hits: Vec<Located> = collect(ws, |s| s.by_name(name))
        .into_iter()
        .filter(|(_, h)| h.symbol.role == Role::Definition)
        .collect();
    rank(&mut hits);
    hits
}

/// The definitions of `name`. Without one, the definitions of the closest names, so a misspelt or
/// half-remembered name still lands; without those, a word search for it.
pub fn def(ws: &Workspace, name: &str) -> Answer {
    let hits = definitions(ws, name);
    if !hits.is_empty() {
        return answer(hits);
    }
    let cover = ws.config.settings.search.fuzzy_cover;
    let nearest: Vec<String> = fuzzy(ws, name, ws.config.settings.limits.nearest_names)
        .into_iter()
        .filter(|close| covers(name, close, cover))
        .collect();
    if nearest.is_empty() {
        let mut answer = search::search(ws, name, false);
        answer.notes.insert(
            0,
            format!("no definition named {name}; files with these words:"),
        );
        return answer;
    }
    // Closest name first: `answer` would rank them all together and bury the best match.
    let hits: Vec<Located> = nearest.iter().flat_map(|n| definitions(ws, n)).collect();
    let mut answer = Answer::new("index");
    answer.tier = tier_of(&hits);
    answer.lines = hits.iter().map(|(repo, hit)| line(repo, hit)).collect();
    answer
        .notes
        .push(format!("no definition named {name}; the closest names:"));
    answer
}

/// Whether `query` makes up at least `share` of `name`: a few letters found in order somewhere in a
/// far longer name do not make that name close.
pub fn covers(query: &str, name: &str, share: f64) -> bool {
    query.chars().count() as f64 >= name.chars().count() as f64 * share
}

/// The definition names of the project closest to `name`, best first: the Helix matcher for letters
/// in order (`BaseVM`), else Damerau-Levenshtein similarity for typos (`BaseViewModle`).
pub fn fuzzy(ws: &Workspace, name: &str, limit: usize) -> Vec<String> {
    use nucleo::pattern::{CaseMatching, Normalization, Pattern};
    let Ok(store) = Store::open(&ws.cache_dir, &ws.project.root) else {
        return Vec::new();
    };
    let names = store.definition_names().unwrap_or_default();
    let mut matcher = nucleo::Matcher::new(nucleo::Config::DEFAULT);
    let pattern = Pattern::parse(name, CaseMatching::Ignore, Normalization::Smart);
    let rules = &ws.config.settings.search;
    let mut scored = pattern.match_list(names.iter(), &mut matcher);
    if !scored.is_empty() {
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.len().cmp(&b.0.len())));
        let floor = f64::from(scored[0].1) * rules.fuzzy_keep;
        return scored
            .into_iter()
            .take_while(|(_, score)| f64::from(*score) >= floor)
            .take(limit)
            .map(|(n, _)| n.clone())
            .collect();
    }
    let wanted = name.to_lowercase();
    let minimum = rules.typo_similarity;
    let mut similar: Vec<(f64, &String)> = names
        .iter()
        .map(|n| {
            let score = strsim::normalized_damerau_levenshtein(&wanted, &n.to_lowercase());
            (score, n)
        })
        .filter(|(score, _)| *score >= minimum)
        .collect();
    similar.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    similar
        .into_iter()
        .take(limit)
        .map(|(_, n)| n.clone())
        .collect()
}

/// References to `name`, definitions that extend or implement it, and foreign references found
/// through the bridges (flagged `bridge:<rule>`). One line per file with a count unless `lines` asks
/// for every line.
pub fn uses(ws: &Workspace, name: &str, lines: bool) -> Answer {
    let opened = stores(ws);
    let mut hits: Vec<Located> = Vec::new();
    let mut definitions: Vec<Hit> = Vec::new();
    for (repo, store) in &opened {
        for hit in store.by_name(name).unwrap_or_default() {
            match hit.symbol.role {
                Role::Reference => hits.push((repo.clone(), hit)),
                Role::Definition if repo.is_none() => definitions.push(hit),
                _ => {}
            }
        }
        hits.extend(
            store
                .by_supertype(name)
                .unwrap_or_default()
                .into_iter()
                .map(|h| (repo.clone(), h)),
        );
    }
    for def in &definitions {
        for (rule, bridged) in bridge::apply(ws, &opened, def) {
            let label = format!("bridge:{rule}");
            for (repo, mut hit) in bridged {
                match hits.iter_mut().find(|(r, h)| {
                    *r == repo && h.path == hit.path && h.symbol.line == hit.symbol.line
                }) {
                    Some((_, existing)) => {
                        if !existing.symbol.flags.contains(&label) {
                            existing.symbol.flags.push(label.clone());
                        }
                    }
                    None => {
                        hit.symbol.flags = vec![label.clone()];
                        hits.push((repo, hit));
                    }
                }
            }
        }
    }
    rank(&mut hits);
    if lines {
        return answer(hits);
    }
    let mut per_file: PerFile = BTreeMap::new();
    let mut order = Vec::new();
    for located in hits {
        let key = (located.0.clone(), located.1.path.clone());
        let labels: Vec<String> = located
            .1
            .symbol
            .flags
            .iter()
            .filter(|f| f.starts_with("bridge:"))
            .cloned()
            .collect();
        match per_file.get_mut(&key) {
            Some((_, count, seen)) => {
                *count += 1;
                for label in labels {
                    if !seen.contains(&label) {
                        seen.push(label);
                    }
                }
            }
            None => {
                order.push(key.clone());
                per_file.insert(key, (located, 1, labels));
            }
        }
    }
    let mut grouped = Vec::new();
    for key in order {
        let (located, count, labels) = per_file.remove(&key).expect("inserted above");
        let (repo, mut hit) = located;
        hit.symbol.flags = std::iter::once(format!("{count}x")).chain(labels).collect();
        grouped.push((repo, hit));
    }
    answer(grouped)
}

pub fn impl_(ws: &Workspace, type_name: &str) -> Answer {
    let mut answer = answer(collect(ws, |s| s.by_supertype(type_name)));
    // Said every time, as for grep, so the agent does not grep again to be sure nothing is missing.
    let repositories = 1 + ws.siblings.len();
    answer.notes.push(format!(
        "every definition of {repositories} repositor{}, gitignored excluded",
        if repositories == 1 { "y" } else { "ies" }
    ));
    answer
}

/// Definitions of `name` across scopes, the `expect` declaration first, then each actual by scope.
pub fn variants(ws: &Workspace, name: &str) -> Answer {
    let mut hits: Vec<Located> = collect(ws, |s| s.by_name(name))
        .into_iter()
        .filter(|(_, h)| h.symbol.role == Role::Definition)
        .collect();
    hits.sort_by(|(ra, a), (rb, b)| {
        let expect = |h: &Hit| !h.symbol.flags.iter().any(|f| f == "expect");
        (ra.is_some(), expect(a), &a.scope, &a.path).cmp(&(
            rb.is_some(),
            expect(b),
            &b.scope,
            &b.path,
        ))
    });
    let mut answer = Answer::new("index");
    answer.tier = tier_of(&hits);
    answer.lines = hits.iter().map(|(repo, hit)| line(repo, hit)).collect();
    answer
}

/// The files of every repository named `name` (case ignored, with or without the extension:
/// `ic_logo` is `ic_logo.png`) or whose path ends with it (`ci/test.yml`); without any, and when
/// `partial`, those whose name contains it.
pub fn files_named(ws: &Workspace, name: &str, partial: bool) -> Answer {
    let wanted = name.trim_start_matches("./").to_lowercase();
    let file_of = |path: &str| path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let mut exact = Vec::new();
    let mut containing = Vec::new();
    for (repo, store) in stores(ws) {
        for crate::index::store::FileRow { path, scope, .. } in store.files().unwrap_or_default() {
            let lower = path.to_lowercase();
            let line = Line {
                repo: repo.clone(),
                path: path.clone(),
                line: 1,
                kind: "file".into(),
                name: path.rsplit('/').next().unwrap_or(&path).to_string(),
                scope,
                flags: String::new(),
            };
            let stem_is = file_of(&lower)
                .strip_prefix(wanted.as_str())
                .is_some_and(|rest| rest.starts_with('.'));
            if lower == wanted || lower.ends_with(&format!("/{wanted}")) || stem_is {
                exact.push(line);
            } else if partial && file_of(&lower).contains(&wanted) {
                containing.push(line);
            }
        }
    }
    let mut answer = Answer::new("index");
    answer.lines = if exact.is_empty() { containing } else { exact };
    if !answer.lines.is_empty() {
        answer.tier = "files".into();
    }
    answer
}

/// Every repository carrying `literal`, project first, with the enclosing definition in the flags.
pub fn link(ws: &Workspace, literal: &str) -> Answer {
    answer(collect(ws, |s| s.literals(literal)))
}

fn role_matches(pattern: &str, candidate: &str) -> bool {
    match (pattern.strip_prefix('*'), pattern.strip_suffix('*')) {
        (Some(suffix), _) => candidate.ends_with(suffix),
        (_, Some(prefix)) => candidate.starts_with(prefix),
        _ => pattern == candidate,
    }
}

/// Definitions under `path` in the project, grouped by the configured roles (a role replaces the kind
/// when the name or a supertype matches one of its patterns), then by kind.
pub fn outline(ws: &Workspace, path: &str) -> Answer {
    let roles: Vec<(&String, &Vec<String>)> = ws.config.roles.iter().collect();
    let role_of = |hit: &Hit| -> Option<usize> {
        roles.iter().position(|(_, patterns)| {
            patterns.iter().any(|p| {
                role_matches(p, &hit.symbol.name)
                    || hit.symbol.supertypes.iter().any(|s| role_matches(p, s))
            })
        })
    };
    let mut hits: Vec<(usize, Located)> = Vec::new();
    if let Some((repo, store)) = stores(ws).into_iter().next()
        && let Ok(found) = store.under(path)
    {
        for mut hit in found
            .into_iter()
            .filter(|h| h.symbol.role == Role::Definition)
        {
            let role = role_of(&hit);
            if let Some(index) = role {
                hit.symbol.kind = roles[index].0.clone();
            }
            hits.push((role.unwrap_or(usize::MAX), (repo.clone(), hit)));
        }
    }
    hits.sort_by(|(ra, (_, a)), (rb, (_, b))| {
        (ra, &a.symbol.kind, &a.path, a.symbol.line).cmp(&(
            rb,
            &b.symbol.kind,
            &b.path,
            b.symbol.line,
        ))
    });
    let located: Vec<Located> = hits.into_iter().map(|(_, l)| l).collect();
    let mut answer = Answer::new("index");
    answer.tier = tier_of(&located);
    answer.lines = located.iter().map(|(repo, hit)| line(repo, hit)).collect();
    answer
}
