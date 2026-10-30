use super::*;

#[test]
fn the_defaults_parse_and_carry_the_prompts() {
    let s = builtin();
    assert_eq!(s.limits.answer_lines, 25);
    assert!(
        s.search.synonyms.is_empty() && s.search.stopwords.is_empty(),
        "nothing guessed"
    );
    assert!(s.parse.declaration_keywords.is_empty());
    assert!(
        s.prompts
            .session
            .starts_with("IMPORTANT: search code with `codemap`")
    );
    assert!(s.agents.iter().any(|a| a.name == "claude"));
}

#[test]
fn overrides_merge_tables_and_replace_lists() {
    let repo = json!({"settings": {"limits": {"answerLines": 40}, "scope": {"testDirs": ["e2e"]},
            "search": {"synonyms": {"flag": ["toggle"]}}}, "prompts": {"session": "short"}});
    let global = json!({"settings": {"limits": {"answerLines": 10, "nearestNames": 2}}});
    let s = resolve(&[&global, &repo]).unwrap();
    assert_eq!(s.limits.answer_lines, 40, "the repository wins");
    assert_eq!(s.limits.nearest_names, 2, "the global layer applies");
    assert_eq!(s.limits.cache_days, 14, "untouched keys keep the default");
    assert_eq!(s.scope.test_dirs, ["e2e"]);
    assert_eq!(s.search.synonyms["flag"], ["toggle"]);
    assert_eq!(s.prompts.session, "short");
    assert!(
        resolve(&[&json!({"settings": {"limits": {"answerLine": 3}}})]).is_err(),
        "a typo is reported"
    );
}

#[test]
fn only_parse_settings_change_the_index_fingerprint() {
    let base = builtin().index_fingerprint();
    let ranking = resolve(&[&json!({"settings": {"limits": {"answerLines": 3}}})]).unwrap();
    assert_eq!(ranking.index_fingerprint(), base);
    let keywords =
        resolve(&[&json!({"settings": {"parse": {"declarationKeywords": ["proc"]}}})]).unwrap();
    assert_ne!(keywords.index_fingerprint(), base);
}
