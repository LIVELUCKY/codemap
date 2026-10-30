//! Reconcile against scripted git histories: every test builds a real repository in a temp dir.

use std::fs;
use std::path::Path;
use std::process::Command;

use codemap::config;
use codemap::index::store::Store;
use codemap::index::{Depth, reconcile};
use codemap::model::Role;
use codemap::workspace::Repo;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Fixture {
    _cache: tempfile::TempDir,
    _dir: tempfile::TempDir,
    repo: Repo,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let cache = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "t"]);
        let root = dir.path().canonicalize().unwrap();
        let store = Store::open(cache.path(), &root).unwrap();
        let repo = Repo {
            name: "fixture".into(),
            root,
        };
        Fixture {
            _cache: cache,
            _dir: dir,
            repo,
            store,
        }
    }

    fn write(&self, path: &str, content: &[u8]) {
        let full = self.repo.root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }

    fn commit(&self, files: &[(&str, &str)]) {
        for (path, content) in files {
            self.write(path, content.as_bytes());
        }
        git(&self.repo.root, &["add", "-A"]);
        git(&self.repo.root, &["commit", "-q", "-m", "x"]);
    }

    fn reconcile(&mut self, depth: Depth) -> codemap::index::Stats {
        let ignore = config::ignore(&self.repo.root);
        let settings = codemap::config::settings::builtin();
        reconcile(&self.repo, &mut self.store, &ignore, depth, settings).unwrap()
    }

    fn names(&self, name: &str) -> usize {
        self.store.by_name(name).unwrap().len()
    }
}

#[test]
fn first_reconcile_parses_every_tracked_file() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n"), ("b.py", "def b():\n    pass\n")]);
    let stats = f.reconcile(Depth::Full);
    assert_eq!((stats.parsed, stats.reused, stats.skipped), (2, 0, 0));
    assert_eq!(f.names("A"), 1);
    assert_eq!(f.names("b"), 1);
}

#[test]
fn second_reconcile_parses_nothing() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n"), ("b.py", "def b():\n    pass\n")]);
    f.reconcile(Depth::Full);
    let stats = f.reconcile(Depth::Full);
    assert_eq!((stats.parsed, stats.reused), (0, 2));
}

#[test]
fn branch_switch_parses_only_new_blobs_and_back_is_free() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n")]);
    f.reconcile(Depth::Full);
    git(&f.repo.root, &["checkout", "-q", "-b", "feature"]);
    f.commit(&[("c.kt", "class C\n")]);
    assert_eq!(f.reconcile(Depth::Full).parsed, 1, "only the new blob");
    git(&f.repo.root, &["checkout", "-q", "main"]);
    assert_eq!(
        f.reconcile(Depth::Full).parsed,
        0,
        "switching back parses nothing"
    );
    assert_eq!(f.names("C"), 0);
    git(&f.repo.root, &["checkout", "-q", "feature"]);
    assert_eq!(
        f.reconcile(Depth::Full).parsed,
        0,
        "the blob is still cached"
    );
    assert_eq!(f.names("C"), 1);
}

#[test]
fn modified_file_is_reparsed_once_until_it_changes_again() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n")]);
    f.reconcile(Depth::Full);
    f.write("a.kt", b"class A2\n");
    assert_eq!(f.reconcile(Depth::Full).parsed, 1);
    assert_eq!(f.reconcile(Depth::Full).parsed, 0);
    assert_eq!((f.names("A"), f.names("A2")), (0, 1));
}

#[test]
fn deleted_path_disappears_from_lookups() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n"), ("b.kt", "class B\n")]);
    f.reconcile(Depth::Full);
    fs::remove_file(f.repo.root.join("a.kt")).unwrap();
    let stats = f.reconcile(Depth::Full);
    assert_eq!(stats.removed, 1);
    assert_eq!((f.names("A"), f.names("B")), (0, 1));
}

#[test]
fn stash_and_pop_leave_the_index_consistent() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n")]);
    f.write("a.kt", b"class A2\n");
    f.reconcile(Depth::Full);
    git(&f.repo.root, &["stash", "-q"]);
    f.reconcile(Depth::Full);
    assert_eq!((f.names("A"), f.names("A2")), (1, 0));
    git(&f.repo.root, &["stash", "pop", "-q"]);
    f.reconcile(Depth::Full);
    assert_eq!((f.names("A"), f.names("A2")), (0, 1));
}

#[test]
fn large_and_non_utf8_files_are_skipped_and_counted() {
    let mut f = Fixture::new();
    f.write("big.kt", &vec![b'x'; 1024 * 1024 + 1]);
    f.write(
        "bin.kt",
        &[0xff, 0xfe, b'c', b'l', b'a', b's', b's', b' ', b'Z', 0xff],
    );
    f.commit(&[("ok.kt", "class Ok\n")]);
    let stats = f.reconcile(Depth::Full);
    assert_eq!((stats.parsed, stats.skipped), (1, 2));
    assert_eq!(f.names("Z"), 0);
}

#[test]
fn files_that_are_not_text_are_still_known_by_path_and_read_once() {
    let mut f = Fixture::new();
    f.write("assets/ic_logo.png", &[0x89, b'P', b'N', b'G', 0, 1, 2]);
    f.commit(&[("ok.kt", "class Ok\n")]);
    let stats = f.reconcile(Depth::Full);
    assert_eq!((stats.parsed, stats.skipped), (1, 1));
    let paths: Vec<String> = f
        .store
        .files()
        .unwrap()
        .into_iter()
        .map(|r| r.path)
        .collect();
    assert_eq!(paths, ["assets/ic_logo.png", "ok.kt"]);
    let again = f.reconcile(Depth::Full);
    assert_eq!((again.parsed, again.skipped, again.reused), (0, 0, 2));
}

#[test]
fn codemapignore_excludes_tracked_tree() {
    let mut f = Fixture::new();
    f.commit(&[
        ("vendor/x.kt", "class X\n"),
        ("src/y.kt", "class Y\n"),
        (".codemapignore", "vendor/\n"),
    ]);
    f.reconcile(Depth::Full);
    assert_eq!((f.names("X"), f.names("Y")), (0, 1));
}

#[test]
fn light_depth_stores_no_references() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A {\n    fun run() { fetch() }\n}\n")]);
    f.reconcile(Depth::Light);
    assert!(f.store.by_name("fetch").unwrap().is_empty());
    f.reconcile(Depth::Full);
    assert_eq!(
        f.reconcile(Depth::Full).parsed,
        0,
        "a light parse was not promoted on its own"
    );
}

#[test]
fn full_after_light_reparses_for_references() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A {\n    fun run() { fetch() }\n}\n")]);
    f.reconcile(Depth::Light);
    let stats = f.reconcile(Depth::Full);
    assert_eq!(
        stats.parsed, 1,
        "a light blob is parsed again at full depth"
    );
    let hits = f.store.by_name("fetch").unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].symbol.role, Role::Reference);
}

#[test]
fn literal_is_indexed_with_enclosing_definition() {
    let mut f = Fixture::new();
    f.commit(&[(
        "Route.kt",
        "@Resource(\"/api/v1/books/{id}\")\nclass BookRoute(val id: String)\n\nclass Other\n",
    )]);
    f.reconcile(Depth::Full);
    let hits = f.store.literals("/api/v1/books/{id}").unwrap();
    assert_eq!(hits.len(), 1);
    assert!(
        hits[0].symbol.flags.contains(&"in:BookRoute".to_string()),
        "{:?}",
        hits[0].symbol.flags
    );
}

#[test]
fn head_is_recorded_in_meta() {
    let mut f = Fixture::new();
    f.commit(&[("a.kt", "class A\n")]);
    f.reconcile(Depth::Full);
    assert_eq!(f.store.meta("head").map(|h| h.len()), Some(40));
}
