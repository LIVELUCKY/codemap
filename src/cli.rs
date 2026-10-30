use std::path::PathBuf;

use clap::{Parser, Subcommand};

const ABOUT: &str = "Find code in one call: definitions, uses, implementations, routes and text \
across every repository and language of a workspace, from an index that follows the working tree.";

const LONG_ABOUT: &str = "Find code in one call: definitions, uses, implementations, routes and \
text across every repository and language of a workspace, from an index that follows the working tree.

Every answer starts with a header naming its source, then one line per hit:

  path:line kind name [scope] flags     (a definition or use)
  path:line: text                       (a grep line)

Paths are relative to this repository, so a hit in a sibling repository starts with its path
(../server/src/...). The scope is printed only when it is not main code: test, generated or
vendored. Answers stop after 25 lines and say how many more there are; --all prints everything.
Gitignored files are never searched, and the index updates itself before every answer.";

const EXAMPLES: &str = "Which command:
  where is X, and its code           codemap X          (a name, part of one, a file name or a few words)
  who calls or references X          codemap uses X
  what extends or implements X       codemap impl X
  the code of every definition of X  codemap show X
  which repositories serve a route   codemap link /api/v1/books/{id}
  a regex over every text file       codemap find 'retry|backoff' src
  words, when you do not know names  codemap search token refresh
  what a folder holds                codemap outline src/main
  the gist of a log or of output     docker logs api | codemap logs   (or codemap logs app.log)

grep and rg in Bash are answered by codemap with the matches the real tool prints, gitignored files
left out, ranked and capped.";

/// The CLI as Tab completion offers it: the hidden commands (hooks, the MCP server, internals) left
/// out, which clap_complete would list otherwise.
pub fn completion_command() -> clap::Command {
    use clap::CommandFactory;
    let full = Cli::command();
    clap::Command::new(env!("CARGO_PKG_NAME"))
        .about(ABOUT)
        .subcommands(full.get_subcommands().filter(|s| !s.is_hide_set()).cloned())
}

/// Find code in one call, across every repository and language of a workspace.
#[derive(Parser, Debug)]
#[command(name = "codemap", version, about = ABOUT, long_about = LONG_ABOUT, after_help = EXAMPLES)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// `codemap X`: a name, part of one, a file name, a route or a few words (`query::show::lookup`).
    #[command(external_subcommand)]
    Bare(Vec<String>),
    /// Where a name is defined: this repository first, then its siblings. A single definition comes
    /// with its code.
    #[command(
        visible_aliases = ["definition"],
        aliases = ["definitions", "define", "declaration", "decl", "goto", "find-definition"],
        after_help = "Example: codemap def BookRoute\nSame as: codemap BookRoute"
    )]
    Def {
        /// The name; a misspelt or partial one answers with the closest names.
        name: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// The source of each definition of a name with line numbers: the block to read or edit.
    #[command(
        aliases = ["cat"],
        after_help = "Example: codemap show BookRepositoryImpl"
    )]
    Show {
        name: String,
        /// Print long definitions in full instead of the first 80 lines.
        #[arg(long)]
        all: bool,
    },
    /// Where a name is used, in every language (Swift calls into Kotlin and imports included).
    #[command(
        visible_aliases = ["usages", "refs"],
        aliases = ["references", "callers", "find-usages", "who-uses"],
        after_help = "Example: codemap uses BookPort --lines"
    )]
    Uses {
        name: String,
        /// One line per use instead of one per file.
        #[arg(long)]
        lines: bool,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// What extends or implements a type; tests last.
    #[command(
        visible_aliases = ["implementations"],
        aliases = ["implements", "implementors", "implementers", "subtypes", "impls"],
        after_help = "Example: codemap impl BookPort"
    )]
    Impl {
        name: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// The platform variants of one definition, for example a Kotlin expect and its actuals.
    #[command(
        aliases = ["actuals", "expect"],
        after_help = "Example: codemap variants platformName"
    )]
    Variants {
        name: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// Every repository that carries a route or URL, with the definition it sits in.
    #[command(
        visible_aliases = ["route"],
        aliases = ["routes"],
        after_help = "Example: codemap link '/api/v1/books/{id}'"
    )]
    Link {
        literal: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// A regular expression over every text file, ranked and capped. A folder limits it to this
    /// repository.
    #[command(
        visible_aliases = ["grep"],
        aliases = ["rg", "regex", "search-text"],
        after_help = "Example: codemap find 'okhttp|retrofit' src"
    )]
    Find {
        /// A Rust regular expression; quote it.
        pattern: String,
        /// A folder of this repository.
        scope: Option<PathBuf>,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// Files whose words match best, for when the names are not known yet.
    #[command(
        aliases = ["fts"],
        after_help = "Example: codemap search token refresh"
    )]
    Search {
        #[arg(required = true)]
        words: Vec<String>,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// The definitions under a folder, grouped by kind (and by role when configured).
    #[command(
        visible_aliases = ["ls"],
        aliases = ["tree", "symbols"],
        after_help = "Example: codemap outline src/main/kotlin/app"
    )]
    Outline {
        path: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// Module dependencies in both directions (Gradle, Cargo, npm).
    #[command(
        aliases = ["dependencies", "modules", "dependents"],
        after_help = "Example: codemap deps :base:ui"
    )]
    Deps {
        module: String,
        /// Print every line instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    /// The gist of a log or of command output: repeats counted, errors first, stack traces kept.
    /// Reads the files, or stdin without any (`docker logs api | codemap logs`).
    #[command(
        visible_aliases = ["log"],
        aliases = ["logcat", "summarize"],
        after_help = "Examples:\n  docker logs api 2>&1 | codemap logs --level warn\n  adb logcat -d | codemap logs -e 'ActivityManager|AndroidRuntime'\n  cargo test 2>&1 | codemap logs -e FAILED\n  codemap logs build.log server.log"
    )]
    Logs {
        /// Log files; stdin when none (or `-`).
        files: Vec<PathBuf>,
        /// Keep only lines matching this regular expression (and their stack traces).
        #[arg(short = 'e', long, alias = "regexp", alias = "pattern")]
        grep: Option<String>,
        /// Keep only lines of this level or above.
        #[arg(short, long, value_enum)]
        level: Option<crate::logs::Level>,
        /// Print every entry instead of the first 25.
        #[arg(short, long)]
        all: bool,
    },
    // Hidden, so an agent never spends a call on it.
    /// Bring the index up to date now; queries also do it by themselves.
    #[command(hide = true)]
    Build {
        /// Forget every cached parse of this repository first.
        #[arg(long)]
        full: bool,
        /// Seconds after which no further sibling repository is started.
        #[arg(long)]
        budget: Option<f64>,
    },
    /// Connect codemap to coding agents: the plugin for Claude Code, the MCP server for the others.
    #[command(
        after_help = "Examples:\n  codemap setup\n  codemap setup --scope project --agent cursor,codex\n  codemap setup --dry-run"
    )]
    Setup {
        /// Comma-separated: claude, codex, copilot, vscode, cursor, windsurf, zed, gemini, antigravity,
        /// junie, or all. Default: the agents found on this machine.
        #[arg(long, value_delimiter = ',')]
        agent: Vec<String>,
        #[arg(long, value_enum, default_value = "global")]
        scope: crate::agent::setup::Scope,
        /// Report what would change and write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the Tab-completion script for a shell.
    #[command(after_help = "Example: eval \"$(codemap completions zsh)\"")]
    Completions { shell: clap_complete::Shell },
    /// Update codemap by rerunning the installer (the binary's checksum is verified).
    SelfUpdate {
        /// Print what would run and exit.
        #[arg(long)]
        dry_run: bool,
    },
    /// Reconcile after editing the given files (the edit hook).
    #[command(hide = true)]
    Update {
        files: Vec<PathBuf>,
        /// Read the edited file from a Claude Code tool event on stdin.
        #[arg(long)]
        from_event: bool,
    },
    /// The PreToolUse hook: reads a tool event on stdin, answers or passes.
    #[command(hide = true)]
    Hook,
    /// The SessionStart hook: builds the index and prints the session prompt.
    #[command(hide = true)]
    SessionStart,
    /// The MCP server over stdio, started by the agents `setup` connects.
    #[command(hide = true)]
    Mcp,
    /// Prints base64 text and nothing else; what a rewritten hook command runs.
    #[command(hide = true)]
    Emit { payload: String },
    /// Benchmark harness: runs a question file through the Claude CLI and reports medians.
    #[command(hide = true)]
    Bench {
        file: PathBuf,
        /// Configurations to run, comma-separated: A (without codemap), B (codemap, subagents
        /// disabled), C (codemap).
        #[arg(long, default_value = "A,C")]
        configs: String,
        /// Runs per question and configuration; the median is reported.
        #[arg(long, default_value_t = 3)]
        runs: u32,
        /// Write the table into this README between the bench markers of --section.
        #[arg(long, requires = "section")]
        readme: Option<PathBuf>,
        #[arg(long)]
        section: Option<String>,
        /// The claude executable.
        #[arg(long, default_value = "claude")]
        claude: String,
        /// Load the plugin from this directory for B and C (for a workspace that does not enable it).
        #[arg(long)]
        plugin_dir: Option<PathBuf>,
        /// The model for every run, for example `haiku` to keep a benchmark cheap.
        #[arg(long)]
        model: Option<String>,
    },
}
