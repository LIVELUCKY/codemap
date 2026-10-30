//! Queries against the fixture workspace; see `common` for how it is materialised.

mod common;

use std::path::PathBuf;

use codemap::output::render;
use codemap::query;
use common::{lines, workspace};

#[test]
fn def_lists_project_before_siblings() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "BookRoute");
    let got = lines(&answer);
    assert_eq!(
        got[0],
        "src/commonMain/kotlin/app/BookRoute.kt:12 class BookRoute [main]"
    );
    assert_eq!(
        got[1],
        "../server/src/main/kotlin/server/Routes.kt:4 class BookRoute [main]"
    );
    assert_eq!(answer.source, "index");
    assert_eq!(answer.tier, "grammar");
}

#[test]
fn uses_ranks_main_before_test_and_counts_per_file() {
    let f = workspace(None);
    let answer = query::uses(&f.ws, "BookPort", false);
    let got = lines(&answer);
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:7 "),
        "{got:?}"
    );
    assert!(
        got.iter()
            .any(|l| l.starts_with("src/commonTest/kotlin/app/FakeBookPort.kt:3 ")),
        "{got:?}"
    );
    assert!(
        got.iter().position(|l| l.contains("commonTest"))
            > got
                .iter()
                .position(|l| l.contains("commonMain/kotlin/app/BookRoute.kt:7"))
    );
    assert_eq!(
        answer
            .lines
            .iter()
            .filter(|l| l.path.ends_with("BookRoute.kt"))
            .count(),
        1,
        "one line per file with a count"
    );
    assert!(
        answer.lines[0].flags.contains("1x") || answer.lines[0].flags.contains("x"),
        "{:?}",
        answer.lines[0].flags
    );
}

#[test]
fn impl_includes_test_fakes_last() {
    let f = workspace(None);
    let got = lines(&query::impl_(&f.ws, "BookPort"));
    assert_eq!(
        got,
        vec![
            "src/commonMain/kotlin/app/BookRoute.kt:7 class BookRepositoryImpl [main]",
            "src/commonTest/kotlin/app/FakeBookPort.kt:3 class FakeBookPort [test]"
        ]
    );
}

#[test]
fn variants_puts_expect_first_then_each_actual() {
    let f = workspace(None);
    let answer = query::variants(&f.ws, "platformName");
    let got = lines(&answer);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(
        got[0].starts_with("src/commonMain/") && answer.lines[0].flags.contains("expect"),
        "{got:?}"
    );
    assert!(
        got[1].starts_with("src/androidMain/") && got[2].starts_with("src/iosMain/"),
        "{got:?}"
    );
}

#[test]
fn link_groups_by_repository_with_enclosing_definition() {
    let f = workspace(None);
    let answer = query::link(&f.ws, "/api/v1/books/{id}");
    let got = lines(&answer);
    assert_eq!(got.len(), 3, "{got:?}");
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:11 literal"),
        "project first: {got:?}"
    );
    assert!(
        got[1].starts_with("../server/") && got[2].starts_with("../web/"),
        "{got:?}"
    );
    assert!(answer.lines[0].flags.contains("in:BookRoute"));
    assert_eq!(
        query::link(&f.ws, "api/v1/books/{id}").lines.len(),
        3,
        "without the leading slash too"
    );
}

#[test]
fn a_lookup_routes_paths_and_names_among_words_by_themselves() {
    use codemap::query::show::lookup;
    let f = workspace(None);
    let route = lookup(&f.ws, "api/v1/books/{id}");
    assert!(
        route.contains("BookRoute.kt:11 literal") && route.contains("../server/"),
        "{route}"
    );
    let named = lookup(&f.ws, "platformName expect declaration and implementations");
    for file in [
        "Platform.kt:3",
        "Platform.android.kt:3",
        "Platform.ios.kt:3",
    ] {
        assert!(named.contains(file), "{file}: {named}");
    }
    let words = lookup(&f.ws, "where is the book route defined");
    assert!(words.starts_with("# fts"), "{words}");
}

#[test]
fn outline_groups_by_configured_role() {
    let f = workspace(Some(r#"{"roles": {"Port": ["*Port"]}}"#));
    let answer = query::outline(&f.ws, "src/commonMain");
    let got = lines(&answer);
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:3 Port BookPort"),
        "{got:?}"
    );
    assert!(
        got[1].starts_with("src/commonMain/kotlin/app/BookRoute.kt:7 Port BookRepositoryImpl"),
        "{got:?}"
    );
    assert!(
        got.iter().any(|l| l.contains(" class BookRoute ")),
        "{got:?}"
    );
    assert!(!got.iter().any(|l| l.contains("commonTest")));
}

#[test]
fn cap_cuts_at_the_configured_limit_and_reports_the_rest() {
    let f = workspace(None);
    let limit = f.ws.config.settings.limits.answer_lines;
    assert_eq!(limit, 25);
    let mut answer = query::def(&f.ws, "same");
    assert_eq!(answer.lines.len(), 30);
    query::cap(&mut answer, false, limit);
    assert_eq!((answer.lines.len(), answer.cut), (25, 5));
    assert!(render(&answer).contains("... 5 more (--all)"));
    let mut all = query::def(&f.ws, "same");
    query::cap(&mut all, true, limit);
    assert_eq!((all.lines.len(), all.cut), (30, 0));
    let small = workspace(Some(r#"{"settings": {"limits": {"answerLines": 3}}}"#));
    assert_eq!(
        small.ws.config.settings.limits.answer_lines, 3,
        "a repository override applies"
    );
}

#[test]
fn no_match_is_explicit() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "Nope");
    assert!(answer.lines.is_empty());
    assert!(render(&answer).contains("no match"));
}

#[test]
fn render_matches_exact_format() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "FakeBookPort");
    assert_eq!(
        render(&answer),
        "# index (grammar)\nsrc/commonTest/kotlin/app/FakeBookPort.kt:3 class FakeBookPort [test]\n"
    );
}

#[test]
fn uses_reports_bridged_hits_with_label() {
    let f = workspace(None);
    let answer = query::uses(&f.ws, "platformName", true);
    let swift = answer
        .lines
        .iter()
        .find(|l| l.path == "ios/App.swift")
        .expect("the Swift call site");
    assert_eq!(swift.line, 5);
    assert!(
        swift.flags.contains("bridge:kmp-swift"),
        "{:?}",
        swift.flags
    );
    let grouped = query::uses(&f.ws, "platformName", false);
    let swift = grouped
        .lines
        .iter()
        .find(|l| l.path == "ios/App.swift")
        .expect("grouped Swift line");
    assert!(
        swift.flags.contains("bridge:kmp-swift"),
        "{:?}",
        swift.flags
    );
}

#[test]
fn render_carries_sibling_prefix_notes_and_flags() {
    let f = workspace(None);
    let answer = query::variants(&f.ws, "platformName");
    let text = render(&answer);
    assert!(text.starts_with("# index (grammar)\n"));
    assert!(
        text.contains("src/commonMain/kotlin/app/Platform.kt:3 function platformName expect\n"),
        "{text}"
    );
    let path = PathBuf::from("src/androidMain/kotlin/app/Platform.android.kt");
    assert!(
        text.contains(&format!(
            "{}:3 function platformName actual\n",
            path.display()
        )),
        "{text}"
    );
}
