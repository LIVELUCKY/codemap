use super::*;

#[test]
fn language_of_maps_extensions_and_keeps_every_other_file() {
    assert_eq!(language_of("a/b.kt"), Language::Kotlin);
    assert_eq!(language_of("a/b.kts"), Language::Kotlin);
    assert_eq!(language_of("x.swift"), Language::Swift);
    assert_eq!(language_of("x.tsx"), Language::Tsx);
    assert_eq!(language_of("x.py"), Language::Python);
    assert_eq!(language_of("notes.md"), Language::Markdown);
    assert_eq!(
        language_of("x.unknownext"),
        Language::Other("unknownext".into())
    );
    assert_eq!(
        language_of("LICENSE"),
        Language::Other("text".into()),
        "whether it is text is decided by its content"
    );
    assert_eq!(language_of("ci/Makefile"), Language::Make, "by file name");
}

#[test]
fn every_language_is_regex_tier_until_a_grammar_arrives() {
    assert_eq!(tier(&Language::Other("ex".into())), Tier::Regex);
}

#[test]
fn the_regex_tier_finds_nothing_until_keywords_are_configured() {
    let lang = Language::Other("ex".into());
    let default = crate::config::settings::builtin().parse.clone();
    assert!(parse(&lang, b"def foo do\n", &default).is_empty());
    let configured = crate::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"parse": {"declarationKeywords": ["def"]}}}
    )])
    .unwrap()
    .parse;
    let symbols = parse(&lang, b"def foo do\n", &configured);
    assert_eq!(symbols.len(), 1);
    assert_eq!((symbols[0].name.as_str(), symbols[0].end_line), ("foo", 1));
}

#[test]
fn prose_has_headings_and_no_declarations() {
    let default = crate::config::settings::builtin().parse.clone();
    let text = b"Release notes\n=============\n\nThe function returns early.\n\nKnown issues\n------------\n| a | b |\n---\n";
    let found: Vec<(String, String, u32)> = parse(&Language::Other("txt".into()), text, &default)
        .into_iter()
        .map(|s| (s.name, s.kind, s.line))
        .collect();
    assert_eq!(
        found,
        [
            ("Release notes".to_string(), "heading".to_string(), 1),
            ("Known issues".to_string(), "heading".to_string(), 6)
        ]
    );
    let adoc = parse(
        &Language::Other("adoc".into()),
        b"= Guide\n\n== Install\n",
        &default,
    );
    assert_eq!(adoc.len(), 2);
    assert_eq!((adoc[1].name.as_str(), adoc[1].line), ("Install", 3));
}
