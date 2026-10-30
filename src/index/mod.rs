//! Reconcile a repository's index with its working tree: a pure function of the git tree.
//!
//! `git ls-files -s` gives every tracked path with its blob sha; `git status --porcelain` gives what is
//! modified, untracked or deleted. A clean tracked file is current when its sha is already parsed; a
//! branch switch therefore costs one git call plus the blobs never seen. Modified and untracked files are
//! keyed by size and mtime and hashed like git does, so their parse is reused after a commit.
//!
//! A light parse (siblings) stores no references and is recorded under `light:<sha>`, so a later full
//! reconcile parses the blob again.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ignore::gitignore::Gitignore;
use rayon::prelude::*;

pub mod embed;
pub mod git;
pub mod parse;
pub mod scope;
pub mod store;
pub mod text;

use crate::config;
use crate::config::settings::Settings;
use crate::model::Role;
use crate::workspace::{Repo, Workspace};
use parse::{Language, language_of, parse};
use scope::scope_of;
use store::{FileRow, ParsedBlob, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Full,
    Light,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub parsed: usize,
    pub reused: usize,
    pub skipped: usize,
    pub removed: usize,
    pub millis: u128,
    /// Where the time went: asking git for the tree, parsing, writing the store.
    pub git_millis: u128,
    pub parse_millis: u128,
    pub write_millis: u128,
}

pub fn reconcile(
    repo: &Repo,
    store: &mut Store,
    ignore: &Gitignore,
    depth: Depth,
    settings: &Settings,
) -> Result<Stats> {
    let start = Instant::now();
    let mut stats = Stats::default();
    // Parses made under other parse settings (keywords, size limits) are not reused.
    let fingerprint = settings.index_fingerprint();
    if store.meta("settings").as_deref() != Some(fingerprint.as_str()) {
        store.clear()?;
        store.set_meta("settings", &fingerprint)?;
    }
    // Three independent git processes; running them side by side costs the slowest, not the sum.
    let (tree, dirty_list, head) = std::thread::scope(|s| {
        let tree = s.spawn(|| git::tree(&repo.root));
        let dirty = s.spawn(|| git::dirty(&repo.root));
        let head = s.spawn(|| git::head(&repo.root));
        (tree.join(), dirty.join(), head.join())
    });
    let tree = tree.map_err(|_| anyhow::anyhow!("git tree thread panicked"))??;
    let dirty_list = dirty_list.map_err(|_| anyhow::anyhow!("git status thread panicked"))??;
    let head = head.map_err(|_| anyhow::anyhow!("git head thread panicked"))?;
    let dirty: HashMap<String, bool> = dirty_list
        .into_iter()
        .map(|d| (d.path, d.deleted))
        .collect();
    let previous: HashMap<String, FileRow> = store
        .files()?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    // path -> blob sha when the file is clean and tracked, None when it must be read
    let mut paths: BTreeMap<String, Option<String>> = BTreeMap::new();
    for tracked in tree {
        match dirty.get(&tracked.path) {
            Some(true) => {}
            Some(false) => {
                paths.insert(tracked.path, None);
            }
            None => {
                paths.insert(tracked.path, Some(tracked.sha));
            }
        }
    }
    for (path, deleted) in &dirty {
        if !deleted {
            paths.entry(path.clone()).or_insert(None);
        }
    }

    paths.retain(|path, _| !ignore.matched_path_or_any_parents(path, false).is_ignore());
    let names: Vec<&str> = paths.keys().map(String::as_str).collect();
    let linguist = git::linguist(&repo.root, &names)?;
    stats.git_millis = start.elapsed().as_millis();

    let mut rows = Vec::new();
    let mut to_parse: Vec<(String, Language, Vec<u8>, String)> = Vec::new();
    let mut queued: HashSet<String> = HashSet::new();
    for (path, clean_sha) in paths {
        let lang = language_of(&path);
        let full = repo.root.join(&path);
        let (sha, dirty_key) = match clean_sha {
            Some(sha) => (sha, None),
            None => {
                let meta =
                    fs::metadata(&full).with_context(|| format!("stat {}", full.display()))?;
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let key = format!("{}:{modified}", meta.len());
                match previous.get(&path) {
                    Some(prev) if prev.dirty_key.as_deref() == Some(key.as_str()) => {
                        (prev.sha.trim_start_matches("light:").to_string(), Some(key))
                    }
                    _ => (git::blob_sha(&fs::read(&full)?), Some(key)),
                }
            }
        };
        let key = match depth {
            Depth::Full => sha,
            Depth::Light if store.has_blob(&sha) => sha,
            Depth::Light => format!("light:{sha}"),
        };
        if store.has_blob(&key) || queued.contains(&key) {
            stats.reused += 1;
        } else {
            // Above the size limit (known without reading it), not UTF-8 or holding a NUL byte: not
            // text to parse. The file is still indexed by its path, with an empty blob so it is not
            // read again: an image or a font is found by name and by the words of its path.
            let limit = settings.limits.index_file_bytes as u64;
            // A link is not followed either: it may point at any file outside the repository.
            let mut bytes = match fs::symlink_metadata(&full) {
                Ok(meta) if meta.len() > limit || meta.file_type().is_symlink() => None,
                _ => Some(fs::read(&full).with_context(|| format!("read {}", full.display()))?),
            };
            if bytes
                .as_ref()
                .is_some_and(|b| b.contains(&0) || std::str::from_utf8(b).is_err())
            {
                bytes = None;
            }
            if bytes.is_none() {
                stats.skipped += 1;
            }
            queued.insert(key.clone());
            to_parse.push((key.clone(), lang, bytes.unwrap_or_default(), path.clone()));
        }
        let scope = scope_of(
            &path,
            linguist.get(&path).copied().unwrap_or_default(),
            &settings.scope,
        );
        rows.push(FileRow {
            path,
            sha: key,
            scope,
            dirty_key,
        });
    }

    let parse_start = Instant::now();
    let parsed: Vec<ParsedBlob> = to_parse
        .into_par_iter()
        .map(|(key, lang, bytes, path)| {
            let mut symbols = parse(&lang, &bytes, &settings.parse);
            if depth == Depth::Light {
                symbols.retain(|s| s.role != Role::Reference);
            }
            let terms = std::str::from_utf8(&bytes)
                .map(|t| {
                    text::terms(
                        &symbols,
                        t,
                        settings.limits.text_bytes,
                        settings.search.stemmer,
                    )
                })
                .unwrap_or_default();
            // A blob at two paths carries the vector of the first; the same content under another
            // name is rare and still close.
            let text = std::str::from_utf8(&bytes).unwrap_or_default();
            let vector = (settings.search.semantic_files > 0)
                .then(|| embed::embed(&embed::document(&path, text)))
                .flatten();
            ParsedBlob {
                key,
                language: lang.name(),
                symbols,
                terms,
                vector,
            }
        })
        .collect();
    stats.parsed = parsed.len() - stats.skipped;
    stats.parse_millis = parse_start.elapsed().as_millis();
    let kept: HashSet<&str> = rows.iter().map(|r| r.path.as_str()).collect();
    let removed: Vec<String> = previous
        .keys()
        .filter(|p| !kept.contains(p.as_str()))
        .cloned()
        .collect();
    stats.removed = removed.len();
    // Only rows that differ are written; a refresh with nothing new writes nothing.
    let changed: Vec<FileRow> = rows
        .into_iter()
        .filter(|r| previous.get(&r.path) != Some(r))
        .collect();
    let write_start = Instant::now();
    store.apply(&parsed, &changed, &removed)?;
    stats.write_millis = write_start.elapsed().as_millis();
    if let Some(head) = head {
        store.set_meta("head", &head)?;
    }
    stats.millis = start.elapsed().as_millis();
    Ok(stats)
}

/// Runs `work` on a pool of `settings.limits.threads` threads (0: one per core, the default pool).
pub fn with_threads<T: Send>(settings: &Settings, work: impl FnOnce() -> T + Send) -> T {
    match settings.limits.threads {
        0 => work(),
        n => match rayon::ThreadPoolBuilder::new().num_threads(n).build() {
            Ok(pool) => pool.install(work),
            Err(_) => work(),
        },
    }
}

/// Reconciles the project in full and the siblings lightly, all repositories at the same time; a
/// sibling not started within `budget` is left to the first query. `full` discards every cached parse
/// of the project first.
pub fn build(ws: &Workspace, full: bool, budget: Option<Duration>) -> Result<Vec<(String, Stats)>> {
    let start = Instant::now();
    let settings = &ws.config.settings;
    // Session starts and explicit builds clean the cache; queries never pay for it.
    store::prune(
        &ws.cache_dir,
        Duration::from_secs(settings.limits.cache_days * 24 * 60 * 60),
    );
    let repos: Vec<(&Repo, Depth)> = std::iter::once((&ws.project, Depth::Full))
        .chain(ws.siblings.iter().map(|s| (s, Depth::Light)))
        .collect();
    let reports: Vec<Option<Result<(String, Stats)>>> = with_threads(settings, || {
        repos
            .par_iter()
            .map(|(repo, depth)| {
                let late = *depth == Depth::Light && budget.is_some_and(|b| start.elapsed() >= b);
                if late {
                    return None;
                }
                Some((|| {
                    let mut store = Store::open(&ws.cache_dir, &repo.root)?;
                    if full && *depth == Depth::Full {
                        store.clear()?;
                    }
                    let ignore = config::ignore(&repo.root);
                    let stats = reconcile(repo, &mut store, &ignore, *depth, settings)?;
                    Ok((repo.name.clone(), stats))
                })())
            })
            .collect()
    });
    reports.into_iter().flatten().collect()
}

/// Re-reads the given files only (the PostToolUse hook after an edit): a reconcile scoped by the
/// whole tree is cheap enough that the paths are only used to decide whether anything is worth doing.
pub fn update(ws: &Workspace, paths: &[&Path]) -> Result<Option<Stats>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut store = Store::open(&ws.cache_dir, &ws.project.root)?;
    Ok(Some(reconcile(
        &ws.project,
        &mut store,
        &config::ignore(&ws.project.root),
        Depth::Full,
        &ws.config.settings,
    )?))
}
