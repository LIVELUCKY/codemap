//! Module dependencies for the build systems codemap knows: Gradle, Cargo workspaces, npm workspaces.
//! Only dependencies between modules of the same workspace count; registry packages are not modules.

use std::path::{Path, PathBuf};

use regex::Regex;

use crate::output::{Answer, Line};
use crate::workspace::Workspace;

/// `from` depends on `to`, declared in `file` (repository-relative) at `line`.
struct Edge {
    from: String,
    to: String,
    file: String,
    line: u32,
}

struct Graph {
    system: &'static str,
    modules: Vec<String>,
    edges: Vec<Edge>,
}

pub fn deps(ws: &Workspace, module: &str) -> Answer {
    let root = &ws.project.root;
    let mut answer = Answer::new("build");
    let Some(graph) = gradle(root).or_else(|| cargo(root)).or_else(|| npm(root)) else {
        answer.notes.push(format!(
            "no build system recognised under {}",
            root.display()
        ));
        return answer;
    };
    answer.tier = graph.system.into();
    let mut lines: Vec<Line> = Vec::new();
    for edge in &graph.edges {
        let (kind, name) = if edge.from == module {
            ("depends-on", edge.to.clone())
        } else if edge.to == module {
            ("depended-on-by", edge.from.clone())
        } else {
            continue;
        };
        lines.push(Line {
            repo: None,
            path: edge.file.clone(),
            line: edge.line,
            kind: kind.into(),
            name,
            scope: "main".into(),
            flags: String::new(),
        });
    }
    lines.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
    if lines.is_empty() && !graph.modules.iter().any(|m| m == module) {
        answer.notes.push(format!(
            "unknown module {module}; modules: {}",
            graph.modules.join(", ")
        ));
    }
    answer.lines = lines;
    answer
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .into_owned()
}

/// The 1-based line of the first line containing `needle`, else 1.
fn line_of(text: &str, needle: &str) -> u32 {
    text.lines()
        .position(|l| l.contains(needle))
        .map(|i| i as u32 + 1)
        .unwrap_or(1)
}

fn gradle(root: &Path) -> Option<Graph> {
    let settings = ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .map(|f| root.join(f))
        .find(|p| p.is_file())?;
    let text = std::fs::read_to_string(&settings).ok()?;
    let include = Regex::new(r#"["']((?::[\w.-]+)+)["']"#).expect("gradle include regex");
    let modules: Vec<String> = text
        .lines()
        .filter(|l| l.trim_start().starts_with("include"))
        .flat_map(|l| {
            include
                .captures_iter(l)
                .map(|c| c[1].to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    let project =
        Regex::new(r#"project\(\s*["']((?::[\w.-]+)+)["']\s*\)"#).expect("gradle project regex");
    let mut edges = Vec::new();
    for module in &modules {
        let dir = root.join(module.trim_start_matches(':').replace(':', "/"));
        if !inside(root, &dir) {
            continue;
        }
        let Some(build) = ["build.gradle.kts", "build.gradle"]
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&build) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            for caps in project.captures_iter(line) {
                edges.push(Edge {
                    from: module.clone(),
                    to: caps[1].to_string(),
                    file: relative(root, &build),
                    line: i as u32 + 1,
                });
            }
        }
    }
    Some(Graph {
        system: "gradle",
        modules,
        edges,
    })
}

/// Expands `crates/*` and plain member paths of a workspace list.
fn expand_members(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for pattern in patterns {
        if let Some(prefix) = pattern.strip_suffix("/*") {
            if let Ok(entries) = std::fs::read_dir(root.join(prefix)) {
                let mut found: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                found.sort();
                dirs.extend(found);
            }
        } else {
            dirs.push(root.join(pattern));
        }
    }
    dirs.retain(|dir| inside(root, dir));
    dirs
}

/// Whether a module folder named by a build file is inside the repository; `..` in a member list
/// must not lead out of it.
fn inside(root: &Path, dir: &Path) -> bool {
    match (root.canonicalize(), dir.canonicalize()) {
        (Ok(root), Ok(dir)) => dir.starts_with(root),
        _ => false,
    }
}

fn cargo(root: &Path) -> Option<Graph> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let table: toml::Table = text.parse().ok()?;
    let members: Vec<String> = table
        .get("workspace")?
        .get("members")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut packages: Vec<(String, PathBuf, String)> = Vec::new();
    for dir in expand_members(root, &members) {
        let manifest = dir.join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        if let Some(name) = table
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            packages.push((name.to_string(), dir.clone(), text));
        }
    }
    let name_of_dir = |dir: &Path| -> Option<String> {
        let dir = dir.canonicalize().ok()?;
        packages
            .iter()
            .find(|(_, d, _)| d.canonicalize().ok().as_deref() == Some(dir.as_path()))
            .map(|(n, _, _)| n.clone())
    };
    let mut edges = Vec::new();
    for (name, dir, text) in &packages {
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = table.get(section).and_then(|d| d.as_table()) else {
                continue;
            };
            for (key, value) in deps {
                let Some(path) = value.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                if let Some(to) = name_of_dir(&dir.join(path)) {
                    edges.push(Edge {
                        from: name.clone(),
                        to,
                        file: relative(root, &dir.join("Cargo.toml")),
                        line: line_of(text, &format!("{key} ")),
                    });
                }
            }
        }
    }
    let modules = packages.iter().map(|(n, _, _)| n.clone()).collect();
    Some(Graph {
        system: "cargo",
        modules,
        edges,
    })
}

fn npm(root: &Path) -> Option<Graph> {
    let text = std::fs::read_to_string(root.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let workspaces = json.get("workspaces")?;
    let patterns: Vec<String> = workspaces
        .as_array()
        .or_else(|| workspaces.get("packages").and_then(|p| p.as_array()))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut packages: Vec<(String, PathBuf, String, serde_json::Value)> = Vec::new();
    for dir in expand_members(root, &patterns) {
        let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
            packages.push((name.to_string(), dir, text, json));
        }
    }
    let names: Vec<String> = packages.iter().map(|(n, _, _, _)| n.clone()).collect();
    let mut edges = Vec::new();
    for (name, dir, text, json) in &packages {
        for section in ["dependencies", "devDependencies", "peerDependencies"] {
            let Some(deps) = json.get(section).and_then(|d| d.as_object()) else {
                continue;
            };
            for (key, value) in deps {
                let spec = value.as_str().unwrap_or("");
                if names.contains(key)
                    || spec.starts_with("workspace:")
                    || spec.starts_with("file:")
                {
                    edges.push(Edge {
                        from: name.clone(),
                        to: key.clone(),
                        file: relative(root, &dir.join("package.json")),
                        line: line_of(text, &format!("\"{key}\"")),
                    });
                }
            }
        }
    }
    Some(Graph {
        system: "npm",
        modules: names,
        edges,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/query/deps.rs"]
mod tests;
