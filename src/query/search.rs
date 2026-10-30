//! Full-text search with query rewriting: the stemmed query words, plus the identifiers of the code
//! base that contain one of them (`timeout` finds `HttpTimeout`). Each variant is searched on its own
//! and the rankings are merged by reciprocal rank fusion. Stop words and synonyms are the user's own
//! word lists in `settings.search`, empty by default.

use std::collections::{HashMap, HashSet};

use crate::config::settings::Search;
use crate::index::embed;
use crate::index::store::Store;
use crate::index::text::{query_term, split_identifier, stem};
use crate::model::Hit;
use crate::output::{Answer, Line};
use crate::query::{cap, fuzzy, stores};
use crate::workspace::Workspace;

/// One rewritten form of the query and how much its ranking counts.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub weight: f64,
    pub words: Vec<String>,
}

fn tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The words of the query that carry meaning; all of them when everything is a stop word.
fn significant(query: &str, rules: &Search) -> Vec<String> {
    let all = tokens(query);
    let mut kept: Vec<String> = Vec::new();
    for word in &all {
        if !rules.stopwords.contains(word) && !kept.contains(word) {
            kept.push(word.clone());
        }
    }
    if kept.is_empty() { all } else { kept }
}

fn push_unique(into: &mut Vec<String>, word: String) {
    if !into.contains(&word) {
        into.push(word);
    }
}

/// The variants that need no index: the query itself and the configured synonyms of its words.
pub fn expand(query: &str, rules: &Search) -> Vec<Variant> {
    let words = significant(query, rules);
    let mut original: Vec<String> = Vec::new();
    for word in &words {
        push_unique(&mut original, query_term(word, rules.stemmer));
    }
    let mut variants = vec![Variant {
        weight: rules.weight_original,
        words: original.clone(),
    }];
    let mut related: Vec<String> = Vec::new();
    for word in &words {
        for synonym in rules.synonyms.get(word).into_iter().flatten() {
            push_unique(&mut related, query_term(synonym, rules.stemmer));
        }
    }
    related.retain(|w| !original.contains(w));
    if !related.is_empty() {
        variants.push(Variant {
            weight: rules.weight_related,
            words: related,
        });
    }
    variants
}

/// `expand` plus the identifiers of this code base that contain a query word, found by fuzzy matching
/// definition names: the query rewritten in the vocabulary of the code.
pub fn rewrite(ws: &Workspace, query: &str) -> Vec<Variant> {
    let rules = &ws.config.settings.search;
    let mut variants = expand(query, rules);
    let words = significant(query, rules);
    let mut names: Vec<String> = Vec::new();
    // Adjacent words joined, as a compound identifier is indexed whole: `base view model` is
    // `baseviewmodel`. A join no identifier has matches nothing and costs nothing.
    for start in 0..words.len() {
        for end in start + 2..=words.len() {
            push_unique(&mut names, words[start..end].concat());
        }
    }
    for word in words
        .iter()
        .filter(|w| w.len() >= rules.identifier_min_length)
    {
        // A stem is not always a prefix of the word (`repository` stems to `repositori`): either counts.
        let root = stem(word, rules.stemmer);
        for name in fuzzy(ws, &root, rules.identifiers_per_word) {
            let lower = name.to_lowercase();
            if lower.contains(root.as_str()) || lower.contains(word.as_str()) {
                push_unique(&mut names, query_term(&name, rules.stemmer));
            }
        }
    }
    if !names.is_empty() {
        variants.push(Variant {
            weight: rules.weight_identifiers,
            words: names,
        });
    }
    variants
}

/// `query` with every word no file holds replaced by the most similar word of the code base (the
/// words of its definition names and file paths, by `search.typoSimilarity`), so a misspelt word
/// still searches: `notificaton icon` is `notification icon`. `None` when every word is known or
/// nothing is close enough.
fn respelt(query: &str, opened: &[(Option<String>, Store)], rules: &Search) -> Option<String> {
    let known = |word: &str| {
        let term = query_term(word, rules.stemmer);
        opened.iter().any(|(_, store)| store.has_term(&term))
    };
    let words = tokens(query);
    if words.iter().all(|w| known(w)) {
        return None;
    }
    let mut vocabulary: HashSet<String> = HashSet::new();
    for (_, store) in opened {
        for name in store.definition_names().unwrap_or_default() {
            vocabulary.extend(split_identifier(&name));
        }
        for file in store.files().unwrap_or_default() {
            vocabulary.extend(file.path.split(['/', '.']).flat_map(split_identifier));
        }
    }
    let mut changed = false;
    let respelt: Vec<String> = words
        .into_iter()
        .map(|word| {
            if known(&word) || vocabulary.contains(&word) {
                return word;
            }
            // One edit always counts from four letters on (`boko` is `book`), where it is a smaller
            // share of the word than the similarity asks for.
            let letters = word.chars().count() as f64;
            let needed = match letters >= 4.0 {
                true => rules.typo_similarity.min(1.0 - 1.0 / letters),
                false => rules.typo_similarity,
            };
            let closest = vocabulary
                .iter()
                .map(|v| (strsim::normalized_damerau_levenshtein(&word, v), v))
                .filter(|(score, _)| *score >= needed)
                .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(a.1)));
            match closest {
                Some((_, v)) => {
                    changed = true;
                    v.clone()
                }
                None => word,
            }
        })
        .collect();
    changed.then(|| respelt.join(" "))
}

pub fn search(ws: &Workspace, query: &str, all: bool) -> Answer {
    let rules = &ws.config.settings.search;
    let mut answer = Answer::new("fts");
    let opened = stores(ws);
    let respelt = respelt(query, &opened, rules);
    if let Some(respelt) = &respelt {
        // Said, so the agent learns the spelling and does not repeat the typo.
        answer.notes.push(format!("searched for: {respelt}"));
    }
    let query = respelt.as_deref().unwrap_or(query);
    let variants = rewrite(ws, query);
    // Every repository's files in one ranking: listed repository by repository, the project alone
    // would fill the cap and a sibling's best file would never show. Fused scores are ranks within one
    // repository, so they do not compare across repositories on their own; how many of the query's
    // words a file holds (in its text or its path) does, and orders first; then how many its path
    // holds, as a name says more than a mention.
    let mut scored: Vec<(usize, usize, f64, Line)> = Vec::new();
    let roots = path_roots(query, rules);
    for (repo, store) in &opened {
        // path -> (line, fused score, query words held, of them in the path), in first-seen order
        let mut fused: Vec<(Line, f64, usize, usize)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut add =
            |line: Line, gain: f64, held: usize, in_path: usize| match index.get(&line.path) {
                Some(&at) => {
                    fused[at].1 += gain;
                    fused[at].2 = fused[at].2.max(held);
                    fused[at].3 = fused[at].3.max(in_path);
                }
                None => {
                    index.insert(line.path.clone(), fused.len());
                    fused.push((line, gain, held, in_path));
                }
            };
        for (number, variant) in variants.iter().enumerate() {
            let Ok(ranked) = store.search(&variant.words.join(" "), rules.per_repo) else {
                continue;
            };
            for (rank, (hit, distinct)) in ranked.into_iter().enumerate() {
                let gain = variant.weight / (rules.rrf_k + rank as f64 + 1.0);
                // Only the query's own words count as held; the rewrites only rank.
                let held = if number == 0 { distinct } else { 0 };
                add(line_of(repo, hit), gain, held, 0);
            }
        }
        // The words of the file paths are one more ranking: `gitlab ci test` is .gitlab/ci/test.yml.
        let files = store.files().unwrap_or_default();
        let files = files.into_iter().map(|f| (f.path, f.scope)).collect();
        for (rank, (path, scope, held)) in by_path_words(files, &roots, rules)
            .into_iter()
            .take(rules.per_repo)
            .enumerate()
        {
            let gain = rules.weight_paths / (rules.rrf_k + rank as f64 + 1.0);
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let line = Line {
                repo: repo.clone(),
                path,
                line: 1,
                kind: "text".into(),
                name,
                scope,
                flags: String::new(),
            };
            add(line, gain, held, held);
        }
        scored.extend(
            fused
                .into_iter()
                .map(|(line, score, held, in_path)| (held, in_path, score, line)),
        );
    }
    // Ties keep the project first, then the path order.
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.total_cmp(&a.2))
            .then_with(|| a.3.repo.is_some().cmp(&b.3.repo.is_some()))
            .then_with(|| a.3.path.cmp(&b.3.path))
    });
    let by_words: Vec<Line> = scored.into_iter().map(|(_, _, _, line)| line).collect();
    // A single word nothing holds is a name that does not exist, and "no match" says so; a
    // description is what the meaning is for.
    let described = !by_words.is_empty() || tokens(query).len() > 1;
    let wanted = (rules.semantic_files > 0 && described)
        .then(|| embed::embed(query))
        .flatten();
    let by_meaning = wanted.map_or_else(Vec::new, |w| closest(&opened, &w, rules.semantic_files));
    // Said, because some file is always the closest: the agent judges whether these are what it meant.
    if by_words.is_empty() && !by_meaning.is_empty() {
        answer
            .notes
            .push("no file has these words; the closest by meaning:".into());
    }
    answer.lines = fuse(by_words, by_meaning);
    answer.tier = if answer.lines.is_empty() {
        "-".into()
    } else {
        "words".into()
    };
    cap(&mut answer, all, ws.config.settings.limits.answer_lines);
    answer
}

/// The files of every repository closest to the query in meaning, best first.
fn closest(opened: &[(Option<String>, Store)], wanted: &[f32], limit: usize) -> Vec<Line> {
    let mut similar: Vec<(f32, Line)> = Vec::new();
    for (repo, store) in opened {
        for (score, path, scope) in store.nearest(wanted, limit).unwrap_or_default() {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let line = Line {
                repo: repo.clone(),
                path,
                line: 1,
                kind: "text".into(),
                name,
                scope,
                flags: String::new(),
            };
            similar.push((score, line));
        }
    }
    similar.sort_by(|a, b| b.0.total_cmp(&a.0));
    similar.truncate(limit);
    similar.into_iter().map(|(_, line)| line).collect()
}

/// The ranking by words and the ranking by meaning as one: first the files both found, in the order
/// of the words, then the rest taking turns (a description that shares no word with the code is the
/// best of the meaning; an exact word the model makes nothing of is the best of the words). A file in
/// both keeps the line the words found.
pub fn fuse(by_words: Vec<Line>, by_meaning: Vec<Line>) -> Vec<Line> {
    let key = |l: &Line| (l.repo.clone(), l.path.clone());
    let meant: HashSet<_> = by_meaning.iter().map(key).collect();
    let worded: HashSet<_> = by_words.iter().map(key).collect();
    let (both, words_only): (Vec<Line>, Vec<Line>) =
        by_words.into_iter().partition(|l| meant.contains(&key(l)));
    let mut meaning_only = by_meaning.into_iter().filter(|l| !worded.contains(&key(l)));
    let mut fused = both;
    for line in words_only {
        fused.push(line);
        fused.extend(meaning_only.next());
    }
    fused.extend(meaning_only);
    fused
}

fn line_of(repo: &Option<String>, hit: Hit) -> Line {
    Line {
        repo: repo.clone(),
        path: hit.path,
        line: hit.symbol.line,
        kind: "text".into(),
        name: hit.symbol.name,
        scope: hit.scope,
        flags: hit.symbol.flags.join(" "),
    }
}

/// The query's words as path words are compared: identifiers split, then stemmed.
fn path_roots(query: &str, rules: &Search) -> Vec<String> {
    let mut roots = Vec::new();
    for word in significant(query, rules) {
        for part in split_identifier(&word) {
            push_unique(&mut roots, stem(&part, rules.stemmer));
        }
    }
    roots
}

/// The files whose path holds a query word, most distinct words first, then the shorter path:
/// (path, scope, distinct query words in the path).
pub fn by_path_words(
    files: Vec<(String, String)>,
    roots: &[String],
    rules: &Search,
) -> Vec<(String, String, usize)> {
    let stemmer = rules.stemmer.map(rust_stemmers::Stemmer::create);
    let mut matched: Vec<(usize, String, String)> = files
        .into_iter()
        .filter_map(|(path, scope)| {
            let words: HashSet<String> = path
                .split(['/', '.'])
                .flat_map(split_identifier)
                .map(|w| match &stemmer {
                    Some(s) => s.stem(&w).into_owned(),
                    None => w,
                })
                .collect();
            let count = roots.iter().filter(|r| words.contains(*r)).count();
            (count > 0).then_some((count, path, scope))
        })
        .collect();
    matched.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.len().cmp(&b.1.len())));
    matched
        .into_iter()
        .map(|(count, path, scope)| (path, scope, count))
        .collect()
}

#[cfg(test)]
#[path = "../../tests/unit/query/search.rs"]
mod tests;
