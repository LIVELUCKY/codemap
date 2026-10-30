use super::*;
use crate::model::{Role, Symbol};

fn def(name: &str, line: u32) -> Symbol {
    Symbol {
        name: name.into(),
        role: Role::Definition,
        kind: "class".into(),
        line,
        start_line: line,
        end_line: line,
        flags: vec![],
        supertypes: vec![],
    }
}

#[test]
fn links_in_strings_carry_the_enclosing_definition() {
    let source = "@Resource(\"/api/v1/books/{id}\")\nclass BookRoute(val id: String) {\n    val url = \"https://example.com/x\"\n    val plain = \"hello world\"\n}\nclass Other\nval top = '/top/level'\n";
    let defs = vec![def("BookRoute", 2), def("Other", 6)];
    let found = literals(&Language::Kotlin, source, &defs);
    let view: Vec<(&str, u32, &str)> = found
        .iter()
        .map(|s| (s.name.as_str(), s.line, s.flags[0].as_str()))
        .collect();
    assert_eq!(
        view,
        vec![
            ("/api/v1/books/{id}", 1, "in:BookRoute"),
            ("https://example.com/x", 3, "in:BookRoute"),
            ("/top/level", 7, "in:Other")
        ]
    );
    assert!(
        found
            .iter()
            .all(|s| s.role == Role::Literal && s.kind == "literal")
    );
}

#[test]
fn plain_strings_and_regex_like_text_are_not_literals() {
    let found = literals(
        &Language::Python,
        "x = \"hello\"\ny = \"a b/c\"\nz = '/'\n",
        &[],
    );
    assert!(found.is_empty(), "{found:?}");
}
