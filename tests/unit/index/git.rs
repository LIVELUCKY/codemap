use super::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8(out.stdout).unwrap()
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "t"]);
    dir
}

fn commit(root: &Path, files: &[(&str, &str)]) {
    for (path, content) in files {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "x"]);
}

#[test]
fn tree_lists_tracked_files_with_blob_sha() {
    let dir = repo();
    commit(dir.path(), &[("a.kt", "class A\n")]);
    let tree = tree(dir.path()).unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].path, "a.kt");
    assert_eq!(tree[0].sha, blob_sha(b"class A\n"));
}

#[test]
fn tree_handles_spaces_and_unicode_in_paths() {
    let dir = repo();
    commit(dir.path(), &[("dir with space/ü.kt", "x")]);
    let tree = tree(dir.path()).unwrap();
    assert_eq!(tree[0].path, "dir with space/ü.kt");
}

#[test]
fn dirty_lists_modified_untracked_and_deleted() {
    let dir = repo();
    commit(dir.path(), &[("a.kt", "a"), ("c.kt", "c")]);
    fs::write(dir.path().join("a.kt"), "changed").unwrap();
    fs::write(dir.path().join("b.kt"), "new").unwrap();
    fs::remove_file(dir.path().join("c.kt")).unwrap();
    let mut dirty = dirty(dir.path()).unwrap();
    dirty.sort_by(|x, y| x.path.cmp(&y.path));
    let view: Vec<(&str, bool)> = dirty.iter().map(|d| (d.path.as_str(), d.deleted)).collect();
    assert_eq!(view, vec![("a.kt", false), ("b.kt", false), ("c.kt", true)]);
}

#[test]
fn blob_sha_matches_git_hash_object() {
    let dir = repo();
    fs::write(dir.path().join("x"), "hello blob\n").unwrap();
    let expected = git(dir.path(), &["hash-object", "x"]);
    assert_eq!(blob_sha(b"hello blob\n"), expected.trim());
}

#[test]
fn toplevel_is_none_outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    assert!(toplevel(dir.path()).is_none());
    let repo = repo();
    assert_eq!(
        toplevel(&repo.path().join("sub")).unwrap_or_default(),
        repo.path().canonicalize().unwrap()
    );
}

#[test]
fn head_is_some_after_a_commit() {
    let dir = repo();
    assert!(head(dir.path()).is_none());
    commit(dir.path(), &[("a", "a")]);
    assert_eq!(head(dir.path()).unwrap().len(), 40);
}
