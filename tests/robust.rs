//! Real code is not tidy: truncated mid-edit, garbled, CRLF, tabs, a byte-order mark, one giant line.
//! Parsing must never panic and must keep what it can still read.

use std::fs;
use std::path::Path;

use codemap::index::parse::Language;
use codemap::model::Role;
use codemap::model::Symbol;

/// The language of a fixture that has a grammar or the fallback tier; `None` for plain text.
fn language_of(path: &str) -> Option<Language> {
    match codemap::index::parse::language_of(path) {
        Language::Other(ext) if ext == "text" => None,
        lang => Some(lang),
    }
}

fn parse(lang: &Language, source: &[u8]) -> Vec<Symbol> {
    codemap::index::parse::parse(lang, source, &codemap::config::settings::builtin().parse)
}

fn fixtures(dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            fixtures(&path, out);
        } else if let Some(name) = path.to_str().filter(|p| language_of(p).is_some()) {
            out.push((name.to_string(), fs::read(&path).unwrap()));
        }
    }
}

fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fixtures(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        &mut out,
    );
    assert!(
        out.len() > 40,
        "expected the whole fixture tree, found {}",
        out.len()
    );
    out
}

fn defined(lang: &Language, source: &[u8]) -> Vec<String> {
    parse(lang, source)
        .into_iter()
        .filter(|s| s.role == Role::Definition)
        .map(|s| s.name)
        .collect()
}

#[test]
fn truncated_at_any_point_never_panics() {
    for (path, bytes) in all_fixtures() {
        let lang = language_of(&path).unwrap();
        for tenth in 0..=10 {
            let cut = bytes.len() * tenth / 10;
            parse(&lang, &bytes[..cut]);
        }
        parse(&lang, &bytes[..bytes.len().saturating_sub(1)]);
    }
}

#[test]
fn garbled_in_the_middle_never_panics() {
    let garbage: &[&[u8]] = &[
        b"}{ ))) ;; @@ ``` <<<",
        b"\0\0\0",
        b"\xff\xfe\xfd",
        b"\"unterminated",
        b"/* open comment",
        b"<<<<<<< HEAD\n",
    ];
    for (path, bytes) in all_fixtures() {
        let lang = language_of(&path).unwrap();
        for junk in garbage {
            let mut broken = bytes[..bytes.len() / 2].to_vec();
            broken.extend_from_slice(junk);
            broken.extend_from_slice(&bytes[bytes.len() / 2..]);
            parse(&lang, &broken);
        }
        let stripes: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| if i % 7 == 0 { b'}' } else { *b })
            .collect();
        parse(&lang, &stripes);
    }
}

#[test]
fn empty_and_binary_input_never_panics() {
    for (path, _) in all_fixtures() {
        let lang = language_of(&path).unwrap();
        parse(&lang, b"");
        parse(&lang, &[0u8; 4096]);
        parse(&lang, &(0..=255u8).cycle().take(8192).collect::<Vec<_>>());
    }
}

const KOTLIN: &str = include_str!("fixtures/kotlin/Sample.kt");

#[test]
fn a_syntax_error_in_the_middle_keeps_definitions_on_both_sides_with_configured_keywords() {
    let broken = KOTLIN.replace(
        "expect fun platformName(): String",
        "expect fun platformName(: String ))) {{{",
    );
    let grammar_only = defined(&Language::Kotlin, broken.as_bytes());
    for before in ["FooPort", "FooRepositoryImpl"] {
        assert!(
            grammar_only.iter().any(|n| n == before),
            "{before} before the error survives with the grammar alone: {grammar_only:?}"
        );
    }
    // The scan that recovers what follows the error uses the fallback keywords, which are opt-in.
    let rules = codemap::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"parse": {"declarationKeywords": ["class", "interface", "object", "fun", "typealias", "val"]}}}
    )])
    .unwrap()
    .parse;
    let names: Vec<String> =
        codemap::index::parse::parse(&Language::Kotlin, broken.as_bytes(), &rules)
            .into_iter()
            .filter(|s| s.role == Role::Definition)
            .map(|s| s.name)
            .collect();
    for expected in [
        "FooPort",
        "FooRepositoryImpl",
        "State",
        "Book",
        "Loader",
        "Id",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} lost to a syntax error: {names:?}"
        );
    }
}

#[test]
fn crlf_tabs_bom_and_no_final_newline_change_nothing() {
    let original = defined(&Language::Kotlin, KOTLIN.as_bytes());
    let crlf = KOTLIN.replace('\n', "\r\n");
    assert_eq!(
        defined(&Language::Kotlin, crlf.as_bytes()),
        original,
        "CRLF"
    );
    let tabs = KOTLIN.replace("    ", "\t");
    assert_eq!(
        defined(&Language::Kotlin, tabs.as_bytes()),
        original,
        "tabs"
    );
    let bom = format!("\u{feff}{KOTLIN}");
    assert_eq!(
        defined(&Language::Kotlin, bom.as_bytes()),
        original,
        "byte-order mark"
    );
    assert_eq!(
        defined(&Language::Kotlin, KOTLIN.trim_end().as_bytes()),
        original,
        "no final newline"
    );
}

#[test]
fn crlf_keeps_line_numbers() {
    let crlf = KOTLIN.replace('\n', "\r\n");
    let lines = |source: &str| -> Vec<(String, u32)> {
        parse(&Language::Kotlin, source.as_bytes())
            .into_iter()
            .filter(|s| s.role == Role::Definition)
            .map(|s| (s.name, s.line))
            .collect()
    };
    assert_eq!(lines(&crlf), lines(KOTLIN));
}

#[test]
fn one_huge_line_finishes() {
    let line = "export const a = f(g(h(1)));".repeat(20_000);
    let started = std::time::Instant::now();
    parse(&Language::TypeScript, line.as_bytes());
    assert!(
        started.elapsed().as_secs() < 10,
        "a 580 KB single line took {:?}",
        started.elapsed()
    );
}

#[test]
fn deeply_nested_input_does_not_overflow_the_stack() {
    let nested = format!("{}1{}", "(".repeat(5_000), ")".repeat(5_000));
    parse(&Language::JavaScript, nested.as_bytes());
    parse(&Language::Python, nested.as_bytes());
    parse(
        &Language::Json,
        format!("{}1{}", "[".repeat(5_000), "]".repeat(5_000)).as_bytes(),
    );
}
