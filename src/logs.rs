//! `codemap logs`: the gist of a log or of command output (a file, or a pipe from `docker logs`,
//! `adb logcat`, a test run). Lines that differ only in numbers, ids, hashes or times are one entry
//! with a count and the range of lines it covers; indented continuation lines (stack traces) stay with
//! the line they follow. Levels come from documented formats only: Android logcat (threadtime and
//! brief), JSON lines with a `level` field, and explicit markers (`ERROR`, `[warn]`, `level=error`).

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::OnceLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, clap::ValueEnum)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn letter(self) -> char {
        match self {
            Level::Trace => 'T',
            Level::Debug => 'D',
            Level::Info => 'I',
            Level::Warn => 'W',
            Level::Error => 'E',
        }
    }

    fn from_word(word: &str) -> Option<Level> {
        Some(match word.to_ascii_lowercase().as_str() {
            "v" | "verbose" | "trace" => Level::Trace,
            "d" | "debug" => Level::Debug,
            "i" | "info" | "notice" => Level::Info,
            "w" | "warn" | "warning" => Level::Warn,
            "e" | "f" | "a" | "error" | "err" | "fatal" | "critical" | "crit" | "panic"
            | "severe" => Level::Error,
            _ => return None,
        })
    }
}

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("log pattern"))
}

/// The level of a line, when its format states one.
pub fn level_of(line: &str) -> Option<Level> {
    static LOGCAT: OnceLock<Regex> = OnceLock::new();
    static BRIEF: OnceLock<Regex> = OnceLock::new();
    static JSON: OnceLock<Regex> = OnceLock::new();
    static KEY: OnceLock<Regex> = OnceLock::new();
    static MARKER: OnceLock<Regex> = OnceLock::new();
    // logcat -v threadtime: `10-30 12:00:00.123  1234  5678 E Tag: message`
    let logcat = regex(
        &LOGCAT,
        r"^\d\d-\d\d \d\d:\d\d:\d\d\.\d+\s+\d+\s+\d+ ([VDIWEFA]) ",
    );
    // logcat -v brief: `E/Tag( 1234): message`
    let brief = regex(&BRIEF, r"^([VDIWEFA])/[^(]+\(\s*\d+\):");
    let json = regex(&JSON, r#""(?:level|severity|lvl)"\s*:\s*"(\w+)""#);
    let key = regex(&KEY, r"\b(?:level|severity|lvl)=(\w+)");
    // An upper-case level word standing alone or in brackets: `ERROR`, `[WARN]`, `<error>` is not one.
    let marker = regex(
        &MARKER,
        r"(?:^|[\s\[(|])(TRACE|DEBUG|INFO|NOTICE|WARN|WARNING|ERROR|ERR|FATAL|CRITICAL|PANIC|SEVERE)(?:$|[\s\]):|])",
    );
    for re in [logcat, brief, json, key, marker] {
        if let Some(caps) = re.captures(line) {
            return Level::from_word(&caps[1]);
        }
    }
    None
}

/// The line with everything that varies between repeats of one message replaced by a placeholder:
/// times, UUIDs, hex, numbers.
pub fn template(line: &str) -> String {
    static TIME: OnceLock<Regex> = OnceLock::new();
    static UUID: OnceLock<Regex> = OnceLock::new();
    static HEX: OnceLock<Regex> = OnceLock::new();
    static NUMBER: OnceLock<Regex> = OnceLock::new();
    let time = regex(
        &TIME,
        r"\d{2,4}[-/]\d\d[-/]\d\d[T ]?|\d\d:\d\d:\d\d(?:[.,]\d+)?(?:Z|[+-]\d\d:?\d\d)?",
    );
    let uuid = regex(
        &UUID,
        r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b",
    );
    let hex = regex(&HEX, r"(?i)\b0x[0-9a-f]+\b|\b[0-9a-f]{12,}\b");
    let number = regex(&NUMBER, r"\d+");
    let line = time.replace_all(line, "<t>");
    let line = uuid.replace_all(&line, "<id>");
    let line = hex.replace_all(&line, "<hex>");
    number.replace_all(&line, "<n>").into_owned()
}

fn continuation(line: &str) -> bool {
    line.starts_with([' ', '\t']) || line.starts_with("Caused by:") || line == "}"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub level: Option<Level>,
    pub first: usize,
    pub last: usize,
    pub count: usize,
    /// The first occurrence as written.
    pub text: String,
    /// Lines that followed it (a stack trace), from the first occurrence.
    pub detail: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Summary {
    pub lines: usize,
    pub entries: Vec<Entry>,
}

/// Reads `input` once, keeping each distinct message and up to `detail` continuation lines of its
/// first occurrence. `filter` keeps only matching lines and their continuations; `min` keeps only
/// lines of that level or above (lines without a level are dropped then).
pub fn summarize(
    input: impl BufRead,
    filter: Option<&Regex>,
    min: Option<Level>,
    detail: usize,
) -> Summary {
    let mut summary = Summary::default();
    let mut index: HashMap<String, usize> = HashMap::new();
    // The entry the next continuation line belongs to, and whether it is its first occurrence.
    let mut current: Option<(usize, bool)> = None;
    for (n, line) in input.lines().map_while(Result::ok).enumerate() {
        let number = n + 1;
        summary.lines = number;
        if continuation(&line) {
            if let Some((at, true)) = current
                && summary.entries[at].detail.len() < detail
            {
                summary.entries[at].detail.push(line.trim_end().to_string());
            }
            continue;
        }
        let level = level_of(&line);
        let kept = filter.is_none_or(|f| f.is_match(&line))
            && min.is_none_or(|m| level.is_some_and(|l| l >= m));
        if !kept || line.trim().is_empty() {
            current = None;
            continue;
        }
        let key = template(&line);
        match index.get(&key) {
            Some(&at) => {
                let entry = &mut summary.entries[at];
                entry.count += 1;
                entry.last = number;
                current = Some((at, false));
            }
            None => {
                index.insert(key, summary.entries.len());
                current = Some((summary.entries.len(), true));
                summary.entries.push(Entry {
                    level,
                    first: number,
                    last: number,
                    count: 1,
                    text: line.trim_end().to_string(),
                    detail: Vec::new(),
                });
            }
        }
    }
    summary
}

/// The summary as text: the most severe and most frequent entries first, `limit` entries unless all,
/// lines cut at `width` characters, `detail` continuation lines per entry.
pub fn render(summary: &Summary, limit: Option<usize>, width: usize, detail: usize) -> String {
    let mut entries: Vec<&Entry> = summary.entries.iter().collect();
    entries.sort_by(|a, b| {
        b.level
            .cmp(&a.level)
            .then(b.count.cmp(&a.count))
            .then(a.first.cmp(&b.first))
    });
    let count = |l: Level| {
        summary
            .entries
            .iter()
            .filter(|e| e.level == Some(l))
            .map(|e| e.count)
            .sum::<usize>()
    };
    let mut out = format!(
        "# logs ({} lines, {} distinct; {} error, {} warn)\n",
        summary.lines,
        summary.entries.len(),
        count(Level::Error),
        count(Level::Warn)
    );
    if entries.is_empty() {
        out.push_str("no match\n");
    }
    let shown = limit.unwrap_or(entries.len()).min(entries.len());
    let cut = |text: &str| -> String {
        let mut short: String = text.chars().take(width).collect();
        if short.len() < text.len() {
            short.push_str(" ...");
        }
        short
    };
    for e in &entries[..shown] {
        let lines = if e.first == e.last {
            format!("{}", e.first)
        } else {
            format!("{}-{}", e.first, e.last)
        };
        let level = e.level.map(Level::letter).unwrap_or('-');
        out.push_str(&format!("{lines} {}x {level} {}\n", e.count, cut(&e.text)));
        for line in e.detail.iter().take(detail) {
            out.push_str(&format!("    {}\n", cut(line)));
        }
        if e.detail.len() > detail {
            out.push_str(&format!("    ... {} more lines\n", e.detail.len() - detail));
        }
    }
    if shown < entries.len() {
        out.push_str(&format!("... {} more (--all)\n", entries.len() - shown));
    }
    out
}

#[cfg(test)]
#[path = "../tests/unit/logs.rs"]
mod tests;
