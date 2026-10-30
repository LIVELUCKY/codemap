use super::*;

#[test]
fn identifiers_split_on_case_underscore_and_digits() {
    assert_eq!(split_identifier("BookRoute"), vec!["book", "route"]);
    assert_eq!(
        split_identifier("fetch_books_v2"),
        vec!["fetch", "books", "v", "2"]
    );
    assert_eq!(split_identifier("HTTPServer"), vec!["http", "server"]);
    assert_eq!(split_identifier("kebab-case"), vec!["kebab", "case"]);
    assert_eq!(split_identifier("x"), vec!["x"]);
}

#[test]
fn terms_hold_whole_identifiers_and_their_parts_with_counts() {
    let found = terms(
        &[],
        "class BookRoute(val id: String) // the route\n",
        1024,
        None,
    );
    let count = |w: &str| found.iter().find(|(t, _)| t == w).map(|(_, n)| *n);
    for expected in ["bookroute", "book", "class", "string", "the"] {
        assert_eq!(count(expected), Some(1), "{expected} in {found:?}");
    }
    assert_eq!(
        count("route"),
        Some(2),
        "the part of BookRoute and the comment word"
    );
    let cut = terms(&[], "alpha beta gamma", 5, None);
    assert!(cut.iter().all(|(t, _)| t == "alpha"), "{cut:?}");
}

#[test]
fn a_stemmer_makes_word_forms_meet() {
    let english = Some(Algorithm::English);
    let found = terms(&[], "fun refreshingTokens()", 1024, english);
    let has = |w: &str| found.iter().any(|(t, _)| t == w);
    assert!(
        has("refreshingtokens") && has("refresh") && has("token"),
        "{found:?}"
    );
    assert_eq!(query_term("refreshed", english), "refresh");
    assert_eq!(query_term("Tokens", english), "token");
    assert_eq!(
        query_term("RefreshToken", english),
        "refreshtoken",
        "a compound stays whole, as indexed"
    );
    assert_eq!(query_term("Tokens", None), "tokens");
}
