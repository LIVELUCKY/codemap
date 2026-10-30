//! The language registry (`languages/registry.toml`) is the single source of truth; the generated
//! enum, extension map and grammar table must agree with it.

use codemap::index::parse::{Language, Tier, grammar::grammar_for, language_of, tier};

#[derive(serde::Deserialize)]
struct Registry {
    language: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    name: String,
    extensions: Vec<String>,
    #[serde(rename = "crate")]
    crate_path: Option<String>,
    query: Option<String>,
    upstream_tags: Option<bool>,
}

impl Entry {
    fn folder(&self) -> std::path::PathBuf {
        std::path::Path::new("languages").join(self.query.as_deref().unwrap_or(&self.name))
    }
}

fn registry() -> Registry {
    toml::from_str(include_str!("../languages/registry.toml")).expect("registry.toml parses")
}

#[test]
fn every_registry_language_has_a_grammar_or_is_regex_only() {
    for entry in registry().language {
        let lang = Language::from_name(&entry.name)
            .unwrap_or_else(|| panic!("no Language for {}", entry.name));
        let has_grammar = grammar_for(&lang).is_some();
        assert_eq!(
            has_grammar,
            entry.crate_path.is_some(),
            "{}: grammar presence must follow the registry",
            entry.name
        );
        let has_query =
            entry.folder().join("tags.scm").exists() || entry.folder().join("extra.scm").exists();
        assert_eq!(
            has_grammar, has_query,
            "{}: a grammar needs a query folder and vice versa",
            entry.name
        );
        assert_eq!(
            tier(&lang),
            if has_grammar {
                Tier::Grammar
            } else {
                Tier::Regex
            }
        );
        assert_eq!(lang.name(), entry.name);
    }
}

/// The upstream `queries/tags.scm` of a grammar crate, found in the local cargo registry after `cargo fetch`.
fn upstream_tags(crate_path: &str) -> Option<(std::path::PathBuf, String)> {
    let crate_name = crate_path.split("::").next()?.replace('_', "-");
    let home = std::env::var("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").expect("HOME")).join(".cargo")
        });
    let registry = home.join("registry").join("src");
    let mut candidates: Vec<std::path::PathBuf> = std::fs::read_dir(&registry)
        .ok()?
        .flatten()
        .flat_map(|index| std::fs::read_dir(index.path()).ok().into_iter().flatten())
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| {
                    n.strip_prefix(&format!("{crate_name}-"))
                        .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit()))
                })
                .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    let dir = candidates.pop()?;
    let file = dir.join("queries").join("tags.scm");
    let text = std::fs::read_to_string(&file).ok()?;
    Some((file, text))
}

#[test]
fn every_tags_scm_matches_its_upstream_copy() {
    for entry in registry().language {
        let Some(crate_path) = entry.crate_path.as_deref() else {
            continue;
        };
        let ours = entry.folder().join("tags.scm");
        if entry.upstream_tags == Some(false) {
            assert!(
                !ours.exists(),
                "{}: upstream tags are switched off, so tags.scm must not exist",
                entry.name
            );
            continue;
        }
        match upstream_tags(crate_path) {
            Some((file, upstream)) => {
                let text = std::fs::read_to_string(&ours).unwrap_or_else(|_| {
                    panic!(
                        "{}: upstream ships {} but languages/{}/tags.scm is missing",
                        entry.name,
                        file.display(),
                        entry.name
                    )
                });
                // two header lines name the crate, version and licence; the rest is the verbatim copy
                let body: String = text.lines().skip(2).collect::<Vec<_>>().join("\n");
                assert_eq!(
                    body.trim_end(),
                    upstream.trim_end(),
                    "{}: languages/{}/tags.scm differs from {}",
                    entry.name,
                    entry.name,
                    file.display()
                );
            }
            None => assert!(
                !ours.exists(),
                "{}: no upstream tags query, so languages/{}/tags.scm must not exist (use extra.scm)",
                entry.name,
                entry.name
            ),
        }
    }
}

#[test]
fn generated_extension_map_matches_registry() {
    for entry in registry().language {
        for ext in &entry.extensions {
            let detected = language_of(&format!("dir/file.{ext}"));
            assert_eq!(detected.name(), entry.name, "extension {ext}");
        }
    }
    assert_eq!(
        language_of("x.unknownext"),
        Language::Other("unknownext".into())
    );
}
