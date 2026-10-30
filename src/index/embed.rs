//! Semantic vectors: a static embedding model made for code retrieval turns a file (its path and the
//! start of its text) and a query into vectors, and the files closest to the query are one more
//! ranking of the word search. "database access object for orders" finds `OrderDao` although they share
//! no word.
//!
//! The model is static: a table with one vector per token, and a text's vector is the mean of the
//! vectors of its tokens. No neural network runs, so embedding a file costs about as much as
//! tokenizing it. The model is compiled into the binary (`models/`, MIT licence), so there is nothing
//! to install and no network call.
//!
//! Everything the model needs is in this file: reading the table (safetensors, half-precision
//! floats), and the tokenizer the model was trained with (BERT's: lower-case, split on whitespace
//! and punctuation, then WordPiece). Two simplifications apply outside ASCII and accented Latin
//! letters, which is where code and its comments live: accents of other scripts are only removed
//! when they are written as combining marks, and every character that is neither a letter nor a
//! digit counts as punctuation.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// The bundled model.
pub const MODEL: &str = "minishlab/potion-code-16M-v2";

static TOKENIZER: &[u8] = include_bytes!("../../models/potion-code-16M-v2/tokenizer.json");
static WEIGHTS: &[u8] = include_bytes!("../../models/potion-code-16M-v2/model.safetensors");

/// What the stored vectors were made with: part of the index fingerprint, so a binary that bundles
/// another model (or other weights under the same name) rebuilds them by itself after an update.
pub fn identity() -> String {
    format!("{MODEL}:{}", WEIGHTS.len())
}

/// Characters of a file's text that describe it, after its path: its imports and first definitions
/// say what it is about, and a mean over a whole large file says little.
const HEAD_CHARS: usize = 2000;

/// Tokens of a text that count, as the model was trained.
const MAX_TOKENS: usize = 512;

struct Model {
    /// Pieces that start a word, and pieces that continue one (stored without their `##`).
    starts: HashMap<String, u32>,
    continues: HashMap<String, u32>,
    unknown: u32,
    /// The longest piece in bytes: no longer prefix needs to be tried.
    longest: usize,
    /// A word longer than this is unknown as a whole.
    max_word_chars: usize,
    /// Tokens that are matched as written, before anything else (`[PAD]`, `[UNK]`).
    literal: Vec<(String, u32)>,
    /// Characters of a text that are tokenized at all: the token limit times the median piece length.
    max_chars: usize,
    /// The table, row after row, as little-endian half-precision floats.
    table: &'static [u8],
    dimensions: usize,
}

fn load() -> Option<Model> {
    let tokenizer: Value = serde_json::from_slice(TOKENIZER).ok()?;
    let model = tokenizer.get("model")?;
    let prefix = model.get("continuing_subword_prefix")?.as_str()?;
    let vocabulary = model.get("vocab")?.as_object()?;
    let mut starts = HashMap::new();
    let mut continues = HashMap::new();
    let mut lengths: Vec<usize> = Vec::with_capacity(vocabulary.len());
    for (piece, id) in vocabulary {
        let id = u32::try_from(id.as_u64()?).ok()?;
        lengths.push(piece.len());
        match piece.strip_prefix(prefix) {
            Some(rest) => continues.insert(rest.to_string(), id),
            None => starts.insert(piece.clone(), id),
        };
    }
    let unknown = *starts.get(model.get("unk_token")?.as_str()?)?;
    lengths.sort_unstable();
    let median = lengths.get(lengths.len() / 2).copied().unwrap_or(1);

    // safetensors: the length of a JSON header, the header, then the tensors' bytes.
    let header_length =
        usize::try_from(u64::from_le_bytes(WEIGHTS.get(..8)?.try_into().ok()?)).ok()?;
    let header: Value = serde_json::from_slice(WEIGHTS.get(8..8 + header_length)?).ok()?;
    let tensor = header.get("embeddings")?;
    if tensor.get("dtype")?.as_str()? != "F16" {
        return None;
    }
    let shape = tensor.get("shape")?.as_array()?;
    let rows = usize::try_from(shape.first()?.as_u64()?).ok()?;
    let dimensions = usize::try_from(shape.get(1)?.as_u64()?).ok()?;
    let offsets = tensor.get("data_offsets")?.as_array()?;
    let from = 8 + header_length + usize::try_from(offsets.first()?.as_u64()?).ok()?;
    let table = WEIGHTS.get(from..from + rows * dimensions * 2)?;
    let literal = tokenizer
        .get("added_tokens")
        .and_then(Value::as_array)
        .map(|tokens| {
            tokens
                .iter()
                .filter_map(|t| {
                    let id = u32::try_from(t.get("id")?.as_u64()?).ok()?;
                    Some((t.get("content")?.as_str()?.to_string(), id))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Model {
        literal,
        longest: lengths.last().copied().unwrap_or(1),
        starts,
        continues,
        unknown,
        max_word_chars: model
            .get("max_input_chars_per_word")
            .and_then(Value::as_u64)
            .unwrap_or(100) as usize,
        max_chars: MAX_TOKENS * median,
        table,
        dimensions,
    })
}

/// The model, read once per process on first use, so a query that needs no vector never pays for it.
fn model() -> Option<&'static Model> {
    static LOADED: OnceLock<Option<Model>> = OnceLock::new();
    LOADED.get_or_init(load).as_ref()
}

/// A half-precision float as a single-precision one.
fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let fraction = u32::from(bits & 0x3ff);
    match exponent {
        // Zero and the subnormal numbers: the fraction counts in steps of 2^-24.
        0 => {
            let value = fraction as f32 / 16_777_216.0;
            if sign == 0 { value } else { -value }
        }
        // Infinity and not-a-number.
        0x1f => f32::from_bits(sign | 0x7f80_0000 | (fraction << 13)),
        _ => f32::from_bits(sign | ((exponent + 112) << 23) | (fraction << 13)),
    }
}

/// Latin letters with an accent and the letter without it, in pairs (`ÀA`, `éE` is `ée`): what
/// Unicode's canonical decomposition gives for U+00C0 to U+024F once the accent is dropped.
const ACCENTED: &str = "ÀAÁAÂAÃAÄAÅAÇCÈEÉEÊEËEÌIÍIÎIÏIÑNÒOÓOÔOÕOÖOÙUÚUÛUÜUÝYàaáaâaãaäaåaçcèeéeêeëeìiíiîiïiñnòoóoôoõoöoùuúuûuüuýyÿyĀAāaĂAăaĄAąaĆCćcĈCĉcĊCċcČCčcĎDďdĒEēeĔEĕeĖEėeĘEęeĚEěeĜGĝgĞGğgĠGġgĢGģgĤHĥhĨIĩiĪIīiĬIĭiĮIįiİIĴJĵjĶKķkĹLĺlĻLļlĽLľlŃNńnŅNņnŇNňnŌOōoŎOŏoŐOőoŔRŕrŖRŗrŘRřrŚSśsŜSŝsŞSşsŠSšsŢTţtŤTťtŨUũuŪUūuŬUŭuŮUůuŰUűuŲUųuŴWŵwŶYŷyŸYŹZźzŻZżzŽZžzƠOơoƯUưuǍAǎaǏIǐiǑOǒoǓUǔuǕUǖuǗUǘuǙUǚuǛUǜuǞAǟaǠAǡaǢÆǣæǦGǧgǨKǩkǪOǫoǬOǭoǮƷǯʒǰjǴGǵgǸNǹnǺAǻaǼÆǽæǾØǿøȀAȁaȂAȃaȄEȅeȆEȇeȈIȉiȊIȋiȌOȍoȎOȏoȐRȑrȒRȓrȔUȕuȖUȗuȘSșsȚTțtȞHȟhȦAȧaȨEȩeȪOȫoȬOȭoȮOȯoȰOȱoȲYȳy";

/// The letter an accented Latin letter is built on; any other character as it is.
fn without_accent(c: char) -> char {
    static BASES: OnceLock<HashMap<char, char>> = OnceLock::new();
    let bases = BASES.get_or_init(|| {
        let letters: Vec<char> = ACCENTED.chars().collect();
        letters
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| (pair[0], pair[1]))
            .collect()
    });
    bases.get(&c).copied().unwrap_or(c)
}

/// BERT's normalizer: control characters dropped, every whitespace a space, a space around each
/// Chinese character, accents removed, lower case.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            match c {
                '\t' | '\n' | '\r' => out.push(' '),
                '\0'..='\x1f' | '\x7f' => {}
                _ => out.push(c.to_ascii_lowercase()),
            }
        } else if c == '\u{fffd}' || c.is_control() || ('\u{300}'..='\u{36f}').contains(&c) {
        } else if c.is_whitespace() {
            out.push(' ');
        } else if is_chinese(c) {
            out.push(' ');
            out.push(c);
            out.push(' ');
        } else {
            out.extend(without_accent(c).to_lowercase());
        }
    }
    out
}

fn is_chinese(c: char) -> bool {
    matches!(
        c as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B920..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

/// BERT's pre-tokenizer: words are split at whitespace, and every punctuation character is a word
/// of its own.
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (at, c) in text.char_indices() {
        let space = c.is_whitespace();
        let punctuation = !space
            && if c.is_ascii() {
                c.is_ascii_punctuation()
            } else {
                !c.is_alphanumeric()
            };
        if space || punctuation {
            if let Some(from) = start.take() {
                out.push(&text[from..at]);
            }
            if punctuation {
                out.push(&text[at..at + c.len_utf8()]);
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(from) = start {
        out.push(&text[from..]);
    }
    out
}

impl Model {
    /// WordPiece: the longest piece that starts the word, then the longest that continues it, until
    /// the word is used up. A word that cannot be split this way is unknown as a whole.
    fn pieces(&self, word: &str, out: &mut Vec<u32>) {
        if word.len() > self.max_word_chars && word.chars().count() > self.max_word_chars {
            out.push(self.unknown);
            return;
        }
        let mark = out.len();
        let mut from = 0;
        while from < word.len() {
            let table = if from == 0 {
                &self.starts
            } else {
                &self.continues
            };
            let mut to = (from + self.longest).min(word.len());
            let found = loop {
                if to <= from {
                    break None;
                }
                if word.is_char_boundary(to)
                    && let Some(id) = table.get(&word[from..to])
                {
                    break Some(*id);
                }
                to -= 1;
            };
            match found {
                Some(id) => {
                    out.push(id);
                    from = to;
                }
                None => {
                    out.truncate(mark);
                    out.push(self.unknown);
                    return;
                }
            }
        }
    }

    /// The token ids of a text, in order.
    fn tokens(&self, text: &str) -> Vec<u32> {
        let cut = text
            .char_indices()
            .nth(self.max_chars)
            .map_or(text, |(at, _)| &text[..at]);
        let mut ids = Vec::new();
        let mut rest = cut;
        while !rest.is_empty() {
            // The earliest literal token; the text before it is tokenized as usual.
            let next = self
                .literal
                .iter()
                .filter_map(|(content, id)| {
                    Some((rest.find(content.as_str())?, content.len(), *id))
                })
                .min_by_key(|(at, _, _)| *at);
            let (plain, after) = match next {
                Some((at, length, _)) => (&rest[..at], &rest[at + length..]),
                None => (rest, ""),
            };
            for word in words(&normalize(plain)) {
                self.pieces(word, &mut ids);
            }
            if let Some((_, _, id)) = next {
                ids.push(id);
            }
            rest = after;
        }
        ids
    }

    /// The mean of the vectors of the first tokens of a text, unknown tokens left out, at length one.
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut sum = vec![0.0_f32; self.dimensions];
        let mut count = 0usize;
        for id in self.tokens(text).into_iter().take(MAX_TOKENS) {
            if id == self.unknown {
                continue;
            }
            let from = id as usize * self.dimensions * 2;
            let Some(row) = self.table.get(from..from + self.dimensions * 2) else {
                continue;
            };
            for (total, bytes) in sum.iter_mut().zip(row.as_chunks::<2>().0) {
                *total += half_to_f32(u16::from_le_bytes(*bytes));
            }
            count += 1;
        }
        let count = count.max(1) as f32;
        let mut squares = 0.0_f32;
        for value in &mut sum {
            *value /= count;
            squares += *value * *value;
        }
        let length = squares.sqrt().max(1e-12);
        for value in &mut sum {
            *value /= length;
        }
        sum
    }
}

/// What is embedded for a file: its path, then the start of its text (nothing for a file that is not
/// text, which is then found by what its path says).
pub fn document(path: &str, text: &str) -> String {
    let head: String = text.chars().take(HEAD_CHARS).collect();
    format!("{path}\n{head}")
}

/// The unit vector of `text`.
pub fn embed(text: &str) -> Option<Vec<f32>> {
    Some(model()?.embed(text))
}

/// A vector as it is stored: every dimension of a unit vector lies between -1 and 1, so it is kept as
/// a 16-bit integer, two bytes a dimension. The index holds and reads half of what floats would take.
pub fn to_bytes(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|v| ((v.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16).to_le_bytes())
        .collect()
}

/// The cosine of a unit vector and a stored one, read straight from its bytes; 0 when the stored
/// vector is not of the same model.
pub fn similarity(wanted: &[f32], stored: &[u8]) -> f32 {
    if stored.len() != wanted.len() * 2 {
        return 0.0;
    }
    let dot: f32 = wanted
        .iter()
        .zip(stored.as_chunks::<2>().0)
        .map(|(w, b)| w * f32::from(i16::from_le_bytes(*b)))
        .sum();
    dot / f32::from(i16::MAX)
}

#[cfg(test)]
#[path = "../../tests/unit/index/embed.rs"]
mod tests;
