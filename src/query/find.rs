//! The text search: the ripgrep engine over the workspace, ranked and capped like every other answer,
//! each hit printed as grep prints it. It honours what an agent's
//! grep carries (case, file lists, include and exclude globs, file types, hidden and tracked files), so
//! a hook can answer those searches with the same matches instead of letting a shell run them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;

use crate::config;
use crate::index::git;
use crate::index::parse::language_of;
use crate::index::scope::scope_of;
use crate::model::{Hit, Role, Symbol};
use crate::output::{Answer, Line};
use crate::query::{Located, cap, rank};
use crate::workspace::{Repo, Workspace};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindOptions {
    /// A Rust regex, matched per line.
    pub pattern: String,
    /// Limits the search to a folder of the project (empty: the project root); without it the
    /// siblings are searched too.
    pub scope: Option<PathBuf>,
    pub ignore_case: bool,
    /// Case-insensitive when the pattern has no upper-case literal (`rg -S`).
    pub smart_case: bool,
    /// One line per file instead of one per matching line (`grep -l`).
    pub files_only: bool,
    /// Only files matching one of these globs (`--include`, `-g`, `--type`).
    pub include: Vec<String>,
    /// Never files matching one of these globs.
    pub exclude: Vec<String>,
    /// Hidden files and folders too (`grep -r`, `git grep`, `rg --hidden`).
    pub hidden: bool,
    /// Only files git tracks (`git grep`).
    pub tracked: bool,
    pub all: bool,
}

/// The globs of a ripgrep file type name (`kotlin`, `ts`, ...), from ripgrep's own type table;
/// `None` for a name ripgrep does not know either.
pub fn type_globs(name: &str) -> Option<Vec<String>> {
    let mut types = ignore::types::TypesBuilder::new();
    types.add_defaults();
    types
        .definitions()
        .into_iter()
        .find(|t| t.name() == name)
        .map(|t| t.globs().to_vec())
}

fn glob_set(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().ok()
}

/// A glob matches the repository-relative path or the bare file name.
fn glob_matches(set: &GlobSet, relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    set.is_match(relative) || set.is_match(name)
}

/// `scope` limits the search to a folder of the project; without it the siblings are searched too.
pub fn find(ws: &Workspace, pattern: &str, scope: Option<&Path>, all: bool) -> Answer {
    find_with(
        ws,
        &FindOptions {
            pattern: pattern.into(),
            scope: scope.map(Path::to_path_buf),
            all,
            ..FindOptions::default()
        },
    )
}

/// Every matching line of one repository, scoped by git's linguist attributes and the scope globs.
fn search_repo(
    ws: &Workspace,
    options: &FindOptions,
    matcher: &grep_regex::RegexMatcher,
    repo_name: &Option<String>,
    repo: &Repo,
) -> Vec<Located> {
    let root = options
        .scope
        .as_ref()
        .map(|s| repo.root.join(s))
        .unwrap_or_else(|| repo.root.clone());
    let include = glob_set(&options.include);
    let exclude = glob_set(&options.exclude);
    let tracked: Option<HashSet<String>> = options.tracked.then(|| {
        git::tree(&repo.root)
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.path)
            .collect()
    });
    let ignore = config::ignore(&repo.root);
    let line_chars = ws.config.settings.limits.find_line_chars;
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0))
        .build();
    let mut hits: Vec<Located> = Vec::new();
    let walk = WalkBuilder::new(&root)
        .hidden(!options.hidden)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walk.flatten() {
        let path = entry.path();
        // A link is never followed (as `grep -r` and `rg` do not): a committed link may point at
        // any file outside the repository.
        if entry.path_is_symlink() || !path.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(&repo.root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        if ignore.matched_path_or_any_parents(&rel, false).is_ignore()
            || tracked.as_ref().is_some_and(|t| !t.contains(&rel))
            || include.as_ref().is_some_and(|set| !glob_matches(set, &rel))
            || exclude.as_ref().is_some_and(|set| glob_matches(set, &rel))
        {
            continue;
        }
        let language = language_of(&rel).name();
        let sink = UTF8(|number, text| {
            let name: String = text.trim().chars().take(line_chars).collect();
            let line = number as u32;
            hits.push((
                repo_name.clone(),
                Hit {
                    repo: repo_name.clone().unwrap_or_default(),
                    path: rel.clone(),
                    scope: String::new(),
                    language: language.clone(),
                    symbol: Symbol {
                        name,
                        role: Role::Reference,
                        kind: "text".into(),
                        line,
                        start_line: line,
                        end_line: line,
                        flags: Vec::new(),
                        supertypes: Vec::new(),
                    },
                },
            ));
            Ok(true)
        });
        let _ = searcher.search_path(matcher, path, sink);
    }
    let paths: Vec<&str> = {
        let mut seen: Vec<&str> = hits.iter().map(|(_, h)| h.path.as_str()).collect();
        seen.dedup();
        seen
    };
    let linguist = git::linguist(&repo.root, &paths).unwrap_or_default();
    let scopes: HashMap<String, String> = paths
        .iter()
        .map(|p| {
            let attributes = linguist.get(*p).copied().unwrap_or_default();
            (
                p.to_string(),
                scope_of(p, attributes, &ws.config.settings.scope),
            )
        })
        .collect();
    for (_, hit) in &mut hits {
        hit.scope = scopes[&hit.path].clone();
    }
    hits
}

pub fn find_with(ws: &Workspace, options: &FindOptions) -> Answer {
    let mut answer = Answer::new("grep");
    let matcher = match RegexMatcherBuilder::new()
        .case_insensitive(options.ignore_case)
        .case_smart(options.smart_case)
        // As ripgrep builds it: `^` and `$` match at every line, and no match spans a line end.
        .multi_line(true)
        .line_terminator(Some(b'\n'))
        .build(&options.pattern)
    {
        Ok(m) => m,
        Err(e) => {
            answer.notes.push(format!("invalid regex: {e}"));
            return answer;
        }
    };
    // A scope is a folder of the project: `..` or an absolute path would search, and print, files
    // outside the workspace.
    if let Some(scope) = &options.scope {
        let inside = ws
            .project
            .root
            .join(scope)
            .canonicalize()
            .is_ok_and(|full| full.starts_with(&ws.project.root));
        if !inside {
            answer.notes.push(format!(
                "{} is not a folder of this repository",
                scope.display()
            ));
            return answer;
        }
    }
    let mut targets: Vec<(Option<String>, &Repo)> = vec![(None, &ws.project)];
    if options.scope.is_none() {
        targets.extend(ws.siblings.iter().map(|s| (Some(s.name.clone()), s)));
    }
    let mut hits: Vec<Located> = Vec::new();
    for (repo_name, repo) in &targets {
        hits.extend(search_repo(ws, options, &matcher, repo_name, repo));
    }
    rank(&mut hits);
    let lines = hits.iter().map(|(repo, hit)| Line {
        repo: repo.clone(),
        path: hit.path.clone(),
        line: hit.symbol.line,
        kind: String::new(),
        name: hit.symbol.name.clone(),
        scope: hit.scope.clone(),
        flags: String::new(),
    });
    answer.lines = if options.files_only {
        files_only(lines.collect())
    } else {
        lines.collect()
    };
    // Said every time, so a caller never has to hedge about what a result covers or re-filter it.
    let repositories = targets.len();
    answer.notes.push(format!(
        "all text files of {repositories} repositor{}, gitignored excluded{}",
        if repositories == 1 { "y" } else { "ies" },
        if options.hidden {
            ""
        } else {
            ", hidden excluded"
        }
    ));
    cap(
        &mut answer,
        options.all,
        ws.config.settings.limits.answer_lines,
    );
    answer
}

/// One line per file, in the order of the first hit: the line of that hit and how many there were.
fn files_only(lines: Vec<Line>) -> Vec<Line> {
    let mut files: Vec<(Line, usize)> = Vec::new();
    for line in lines {
        match files
            .iter_mut()
            .find(|(f, _)| f.repo == line.repo && f.path == line.path)
        {
            Some((_, count)) => *count += 1,
            None => files.push((line, 1)),
        }
    }
    files
        .into_iter()
        .map(|(mut line, count)| {
            line.kind = "file".into();
            line.name = if count == 1 {
                "1 match".into()
            } else {
                format!("{count} matches")
            };
            line
        })
        .collect()
}
