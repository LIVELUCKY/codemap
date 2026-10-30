//! The text search fallback over the fixture workspace.

mod common;

use std::path::Path;

use codemap::query::find::{FindOptions, find, find_with};
use common::{lines, workspace, workspace_with};

#[test]
fn find_ranks_main_hits_before_test_hits() {
    let f = workspace(None);
    let answer = find(&f.ws, "BookRoute", None, false);
    let got = lines(&answer);
    assert_eq!(answer.source, "grep");
    let first_test = got
        .iter()
        .position(|l| l.contains("[test]"))
        .expect("a test hit");
    let last_main = got
        .iter()
        .rposition(|l| !l.contains("[test]"))
        .expect("a main hit");
    assert!(last_main < first_test, "{got:?}");
    assert!(
        got.iter().any(|l| l.starts_with("../server/")),
        "siblings are searched too: {got:?}"
    );
}

#[test]
fn find_honours_codemapignore() {
    let open = workspace(None);
    assert_eq!(
        lines(&find(&open.ws, "VendoredThing", None, false)).len(),
        1
    );
    let closed = workspace_with(None, Some("vendor/\n"));
    assert!(lines(&find(&closed.ws, "VendoredThing", None, false)).is_empty());
}

#[test]
fn find_scoped_to_a_folder_searches_only_there() {
    let f = workspace(None);
    let answer = find(&f.ws, "BookPort", Some(Path::new("src/commonTest")), false);
    let got = lines(&answer);
    assert!(!got.is_empty());
    assert!(
        got.iter().all(|l| l.starts_with("src/commonTest/")),
        "{got:?}"
    );
}

#[test]
fn find_invalid_regex_is_an_explicit_error_line() {
    let f = workspace(None);
    let answer = find(&f.ws, "(", None, false);
    assert!(answer.lines.is_empty());
    assert!(
        answer
            .notes
            .iter()
            .any(|n| n.starts_with("invalid regex: ")),
        "{:?}",
        answer.notes
    );
}

#[test]
fn find_ignore_case_matches_other_cases() {
    let f = workspace(None);
    let opts = |ignore_case| FindOptions {
        pattern: "bookroute".into(),
        ignore_case,
        ..FindOptions::default()
    };
    assert!(find_with(&f.ws, &opts(false)).lines.is_empty());
    assert!(!find_with(&f.ws, &opts(true)).lines.is_empty());
}

#[test]
fn find_include_and_exclude_globs() {
    let f = workspace(None);
    let only_kotlin = FindOptions {
        pattern: "BookRoute".into(),
        include: vec!["*.kt".into()],
        ..FindOptions::default()
    };
    let got = lines(&find_with(&f.ws, &only_kotlin));
    assert!(
        !got.is_empty() && got.iter().all(|l| l.contains(".kt:")),
        "{got:?}"
    );
    let no_tests = FindOptions {
        pattern: "BookPort".into(),
        include: vec!["*.kt".into()],
        exclude: vec!["**/commonTest/**".into()],
        ..FindOptions::default()
    };
    let got = lines(&find_with(&f.ws, &no_tests));
    assert!(
        !got.is_empty() && got.iter().all(|l| !l.contains("commonTest")),
        "{got:?}"
    );
}

#[test]
fn find_searches_files_that_are_not_a_source_language() {
    let f = workspace(None);
    let got = lines(&find(&f.ws, "okhttp3", None, false));
    assert!(
        got.iter().any(|l| l.starts_with("proguard-rules.pro:1 ")),
        "{got:?}"
    );
    let toml = lines(&find(&f.ws, "ktor-client-okhttp", None, false));
    assert!(
        toml.iter().any(|l| l.starts_with("libs.versions.toml:1 ")),
        "{toml:?}"
    );
}

#[test]
fn find_says_what_it_covered() {
    let f = workspace(None);
    let everywhere = find(&f.ws, "BookRoute", None, false);
    assert!(
        everywhere
            .notes
            .iter()
            .any(|n| n == "all text files of 3 repositories, gitignored excluded, hidden excluded"),
        "{:?}",
        everywhere.notes
    );
    let scoped = find(&f.ws, "BookRoute", Some(Path::new("src")), false);
    assert!(
        scoped
            .notes
            .iter()
            .any(|n| n == "all text files of 1 repository, gitignored excluded, hidden excluded"),
        "{:?}",
        scoped.notes
    );
}

#[test]
fn gitignored_files_never_reach_an_answer() {
    let f = workspace(None);
    let root = &f.ws.project.root;
    std::fs::write(root.join(".gitignore"), "out/\n").unwrap();
    std::fs::create_dir_all(root.join("out")).unwrap();
    std::fs::write(
        root.join("out/Gen.kt"),
        "class BookRoute\nval GENERATED_ONLY = 1\n",
    )
    .unwrap();
    std::fs::write(root.join("Loose.kt"), "val UNTRACKED_BUT_KEPT = 1\n").unwrap();
    let text = |a: &codemap::output::Answer| codemap::output::render(a);
    assert!(text(&find(&f.ws, "GENERATED_ONLY", None, false)).contains("no match"));
    assert!(!text(&codemap::query::def(&f.ws, "BookRoute")).contains("out/Gen.kt"));
    assert!(
        text(&find(&f.ws, "UNTRACKED_BUT_KEPT", None, false)).contains("Loose.kt:1"),
        "a new file that is not ignored is searched"
    );
}

#[test]
fn find_files_only_lists_each_file_once_with_its_match_count() {
    let f = workspace(None);
    let options = FindOptions {
        pattern: "okhttp".into(),
        files_only: true,
        ..FindOptions::default()
    };
    let answer = find_with(&f.ws, &options);
    let pro: Vec<_> = answer
        .lines
        .iter()
        .filter(|l| l.path == "proguard-rules.pro")
        .collect();
    assert_eq!(pro.len(), 1);
    assert_eq!(
        (pro[0].kind.as_str(), pro[0].name.as_str(), pro[0].line),
        ("file", "2 matches", 1)
    );
}

#[test]
fn find_names_the_matching_line_text() {
    let f = workspace(None);
    let answer = find(&f.ws, "BOOK_URL = ", None, false);
    let hit = answer
        .lines
        .iter()
        .find(|l| l.repo.as_deref() == Some("../web"))
        .expect("web hit");
    assert_eq!((hit.path.as_str(), hit.line), ("src/api.ts", 1));
    assert_eq!(hit.name, "export const BOOK_URL = \"/api/v1/books/{id}\";");
}

#[test]
fn a_scope_outside_the_repository_is_refused() {
    let f = workspace(None);
    for outside in ["..", "/", "src/../.."] {
        let answer = find(&f.ws, ".", Some(std::path::Path::new(outside)), false);
        assert!(answer.lines.is_empty(), "{outside}: {:?}", answer.lines);
        assert!(
            answer
                .notes
                .iter()
                .any(|n| n.contains("not a folder of this repository")),
            "{outside}: {:?}",
            answer.notes
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_is_not_followed_out_of_the_repository() {
    let f = workspace(None);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "outside_only_marker\n").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        f.ws.project.root.join("link.txt"),
    )
    .unwrap();
    assert!(lines(&find(&f.ws, "outside_only_marker", None, false)).is_empty());
    let words = codemap::query::search::search(&f.ws, "outside_only_marker", false);
    assert!(
        words
            .notes
            .iter()
            .any(|n| n.contains("no file has these words")),
        "the link's target is not indexed: {:?}",
        words.notes
    );
}
