//! Generates the language enum, the extension map and the grammar table from `languages/registry.toml`,
//! and the query of every grammar language from `languages/<folder>/tags.scm` (the upstream copy, when
//! there is one) followed by `languages/<folder>/extra.scm` (codemap's additions).
//!
//! Output: `$OUT_DIR/languages_enum.rs` (included by `index/parse/mod.rs`), `$OUT_DIR/languages_grammars.rs`
//! (included by `index/parse/grammar.rs`) and `$OUT_DIR/queries/<folder>.scm`. The enum variant is the entry's
//! `variant` when given, else the capitalised `name`.

use std::env;
use std::fs;
use std::path::Path;

fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let languages_dir = Path::new(&manifest).join("languages");
    let registry_path = languages_dir.join("registry.toml");
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let text = fs::read_to_string(&registry_path).expect("languages/registry.toml is readable");
    let table: toml::Table = text.parse().expect("languages/registry.toml is valid TOML");
    let entries = table
        .get("language")
        .and_then(|v| v.as_array())
        .expect("registry.toml has [[language]] entries");

    let out = env::var("OUT_DIR").expect("OUT_DIR");
    let queries_out = Path::new(&out).join("queries");
    fs::create_dir_all(&queries_out).expect("create OUT_DIR/queries");

    // Changes whenever a language or a query does, so indexes parsed by an older set are rebuilt.
    let mut grammars_hash = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&text, &mut grammars_hash);

    let mut variants = String::new();
    let mut names = String::new();
    let mut from_names = String::new();
    let mut extensions = String::new();
    let mut filenames = String::new();
    let mut grammars = String::new();

    for entry in entries {
        let name = entry["name"].as_str().expect("language.name is a string");
        let variant = entry
            .get("variant")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| capitalise(name));
        variants.push_str(&format!("    {variant},\n"));
        names.push_str(&format!(
            "            Language::{variant} => \"{name}\".into(),\n"
        ));
        from_names.push_str(&format!(
            "            \"{name}\" => Some(Language::{variant}),\n"
        ));
        for ext in entry["extensions"]
            .as_array()
            .expect("language.extensions is a list")
        {
            let ext = ext.as_str().expect("extension is a string");
            extensions.push_str(&format!(
                "        \"{ext}\" => Some(Language::{variant}),\n"
            ));
        }
        for file in entry
            .get("filenames")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            let file = file.as_str().expect("filename is a string");
            filenames.push_str(&format!(
                "        \"{file}\" => Some(Language::{variant}),\n"
            ));
        }
        if let Some(path) = entry.get("crate").and_then(|v| v.as_str()) {
            let folder = entry.get("query").and_then(|v| v.as_str()).unwrap_or(name);
            let mut query = String::new();
            let mut found = false;
            for file in ["tags.scm", "extra.scm"] {
                let path = languages_dir.join(folder).join(file);
                println!("cargo:rerun-if-changed={}", path.display());
                if let Ok(text) = fs::read_to_string(&path) {
                    query.push_str(&text);
                    query.push('\n');
                    found = true;
                }
            }
            assert!(
                found,
                "language {name}: neither languages/{folder}/tags.scm nor extra.scm exists"
            );
            std::hash::Hash::hash(&query, &mut grammars_hash);
            fs::write(queries_out.join(format!("{folder}.scm")), query).expect("write query");
            grammars.push_str(&format!(
                "        Language::{variant} => ({path}.into(), include_str!(concat!(env!(\"OUT_DIR\"), \"/queries/{folder}.scm\"))),\n"
            ));
        }
    }

    println!(
        "cargo:rustc-env=CODEMAP_GRAMMARS={:x}",
        std::hash::Hasher::finish(&grammars_hash)
    );

    let enum_source = format!(
        "/// A language the index knows, generated from languages/registry.toml.\n\
         #[derive(Debug, Clone, PartialEq, Eq, Hash)]\n\
         pub enum Language {{\n{variants}    /// Any other source extension; parsed by the regex tier.\n    Other(String),\n}}\n\n\
         impl Language {{\n\
         \x20   pub fn name(&self) -> String {{\n        match self {{\n{names}            Language::Other(ext) => ext.clone(),\n        }}\n    }}\n\n\
         \x20   pub fn from_name(name: &str) -> Option<Language> {{\n        match name {{\n{from_names}            _ => None,\n        }}\n    }}\n\
         }}\n\n\
         /// The language registered for a lower-case file extension, if any.\n\
         fn by_extension(ext: &str) -> Option<Language> {{\n    match ext {{\n{extensions}        _ => None,\n    }}\n}}\n\n\
         /// The language registered for a whole file name (`Makefile`), if any.\n\
         fn by_filename(file: &str) -> Option<Language> {{\n    match file {{\n{filenames}        _ => None,\n    }}\n}}\n"
    );
    let grammars_source = format!(
        "pub fn grammar_for(lang: &Language) -> Option<Grammar> {{\n\
         \x20   let (language, query): (TsLanguage, &'static str) = match lang {{\n{grammars}        _ => return None,\n    }};\n\
         \x20   Some(Grammar {{ language, query }})\n}}\n"
    );

    fs::write(Path::new(&out).join("languages_enum.rs"), enum_source)
        .expect("write languages_enum.rs");
    fs::write(
        Path::new(&out).join("languages_grammars.rs"),
        grammars_source,
    )
    .expect("write languages_grammars.rs");
}
