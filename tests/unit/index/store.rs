use super::*;
use crate::model::{Role, Symbol};

fn sym(name: &str, role: Role, kind: &str, line: u32, supertypes: &[&str]) -> Symbol {
    Symbol {
        name: name.into(),
        role,
        kind: kind.into(),
        line,
        start_line: line,
        end_line: line,
        flags: vec!["sealed".into()],
        supertypes: supertypes.iter().map(|s| s.to_string()).collect(),
    }
}

fn store() -> (tempfile::TempDir, Store) {
    let cache = tempfile::tempdir().unwrap();
    let repo = cache.path().join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let store = Store::open(cache.path(), &repo).unwrap();
    (cache, store)
}

#[test]
fn prune_removes_stores_unused_too_long_or_of_a_repository_that_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let (kept, old, gone) = (
        dir.path().join("kept"),
        dir.path().join("old"),
        dir.path().join("gone"),
    );
    for repo in [&kept, &old, &gone] {
        std::fs::create_dir_all(repo).unwrap();
        drop(Store::open(&cache, repo).unwrap());
    }
    std::fs::remove_dir_all(&gone).unwrap();
    let old_db = std::fs::read_dir(&cache)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.extension().is_some_and(|e| e == "root")
                && std::fs::read_to_string(p).unwrap().ends_with("/old")
        })
        .unwrap()
        .with_extension("db");
    let month_ago = std::time::SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
    std::fs::File::options()
        .write(true)
        .open(&old_db)
        .unwrap()
        .set_modified(month_ago)
        .unwrap();
    assert_eq!(prune(&cache, Duration::from_secs(14 * 24 * 60 * 60)), 2);
    let left: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "db"))
        .collect();
    assert_eq!(left.len(), 1, "only the store in use stays");
    assert!(Store::open(&cache, &kept).is_ok());
}

fn row(path: &str, sha: &str, scope: &str) -> FileRow {
    FileRow {
        path: path.into(),
        sha: sha.into(),
        scope: scope.into(),
        dirty_key: None,
    }
}

#[test]
fn insert_then_by_name_returns_hit_with_path_and_scope() {
    let (_cache, mut store) = store();
    store
        .insert_blob(
            "aaa",
            "kotlin",
            &[sym("Foo", Role::Definition, "class", 3, &[])],
        )
        .unwrap();
    store
        .replace_files(&[row("src/Foo.kt", "aaa", "main")])
        .unwrap();
    let hits = store.by_name("Foo").unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "src/Foo.kt");
    assert_eq!(hits[0].scope, "main");
    assert_eq!(hits[0].repo, "myrepo");
    assert_eq!(hits[0].language, "kotlin");
    assert_eq!(hits[0].symbol.line, 3);
    assert_eq!(hits[0].symbol.flags, vec!["sealed".to_string()]);
}

#[test]
fn blob_at_two_paths_yields_two_hits() {
    let (_cache, mut store) = store();
    store
        .insert_blob(
            "aaa",
            "kotlin",
            &[sym("Foo", Role::Definition, "class", 1, &[])],
        )
        .unwrap();
    store
        .replace_files(&[
            row("a/Foo.kt", "aaa", "main"),
            row("b/Foo.kt", "aaa", "test"),
        ])
        .unwrap();
    let mut paths: Vec<String> = store
        .by_name("Foo")
        .unwrap()
        .into_iter()
        .map(|h| h.path)
        .collect();
    paths.sort();
    assert_eq!(paths, vec!["a/Foo.kt", "b/Foo.kt"]);
}

#[test]
fn replace_files_drops_paths_no_longer_present() {
    let (_cache, mut store) = store();
    store
        .insert_blob(
            "aaa",
            "kotlin",
            &[sym("Foo", Role::Definition, "class", 1, &[])],
        )
        .unwrap();
    store
        .replace_files(&[
            row("a/Foo.kt", "aaa", "main"),
            row("gone.kt", "aaa", "main"),
        ])
        .unwrap();
    store
        .replace_files(&[row("a/Foo.kt", "aaa", "main")])
        .unwrap();
    assert_eq!(store.files().unwrap().len(), 1);
    assert_eq!(store.by_name("Foo").unwrap().len(), 1);
    assert!(
        store.has_blob("aaa"),
        "blobs stay cached for a later reappearance"
    );
}

#[test]
fn by_supertype_finds_implementations() {
    let (_cache, mut store) = store();
    store
        .insert_blob(
            "bbb",
            "kotlin",
            &[
                sym(
                    "FooImpl",
                    Role::Definition,
                    "class",
                    1,
                    &["FooPort", "Base"],
                ),
                sym("Other", Role::Definition, "class", 9, &["Base"]),
            ],
        )
        .unwrap();
    store
        .replace_files(&[row("src/Impl.kt", "bbb", "main")])
        .unwrap();
    let names: Vec<String> = store
        .by_supertype("FooPort")
        .unwrap()
        .into_iter()
        .map(|h| h.symbol.name)
        .collect();
    assert_eq!(names, vec!["FooImpl"]);
    assert_eq!(store.by_supertype("Base").unwrap().len(), 2);
}

#[test]
fn literals_and_under_filter_rows() {
    let (_cache, mut store) = store();
    store
        .insert_blob(
            "ccc",
            "kotlin",
            &[
                sym("/api/v1/books", Role::Literal, "literal", 4, &[]),
                sym("Route", Role::Definition, "class", 2, &[]),
            ],
        )
        .unwrap();
    store
        .replace_files(&[row("app/src/Route.kt", "ccc", "main")])
        .unwrap();
    assert_eq!(store.literals("/api/v1/books").unwrap()[0].symbol.line, 4);
    assert_eq!(store.under("app/src").unwrap().len(), 2);
    assert_eq!(store.under("other").unwrap().len(), 0);
}

#[test]
fn two_connections_write_concurrently_without_lock_error() {
    let cache = tempfile::tempdir().unwrap();
    let repo = cache.path().join("r");
    std::fs::create_dir_all(&repo).unwrap();
    let cache_path = cache.path().to_path_buf();
    let handles: Vec<_> = (0..2)
        .map(|t| {
            let cache_path = cache_path.clone();
            let repo = repo.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(&cache_path, &repo).unwrap();
                for i in 0..200 {
                    let sha = format!("{t}-{i}");
                    store
                        .insert_blob(&sha, "rust", &[sym("x", Role::Definition, "fn", 1, &[])])
                        .unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let store = Store::open(&cache_path, &repo).unwrap();
    assert!(store.has_blob("0-199") && store.has_blob("1-199"));
}

#[test]
fn schema_version_is_recorded_and_an_older_store_starts_over() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let cache = dir.path().join("cache");
    let mut old = Store::open(&cache, &repo).unwrap();
    assert_eq!(old.meta("schema").as_deref(), Some(SCHEMA_VERSION));
    old.insert_blob("x", "kotlin", &[]).unwrap();
    old.set_meta("schema", "2").unwrap();
    drop(old);
    let reopened = Store::open(&cache, &repo).unwrap();
    assert_eq!(reopened.meta("schema").as_deref(), Some(SCHEMA_VERSION));
    assert!(
        !reopened.has_blob("x"),
        "a store of another version is dropped"
    );
}

fn blob(key: &str, symbols: Vec<Symbol>, words: &str) -> ParsedBlob {
    let mut terms: Vec<(String, u32)> = Vec::new();
    for w in words.split(' ') {
        match terms.iter_mut().find(|(t, _)| t == w) {
            Some((_, n)) => *n += 1,
            None => terms.push((w.to_string(), 1)),
        }
    }
    ParsedBlob {
        key: key.into(),
        language: "kotlin".into(),
        symbols,
        terms,
        vector: None,
    }
}

#[test]
fn search_ranks_the_blob_with_more_query_words() {
    let (_cache, mut store) = store();
    let blobs = [
        blob(
            "a",
            vec![sym("BookRoute", Role::Definition, "class", 4, &[])],
            "bookroute book route book route id",
        ),
        blob(
            "b",
            vec![sym("Other", Role::Definition, "class", 1, &[])],
            "other book",
        ),
    ];
    let files = [row("a.kt", "a", "main"), row("b.kt", "b", "main")];
    store.apply(&blobs, &files, &[]).unwrap();
    let hits = store.search("book route", 10).unwrap();
    assert_eq!(hits[0].0.path, "a.kt");
    assert_eq!(
        (hits[0].0.symbol.name.as_str(), hits[0].0.symbol.line),
        ("BookRoute", 4)
    );
    assert_eq!(hits[1].0.symbol.line, 1, "no matching definition: line 1");
    assert!(store.search("zzz", 10).unwrap().is_empty());
}

#[test]
fn meta_round_trips() {
    let (_cache, mut store) = store();
    store.set_meta("head", "abc").unwrap();
    assert_eq!(store.meta("head").as_deref(), Some("abc"));
}
