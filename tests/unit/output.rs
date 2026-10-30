use super::*;

fn line(repo: Option<&str>, path: &str, number: u32, name: &str) -> Line {
    Line {
        repo: repo.map(str::to_string),
        path: path.into(),
        line: number,
        kind: "class".into(),
        name: name.into(),
        scope: "main".into(),
        flags: String::new(),
    }
}

#[test]
fn render_without_lines_says_no_match() {
    let answer = Answer::new("index");
    assert_eq!(render(&answer), "# index (-)\nno match\n");
}

#[test]
fn render_prefixes_sibling_lines_and_joins_flags() {
    let mut answer = Answer::new("index");
    answer.tier = "grammar".into();
    answer.notes.push("a note".into());
    let mut a = line(Some("../server"), "src/A.kt", 4, "A");
    a.flags = "sealed data".into();
    answer.lines.push(a);
    answer.cut = 2;
    assert_eq!(
        render(&answer),
        "# index (grammar)\na note\n../server/src/A.kt:4 class A sealed data\n... 2 more (--all)\n"
    );
}

#[test]
fn only_a_scope_other_than_main_is_printed_and_grep_lines_read_like_grep() {
    let mut test = line(None, "src/ATest.kt", 3, "ATest");
    test.scope = "test".into();
    let mut grep = line(None, "lib/b.rs", 7, "let b = 1;");
    grep.kind = String::new();
    let mut answer = Answer::new("grep");
    answer.lines = vec![test, grep];
    assert_eq!(
        render(&answer),
        "# grep (-)\nsrc/ATest.kt:3 class ATest [test]\nlib/b.rs:7: let b = 1;\n"
    );
}

#[test]
fn consecutive_lines_in_one_folder_name_it_once() {
    let mut answer = Answer::new("index");
    answer.lines = vec![
        line(None, "src/deep/app/A.kt", 1, "A"),
        line(None, "src/deep/app/B.kt", 2, "B"),
        line(Some("../web"), "src/deep/app/C.ts", 3, "C"),
        line(None, "README.md", 4, "D"),
        line(None, "README.md", 5, "E"),
    ];
    assert_eq!(
        render(&answer),
        "# index (-)\n\
         src/deep/app/\n  A.kt:1 class A\n  B.kt:2 class B\n\
         ../web/src/deep/app/C.ts:3 class C\n\
         README.md:4 class D\nREADME.md:5 class E\n",
        "another repository is another folder; files at the root keep their name"
    );
}
