use super::*;

#[test]
fn levels_come_from_documented_formats() {
    assert_eq!(
        level_of("10-30 12:00:00.123  1234  5678 E ActivityManager: ANR in app"),
        Some(Level::Error)
    );
    assert_eq!(
        level_of("W/Choreographer( 1234): Skipped 30 frames"),
        Some(Level::Warn)
    );
    assert_eq!(
        level_of(r#"{"level":"info","msg":"ready"}"#),
        Some(Level::Info)
    );
    assert_eq!(level_of("ts=1 level=error msg=boom"), Some(Level::Error));
    assert_eq!(
        level_of("2026-10-30 12:00:00 [WARN] slow query"),
        Some(Level::Warn)
    );
    assert_eq!(level_of("ERROR: connection refused"), Some(Level::Error));
    assert_eq!(
        level_of("an error happened in lower case"),
        None,
        "prose is not a level"
    );
    assert_eq!(level_of("<ErrorBoundary> rendered"), None);
}

#[test]
fn repeats_differing_only_in_numbers_ids_and_times_are_one_entry() {
    assert_eq!(
        template("12:00:01.5 user 42 took 17ms id=1b4e28ba-2fa1-11d2-883f-0016d3cca427 at 0x7ffe"),
        template("12:09:59.0 user 7 took 3ms id=6fa459ea-ee8a-3ca4-894e-db77e160355e at 0x1")
    );
    assert_ne!(template("user 1 logged in"), template("user 1 logged out"));
}

#[test]
fn a_log_becomes_counted_entries_with_their_stack_traces() {
    let log = "\
2026-10-30 12:00:00 INFO started in 120 ms
2026-10-30 12:00:01 ERROR request 1 failed
java.lang.IllegalStateException: boom
\tat app.Api.load(Api.kt:12)
\tat app.Main.run(Main.kt:3)
2026-10-30 12:00:02 ERROR request 2 failed
\tat app.Api.load(Api.kt:12)
2026-10-30 12:00:03 WARN retrying in 5 s
";
    let summary = summarize(log.as_bytes(), None, None, 20);
    assert_eq!(summary.lines, 8);
    let failed = summary
        .entries
        .iter()
        .find(|e| e.text.contains("request 1 failed"))
        .unwrap();
    assert_eq!((failed.count, failed.first, failed.last), (2, 2, 6));
    let text = render(&summary, Some(25), 200, 5);
    assert!(
        text.starts_with("# logs (8 lines, 4 distinct; 2 error, 1 warn)\n"),
        "{text}"
    );
    let first_entry = text.lines().nth(1).unwrap();
    assert!(first_entry.starts_with("2-6 2x E "), "errors first: {text}");
}

#[test]
fn stack_frames_stay_with_their_line() {
    let log = "E/App(1): crash\n\tat a.B.c(B.kt:1)\n\tat a.B.d(B.kt:2)\nI/App(1): ok\n";
    let summary = summarize(log.as_bytes(), None, None, 20);
    let crash = &summary.entries[0];
    assert_eq!(crash.detail, ["\tat a.B.c(B.kt:1)", "\tat a.B.d(B.kt:2)"]);
    assert_eq!(summary.entries.len(), 2);
}

#[test]
fn a_filter_and_a_minimum_level_narrow_it() {
    let log = "INFO a\nWARN b 1\nWARN b 2\nERROR c\nplain line\n";
    let warn_up = summarize(log.as_bytes(), None, Some(Level::Warn), 20);
    assert_eq!(warn_up.entries.len(), 2);
    let only_c = summarize(log.as_bytes(), Some(&Regex::new("c").unwrap()), None, 20);
    assert_eq!(only_c.entries.len(), 1);
    assert_eq!(only_c.lines, 5, "every line is counted, kept or not");
}
