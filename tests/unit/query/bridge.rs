use super::*;
use crate::model::{Hit, Role, Symbol};

fn def(path: &str, name: &str, kind: &str, language: &str) -> Hit {
    Hit {
        repo: "app".into(),
        path: path.into(),
        scope: "commonMain".into(),
        language: language.into(),
        symbol: Symbol {
            name: name.into(),
            role: Role::Definition,
            kind: kind.into(),
            line: 1,
            start_line: 1,
            end_line: 1,
            flags: vec![],
            supertypes: vec![],
        },
    }
}

fn rule(name: &str) -> BridgeRule {
    crate::config::settings::builtin()
        .bridges
        .iter()
        .find(|r| r.name == name)
        .cloned()
        .unwrap()
}

fn foreign_names(rule: &BridgeRule, def: &Hit) -> Vec<String> {
    super::foreign_names(rule, def, crate::config::settings::builtin())
}

#[test]
fn kmp_top_level_function_is_fookt_in_swift() {
    let kmp = rule("kmp-swift");
    assert_eq!(
        foreign_names(
            &kmp,
            &def(
                "src/commonMain/kotlin/app/Platform.kt",
                "platformName",
                "function",
                "kotlin"
            )
        ),
        vec!["PlatformKt"]
    );
    assert_eq!(
        foreign_names(
            &kmp,
            &def(
                "src/androidMain/kotlin/app/Platform.android.kt",
                "platformName",
                "function",
                "kotlin"
            )
        ),
        vec!["Platform_androidKt"]
    );
    assert!(
        foreign_names(
            &kmp,
            &def(
                "src/commonMain/kotlin/app/Book.kt",
                "Book",
                "class",
                "kotlin"
            )
        )
        .is_empty(),
        "classes keep their name"
    );
    assert!(
        foreign_names(&kmp, &def("src/x.swift", "f", "function", "swift")).is_empty(),
        "only from the rule's language"
    );
}

#[test]
fn ts_export_matches_import_by_path() {
    let ts = rule("ts-import");
    assert!(import_matches(
        &ts,
        &def("src/a/b.ts", "fetchBook", "function", "typescript"),
        "./a/b"
    ));
    assert!(import_matches(
        &ts,
        &def("src/core/greeter.ts", "Greeter", "interface", "typescript"),
        "../core/greeter"
    ));
    assert!(!import_matches(
        &ts,
        &def("src/a/b.ts", "fetchBook", "function", "typescript"),
        "./x"
    ));
    let py = rule("py-import");
    assert!(import_matches(
        &py,
        &def("pkg/mod.py", "helper", "function", "python"),
        "pkg.mod"
    ));
    assert!(!import_matches(
        &py,
        &def("pkg/mod.py", "helper", "function", "python"),
        "pkg.other"
    ));
}
