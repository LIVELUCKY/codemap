//! The launcher's resolution order, run against a copy of the plugin in a temp dir so no real binary
//! of this checkout (target/release) can interfere.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Sandbox {
    _dir: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    cache: PathBuf,
}

fn sandbox() -> Sandbox {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("plugin");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join(".claude-plugin")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::copy(source.join("bin/codemap"), root.join("bin/codemap")).unwrap();
    fs::copy(
        source.join(".claude-plugin/plugin.json"),
        root.join(".claude-plugin/plugin.json"),
    )
    .unwrap();
    let home = dir.path().join("home");
    let cache = dir.path().join("cache");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cache).unwrap();
    Sandbox {
        _dir: dir,
        root,
        home,
        cache,
    }
}

fn fake(path: &Path, says: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("#!/bin/sh\necho {says} \"$@\"\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn launch(
    sb: &Sandbox,
    args: &[&str],
    env: &[(&str, &Path)],
    offline: bool,
) -> (i32, String, String) {
    let mut cmd = Command::new("sh");
    cmd.arg(sb.root.join("bin/codemap"))
        .args(args)
        .env("HOME", &sb.home)
        .env("XDG_CACHE_HOME", &sb.cache);
    cmd.env_remove("CODEMAP_BIN")
        .env_remove("CODEMAP_RELEASE_BASE")
        .env_remove("CODEMAP_TARGET");
    if offline {
        cmd.env("CODEMAP_OFFLINE", "1");
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn codemap_bin_wins() {
    let sb = sandbox();
    let explicit = sb.home.join("explicit");
    fake(&explicit, "explicit");
    fake(&sb.home.join(".cargo/bin/codemap"), "cargo");
    assert_eq!(
        launch(&sb, &["def", "X"], &[("CODEMAP_BIN", &explicit)], true).1,
        "explicit def X"
    );
}

#[test]
fn cargo_bin_is_second_then_checkout_then_local_bin() {
    let sb = sandbox();
    fake(&sb.home.join(".local/bin/codemap"), "local");
    assert_eq!(launch(&sb, &["a"], &[], true).1, "local a");
    fake(&sb.root.join("target/release/codemap"), "checkout");
    assert_eq!(launch(&sb, &["a"], &[], true).1, "checkout a");
    fake(&sb.home.join(".cargo/bin/codemap"), "cargo");
    assert_eq!(launch(&sb, &["a"], &[], true).1, "cargo a");
}

#[test]
fn cached_download_is_used_without_network() {
    let sb = sandbox();
    fake(
        &sb.cache.join(concat!(
            "codemap/bin/",
            env!("CARGO_PKG_VERSION"),
            "/codemap"
        )),
        "cached",
    );
    assert_eq!(launch(&sb, &["a"], &[], true).1, "cached a");
}

#[test]
fn hook_with_no_binary_exits_zero_and_prints_nothing_to_stdout() {
    let sb = sandbox();
    let (code, stdout, stderr) = launch(&sb, &["hook"], &[], true);
    assert_eq!((code, stdout.as_str()), (0, ""));
    assert!(stderr.contains("cargo install"), "{stderr}");
}

#[test]
fn other_command_with_no_binary_exits_one() {
    let sb = sandbox();
    let (code, stdout, stderr) = launch(&sb, &["def", "X"], &[], true);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(stderr.contains("no binary found"), "{stderr}");
}

fn release(sb: &Sandbox, sums_for: &str) -> PathBuf {
    let release = sb.home.join("release");
    fs::create_dir_all(&release).unwrap();
    let build = sb.home.join("build");
    fake(&build.join("codemap"), "downloaded");
    let status = Command::new("tar")
        .args(["-czf"])
        .arg(release.join("codemap-test-target.tar.gz"))
        .arg("-C")
        .arg(&build)
        .arg("codemap")
        .status()
        .unwrap();
    assert!(status.success());
    let sha = Command::new("shasum")
        .args(["-a", "256"])
        .arg(release.join("codemap-test-target.tar.gz"))
        .output()
        .unwrap();
    let sha = String::from_utf8_lossy(&sha.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    let line = if sums_for == "right" {
        format!("{sha}  codemap-test-target.tar.gz\n")
    } else {
        format!("{}  codemap-test-target.tar.gz\n", "0".repeat(64))
    };
    fs::write(release.join("SHA256SUMS"), line).unwrap();
    release
}

#[test]
fn download_verifies_checksum() {
    let sb = sandbox();
    let release = release(&sb, "wrong");
    let base = PathBuf::from(format!("file://{}", release.display()));
    let (code, stdout, stderr) = launch(
        &sb,
        &["def", "X"],
        &[
            ("CODEMAP_RELEASE_BASE", &base),
            ("CODEMAP_TARGET", Path::new("test-target")),
        ],
        false,
    );
    assert_eq!((code, stdout.as_str()), (1, ""), "{stderr}");
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(
        !sb.cache
            .join(concat!(
                "codemap/bin/",
                env!("CARGO_PKG_VERSION"),
                "/codemap"
            ))
            .exists()
    );
}

#[test]
fn download_with_the_right_checksum_runs_and_caches() {
    let sb = sandbox();
    let release = release(&sb, "right");
    let base = PathBuf::from(format!("file://{}", release.display()));
    let env: &[(&str, &Path)] = &[
        ("CODEMAP_RELEASE_BASE", &base),
        ("CODEMAP_TARGET", Path::new("test-target")),
    ];
    assert_eq!(launch(&sb, &["def", "X"], env, false).1, "downloaded def X");
    assert!(
        sb.cache
            .join(concat!(
                "codemap/bin/",
                env!("CARGO_PKG_VERSION"),
                "/codemap"
            ))
            .exists()
    );
    assert_eq!(
        launch(&sb, &["again"], &[], true).1,
        "downloaded again",
        "the cached copy needs no network"
    );
}
