//! The words of a file for full-text search: every identifier-like token of the source, each also
//! split on case and underscores, lower-cased, and each part reduced to its stem by the configured
//! Snowball stemmer. `RefreshingTokens` is indexed as `refreshingtokens refresh token`.

use std::collections::HashMap;

use rust_stemmers::{Algorithm, Stemmer};

use crate::model::Symbol;

/// A lower-cased word reduced to its stem; unchanged without a stemmer.
pub fn stem(word: &str, stemmer: Option<Algorithm>) -> String {
    match stemmer {
        Some(algorithm) => Stemmer::create(algorithm).stem(word).into_owned(),
        None => word.to_string(),
    }
}

/// The term a query word is looked up by, the same one `terms` stores for it: a compound identifier
/// whole, a single word by its stem.
pub fn query_term(word: &str, stemmer: Option<Algorithm>) -> String {
    let lower = word.to_lowercase();
    if split_identifier(word).len() > 1 {
        lower
    } else {
        stem(&lower, stemmer)
    }
}

/// `camelCase`, `PascalCase`, `snake_case`, `kebab-case` and digit boundaries become separate words.
pub fn split_identifier(token: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = token.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            flush(&mut current, &mut words);
            continue;
        }
        let boundary = i > 0 && {
            let prev = chars[i - 1];
            (c.is_uppercase() && prev.is_lowercase())
                || (c.is_uppercase()
                    && prev.is_uppercase()
                    && chars.get(i + 1).is_some_and(|n| n.is_lowercase()))
                || (c.is_ascii_digit() != prev.is_ascii_digit())
        };
        if boundary {
            flush(&mut current, &mut words);
        }
        current.push(c.to_ascii_lowercase());
    }
    flush(&mut current, &mut words);
    words
}

fn flush(current: &mut String, words: &mut Vec<String>) {
    if !current.is_empty() {
        words.push(std::mem::take(current));
    }
}

/// The searchable words of a blob with how often each occurs: whole identifiers plus their parts,
/// from the symbols and the first `max_text` bytes of the text; the rest of a huge file is not
/// searched by words. Counted here, in the parsing threads, so the store only writes rows.
pub fn terms(
    symbols: &[Symbol],
    source: &str,
    max_text: usize,
    stemmer: Option<Algorithm>,
) -> Vec<(String, u32)> {
    let stemmer = stemmer.map(Stemmer::create);
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut push = |token: &str| {
        let parts = split_identifier(token);
        if parts.len() > 1 {
            *counts.entry(token.to_lowercase()).or_default() += 1;
        }
        for part in parts {
            let term = match &stemmer {
                Some(s) => s.stem(&part).into_owned(),
                None => part,
            };
            *counts.entry(term).or_default() += 1;
        }
    };
    for symbol in symbols {
        push(&symbol.name);
    }
    let text = if source.len() > max_text {
        &source[..source
            .char_indices()
            .take_while(|(i, _)| *i < max_text)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0)]
    } else {
        source
    };
    for token in text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| t.len() > 1)
    {
        push(token);
    }
    counts.into_iter().collect()
}

#[cfg(test)]
#[path = "../../tests/unit/index/text.rs"]
mod tests;
