//! The one output shape every command renders: a header naming the source, compact lines, a cap.

#[derive(Debug, Clone, Default)]
pub struct Answer {
    /// `index`, `grep`, `fts` or `bridge:<name>`.
    pub source: String,
    /// `grammar`, `regex`, `mixed`, or `-` when nothing was found.
    pub tier: String,
    pub lines: Vec<Line>,
    /// How many lines the cap removed.
    pub cut: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The path of a sibling repository from the project (`../server`); `None` for the project.
    pub repo: Option<String>,
    pub path: String,
    pub line: u32,
    /// Empty for a grep line, which then reads like grep's own `path:line: text`.
    pub kind: String,
    pub name: String,
    /// `main` is the default and is not printed, so an unconfigured test folder is never mislabelled.
    pub scope: String,
    /// Space-joined flags, possibly empty.
    pub flags: String,
}

impl Answer {
    pub fn new(source: &str) -> Answer {
        Answer {
            source: source.into(),
            tier: "-".into(),
            ..Answer::default()
        }
    }
}

/// The folder a line's file is in, with the repository prefix: what consecutive lines can share.
fn folder(l: &Line) -> (Option<&str>, &str) {
    let dir = l.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    (l.repo.as_deref(), dir)
}

fn hit(l: &Line, path: &str) -> String {
    let flags = if l.flags.is_empty() {
        String::new()
    } else {
        format!(" {}", l.flags)
    };
    let scope = match l.scope.as_str() {
        "" | "main" => String::new(),
        scope => format!(" [{scope}]"),
    };
    if l.kind.is_empty() {
        format!("{path}:{}: {}{scope}{flags}\n", l.line, l.name)
    } else {
        format!("{path}:{} {} {}{scope}{flags}\n", l.line, l.kind, l.name)
    }
}

/// ```text
/// # index (grammar)
/// <note>
/// [../sibling/]path:line kind name [scope] flags
/// [../sibling/]folder/
///   file:line: grep line [scope]
/// ... N more (--all)
/// ```
/// Consecutive lines in the same folder name it once, so a deep path costs its tokens once.
pub fn render(answer: &Answer) -> String {
    let mut out = format!("# {} ({})\n", answer.source, answer.tier);
    for note in &answer.notes {
        out.push_str(note);
        out.push('\n');
    }
    if answer.lines.is_empty() {
        out.push_str("no match\n");
    }
    let lines = &answer.lines;
    let mut i = 0;
    while i < lines.len() {
        let shared = folder(&lines[i]);
        let run = lines[i..]
            .iter()
            .take_while(|l| folder(l) == shared)
            .count();
        let prefix = shared.0.map(|r| format!("{r}/")).unwrap_or_default();
        if run > 1 && !shared.1.is_empty() {
            out.push_str(&format!("{prefix}{}/\n", shared.1));
            for l in &lines[i..i + run] {
                let file = &l.path[shared.1.len() + 1..];
                out.push_str(&format!("  {}", hit(l, file)));
            }
        } else {
            for l in &lines[i..i + run] {
                out.push_str(&hit(l, &format!("{prefix}{}", l.path)));
            }
        }
        i += run;
    }
    if answer.cut > 0 {
        out.push_str(&format!("... {} more (--all)\n", answer.cut));
    }
    out
}

#[cfg(test)]
#[path = "../tests/unit/output.rs"]
mod tests;
