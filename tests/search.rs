//! Full-text search (FTS5, BM25) and fuzzy symbol suggestions over the fixture workspace.

mod common;

use codemap::output::render;
use codemap::query;
use codemap::query::search::search;
use common::{lines, workspace};

#[test]
fn search_ranks_file_with_more_matching_words_first() {
    let f = workspace(None);
    let answer = search(&f.ws, "book route", false);
    let got = lines(&answer);
    assert_eq!(answer.source, "fts");
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:"),
        "{got:?}"
    );
    assert!(
        got.iter().any(|l| l.starts_with("../server/")),
        "siblings are searched: {got:?}"
    );
    assert!(answer.lines.iter().all(|l| l.kind == "text"));
}

#[test]
fn a_siblings_best_file_outranks_weaker_project_files() {
    let f = workspace(None);
    let got = lines(&search(&f.ws, "book url", false));
    let at = |file: &str| got.iter().position(|l| l.starts_with(file));
    assert!(
        at("../web/src/api.ts:") < at("ios/App.swift:"),
        "one ranking over every repository: {got:?}"
    );
}

#[test]
fn camel_case_name_is_found_by_its_words() {
    let f = workspace(None);
    let got = lines(&search(&f.ws, "repository impl", false));
    assert!(
        got.first().is_some_and(|l| l.contains("BookRoute.kt")),
        "{got:?}"
    );
}

#[test]
fn a_misspelt_word_is_searched_as_the_closest_word_of_the_code_base() {
    let f = workspace(None);
    let answer = search(&f.ws, "boko ruote", false);
    let got = lines(&answer);
    assert!(
        got.first().is_some_and(|l| l.contains("BookRoute.kt")),
        "{got:?}"
    );
    assert!(
        answer
            .notes
            .iter()
            .any(|n| n.contains("searched for: book route")),
        "the agent is told the spelling: {:?}",
        answer.notes
    );
    assert!(search(&f.ws, "book route", false).notes.is_empty());
}

#[test]
fn search_with_no_hit_is_explicit() {
    let f = workspace(None);
    let answer = search(&f.ws, "ZzzzThing", false);
    assert!(answer.lines.is_empty());
    assert!(render(&answer).contains("no match"));
    // A description none of whose words is anywhere still gets the closest files, and is told so:
    // the agent judges them. One plain word is a description too.
    for described in ["zzzz qqqq", "frontend"] {
        let answer = search(&f.ws, described, false);
        assert!(!answer.lines.is_empty(), "{described}");
        assert_eq!(
            answer.notes,
            ["no file has these words; the closest by meaning:"],
            "{described}"
        );
    }
}

#[test]
fn a_name_that_is_not_there_answers_with_the_closest_definitions() {
    let f = workspace(None);
    for asked in ["BookRout", "BookRuote", "bookroute"] {
        let answer = query::def(&f.ws, asked);
        assert!(
            answer
                .lines
                .iter()
                .any(|l| l.name == "BookRoute" && l.path.ends_with("BookRoute.kt")),
            "{asked}: {:?}",
            answer.lines
        );
        assert!(
            answer.notes[0].starts_with(&format!("no definition named {asked}")),
            "{:?}",
            answer.notes
        );
    }
    assert_eq!(
        query::fuzzy(&f.ws, "BRepoImpl", 3)
            .first()
            .map(String::as_str),
        Some("BookRepositoryImpl")
    );
}

#[test]
fn a_name_like_nothing_falls_back_to_the_words() {
    let f = workspace(None);
    let answer = query::def(&f.ws, "proguard");
    assert!(
        answer.notes[0].contains("files with these words"),
        "{:?}",
        answer.notes
    );
}

#[test]
fn a_natural_language_question_finds_the_file() {
    let f = workspace(None);
    let answer = search(&f.ws, "where is the book route defined", false);
    let got = lines(&answer);
    assert!(
        got[0].starts_with("src/commonMain/kotlin/app/BookRoute.kt:"),
        "{got:?}"
    );
    assert!(
        answer.notes.is_empty(),
        "no rewrite noise: {:?}",
        answer.notes
    );
}

#[test]
fn files_are_found_by_name_and_by_the_words_of_their_path() {
    let f = workspace(None);
    let named = codemap::query::show::lookup(&f.ws, "libs.versions.toml");
    assert!(named.contains("libs.versions.toml:1 file"), "{named}");
    // An asset is asked for by its name, without the extension.
    let stem = codemap::query::show::lookup(&f.ws, "proguard-rules");
    assert!(stem.contains("proguard-rules.pro:1 file"), "{stem}");
    let by_path = codemap::query::show::lookup(&f.ws, "proguard rules");
    assert!(
        by_path
            .lines()
            .nth(1)
            .is_some_and(|l| l.contains("proguard-rules.pro")),
        "{by_path}"
    );
}

#[test]
fn the_rewrite_uses_the_vocabulary_of_the_code_base() {
    let f = workspace(None);
    let variants = codemap::query::search::rewrite(&f.ws, "repository");
    let grounded = variants
        .iter()
        .find(|v| v.words.iter().any(|w| w == "bookrepositoryimpl"));
    assert!(grounded.is_some(), "{variants:?}");
}

#[test]
fn search_hits_name_the_best_line() {
    let f = workspace(None);
    let answer = search(&f.ws, "platform name", false);
    let first = &answer.lines[0];
    assert!(
        first.path.contains("Platform") || first.path.ends_with("BookRoute.kt"),
        "{:?}",
        first
    );
    assert!(first.line >= 1);
}
