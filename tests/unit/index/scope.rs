use super::*;

fn none() -> Linguist {
    Linguist::default()
}

#[test]
fn by_default_every_file_is_main() {
    let rules = &crate::config::settings::builtin().scope;
    for path in [
        "shared/src/commonMain/kotlin/A.kt",
        "src/test/java/com/x/FooTest.java",
        "tests/index.rs",
        "web/src/a.test.ts",
        "build/generated/source/X.kt",
        "node_modules/x/index.js",
    ] {
        assert_eq!(
            scope_of(path, none(), rules),
            "main",
            "{path}: nothing guessed"
        );
    }
}

#[test]
fn git_attributes_mark_generated_and_vendored() {
    let rules = &crate::config::settings::builtin().scope;
    let generated = Linguist {
        generated: true,
        vendored: false,
    };
    let vendored = Linguist {
        generated: true,
        vendored: true,
    };
    assert_eq!(scope_of("api/Client.kt", generated, rules), "generated");
    assert_eq!(
        scope_of("lib/x.js", vendored, rules),
        "vendored",
        "vendored wins"
    );
}

#[test]
fn a_repository_names_its_own_folders_and_files() {
    let rules = crate::config::settings::resolve(&[&serde_json::json!(
        {"settings": {"scope": {"testDirs": ["e2e"], "testFiles": ["*.it.kt"],
                                "vendoredDirs": ["third_party"], "generatedDirs": ["build"]}}}
    )])
    .unwrap()
    .scope;
    assert_eq!(scope_of("e2e/login.kt", none(), &rules), "test");
    assert_eq!(scope_of("app/Login.it.kt", none(), &rules), "test");
    assert_eq!(scope_of("third_party/e2e/x.kt", none(), &rules), "vendored");
    assert_eq!(scope_of("app/build/X.kt", none(), &rules), "generated");
    assert_eq!(scope_of("tests/index.rs", none(), &rules), "main");
}

#[test]
fn ranking_classes() {
    for (scope, class) in [
        ("main", ScopeClass::Main),
        ("test", ScopeClass::Test),
        ("generated", ScopeClass::Generated),
        ("vendored", ScopeClass::Vendored),
    ] {
        assert_eq!(class_of(scope), class);
    }
}
