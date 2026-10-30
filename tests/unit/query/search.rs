use super::*;

fn file(path: &str) -> Line {
    Line {
        repo: None,
        path: path.into(),
        line: 1,
        kind: "text".into(),
        name: path.into(),
        scope: "main".into(),
        flags: String::new(),
    }
}

#[test]
fn words_and_meaning_are_one_ranking() {
    let mut found = file("b.kt");
    found.line = 40;
    let by_words = vec![file("a.kt"), found, file("c.kt")];
    let by_meaning = vec![file("d.kt"), file("b.kt")];
    let fused = fuse(by_words, by_meaning);
    let paths: Vec<&str> = fused.iter().map(|l| l.path.as_str()).collect();
    assert_eq!(
        paths,
        ["b.kt", "a.kt", "d.kt", "c.kt"],
        "in both first, then the two take turns"
    );
    assert_eq!(fused[0].line, 40, "the line the words found is kept");
}

fn rules() -> &'static Search {
    &crate::config::settings::builtin().search
}

fn words(variant: &Variant) -> Vec<&str> {
    variant.words.iter().map(String::as_str).collect()
}

#[test]
fn by_default_the_query_is_searched_by_its_stems() {
    let variants = expand("check if the okhttp client is configured", rules());
    assert_eq!(variants.len(), 1, "no guessed synonyms or stop words");
    assert_eq!(
        words(&variants[0]),
        vec!["check", "if", "the", "okhttp", "client", "is", "configur"]
    );
    assert_eq!(variants[0].weight, 1.0);
}

#[test]
fn configured_stop_words_and_synonyms_apply() {
    let rules = crate::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"search": {"stopwords": ["the", "is"], "synonyms": {"client": ["httpclient"]}}}}
    )])
    .unwrap()
    .search;
    let variants = expand("the client is ready", &rules);
    assert_eq!(words(&variants[0]), vec!["client", "readi"]);
    assert_eq!(words(&variants[1]), vec!["httpclient"]);
    assert!(variants[1].weight < variants[0].weight);
}

#[test]
fn a_query_of_only_stop_words_keeps_its_words() {
    let rules = crate::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"search": {"stopwords": ["what", "is", "the"]}}}
    )])
    .unwrap()
    .search;
    assert_eq!(
        words(&expand("what is the", &rules)[0]),
        vec!["what", "is", "the"]
    );
}
