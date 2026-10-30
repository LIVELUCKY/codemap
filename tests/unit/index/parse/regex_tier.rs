use super::*;

/// The fallback tier with keywords a user configured; the default list is empty.
fn parse(source: &str) -> Vec<Symbol> {
    let rules = crate::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"parse": {"declarationKeywords": ["class", "struct", "fun", "function", "def"]}}}
    )])
    .unwrap()
    .parse;
    super::parse(source, &rules)
}

#[test]
fn link_literals_finds_markdown_targets() {
    let found = link_literals(
        "see [a](/docs/guide) and [b](https://x.y/z) and [c](#local)\n<https://q.r/s>\n",
    );
    let names: Vec<(&str, u32)> = found.iter().map(|s| (s.name.as_str(), s.line)).collect();
    assert_eq!(
        names,
        vec![
            ("/docs/guide", 1),
            ("https://x.y/z", 1),
            ("https://q.r/s", 2)
        ]
    );
}

#[test]
fn finds_definitions_in_elixir_lua_and_bash_like_text() {
    let source = "defmodule X do\n  def foo(a) do\n  end\nend\nfunction bar()\nend\nlocal function baz()\nend\n";
    let symbols = parse(source);
    let view: Vec<(&str, &str, u32)> = symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str(), s.line))
        .collect();
    assert_eq!(
        view,
        vec![
            ("foo", "def", 2),
            ("bar", "function", 5),
            ("baz", "function", 7)
        ]
    );
    assert!(symbols.iter().all(|s| s.role == Role::Definition));
}

#[test]
fn skips_keywords_inside_comments_and_strings() {
    let source =
        "// class NotOne\n\"def nope\"\nx = \"class X\"\n# def commented\n/* struct Y */\n";
    assert!(parse(source).is_empty());
}

#[test]
fn keyword_without_identifier_is_not_a_definition() {
    assert!(parse("class\nfun (\n").is_empty());
}
