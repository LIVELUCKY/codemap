//! The benchmark harness: runs a question file through `claude -p` under each configuration, several
//! times, and reports correctness, turns, seconds, tokens and cost as medians. Correctness is checked
//! without a judge: every question names the `path:line` its answer must mention.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

/// Times one `claude` call is tried before the benchmark gives up.
const ATTEMPTS: u32 = 3;

/// How far a reported line may be from the expected one and still count.
const LINE_TOLERANCE: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cfg {
    /// Plugin disabled: plain Claude Code.
    A,
    /// codemap with every subagent disabled, so the answer comes from the main context alone.
    B,
    /// codemap as installed.
    C,
}

impl Cfg {
    pub fn parse(text: &str) -> Option<Cfg> {
        match text.trim() {
            "A" | "a" => Some(Cfg::A),
            "B" | "b" => Some(Cfg::B),
            "C" | "c" => Some(Cfg::C),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Cfg::A => "A",
            Cfg::B => "B",
            Cfg::C => "C",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Cfg::A => "without codemap",
            Cfg::B => "codemap, subagents disabled",
            Cfg::C => "codemap",
        }
    }

    fn flags(self, plugin_dir: Option<&Path>) -> Vec<String> {
        let mut flags: Vec<String> = Vec::new();
        if self == Cfg::A {
            let disabled =
                serde_json::json!({"enabledPlugins": {crate::agent::setup::plugin_id(): false}});
            flags.extend(["--settings".into(), disabled.to_string()]);
            return flags;
        }
        if let Some(dir) = plugin_dir {
            flags.extend(["--plugin-dir".into(), dir.to_string_lossy().into_owned()]);
        }
        if self == Cfg::B {
            flags.extend(["--disallowedTools".into(), "Agent".into()]);
        }
        flags
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub cfgs: Vec<Cfg>,
    pub runs: u32,
    /// The `claude` executable; a fake one in tests.
    pub claude: String,
    /// Loads the plugin from this directory for B and C, for a workspace that does not enable it.
    pub plugin_dir: Option<PathBuf>,
    /// The model for every run (`haiku` keeps a benchmark cheap); the CLI default when unset.
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QuestionFile {
    /// The repository the questions are asked in, relative to the question file (or the materialised copy).
    workspace: String,
    /// A folder of plain directories to copy into a temp dir as git repositories first.
    #[serde(default)]
    materialise: Option<String>,
    /// Repository name (a sibling of the workspace) to the commit prefix its answers are valid for.
    #[serde(default)]
    pins: BTreeMap<String, String>,
    questions: Vec<Question>,
}

#[derive(Debug, Deserialize)]
struct Question {
    q: String,
    /// `path` or `path:line`, every one of which the answer must mention.
    expect: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub cfg: Cfg,
    pub question: usize,
    pub correct: bool,
    pub turns: u32,
    pub seconds: f64,
    pub tokens: u64,
    pub cost: f64,
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("git {args:?}"))?;
    if !out.status.success() {
        bail!(
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copies every subdirectory of `source` into `target` and makes each one a git repository with one
/// commit, so the copy behaves like a real workspace of sibling repositories.
pub fn materialise(source: &Path, target: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("reading {}", source.display()))?
        .flatten()
    {
        if !entry.path().is_dir() {
            continue;
        }
        let root = target.join(entry.file_name());
        copy_tree(&entry.path(), &root)?;
        git(&root, &["init", "-q", "-b", "main"])?;
        git(
            &root,
            &[
                "-c",
                "user.email=bench@example.com",
                "-c",
                "user.name=bench",
                "add",
                "-A",
            ],
        )?;
        git(
            &root,
            &[
                "-c",
                "user.email=bench@example.com",
                "-c",
                "user.name=bench",
                "commit",
                "-q",
                "-m",
                "fixture",
            ],
        )?;
    }
    Ok(())
}

/// Whether `answer` mentions every expectation: the path as written and, when a line is given, a number
/// within two lines of it right after the path (`path:12`, `path line 12`).
pub fn correct(answer: &str, expect: &[String]) -> bool {
    let after = Regex::new(r"^\D{0,12}?(\d+)").expect("line regex");
    expect.iter().all(|e| {
        let (path, line) = match e.rsplit_once(':') {
            Some((p, l)) if !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()) => {
                (p, l.parse::<u32>().ok())
            }
            _ => (e.as_str(), None),
        };
        answer.match_indices(path).any(|(at, _)| match line {
            None => true,
            Some(line) => after
                .captures(&answer[at + path.len()..])
                .and_then(|c| c[1].parse::<u32>().ok())
                .is_some_and(|found| found.abs_diff(line) <= LINE_TOLERANCE),
        })
    })
}

/// The arguments of one `claude -p` call.
pub fn claude_args(
    cfg: Cfg,
    question: &str,
    plugin_dir: Option<&Path>,
    model: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = vec!["-p".into(), question.into()];
    args.extend(cfg.flags(plugin_dir));
    if let Some(model) = model {
        args.extend(["--model".into(), model.into()]);
    }
    args.extend([
        "--output-format".into(),
        "json".into(),
        "--dangerously-skip-permissions".into(),
    ]);
    args
}

fn ask(options: &Options, cfg: Cfg, project: &Path, question: &str) -> Result<Value> {
    let args = claude_args(
        cfg,
        question,
        options.plugin_dir.as_deref(),
        options.model.as_deref(),
    );
    // A call can fail for reasons that have nothing to do with the question (the CLI still starting
    // up, a dropped connection); one such failure must not throw away an hour of runs.
    let mut attempt = 0;
    loop {
        attempt += 1;
        let out = Command::new(&options.claude)
            .args(&args)
            .current_dir(project)
            .output()
            .with_context(|| format!("running {}", options.claude))?;
        match serde_json::from_slice(&out.stdout) {
            Ok(value) => return Ok(value),
            Err(_) if attempt < ATTEMPTS => std::thread::sleep(std::time::Duration::from_secs(5)),
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "{} gave no JSON result {ATTEMPTS} times (exit {:?}): {}",
                        options.claude,
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr)
                            .chars()
                            .take(400)
                            .collect::<String>()
                    )
                });
            }
        }
    }
}

pub fn run(file: &Path, options: &Options) -> Result<Vec<Run>> {
    let text = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let questions: QuestionFile =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
    let base = file.parent().unwrap_or(Path::new("."));
    // Kept alive until the runs are done: a materialised workspace lives in it.
    let temp = tempfile::tempdir()?;
    let project = match &questions.materialise {
        Some(source) => {
            materialise(&base.join(source), temp.path())?;
            temp.path().join(&questions.workspace)
        }
        None => base.join(&questions.workspace),
    };
    // `../..` from a question file inside the project: the parent must be the real one.
    let project = project
        .canonicalize()
        .with_context(|| format!("workspace {} does not exist", project.display()))?;
    let parent = project.parent().unwrap_or(&project).to_path_buf();
    for (repo, pin) in &questions.pins {
        let head = git(&parent.join(repo), &["rev-parse", "HEAD"]).unwrap_or_default();
        if head.is_empty() || !head.starts_with(pin.as_str()) {
            bail!(
                "{repo} is not at the pinned commit {pin} (HEAD is {}); the expected answers would not hold",
                if head.is_empty() { "missing" } else { &head }
            );
        }
    }
    let mut runs = Vec::new();
    for &cfg in &options.cfgs {
        for (index, question) in questions.questions.iter().enumerate() {
            for _ in 0..options.runs.max(1) {
                let value = ask(options, cfg, &project, &question.q)?;
                let usage = &value["usage"];
                let tokens = [
                    "input_tokens",
                    "output_tokens",
                    "cache_creation_input_tokens",
                    "cache_read_input_tokens",
                ]
                .iter()
                .map(|k| usage[*k].as_u64().unwrap_or(0))
                .sum();
                runs.push(Run {
                    cfg,
                    question: index,
                    correct: correct(value["result"].as_str().unwrap_or(""), &question.expect),
                    turns: value["num_turns"].as_u64().unwrap_or(0) as u32,
                    seconds: value["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0,
                    tokens,
                    cost: value["total_cost_usd"].as_f64().unwrap_or(0.0),
                });
            }
        }
    }
    Ok(runs)
}

fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    }
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// The medians of one configuration: turns, seconds, tokens, cost.
fn medians(runs: &[&Run]) -> [f64; 4] {
    [
        median(runs.iter().map(|r| r.turns as f64).collect()),
        median(runs.iter().map(|r| r.seconds).collect()),
        median(runs.iter().map(|r| r.tokens as f64).collect()),
        median(runs.iter().map(|r| r.cost).collect()),
    ]
}

/// A markdown table, one row per configuration, each value with its factor against A when A ran.
pub fn table(runs: &[Run], header: &str) -> String {
    let mut cfgs: Vec<Cfg> = runs.iter().map(|r| r.cfg).collect();
    cfgs.sort();
    cfgs.dedup();
    let of = |cfg: Cfg| runs.iter().filter(|r| r.cfg == cfg).collect::<Vec<_>>();
    let baseline = cfgs.contains(&Cfg::A).then(|| medians(&of(Cfg::A)));
    let factor = |value: f64, column: usize, cfg: Cfg| match baseline {
        Some(base) if cfg != Cfg::A && base[column] > 0.0 => {
            format!(" (x{:.2})", value / base[column])
        }
        _ => String::new(),
    };
    let mut out = format!(
        "{header}\n\n| Configuration | Correct | Turns | Seconds | Tokens | Cost |\n|---|---|---|---|---|---|\n"
    );
    for &cfg in &cfgs {
        let these = of(cfg);
        let right = these.iter().filter(|r| r.correct).count();
        let [turns, seconds, tokens, cost] = medians(&these);
        out.push_str(&format!(
            "| {} | {}/{} | {}{} | {:.1}{} | {:.0}{} | ${:.3}{} |\n",
            cfg.name(),
            right,
            these.len(),
            number(turns),
            factor(turns, 0, cfg),
            seconds,
            factor(seconds, 1, cfg),
            tokens,
            factor(tokens, 2, cfg),
            cost,
            factor(cost, 3, cfg),
        ));
    }
    out.push('\n');
    for &cfg in &cfgs {
        out.push_str(&format!("- {}: {}\n", cfg.name(), cfg.describe()));
    }
    out
}

/// Replaces the text between `<!-- bench:<section>:start -->` and `<!-- bench:<section>:end -->`.
pub fn update_readme(readme: &Path, table: &str, section: &str) -> Result<()> {
    let text =
        fs::read_to_string(readme).with_context(|| format!("reading {}", readme.display()))?;
    let start = format!("<!-- bench:{section}:start -->");
    let end = format!("<!-- bench:{section}:end -->");
    let (Some(from), Some(to)) = (text.find(&start), text.find(&end)) else {
        bail!("{} has no {start} ... {end} block", readme.display());
    };
    if to < from {
        bail!(
            "{} has the {section} markers in the wrong order",
            readme.display()
        );
    }
    let body = if table.ends_with('\n') {
        table.to_string()
    } else {
        format!("{table}\n")
    };
    let updated = format!("{}{start}\n{body}{}", &text[..from], &text[to..]);
    fs::write(readme, updated)?;
    Ok(())
}
