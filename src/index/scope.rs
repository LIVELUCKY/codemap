//! Where a file sits: main, test, generated or vendored. Generated and vendored come from git's
//! linguist attributes; the globs of `settings.scope` add to them and are the only source of `test`.

use crate::config::settings::Scope;
use crate::index::git::Linguist;

/// The ranking class of a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScopeClass {
    Main,
    Test,
    Generated,
    Vendored,
}

/// The scope of a repository-relative path: `vendored`, `generated`, `test` or `main`. Vendored and
/// generated win over test, so a test inside a vendored folder is still vendored.
pub fn scope_of(path: &str, linguist: Linguist, rules: &Scope) -> String {
    let [vendored, generated, test_dirs, test_files] = rules.globs();
    let segments: Vec<&str> = path.split('/').collect();
    let folders = segments.len().saturating_sub(1);
    let in_folder = |set: &globset::GlobSet| {
        (1..=folders)
            .any(|n| set.is_match(segments[n - 1]) || set.is_match(segments[..n].join("/")))
    };
    let file = segments.last().copied().unwrap_or("");
    if linguist.vendored || in_folder(vendored) {
        "vendored"
    } else if linguist.generated || in_folder(generated) {
        "generated"
    } else if in_folder(test_dirs) || test_files.is_match(file) || test_files.is_match(path) {
        "test"
    } else {
        "main"
    }
    .into()
}

pub fn class_of(scope: &str) -> ScopeClass {
    match scope {
        "vendored" => ScopeClass::Vendored,
        "generated" => ScopeClass::Generated,
        "test" => ScopeClass::Test,
        _ => ScopeClass::Main,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/index/scope.rs"]
mod tests;
