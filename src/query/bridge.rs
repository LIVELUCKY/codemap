//! Cross-language bridges: how a definition in one language is named in another, so `uses` can find
//! the foreign call sites and label them.
//!
//! Two shapes of rule: a facade rename (`{file}Kt`: a Kotlin top-level function in `Platform.kt` is
//! reached as `PlatformKt` from Swift), and an import match (`{name}`: a TypeScript export is reached
//! by an import whose path ends in the file, a Python definition by `from pkg.mod import name`).

use crate::config::BridgeRule;
use crate::config::settings::Settings;
use crate::index::store::Store;
use crate::model::{Hit, Role};
use crate::workspace::Workspace;

/// Whether `language` is the rule's language or one of its family (`settings.languageFamilies`).
fn same_language(rule_language: &str, language: &str, settings: &Settings) -> bool {
    rule_language == language
        || settings
            .language_families
            .get(rule_language)
            .is_some_and(|family| family.iter().any(|l| l == language))
}

fn file_stem(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    stem.replace('.', "_")
}

/// The names `def` is reachable by in the rule's target language, beyond its own name.
pub fn foreign_names(rule: &BridgeRule, def: &Hit, settings: &Settings) -> Vec<String> {
    if !same_language(&rule.from_language, &def.language, settings)
        || !rule.rename.contains("{file}")
    {
        return Vec::new();
    }
    // Classes keep their name across a facade; only the kinds listed get renamed.
    if !settings.kinds.facades.contains(&def.symbol.kind) {
        return Vec::new();
    }
    vec![
        rule.rename
            .replace("{file}", &file_stem(&def.path))
            .replace("{name}", &def.symbol.name),
    ]
}

/// Whether an import of `import_path` (the flag of an import reference) can mean `def`'s file.
pub fn import_matches(rule: &BridgeRule, def: &Hit, import_path: &str) -> bool {
    let def_path = def
        .path
        .rsplit_once('.')
        .map(|(p, _)| p)
        .unwrap_or(&def.path);
    let def_segments: Vec<&str> = def_path.split('/').collect();
    let import_segments: Vec<&str> = import_path
        .split(rule.import_separator.as_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect();
    !import_segments.is_empty() && def_segments.ends_with(&import_segments)
}

/// Foreign references grouped by the name of the rule that found them.
pub type Bridged = Vec<(String, Vec<(Option<String>, Hit)>)>;

/// The foreign references of `def` through every rule (`settings.bridges` and the repository's
/// `bridges`), by rule name.
pub fn apply(ws: &Workspace, stores: &[(Option<String>, Store)], def: &Hit) -> Bridged {
    let settings = &ws.config.settings;
    let rules = settings.bridges.iter().chain(&ws.config.bridges);
    let mut out = Vec::new();
    for rule in rules.filter(|r| same_language(&r.from_language, &def.language, settings)) {
        let mut hits = Vec::new();
        if rule.rename.contains("{file}") {
            for foreign in foreign_names(rule, def, settings) {
                for (repo, store) in stores {
                    let found = store.by_name(&foreign).unwrap_or_default();
                    hits.extend(
                        found
                            .into_iter()
                            .filter(|h| {
                                h.symbol.role == Role::Reference
                                    && same_language(&rule.to_language, &h.language, settings)
                            })
                            .map(|h| (repo.clone(), h)),
                    );
                }
            }
        } else {
            for (repo, store) in stores {
                let found = store.by_name(&def.symbol.name).unwrap_or_default();
                hits.extend(
                    found
                        .into_iter()
                        .filter(|h| {
                            h.symbol.role == Role::Reference
                                && h.symbol.kind == "import"
                                && same_language(&rule.to_language, &h.language, settings)
                                && h.symbol
                                    .flags
                                    .first()
                                    .is_some_and(|p| import_matches(rule, def, p))
                        })
                        .map(|h| (repo.clone(), h)),
                );
            }
        }
        if !hits.is_empty() {
            out.push((rule.name.clone(), hits));
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/unit/query/bridge.rs"]
mod tests;
