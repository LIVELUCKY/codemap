//! Access to git: the tracked tree with blob shas, the dirty set, and blob hashing.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// A tracked path and the sha of the blob git holds for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracked {
    pub path: String,
    pub sha: String,
}

/// A path that differs from the index: modified, untracked or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirty {
    pub path: String,
    pub deleted: bool,
}

fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("running git {:?}", args))?;
    if !out.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// The repository root containing `cwd`, or `None` outside any repository.
/// `cwd` may not exist yet; the nearest existing ancestor is used.
pub fn toplevel(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd;
    while !dir.is_dir() {
        dir = dir.parent()?;
    }
    let out = run(dir, &["rev-parse", "--show-toplevel"]).ok()?;
    let text = String::from_utf8(out).ok()?;
    let line = text.trim();
    (!line.is_empty()).then(|| PathBuf::from(line))
}

/// Every tracked path with its blob sha, from `git ls-files -s -z`.
pub fn tree(root: &Path) -> Result<Vec<Tracked>> {
    let out = run(root, &["ls-files", "-s", "-z"])?;
    let text = String::from_utf8(out).context("ls-files output is not UTF-8")?;
    let mut files = Vec::new();
    for entry in text.split('\0').filter(|e| !e.is_empty()) {
        let (meta, path) = entry
            .split_once('\t')
            .context("ls-files entry without a tab")?;
        let sha = meta
            .split_whitespace()
            .nth(1)
            .context("ls-files entry without a sha")?;
        files.push(Tracked {
            path: path.to_string(),
            sha: sha.to_string(),
        });
    }
    Ok(files)
}

/// Modified, untracked and deleted paths, from `git status --porcelain -z`.
pub fn dirty(root: &Path) -> Result<Vec<Dirty>> {
    let out = run(
        root,
        &["status", "--porcelain", "-z", "--untracked-files=all"],
    )?;
    let text = String::from_utf8(out).context("status output is not UTF-8")?;
    let mut entries = text.split('\0').filter(|e| !e.is_empty());
    let mut dirty = Vec::new();
    while let Some(entry) = entries.next() {
        let (status, path) = entry.split_at(
            entry
                .char_indices()
                .nth(3)
                .map(|(i, _)| i)
                .unwrap_or(entry.len()),
        );
        let status = status.as_bytes();
        if status.len() >= 2 && (status[0] == b'R' || status[0] == b'C') {
            // A rename or copy carries the old path as its own entry; the old path is gone.
            if let Some(old) = entries.next() {
                dirty.push(Dirty {
                    path: old.to_string(),
                    deleted: true,
                });
            }
        }
        let deleted = status.len() >= 2 && (status[0] == b'D' || status[1] == b'D');
        dirty.push(Dirty {
            path: path.to_string(),
            deleted,
        });
    }
    Ok(dirty)
}

/// What .gitattributes says about a path: `linguist-generated` and `linguist-vendored`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Linguist {
    pub generated: bool,
    pub vendored: bool,
}

/// The linguist attributes of `paths` (repository-relative), from one `git check-attr -z --stdin`.
/// A path with neither attribute set is absent.
pub fn linguist(root: &Path, paths: &[&str]) -> Result<HashMap<String, Linguist>> {
    let mut found: HashMap<String, Linguist> = HashMap::new();
    // Without an attributes file there is nothing to ask, and most repositories have none: one git
    // process saved on every query. ponytail: a custom core.attributesFile alone is not seen; read the
    // git config here if that is ever needed.
    let home_attributes = dirs::home_dir().map(|h| h.join(".config/git/attributes"));
    let any_file = paths.iter().any(|p| p.ends_with(".gitattributes"))
        || root.join(".git/info/attributes").is_file()
        || home_attributes.is_some_and(|f| f.is_file());
    if paths.is_empty() || !any_file {
        return Ok(found);
    }
    let mut child = Command::new("git")
        .args([
            "check-attr",
            "-z",
            "--stdin",
            "linguist-generated",
            "linguist-vendored",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running git check-attr")?;
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let mut stdin = child.stdin.take().context("git check-attr stdin")?;
    // Written from another thread: git answers while it reads, and a full stdout pipe would stall both.
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output()?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("git check-attr writer panicked"))??;
    if !out.status.success() {
        bail!(
            "git check-attr failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let text = String::from_utf8(out.stdout).context("check-attr output is not UTF-8")?;
    let fields: Vec<&str> = text.split('\0').collect();
    for [path, attribute, value] in fields.as_chunks::<3>().0 {
        if *value != "set" && *value != "true" {
            continue;
        }
        let entry = found.entry(path.to_string()).or_default();
        match *attribute {
            "linguist-generated" => entry.generated = true,
            "linguist-vendored" => entry.vendored = true,
            _ => {}
        }
    }
    Ok(found)
}

/// The sha git would give these bytes as a blob.
pub fn blob_sha(bytes: &[u8]) -> String {
    let mut hasher = sha1_smol::Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    hasher.digest().to_string()
}

/// The commit at HEAD, or `None` before the first commit.
pub fn head(root: &Path) -> Option<String> {
    let out = run(root, &["rev-parse", "--verify", "HEAD"]).ok()?;
    let text = String::from_utf8(out).ok()?;
    let line = text.trim();
    (!line.is_empty()).then(|| line.to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/index/git.rs"]
mod tests;
