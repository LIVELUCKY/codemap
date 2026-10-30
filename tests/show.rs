//! `codemap show`: the source of a definition, ready to edit, over the fixture workspace.

mod common;

use codemap::query::show::show;
use common::workspace;

#[test]
fn show_prints_the_whole_declaration_with_its_annotation_and_line_numbers() {
    let f = workspace(None);
    let text = show(&f.ws, "BookRoute", false);
    assert!(
        text.contains("src/commonMain/kotlin/app/BookRoute.kt:11-12 class BookRoute\n"),
        "{text}"
    );
    assert!(
        text.contains("  11 | @Resource(\"/api/v1/books/{id}\")"),
        "the annotation above is part of the edit: {text}"
    );
    assert!(
        text.contains("  12 | class BookRoute(val id: String)"),
        "{text}"
    );
    assert!(
        text.contains("../server/src/main/kotlin/server/Routes.kt:3-4 class BookRoute\n"),
        "siblings too: {text}"
    );
}

#[test]
fn show_covers_a_class_body_to_its_closing_brace() {
    let f = workspace(None);
    let text = show(&f.ws, "BookRepositoryImpl", false);
    assert!(
        text.contains("BookRoute.kt:7-9 class BookRepositoryImpl"),
        "{text}"
    );
    assert!(text.contains("   9 | }"), "{text}");
}

#[test]
fn show_lists_every_variant_of_an_expect_declaration() {
    let f = workspace(None);
    let text = show(&f.ws, "platformName", false);
    for file in [
        "Platform.kt:3-3",
        "Platform.android.kt:3-3",
        "Platform.ios.kt:3-3",
    ] {
        assert!(text.contains(file), "{file} missing: {text}");
    }
}

#[test]
fn show_of_an_unknown_name_suggests_the_nearest() {
    let f = workspace(None);
    let text = show(&f.ws, "BookRout", false);
    assert!(
        text.contains("no match") && text.contains("BookRoute"),
        "{text}"
    );
}

#[test]
fn a_long_definition_is_cut_with_the_range_to_read() {
    let f = workspace(None);
    let file = f.ws.project.root.join("src/commonMain/kotlin/app/Long.kt");
    let body: String = (0..200).map(|i| format!("    val v{i} = {i}\n")).collect();
    std::fs::write(&file, format!("package app\n\nobject Long {{\n{body}}}\n")).unwrap();
    let text = show(&f.ws, "Long", false);
    assert!(text.contains("Long.kt:3-204 object Long"), "{text}");
    assert!(
        text.contains("more lines; read src/commonMain/kotlin/app/Long.kt:"),
        "{text}"
    );
    assert!(
        show(&f.ws, "Long", true).contains(" 204 | }"),
        "--all prints everything"
    );
}
