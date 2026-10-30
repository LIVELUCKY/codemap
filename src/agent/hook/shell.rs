//! Shell command lines: the grep a Bash call runs, reproduced exactly or not at all, and unquoted
//! globs the shell would expand.

use crate::config::settings::Shell;
use crate::query::find::{FindOptions, type_globs};

/// How a command reads its patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Syntax {
    /// POSIX basic regular expressions: `grep`, `git grep`.
    Basic,
    /// POSIX extended regular expressions: `grep -E`, `egrep`, `git grep -E`.
    Extended,
    /// Fixed strings: `-F`.
    Fixed,
    /// The Rust regex crate: `rg`.
    Rust,
}

/// A grep-like command line the hook can reproduce: its patterns, paths and what changes the matches.
pub(super) struct Grep {
    pub(super) patterns: Vec<String>,
    pub(super) paths: Vec<String>,
    pub(super) syntax: Syntax,
    /// Without it `grep` reads stdin or the named files, never a folder.
    pub(super) recursive: bool,
    pub(super) word: bool,
    pub(super) line: bool,
    /// Everything but the pattern and the scope.
    pub(super) options: FindOptions,
}

impl Grep {
    /// The patterns as one Rust regex with the same matches, `None` when that cannot be said exactly.
    pub(super) fn regex(&self) -> Option<String> {
        let mut parts = Vec::new();
        for pattern in &self.patterns {
            // grep and git grep take each line of a pattern as a pattern of its own; rg refuses one.
            let lines: Vec<&str> = if self.syntax == Syntax::Rust {
                if pattern.contains('\n') {
                    return None;
                }
                vec![pattern]
            } else {
                pattern.split('\n').collect()
            };
            for line in lines {
                parts.push(match self.syntax {
                    Syntax::Basic => posix_to_rust(line, false)?,
                    Syntax::Extended => posix_to_rust(line, true)?,
                    Syntax::Fixed => regex::escape(line),
                    Syntax::Rust => line.to_string(),
                });
            }
        }
        let joined = match parts.as_slice() {
            [] => return None,
            [one] => one.clone(),
            _ => parts
                .iter()
                .map(|p| format!("(?:{p})"))
                .collect::<Vec<_>>()
                .join("|"),
        };
        Some(if self.line {
            format!("^(?:{joined})$")
        } else if self.word {
            // A match with no word character right before or after it, as grep -w and rg -w test.
            format!(r"\b{{start-half}}(?:{joined})\b{{end-half}}")
        } else {
            joined
        })
    }
}

fn add_glob(options: &mut FindOptions, glob: &str) {
    match glob.strip_prefix('!') {
        Some(negated) => options.exclude.push(negated.to_string()),
        None => options.include.push(glob.to_string()),
    }
}

fn add_type(into: &mut Vec<String>, name: &str) -> Option<()> {
    into.extend(type_globs(name)?);
    Some(())
}

/// Whether a short option takes a value (`-e PATTERN`), per program.
fn takes_value(program: &str, flag: char) -> bool {
    match program {
        "rg" => matches!(flag, 'e' | 'g' | 't' | 'T'),
        _ => flag == 'e',
    }
}

/// Whether a long option takes a value as the next word when it has no `=`, per program.
fn long_takes_value(program: &str, flag: &str) -> bool {
    match program {
        "rg" => matches!(
            flag,
            "regexp" | "glob" | "type" | "type-not" | "color" | "colour"
        ),
        "git grep" => flag == "regexp",
        _ => matches!(flag, "regexp" | "include" | "exclude" | "exclude-dir"),
    }
}

/// One option applied to the command; `None` for an option that changes the output in a way the
/// hook does not reproduce, or one it does not know.
fn apply(grep: &mut Grep, program: &str, flag: &str, value: Option<String>) -> Option<()> {
    let rg = program == "rg";
    let git = program == "git grep";
    let posix = !rg;
    let o = &mut grep.options;
    match flag {
        // Output format only.
        "n" | "H" | "line-number" | "with-filename" | "no-filename" | "color" | "colour" => {}
        "h" if !rg => {}
        "I" | "no-heading" | "heading" | "N" | "no-line-number" if rg => {}
        "I" if posix => {}
        "s" | "no-messages" if !rg && !git => {}
        "full-name" | "no-color" if git => {}
        "binary-files" if value.as_deref() == Some("without-match") && !rg && !git => {}
        "r" | "R" | "recursive" | "dereference-recursive" if !rg && !git => grep.recursive = true,
        "recursive" if git => {}
        "i" | "ignore-case" => {
            o.ignore_case = true;
            o.smart_case = false;
        }
        "y" if !rg && !git => o.ignore_case = true,
        "S" | "smart-case" if rg => {
            o.smart_case = true;
            o.ignore_case = false;
        }
        "s" | "case-sensitive" if rg => {
            o.smart_case = false;
            o.ignore_case = false;
        }
        "w" | "word-regexp" => grep.word = true,
        "x" | "line-regexp" => grep.line = true,
        "F" | "fixed-strings" => grep.syntax = Syntax::Fixed,
        "E" | "extended-regexp" if posix => grep.syntax = Syntax::Extended,
        "G" | "basic-regexp" if posix => grep.syntax = Syntax::Basic,
        "l" | "files-with-matches" => o.files_only = true,
        "name-only" if git => o.files_only = true,
        "." | "hidden" if rg => o.hidden = true,
        "e" | "regexp" => grep.patterns.push(value?),
        "g" | "glob" if rg => add_glob(o, &value?),
        "t" | "type" if rg => add_type(&mut o.include, &value?)?,
        "T" | "type-not" if rg => add_type(&mut o.exclude, &value?)?,
        "include" if !rg && !git => o.include.push(value?),
        "exclude" if !rg && !git => o.exclude.push(value?),
        "exclude-dir" if !rg && !git => o.exclude.push(format!("**/{}/**", value?)),
        _ => return None,
    }
    Some(())
}

/// The command `program args` as a reproducible grep, `None` when it is not one.
pub(super) fn parse_grep(program: &str, args: &[String]) -> Option<Grep> {
    let mut grep = Grep {
        patterns: Vec::new(),
        paths: Vec::new(),
        syntax: match program {
            "rg" => Syntax::Rust,
            "egrep" => Syntax::Extended,
            _ => Syntax::Basic,
        },
        recursive: program != "grep" && program != "egrep",
        word: false,
        line: false,
        options: FindOptions {
            // grep -r and git grep read hidden files; rg skips them unless told.
            hidden: program != "rg",
            tracked: program == "git grep",
            ..FindOptions::default()
        },
    };
    let mut positional: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if only_positional || !arg.starts_with('-') || arg == "-" {
            positional.push(arg.clone());
        } else if arg == "--" {
            only_positional = true;
        } else if let Some(long) = arg.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None if long_takes_value(program, long) => {
                    i += 1;
                    (long, Some(args.get(i)?.clone()))
                }
                None => (long, None),
            };
            apply(&mut grep, program, name, value)?;
        } else {
            let cluster: Vec<char> = arg[1..].chars().collect();
            for (n, &c) in cluster.iter().enumerate() {
                if takes_value(program, c) {
                    let rest: String = cluster[n + 1..].iter().collect();
                    let value = if rest.is_empty() {
                        i += 1;
                        args.get(i)?.clone()
                    } else {
                        rest
                    };
                    apply(&mut grep, program, &c.to_string(), Some(value))?;
                    break;
                }
                apply(&mut grep, program, &c.to_string(), None)?;
            }
        }
        i += 1;
    }
    let mut positional = positional.into_iter();
    if grep.patterns.is_empty() {
        grep.patterns.push(positional.next()?);
    }
    grep.paths = positional.collect();
    Some(grep)
}

/// `{m}`, `{m,}`, `{m,n}` or `{,n}` from just after the opening brace up to `close`, as Rust syntax,
/// and how many characters it took including `close`.
fn interval(chars: &[char], close: &str) -> Option<(String, usize)> {
    let text: String = chars.iter().collect();
    let end = text.find(close)?;
    let body = &text[..end];
    let (low, high) = match body.split_once(',') {
        Some((low, high)) => (low, Some(high)),
        None => (body, None),
    };
    let number = |s: &str| -> Option<u32> {
        (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let low = if low.is_empty() && high.is_some() {
        0
    } else {
        number(low)?
    };
    let rust = match high {
        None => format!("{{{low}}}"),
        Some("") => format!("{{{low},}}"),
        Some(high) => {
            let high = number(high)?;
            if high < low {
                return None;
            }
            format!("{{{low},{high}}}")
        }
    };
    Some((rust, body.chars().count() + close.chars().count()))
}

/// A character of a bracket expression, escaped where a Rust class would read it as syntax.
fn class_char(c: char) -> String {
    if matches!(c, '\\' | '[' | ']' | '^' | '-' | '&' | '~') {
        format!("\\{c}")
    } else {
        c.to_string()
    }
}

/// A repetition `text` after the last token, which must be one that can repeat.
fn repeated(out: &mut String, text: &str, atom: &mut bool) -> Option<()> {
    if !*atom {
        return None;
    }
    out.push_str(text);
    *atom = false;
    Some(())
}

const POSIX_CLASSES: [&str; 12] = [
    "alpha", "digit", "alnum", "upper", "lower", "space", "blank", "punct", "print", "graph",
    "cntrl", "xdigit",
];

/// A POSIX bracket expression starting at `chars[0] == '['`, as a Rust class, and its length. A
/// backslash is an ordinary character inside it, and `]` right after the opening is a member.
fn bracket(chars: &[char]) -> Option<(String, usize)> {
    enum Item {
        Char(char),
        Class(String),
    }
    let mut out = String::from("[");
    let mut i = 1;
    if chars.get(i) == Some(&'^') {
        out.push('^');
        i += 1;
    }
    let mut items = Vec::new();
    let mut first = true;
    loop {
        let c = *chars.get(i)?;
        if c == ']' && !first {
            i += 1;
            break;
        }
        first = false;
        if c == '[' {
            match chars.get(i + 1) {
                Some(':') => {
                    let rest: String = chars[i + 2..].iter().collect();
                    let name = &rest[..rest.find(":]")?];
                    if !POSIX_CLASSES.contains(&name) {
                        return None;
                    }
                    items.push(Item::Class(format!("[:{name}:]")));
                    i += name.chars().count() + 4;
                    continue;
                }
                // Equivalence classes and collating symbols depend on the locale.
                Some('=' | '.') => return None,
                _ => {}
            }
        }
        items.push(Item::Char(c));
        i += 1;
    }
    let mut k = 0;
    while k < items.len() {
        match (&items[k], items.get(k + 1), items.get(k + 2)) {
            (Item::Char(a), Some(Item::Char('-')), Some(Item::Char(b))) => {
                out.push_str(&format!("{}-{}", class_char(*a), class_char(*b)));
                k += 3;
            }
            (Item::Char(c), _, _) => {
                out.push_str(&class_char(*c));
                k += 1;
            }
            (Item::Class(class), _, _) => {
                out.push_str(class);
                k += 1;
            }
        }
    }
    out.push(']');
    Some((out, i))
}

/// A POSIX basic (`extended` false) or extended regular expression in Rust regex syntax with the
/// same matches, or `None` where the two would differ or POSIX leaves the meaning open (back
/// references, a repetition with nothing to repeat, unknown escapes). In a basic expression
/// `\?` `\+` `\{` `\}` `\|` `\(` `\)` are the operators and the bare characters are literals; `*` at
/// the start, after `\(`, `\|` or a leading `^` is a literal, and `^` and `$` anchor only at the ends
/// of the expression or of a group or alternative.
pub(super) fn posix_to_rust(pattern: &str, extended: bool) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut depth = 0usize;
    // Where a basic expression lets `^` anchor: the start, after `\(` or `\|`.
    let mut anchor_here = true;
    // Where a basic expression reads `*` as a literal: there, or right after that `^`.
    let mut literal_star = true;
    // Whether the last token can be repeated.
    let mut atom = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let (anchor_was, star_was) = (anchor_here, literal_star);
        anchor_here = false;
        literal_star = false;
        match c {
            '\\' => {
                i += 1;
                let next = *chars.get(i)?;
                match next {
                    '(' if !extended => {
                        out.push('(');
                        depth += 1;
                        (anchor_here, literal_star, atom) = (true, true, false);
                    }
                    ')' if !extended => {
                        depth = depth.checked_sub(1)?;
                        out.push(')');
                        atom = true;
                    }
                    '|' if !extended => {
                        out.push('|');
                        (anchor_here, literal_star, atom) = (true, true, false);
                    }
                    '{' if !extended => {
                        let (text, len) = interval(&chars[i + 1..], "\\}")?;
                        repeated(&mut out, &text, &mut atom)?;
                        i += len;
                    }
                    '?' | '+' if !extended => repeated(&mut out, &next.to_string(), &mut atom)?,
                    '}' if !extended => return None,
                    'w' | 'W' | 's' | 'S' => {
                        out.push('\\');
                        out.push(next);
                        atom = true;
                    }
                    'b' | 'B' => {
                        out.push('\\');
                        out.push(next);
                        atom = false;
                    }
                    '<' => {
                        out.push_str(r"\b{start}");
                        atom = false;
                    }
                    '>' => {
                        out.push_str(r"\b{end}");
                        atom = false;
                    }
                    // GNU buffer anchors, which a line search cannot reproduce.
                    '\'' | '`' => return None,
                    n if n.is_ascii_punctuation() => {
                        out.push_str(&regex::escape(&n.to_string()));
                        atom = true;
                    }
                    _ => return None,
                }
            }
            '[' => {
                let (text, len) = bracket(&chars[i..])?;
                out.push_str(&text);
                i += len - 1;
                atom = true;
            }
            '.' => {
                out.push('.');
                atom = true;
            }
            '*' if !extended && star_was => {
                out.push_str(r"\*");
                atom = true;
            }
            '*' => repeated(&mut out, "*", &mut atom)?,
            '^' if extended || anchor_was => {
                out.push('^');
                literal_star = !extended;
                anchor_here = false;
                atom = false;
            }
            '$' if extended
                || i + 1 == chars.len()
                || matches!(chars.get(i + 1..i + 3), Some(['\\', ')' | '|'])) =>
            {
                out.push('$');
                atom = false;
            }
            '+' | '?' if extended => repeated(&mut out, &c.to_string(), &mut atom)?,
            '{' if extended => {
                let (text, len) = interval(&chars[i + 1..], "}")?;
                repeated(&mut out, &text, &mut atom)?;
                i += len;
            }
            '(' if extended => {
                out.push('(');
                depth += 1;
                atom = false;
            }
            ')' if extended => {
                depth = depth.checked_sub(1)?;
                out.push(')');
                atom = true;
            }
            '|' if extended => {
                out.push('|');
                atom = false;
            }
            other => {
                out.push_str(&regex::escape(&other.to_string()));
                atom = true;
            }
        }
        i += 1;
    }
    (depth == 0).then_some(out)
}

/// A shell redirection (`2>/dev/null`, `>out`, `&>log`): `Some(true)` when its target is the next word.
fn redirection(token: &str) -> Option<bool> {
    let rest = token.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&');
    let operator = rest.chars().next()?;
    if operator != '>' && operator != '<' {
        return None;
    }
    let target = rest.trim_start_matches(['>', '<', '&', '|']);
    Some(target.is_empty())
}

/// The first non-piped `grep`, `egrep`, `rg` or `git grep` of a shell command line, with its
/// arguments.
pub(super) fn bash_search(command: &str, sh: &Shell) -> Option<(String, Vec<String>)> {
    let tokens = shlex::split(command)?;
    let mut piped = false;
    let mut segment: Vec<String> = Vec::new();
    let mut found = None;
    let flush =
        |segment: &mut Vec<String>, piped: bool, found: &mut Option<(String, Vec<String>)>| {
            if found.is_none() && !piped && !segment.is_empty() {
                if segment[0] == "git" && segment.get(1).map(String::as_str) == Some("grep") {
                    *found = Some(("git grep".to_string(), segment[2..].to_vec()));
                } else if matches!(segment[0].as_str(), "grep" | "egrep" | "rg") {
                    *found = Some((segment[0].clone(), segment[1..].to_vec()));
                }
            }
            segment.clear();
        };
    let mut skip_next = false;
    for token in tokens {
        if skip_next {
            skip_next = false;
            continue;
        }
        if sh.separators.contains(&token) {
            flush(&mut segment, piped, &mut found);
            piped = token == "|";
        } else if let Some(takes_next) = redirection(&token) {
            skip_next = takes_next;
        } else {
            segment.push(token);
        }
    }
    flush(&mut segment, piped, &mut found);
    found
}

/// Words of a command line that carry an unquoted glob where the program, not the shell, is meant to
/// read it: `--include=*.kt`, `-g *.kt`, `find -name *.kt` (`settings.shell.globOptions`). A shell expands such a glob before the
/// program starts; zsh aborts the whole command with `no matches found` when nothing matches, bash
/// silently passes something else. A plain `ls *.kt` is an intended expansion and is not reported.
pub(super) fn unquoted_glob_words(command: &str, sh: &Shell) -> Vec<String> {
    let mut words: Vec<(String, bool)> = Vec::new();
    let (mut word, mut glob) = (String::new(), false);
    let (mut single, mut double, mut escaped) = (false, false, false);
    fn flush(words: &mut Vec<(String, bool)>, word: &mut String, glob: &mut bool) {
        if !word.is_empty() {
            words.push((std::mem::take(word), *glob));
        }
        *glob = false;
    }
    for c in command.chars() {
        if escaped {
            word.push(c);
            escaped = false;
            continue;
        }
        let quoted = single || double;
        match c {
            '\\' if !single => {
                word.push(c);
                escaped = true;
            }
            '\'' if !double => {
                single = !single;
                word.push(c);
            }
            '"' if !single => {
                double = !double;
                word.push(c);
            }
            c if !quoted && (c.is_whitespace() || matches!(c, '|' | ';' | '&')) => {
                flush(&mut words, &mut word, &mut glob);
                // an operator ends the command: the next word has no previous option
                if !c.is_whitespace() {
                    words.push((String::new(), false));
                }
            }
            '*' | '?' | '[' if !quoted => {
                glob = true;
                word.push(c);
            }
            _ => word.push(c),
        }
    }
    flush(&mut words, &mut word, &mut glob);
    let mut found = Vec::new();
    let mut previous = String::new();
    for (text, has_glob) in words {
        if has_glob && (text.starts_with('-') || sh.glob_options.contains(&previous)) {
            found.push(text.clone());
        }
        previous = text;
    }
    found
}

pub(super) fn glob_rejection(words: &[String]) -> String {
    let fixed: Vec<String> = words
        .iter()
        .map(|w| match w.split_once('=') {
            Some((flag, value)) if w.starts_with('-') => format!("{flag}='{value}'"),
            _ => format!("'{w}'"),
        })
        .collect();
    format!(
        "Not run: {} has an unquoted glob. The shell expands it before the program sees it, and zsh aborts \
         the whole command with `no matches found`. Quote it ({}) and run it again.",
        words.join(", "),
        fixed.join(", ")
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/agent/hook/shell.rs"]
mod tests;
