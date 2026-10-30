use super::*;
use crate::index::parse::{Language, Tier, tier};
use crate::model::{Role, Symbol};

const KOTLIN: &str = include_str!("../../../fixtures/kotlin/Sample.kt");

fn kotlin() -> Vec<Symbol> {
    let grammar = grammar_for(&Language::Kotlin).expect("kotlin grammar");
    parse(&grammar, KOTLIN.as_bytes())
}

fn def<'a>(symbols: &'a [Symbol], name: &str) -> &'a Symbol {
    symbols
        .iter()
        .find(|s| s.role == Role::Definition && s.name == name)
        .unwrap_or_else(|| panic!("no definition {name} in {symbols:?}"))
}

#[test]
fn kotlin_definitions_with_kinds() {
    let symbols = kotlin();
    let view = |name: &str| {
        let s = def(&symbols, name);
        (s.kind.clone(), s.line)
    };
    assert_eq!(view("FooPort"), ("interface".into(), 5));
    assert_eq!(view("FooRepositoryImpl"), ("class".into(), 11));
    assert_eq!(view("platformName"), ("function".into(), 15));
    assert_eq!(view("Idle"), ("object".into(), 18));
    assert_eq!(view("Loaded"), ("class".into(), 19));
    assert_eq!(view("load"), ("function".into(), 29));
    assert_eq!(view("Id"), ("typealias".into(), 35));
}

#[test]
fn kotlin_supertypes_and_modifiers() {
    let symbols = kotlin();
    assert_eq!(
        def(&symbols, "FooRepositoryImpl").supertypes,
        vec!["FooPort", "BaseRepository"]
    );
    assert_eq!(def(&symbols, "Loaded").supertypes, vec!["State"]);
    assert!(def(&symbols, "State").flags.contains(&"sealed".to_string()));
    assert!(
        def(&symbols, "platformName")
            .flags
            .contains(&"expect".to_string())
    );
    assert!(def(&symbols, "Book").flags.contains(&"data".to_string()));
    assert!(def(&symbols, "load").flags.contains(&"suspend".to_string()));
}

#[test]
fn kotlin_annotations_land_in_flags() {
    let symbols = kotlin();
    let flags = &def(&symbols, "BookRoute").flags;
    assert!(flags.contains(&"@Serializable".to_string()), "{flags:?}");
    assert!(flags.contains(&"@Resource".to_string()), "{flags:?}");
}

#[test]
fn kotlin_call_references() {
    let symbols = kotlin();
    let calls: Vec<(&str, u32)> = symbols
        .iter()
        .filter(|s| s.role == Role::Reference && s.kind == "call")
        .map(|s| (s.name.as_str(), s.line))
        .collect();
    assert!(calls.contains(&("fetch", 30)), "{calls:?}");
    assert!(calls.contains(&("books", 12)), "{calls:?}");
}

#[test]
fn kotlin_annotations_before_a_declaration_after_another_are_adopted() {
    // The grammar parses `@A @B("x")` after a bodiless declaration as an expression statement.
    let grammar = grammar_for(&Language::Kotlin).unwrap();
    let src = "data class Book(val id: String)\n\n@Serializable\n@Resource(\"/x\")\nclass BookRoute(val id: String)\n";
    let symbols = parse(&grammar, src.as_bytes());
    let route = def(&symbols, "BookRoute");
    assert!(route.flags.contains(&"@Resource".to_string()), "{route:?}");
    assert!(
        def(&symbols, "Book")
            .flags
            .iter()
            .all(|f| !f.starts_with('@')),
        "not adopted upwards"
    );
}

#[test]
fn kotlin_multiplatform_actual_and_compose_annotation_are_flags() {
    let grammar = grammar_for(&Language::Kotlin).unwrap();
    let src = "actual fun platformName(): String = \"android\"\n\n@Composable\nfun BookScreen(state: State) {\n    Text(state.title)\n}\n";
    let symbols = parse(&grammar, src.as_bytes());
    assert!(
        def(&symbols, "platformName")
            .flags
            .contains(&"actual".to_string())
    );
    let screen = def(&symbols, "BookScreen");
    assert!(
        screen.flags.contains(&"@Composable".to_string()),
        "{screen:?}"
    );
    assert!(refs(&symbols, "call").contains(&("Text", 5)));
}

#[test]
fn kotlin_tier_is_grammar() {
    assert_eq!(tier(&Language::Kotlin), Tier::Grammar);
    assert!(grammar_for(&Language::Other("ex".into())).is_none());
}

fn symbols_of(lang: Language, fixture: &str) -> Vec<Symbol> {
    let grammar = grammar_for(&lang).unwrap_or_else(|| panic!("{lang:?} grammar"));
    parse(&grammar, fixture.as_bytes())
}

fn refs<'a>(symbols: &'a [Symbol], kind: &str) -> Vec<(&'a str, u32)> {
    symbols
        .iter()
        .filter(|s| s.role == Role::Reference && s.kind == kind)
        .map(|s| (s.name.as_str(), s.line))
        .collect()
}

#[test]
fn kotlin_types_member_receivers_and_callable_references_are_uses() {
    let grammar = grammar_for(&Language::Kotlin).unwrap();
    let src = "fun screen(vm: CartViewModel) {\n    val c = AppTheme.colors\n    val m = inject<CartViewModel>()\n    val f = ::CartViewModel\n}\n";
    let symbols = parse(&grammar, src.as_bytes());
    let types = refs(&symbols, "type");
    assert!(types.contains(&("CartViewModel", 1)), "{types:?}");
    assert!(
        types.contains(&("CartViewModel", 3)),
        "a type argument: {types:?}"
    );
    for read in [("AppTheme", 2), ("colors", 2)] {
        assert!(refs(&symbols, "read").contains(&read), "{symbols:?}");
    }
    assert!(
        refs(&symbols, "call").contains(&("CartViewModel", 4)),
        "{symbols:?}"
    );
}

#[test]
fn swift_definitions_supertypes_and_calls() {
    let symbols = symbols_of(
        Language::Swift,
        include_str!("../../../fixtures/swift/Sample.swift"),
    );
    assert_eq!(
        (
            def(&symbols, "Greeter").kind.as_str(),
            def(&symbols, "Greeter").line
        ),
        ("protocol", 4)
    );
    assert_eq!(
        (
            def(&symbols, "Book").kind.as_str(),
            def(&symbols, "Book").line
        ),
        ("struct", 8)
    );
    assert_eq!(
        def(&symbols, "Book").supertypes,
        vec!["Codable", "Equatable"]
    );
    assert_eq!(
        (
            def(&symbols, "BookStore").kind.as_str(),
            def(&symbols, "BookStore").line
        ),
        ("class", 13)
    );
    assert_eq!(def(&symbols, "BookStore").supertypes, vec!["Greeter"]);
    assert_eq!(
        (
            def(&symbols, "load").kind.as_str(),
            def(&symbols, "load").line
        ),
        ("function", 20)
    );
    assert_eq!(
        (
            def(&symbols, "State").kind.as_str(),
            def(&symbols, "State").line
        ),
        ("enum", 31)
    );
    assert_eq!(def(&symbols, "platformName").line, 36);
    let extensions: Vec<u32> = symbols
        .iter()
        .filter(|s| s.role == Role::Definition && s.kind == "extension" && s.name == "BookStore")
        .map(|s| s.line)
        .collect();
    assert_eq!(extensions, vec![27]);
    let calls = refs(&symbols, "call");
    assert!(calls.contains(&("doInit", 21)), "{calls:?}");
    assert!(calls.contains(&("BookRoute", 22)), "{calls:?}");
}

#[test]
fn typescript_definitions_imports_and_calls() {
    let symbols = symbols_of(
        Language::TypeScript,
        include_str!("../../../fixtures/typescript/sample.ts"),
    );
    assert_eq!(
        (
            def(&symbols, "Store").kind.as_str(),
            def(&symbols, "Store").line
        ),
        ("interface", 4)
    );
    assert_eq!(
        (
            def(&symbols, "BookStore").kind.as_str(),
            def(&symbols, "BookStore").line
        ),
        ("class", 8)
    );
    assert_eq!(
        def(&symbols, "BookStore").supertypes,
        vec!["Store", "Greeter"]
    );
    // The interface's signature and the class's implementation are both methods named load.
    let loads: Vec<u32> = symbols
        .iter()
        .filter(|s| s.role == Role::Definition && s.kind == "method" && s.name == "load")
        .map(|s| s.line)
        .collect();
    assert_eq!(loads, vec![5, 11]);
    assert_eq!(
        (
            def(&symbols, "fetchBooks").kind.as_str(),
            def(&symbols, "fetchBooks").line
        ),
        ("function", 17)
    );
    assert_eq!(
        (
            def(&symbols, "greet").kind.as_str(),
            def(&symbols, "greet").line
        ),
        ("function", 21)
    );
    assert_eq!(
        (
            def(&symbols, "API_BASE").kind.as_str(),
            def(&symbols, "API_BASE").line
        ),
        ("constant", 23)
    );
    let imports: Vec<(&Symbol, &str)> = symbols
        .iter()
        .filter(|s| s.role == Role::Reference && s.kind == "import")
        .map(|s| (s, s.flags.first().map(String::as_str).unwrap_or("")))
        .collect();
    assert!(
        imports
            .iter()
            .any(|(s, path)| s.name == "BookRoute" && s.line == 1 && *path == "./a/b"),
        "{imports:?}"
    );
    assert!(
        imports
            .iter()
            .any(|(s, path)| s.name == "Greeter" && s.line == 2 && *path == "../core/greeter"),
        "{imports:?}"
    );
    assert!(refs(&symbols, "call").contains(&("fetchBooks", 13)));
}

#[test]
fn tsx_shares_the_typescript_query() {
    let symbols = symbols_of(
        Language::Tsx,
        include_str!("../../../fixtures/typescript/sample.tsx"),
    );
    assert_eq!(def(&symbols, "Hello").line, 4);
    assert_eq!(def(&symbols, "Counter").line, 8);
    assert!(refs(&symbols, "call").contains(&("greet", 5)));
}

#[test]
fn javascript_definitions_and_imports() {
    let symbols = symbols_of(
        Language::JavaScript,
        include_str!("../../../fixtures/javascript/sample.js"),
    );
    assert_eq!(
        (
            def(&symbols, "Widget").kind.as_str(),
            def(&symbols, "Widget").line
        ),
        ("class", 3)
    );
    assert_eq!(
        (
            def(&symbols, "render").kind.as_str(),
            def(&symbols, "render").line
        ),
        ("method", 4)
    );
    assert_eq!(
        (
            def(&symbols, "makeWidget").kind.as_str(),
            def(&symbols, "makeWidget").line
        ),
        ("function", 9)
    );
    assert_eq!(def(&symbols, "helper").line, 13);
    assert!(refs(&symbols, "import").contains(&("greet", 1)));
    assert!(refs(&symbols, "call").contains(&("greet", 5)));
}

fn has_def(symbols: &[Symbol], name: &str, kind: &str, line: u32) -> bool {
    symbols
        .iter()
        .any(|s| s.role == Role::Definition && s.name == name && s.kind == kind && s.line == line)
}

fn assert_defs(symbols: &[Symbol], expected: &[(&str, &str, u32)]) {
    for (name, kind, line) in expected {
        assert!(
            has_def(symbols, name, kind, *line),
            "missing {kind} {name} at {line} in {symbols:#?}"
        );
    }
}

#[test]
fn go_definitions_and_calls() {
    let s = symbols_of(Language::Go, include_str!("../../../fixtures/go/sample.go"));
    assert_defs(
        &s,
        &[
            ("Greeter", "interface", 5),
            ("Book", "struct", 9),
            ("Greet", "method", 14),
            ("FetchBooks", "function", 18),
            ("APIBase", "constant", 22),
        ],
    );
    assert!(refs(&s, "call").contains(&("load", 19)));
    assert!(refs(&s, "call").contains(&("Sprintf", 15)));
}

#[test]
fn rust_definitions_impls_and_calls() {
    let s = symbols_of(
        Language::Rust,
        include_str!("../../../fixtures/rust/sample.rs"),
    );
    assert_defs(
        &s,
        // kinds follow the upstream tags query: traits are interfaces, structs and enums are classes
        &[
            ("Greeter", "interface", 3),
            ("Book", "class", 7),
            ("State", "class", 17),
            ("fetch_books", "function", 22),
            ("API_BASE", "constant", 26),
        ],
    );
    let imp = s
        .iter()
        .find(|x| x.role == Role::Definition && x.kind == "implementation" && x.line == 11)
        .expect("impl Greeter for Book");
    assert_eq!(
        (imp.name.as_str(), imp.supertypes.clone()),
        ("Book", vec!["Greeter".to_string()])
    );
    assert!(refs(&s, "call").contains(&("load_all", 23)));
}

#[test]
fn java_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::Java,
        include_str!("../../../fixtures/java/Sample.java"),
    );
    assert_defs(
        &s,
        &[
            ("Greeter", "interface", 5),
            ("Base", "class", 9),
            ("BookStore", "class", 11),
            ("greet", "method", 14),
            ("run", "method", 18),
        ],
    );
    assert_eq!(
        def(&s, "BookStore").supertypes,
        vec!["Base", "Greeter", "Runnable"]
    );
    assert!(refs(&s, "call").contains(&("load", 19)));
}

#[test]
fn c_definitions_and_calls() {
    let s = symbols_of(Language::C, include_str!("../../../fixtures/c/sample.c"));
    // kinds follow the upstream tags query: a struct is a class, a typedef is a type
    assert_defs(
        &s,
        &[
            ("book", "class", 3),
            ("book_t", "type", 7),
            ("load", "function", 9),
            ("fetch_books", "function", 13),
        ],
    );
    assert!(refs(&s, "call").contains(&("load", 14)));
}

#[test]
fn cpp_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::Cpp,
        include_str!("../../../fixtures/cpp/sample.cpp"),
    );
    assert_defs(
        &s,
        &[
            ("sample", "module", 3),
            ("Base", "class", 5),
            ("BookStore", "class", 10),
            ("greet", "method", 15),
            ("fetchBooks", "function", 19),
        ],
    );
    assert_eq!(def(&s, "BookStore").supertypes, vec!["Base"]);
    assert!(refs(&s, "call").contains(&("load", 16)));
}

#[test]
fn csharp_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::CSharp,
        include_str!("../../../fixtures/csharp/Sample.cs"),
    );
    assert_defs(
        &s,
        &[
            ("Sample", "module", 3),
            ("IGreeter", "interface", 5),
            ("Base", "class", 10),
            ("BookStore", "class", 12),
            ("Greet", "method", 14),
        ],
    );
    assert_eq!(def(&s, "BookStore").supertypes, vec!["Base", "IGreeter"]);
    assert!(refs(&s, "call").contains(&("Load", 16)));
}

#[test]
fn ruby_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::Ruby,
        include_str!("../../../fixtures/ruby/sample.rb"),
    );
    assert_defs(
        &s,
        &[
            ("Greeter", "module", 3),
            ("greet", "method", 4),
            ("Base", "class", 9),
            ("BookStore", "class", 11),
            ("load", "method", 14),
        ],
    );
    assert_eq!(def(&s, "BookStore").supertypes, vec!["Base"]);
    assert!(refs(&s, "call").contains(&("fetch_books", 15)));
}

#[test]
fn php_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::Php,
        include_str!("../../../fixtures/php/sample.php"),
    );
    // upstream tags methods as functions
    assert_defs(
        &s,
        &[
            ("Greeter", "interface", 5),
            ("Base", "class", 9),
            ("BookStore", "class", 11),
            ("greet", "function", 12),
            ("fetchBooks", "function", 17),
        ],
    );
    assert_eq!(def(&s, "BookStore").supertypes, vec!["Base", "Greeter"]);
    assert!(refs(&s, "call").contains(&("load", 13)));
}

fn literals(symbols: &[Symbol]) -> Vec<(&str, u32)> {
    symbols
        .iter()
        .filter(|s| s.role == Role::Literal)
        .map(|s| (s.name.as_str(), s.line))
        .collect()
}

#[test]
fn markdown_headings_and_links() {
    let s = crate::index::parse::parse(
        &Language::Markdown,
        include_bytes!("../../../fixtures/markdown/sample.md"),
        &crate::config::settings::builtin().parse,
    );
    assert_defs(
        &s,
        &[
            ("Book service", "heading", 1),
            ("Endpoints", "heading", 5),
            ("Delete", "heading", 9),
        ],
    );
    assert!(
        literals(&s).contains(&("https://example.com/api/v1/books", 7)),
        "{:?}",
        literals(&s)
    );
    assert!(literals(&s).contains(&("/docs/guide", 7)));
}

#[test]
fn json_keys_and_link_values() {
    let s = symbols_of(
        Language::Json,
        include_str!("../../../fixtures/json/sample.json"),
    );
    assert_defs(
        &s,
        &[
            ("name", "key", 2),
            ("api", "key", 3),
            ("base", "key", 4),
            ("timeout", "key", 5),
            ("features", "key", 7),
        ],
    );
    assert_eq!(literals(&s), vec![("/api/v1/books", 4)]);
}

#[test]
fn yaml_keys_and_link_values() {
    let s = symbols_of(
        Language::Yaml,
        include_str!("../../../fixtures/yaml/sample.yaml"),
    );
    assert_defs(
        &s,
        &[
            ("name", "key", 1),
            ("api", "key", 2),
            ("base", "key", 3),
            ("timeout", "key", 4),
            ("features", "key", 5),
        ],
    );
    assert_eq!(literals(&s), vec![("/api/v1/books", 3)]);
}

#[test]
fn toml_keys_tables_and_link_values() {
    let s = symbols_of(
        Language::Toml,
        include_str!("../../../fixtures/toml/sample.toml"),
    );
    assert_defs(
        &s,
        &[
            ("name", "key", 1),
            ("api", "key", 3),
            ("base", "key", 4),
            ("timeout", "key", 5),
            ("feature", "key", 7),
            ("id", "key", 8),
        ],
    );
    assert_eq!(literals(&s), vec![("/api/v1/books", 4)]);
}

#[test]
fn bash_functions_variables_and_commands() {
    let s = symbols_of(
        Language::Bash,
        include_str!("../../../fixtures/bash/sample.sh"),
    );
    assert_defs(
        &s,
        &[
            ("API_BASE", "property", 3),
            ("fetch_books", "function", 5),
            ("greet", "function", 9),
        ],
    );
    let calls = refs(&s, "call");
    for expected in [
        ("curl", 6),
        ("echo", 10),
        ("fetch_books", 13),
        ("greet", 14),
    ] {
        assert!(calls.contains(&expected), "{calls:?}");
    }
    assert_eq!(literals(&s), vec![("/api/v1/books", 3)]);
}

#[test]
fn html_ids_and_link_attributes() {
    let s = symbols_of(
        Language::Html,
        include_str!("../../../fixtures/html/sample.html"),
    );
    assert_defs(&s, &[("book-list", "id", 8)]);
    assert_eq!(
        literals(&s),
        vec![
            ("/static/app.css", 4),
            ("/static/app.js", 5),
            ("https://example.com/api/v1/books", 9)
        ]
    );
}

#[test]
fn css_selectors_and_custom_properties() {
    let s = symbols_of(
        Language::Css,
        include_str!("../../../fixtures/css/sample.css"),
    );
    assert_defs(
        &s,
        &[
            ("--brand", "property", 2),
            ("list", "class", 5),
            ("book-list", "id", 9),
            ("row", "class", 9),
        ],
    );
}

#[test]
fn scala_definitions_supertypes_and_calls() {
    let s = symbols_of(
        Language::Scala,
        include_str!("../../../fixtures/scala/Sample.scala"),
    );
    assert_defs(
        &s,
        // upstream kinds: a trait is an interface, a val is a variable
        &[
            ("Greeter", "interface", 3),
            ("Base", "class", 7),
            ("BookStore", "class", 9),
            ("greet", "function", 10),
            ("load", "function", 12),
            ("BookStore", "object", 15),
            ("ApiBase", "variable", 16),
            ("Book", "class", 19),
        ],
    );
    let store = s
        .iter()
        .find(|x| x.role == Role::Definition && x.name == "BookStore" && x.kind == "class")
        .unwrap();
    assert_eq!(store.supertypes, vec!["Base", "Greeter"]);
    assert!(refs(&s, "call").contains(&("fetchBooks", 12)));
}

#[test]
fn lua_functions_and_calls() {
    let s = symbols_of(
        Language::Lua,
        include_str!("../../../fixtures/lua/sample.lua"),
    );
    assert_defs(
        &s,
        &[
            ("API_BASE", "property", 3),
            ("greet", "function", 5),
            ("load", "function", 9),
            ("load_all", "function", 13),
        ],
    );
    assert!(refs(&s, "call").contains(&("fetch_books", 10)));
    assert!(refs(&s, "call").contains(&("load", 14)));
}

#[test]
fn elixir_modules_functions_and_calls() {
    let s = symbols_of(
        Language::Elixir,
        include_str!("../../../fixtures/elixir/sample.ex"),
    );
    assert_defs(
        &s,
        &[
            ("Sample.BookStore", "module", 1),
            ("api_base", "constant", 2),
            ("greet", "function", 4),
            ("load", "function", 8),
            ("load_all", "function", 12),
        ],
    );
    assert!(refs(&s, "call").contains(&("fetch_books", 9)));
    assert!(
        !refs(&s, "call")
            .iter()
            .any(|(n, _)| *n == "def" || *n == "defmodule")
    );
}

#[test]
fn haskell_module_types_classes_and_functions() {
    let s = symbols_of(
        Language::Haskell,
        include_str!("../../../fixtures/haskell/Sample.hs"),
    );
    // A binding without parameters is a `bind` (constant); its signature on the line above is the function.
    assert_defs(
        &s,
        &[
            ("Sample", "module", 1),
            ("Book", "struct", 3),
            ("Wrapper", "struct", 5),
            ("Greeter", "trait", 7),
            ("Greeter", "implementation", 10),
            ("fetchBooks", "function", 13),
            ("apiBase", "constant", 17),
        ],
    );
}

#[test]
fn ocaml_modules_types_values_and_calls() {
    let s = symbols_of(
        Language::Ocaml,
        include_str!("../../../fixtures/ocaml/sample.ml"),
    );
    // upstream kind for a type definition is `type`
    assert_defs(
        &s,
        &[
            ("book", "type", 1),
            ("Store", "module", 3),
            ("load", "function", 4),
            ("greet", "function", 7),
            ("api_base", "constant", 9),
        ],
    );
    assert!(refs(&s, "call").contains(&("fetch_books", 4)));
}

#[test]
fn zig_containers_functions_and_imports() {
    let s = symbols_of(
        Language::Zig,
        include_str!("../../../fixtures/zig/sample.zig"),
    );
    assert_defs(
        &s,
        &[
            ("api_base", "constant", 3),
            ("Book", "struct", 5),
            ("greet", "function", 9),
            ("load", "function", 13),
        ],
    );
    assert!(refs(&s, "call").contains(&("load", 10)));
    assert!(refs(&s, "import").contains(&("std", 1)));
}

#[test]
fn dart_classes_supertypes_methods_and_calls() {
    let s = symbols_of(
        Language::Dart,
        include_str!("../../../fixtures/dart/sample.dart"),
    );
    assert_defs(
        &s,
        &[
            ("Greeter", "class", 3),
            ("Base", "class", 7),
            ("BookStore", "class", 9),
            ("greet", "method", 11),
            ("load", "method", 13),
            ("fetchBooks", "function", 18),
        ],
    );
    assert_eq!(def(&s, "BookStore").supertypes, vec!["Base", "Greeter"]);
    assert!(refs(&s, "call").contains(&("fetchBooks", 14)));
}

#[test]
fn python_definitions_bases_and_imports() {
    let symbols = symbols_of(
        Language::Python,
        include_str!("../../../fixtures/python/sample.py"),
    );
    assert_eq!(
        (
            def(&symbols, "Base").kind.as_str(),
            def(&symbols, "Base").line
        ),
        ("class", 4)
    );
    assert_eq!(
        def(&symbols, "BookStore").supertypes,
        vec!["Base", "object"]
    );
    assert_eq!(
        (
            def(&symbols, "load").kind.as_str(),
            def(&symbols, "load").line
        ),
        ("method", 11)
    );
    assert_eq!(
        (
            def(&symbols, "fetch_books").kind.as_str(),
            def(&symbols, "fetch_books").line
        ),
        ("function", 14)
    );
    assert_eq!(
        (
            def(&symbols, "API_BASE").kind.as_str(),
            def(&symbols, "API_BASE").line
        ),
        ("constant", 17)
    );
    let import = symbols
        .iter()
        .find(|s| s.role == Role::Reference && s.kind == "import" && s.name == "helper")
        .expect("import helper");
    assert_eq!(
        (import.line, import.flags.first().map(String::as_str)),
        (1, Some("pkg.mod"))
    );
    assert!(refs(&symbols, "call").contains(&("helper", 12)));
}

fn all_refs(symbols: &[Symbol]) -> Vec<(&str, &str, u32)> {
    symbols
        .iter()
        .filter(|s| s.role == Role::Reference)
        .map(|s| (s.name.as_str(), s.kind.as_str(), s.line))
        .collect()
}

#[test]
fn xml_named_elements_ids_and_resource_references() {
    let s = symbols_of(
        Language::Xml,
        include_str!("../../../fixtures/xml/sample.xml"),
    );
    assert_defs(
        &s,
        &[
            ("app_name", "name", 3),
            ("brand", "name", 4),
            ("logo", "id", 5),
        ],
    );
    assert_eq!(all_refs(&s), vec![("ic_logo", "resource", 5)]);
}

#[test]
fn sql_tables_views_indexes_and_functions() {
    let s = symbols_of(
        Language::Sql,
        include_str!("../../../fixtures/sql/sample.sql"),
    );
    assert_defs(
        &s,
        &[
            ("books", "table", 1),
            ("books_name", "index", 6),
            ("cheap_books", "view", 8),
            ("book_count", "function", 10),
        ],
    );
    assert!(all_refs(&s).contains(&("books", "table", 8)), "{s:?}");
}

#[test]
fn proto_messages_enums_services_and_rpcs() {
    let s = symbols_of(
        Language::Proto,
        include_str!("../../../fixtures/proto/sample.proto"),
    );
    assert_defs(
        &s,
        &[
            ("Book", "message", 7),
            ("Status", "enum", 12),
            ("BookService", "service", 16),
            ("GetBook", "rpc", 17),
        ],
    );
    for expected in [("Money", "type", 9), ("GetBookRequest", "type", 17)] {
        assert!(all_refs(&s).contains(&expected), "{s:?}");
    }
}

#[test]
fn graphql_types_operations_and_fragments() {
    let s = symbols_of(
        Language::GraphQl,
        include_str!("../../../fixtures/graphql/sample.graphql"),
    );
    assert_defs(
        &s,
        &[
            ("Node", "interface", 1),
            ("Book", "type", 5),
            ("Status", "enum", 10),
            ("BookFilter", "input", 14),
            ("GetBook", "operation", 18),
            ("BookFields", "fragment", 24),
        ],
    );
    assert_eq!(def(&s, "Book").supertypes, ["Node"]);
    for expected in [("Money", "type", 7), ("BookFields", "fragment", 20)] {
        assert!(all_refs(&s).contains(&expected), "{s:?}");
    }
}

#[test]
fn hcl_blocks_are_named_by_their_last_label() {
    let s = symbols_of(
        Language::Hcl,
        include_str!("../../../fixtures/hcl/sample.tf"),
    );
    assert_defs(
        &s,
        &[
            ("region", "block", 1),
            ("assets", "block", 5),
            ("network", "block", 9),
            ("bucket_name", "block", 13),
        ],
    );
    assert_eq!(def(&s, "assets").flags, ["resource"]);
    for expected in [("region", "attribute", 6), ("assets", "attribute", 14)] {
        assert!(all_refs(&s).contains(&expected), "{s:?}");
    }
}

#[test]
fn make_targets_variables_and_prerequisites() {
    let s = symbols_of(
        Language::Make,
        include_str!("../../../fixtures/make/Makefile"),
    );
    assert_defs(
        &s,
        &[
            ("CC", "variable", 1),
            ("build", "target", 3),
            ("test", "target", 7),
            ("greet", "function", 10),
        ],
    );
    for expected in [("CC", "variable", 4), ("build", "target", 7)] {
        assert!(all_refs(&s).contains(&expected), "{s:?}");
    }
    assert_eq!(
        crate::index::parse::language_of("ci/Makefile"),
        Language::Make
    );
}

#[test]
fn objc_classes_protocols_methods_and_calls() {
    let s = symbols_of(
        Language::ObjC,
        include_str!("../../../fixtures/objc/Sample.m"),
    );
    assert_defs(
        &s,
        &[
            ("BookPort", "protocol", 3),
            ("BookStore", "class", 7),
            ("BookStore", "implementation", 13),
            ("load", "method", 14),
            ("shared", "method", 18),
            ("helper", "function", 23),
        ],
    );
    let class = s
        .iter()
        .find(|d| d.name == "BookStore" && d.kind == "class")
        .unwrap();
    assert_eq!(class.supertypes, ["NSObject", "BookPort"]);
    for expected in [("refresh", "call", 15), ("helper", "call", 24)] {
        assert!(all_refs(&s).contains(&expected), "{s:?}");
    }
}

#[test]
fn languages_that_share_a_query_text_each_compile_their_own_query() {
    // TSX first, then TypeScript: the query of one grammar must not answer for the other.
    let tsx = symbols_of(
        Language::Tsx,
        include_str!("../../../fixtures/typescript/sample.tsx"),
    );
    assert!(!tsx.is_empty());
    let ts = symbols_of(
        Language::TypeScript,
        include_str!("../../../fixtures/typescript/sample.ts"),
    );
    assert_eq!(def(&ts, "Store").kind, "interface");
}
