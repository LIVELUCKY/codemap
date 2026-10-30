use super::*;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn regex_of(program: &str, words: &[&str]) -> Option<String> {
    parse_grep(program, &args(words))?.regex()
}

#[test]
fn basic_expressions_swap_operators_and_literals() {
    let basic = |p| posix_to_rust(p, false);
    assert_eq!(basic(r"a\|b").as_deref(), Some("a|b"));
    assert_eq!(
        basic("a|b").as_deref(),
        Some(r"a\|b"),
        "a bare bar is a literal"
    );
    assert_eq!(basic(r"\(ab\)\+").as_deref(), Some("(ab)+"));
    assert_eq!(basic("(ab)+").as_deref(), Some(r"\(ab\)\+"));
    assert_eq!(basic(r"a\{2,3\}").as_deref(), Some("a{2,3}"));
    assert_eq!(basic("a{2}").as_deref(), Some(r"a\{2\}"));
    assert_eq!(
        basic("*x").as_deref(),
        Some(r"\*x"),
        "a leading star is a literal"
    );
    assert_eq!(basic("^*x").as_deref(), Some(r"^\*x"));
    assert_eq!(basic("x*").as_deref(), Some("x*"));
    assert_eq!(
        basic("a^b").as_deref(),
        Some(r"a\^b"),
        "^ anchors only at the start"
    );
    assert_eq!(
        basic("a$b").as_deref(),
        Some(r"a\$b"),
        "$ anchors only at the end"
    );
    assert_eq!(basic("foo.*bar$").as_deref(), Some("foo.*bar$"));
}

#[test]
fn extended_expressions_keep_their_operators() {
    let extended = |p| posix_to_rust(p, true);
    assert_eq!(extended("a|b").as_deref(), Some("a|b"));
    assert_eq!(extended("(ab)+c?").as_deref(), Some("(ab)+c?"));
    assert_eq!(extended("x{2,}").as_deref(), Some("x{2,}"));
    assert!(extended("[[:alpha:]]+").is_some());
}

#[test]
fn what_cannot_be_said_exactly_is_refused() {
    assert_eq!(posix_to_rust(r"\(a\)\1", false), None, "back references");
    assert_eq!(posix_to_rust(r"\(a", false), None, "an open group");
    assert_eq!(posix_to_rust("a)", true), None);
    assert_eq!(posix_to_rust(r"\`a", false), None, "GNU buffer anchors");
}

#[test]
fn each_program_reads_its_own_dialect() {
    assert_eq!(
        regex_of("grep", &["-r", r"a\|b", "."]).as_deref(),
        Some("a|b")
    );
    assert_eq!(
        regex_of("grep", &["-rE", "a|b", "."]).as_deref(),
        Some("a|b")
    );
    assert_eq!(
        regex_of("egrep", &["-r", "a|b", "."]).as_deref(),
        Some("a|b")
    );
    assert_eq!(
        regex_of("rg", &[r"a\|b"]).as_deref(),
        Some(r"a\|b"),
        "rg is the Rust dialect"
    );
    assert_eq!(
        regex_of("grep", &["-rF", "a.b", "."]).as_deref(),
        Some(r"a\.b")
    );
    assert_eq!(regex_of("rg", &["-x", "ab"]).as_deref(), Some("^(?:ab)$"));
    assert!(regex_of("rg", &["-w", "ab"]).unwrap().contains("(?:ab)"));
}

#[test]
fn case_options_follow_each_program() {
    let rg_smart = parse_grep("rg", &args(&["-S", "abc"])).unwrap();
    assert!(rg_smart.options.smart_case && !rg_smart.options.ignore_case);
    let grep_i = parse_grep("grep", &args(&["-ri", "abc", "."])).unwrap();
    assert!(grep_i.options.ignore_case);
    let later_wins = parse_grep("rg", &args(&["-i", "-s", "abc"])).unwrap();
    assert!(!later_wins.options.ignore_case && !later_wins.options.smart_case);
}

#[test]
fn unknown_options_and_pcre_are_not_reproduced() {
    assert!(parse_grep("grep", &args(&["-rP", r"\d+", "."])).is_none());
    assert!(parse_grep("rg", &args(&["--pcre2", "x"])).is_none());
    assert!(parse_grep("grep", &args(&["--no-such-flag", "x", "."])).is_none());
}

#[test]
fn globs_types_and_paths_are_kept() {
    let rg = parse_grep(
        "rg",
        &args(&["-g", "!*Test.kt", "-t", "kotlin", "foo", "src"]),
    )
    .unwrap();
    assert_eq!(rg.options.exclude, ["*Test.kt"]);
    assert_eq!(rg.options.include, ["*.kt", "*.kts"]);
    assert_eq!(rg.paths, ["src"]);
    let grep = parse_grep("grep", &args(&["-r", "--exclude-dir=build", "foo", "."])).unwrap();
    assert_eq!(grep.options.exclude, ["**/build/**"]);
    assert!(parse_grep("rg", &args(&["-t", "nosuchtype", "x"])).is_none());
}
