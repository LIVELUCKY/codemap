//! The project (git top level of the working directory) and the sibling repositories its
//! `.codemap.json` lists.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::config::{self, Config};
use crate::index::git;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub root: PathBuf,
    /// The folder name for the project; for a sibling the path `.codemap.json` gives (`../server`),
    /// which answers put before its paths so each one can be opened as printed.
    pub name: String,
}

impl Repo {
    pub fn at(root: PathBuf) -> Repo {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Repo { root, name }
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub project: Repo,
    pub siblings: Vec<Repo>,
    pub config: Config,
    pub cache_dir: PathBuf,
}

impl Workspace {
    /// `prompt` with `{repositories}` replaced by what a query covers ("this repository and
    /// ../server"), so the agent neither changes into a sibling nor greps it.
    pub fn prompt(&self, prompt: &str) -> String {
        let mut covered = String::from("this repository");
        let names: Vec<&str> = self.siblings.iter().map(|s| s.name.as_str()).collect();
        if let Some((last, rest)) = names.split_last() {
            if !rest.is_empty() {
                covered.push_str(&format!(", {}", rest.join(", ")));
            }
            covered.push_str(&format!(" and {last}"));
        }
        prompt.replace("{repositories}", &covered)
    }
}

/// The workspace of `cwd`: its repository, and the siblings `.codemap.json` lists. Outside a
/// repository the error names the repositories right below, so the agent knows where to run it.
pub fn discover(cwd: &Path) -> Result<Workspace> {
    let root = git::toplevel(cwd).ok_or_else(|| {
        let mut below: Vec<String> = std::fs::read_dir(cwd)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().join(".git").exists())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        below.sort();
        if below.is_empty() {
            anyhow!("not a git repository: {}", cwd.display())
        } else {
            anyhow!(
                "not a git repository: {}; run codemap inside one of the repositories here: {}",
                cwd.display(),
                below.join(", ")
            )
        }
    })?;
    let project = Repo::at(canonical(root));
    let config = config::load(&project.root);
    // The index is a cache rebuilt in about a second. It lives in the user's own cache folder, not the
    // temp folder: `/tmp` is shared on Linux, where another user could plant an index or a link.
    // `prune` keeps it from growing. CODEMAP_CACHE_DIR moves it, which keeps tests apart.
    let cache_dir = std::env::var_os("CODEMAP_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::cache_dir().map(|d| d.join(env!("CARGO_PKG_NAME"))))
        .unwrap_or_else(|| std::env::temp_dir().join(env!("CARGO_PKG_NAME")));
    // A sibling is searched and printed without asking, and the list comes from the repository's own
    // file: it must be a git repository next to the project (inside the project's parent folder),
    // never an arbitrary folder such as `../../.ssh`.
    let beside = project.root.parent().unwrap_or(&project.root).to_path_buf();
    let siblings = config
        .siblings
        .iter()
        .map(|p| Repo {
            root: canonical(project.root.join(p)),
            name: p.to_string_lossy().trim_end_matches('/').to_string(),
        })
        .filter(|s| {
            let ok = s.root.starts_with(&beside)
                && s.root != project.root
                && s.root.join(".git").exists();
            if !ok && s.root.exists() {
                eprintln!(
                    "codemap: ignoring sibling {}: not a git repository inside {}",
                    s.name,
                    beside.display()
                );
            }
            ok
        })
        .collect();
    Ok(Workspace {
        project,
        siblings,
        config,
        cache_dir,
    })
}

fn canonical(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[cfg(test)]
#[path = "../tests/unit/workspace.rs"]
mod tests;
