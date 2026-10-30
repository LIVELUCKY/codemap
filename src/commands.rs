//! The command dispatch: runs one parsed command and returns its exit code.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::Command;
use crate::index::{self, Stats};
use crate::output::{Answer, render};
use crate::query::{self, deps, find, search};
use crate::workspace::{Workspace, discover};

fn workspace() -> Result<Workspace, i32> {
    let cwd = std::env::current_dir().map_err(|_| 2)?;
    discover(&cwd).map_err(|e| {
        eprintln!("codemap: {e}");
        2
    })
}

fn print(mut answer: Answer, all: bool, ws: &Workspace) -> i32 {
    query::cap(&mut answer, all, ws.config.settings.limits.answer_lines);
    print!("{}", render(&answer));
    0
}

fn stats_line(name: &str, s: &Stats) -> String {
    format!(
        "{name}: parsed {} reused {} skipped {} removed {} ({} ms: git {}, parse {}, write {})",
        s.parsed,
        s.reused,
        s.skipped,
        s.removed,
        s.millis,
        s.git_millis,
        s.parse_millis,
        s.write_millis
    )
}

/// The edited file of a Claude Code tool event (`tool_input.file_path`) on stdin.
fn event_path() -> Option<PathBuf> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).ok()?;
    let event: serde_json::Value = serde_json::from_str(&text).ok()?;
    event
        .get("tool_input")?
        .get("file_path")?
        .as_str()
        .map(PathBuf::from)
}

/// `codemap logs`: works anywhere; inside a repository its settings apply.
fn logs(
    files: &[PathBuf],
    grep: Option<&str>,
    level: Option<crate::logs::Level>,
    all: bool,
) -> i32 {
    let settings = std::env::current_dir()
        .ok()
        .and_then(|cwd| discover(&cwd).ok())
        .map(|ws| ws.config.settings)
        .unwrap_or_default();
    let filter = match grep.map(regex::Regex::new).transpose() {
        Ok(filter) => filter,
        Err(e) => {
            eprintln!("codemap logs: {e}");
            return 2;
        }
    };
    let stdin_only = files.is_empty() || files.iter().all(|f| f.as_os_str() == "-");
    let limit = (!all).then_some(settings.limits.answer_lines);
    let detail = settings.logs.detail_kept;
    let mut out = String::new();
    if stdin_only {
        let summary =
            crate::logs::summarize(std::io::stdin().lock(), filter.as_ref(), level, detail);
        out = crate::logs::render(
            &summary,
            limit,
            settings.logs.width,
            settings.logs.detail_shown,
        );
    }
    for file in files.iter().filter(|f| f.as_os_str() != "-") {
        let input = match std::fs::File::open(file) {
            Ok(f) => std::io::BufReader::new(f),
            Err(e) => {
                eprintln!("codemap logs: {}: {e}", file.display());
                return 1;
            }
        };
        let summary = crate::logs::summarize(input, filter.as_ref(), level, detail);
        if files.len() > 1 {
            out.push_str(&format!("== {}\n", file.display()));
        }
        out.push_str(&crate::logs::render(
            &summary,
            limit,
            settings.logs.width,
            settings.logs.detail_shown,
        ));
    }
    print!("{out}");
    0
}

pub fn run(command: Command) -> i32 {
    match command {
        Command::Show { name, all } => workspace().map_or_else(
            |c| c,
            |ws| {
                print!("{}", query::show::show(&ws, &name, all));
                0
            },
        ),
        Command::Def { name, all } => workspace().map_or_else(
            |c| c,
            |ws| {
                print!("{}", query::show::def_with_code(&ws, &name, all));
                0
            },
        ),
        Command::Logs {
            files,
            grep,
            level,
            all,
        } => logs(&files, grep.as_deref(), level, all),
        Command::Bare(words) => workspace().map_or_else(
            |c| c,
            |ws| {
                print!("{}", query::show::lookup(&ws, &words.join(" ")));
                0
            },
        ),
        Command::Uses { name, lines, all } => {
            workspace().map_or_else(|c| c, |ws| print(query::uses(&ws, &name, lines), all, &ws))
        }
        Command::Impl { name, all } => {
            workspace().map_or_else(|c| c, |ws| print(query::impl_(&ws, &name), all, &ws))
        }
        Command::Variants { name, all } => {
            workspace().map_or_else(|c| c, |ws| print(query::variants(&ws, &name), all, &ws))
        }
        Command::Link { literal, all } => {
            workspace().map_or_else(|c| c, |ws| print(query::link(&ws, &literal), all, &ws))
        }
        Command::Outline { path, all } => {
            workspace().map_or_else(|c| c, |ws| print(query::outline(&ws, &path), all, &ws))
        }
        Command::Deps { module, all } => {
            workspace().map_or_else(|c| c, |ws| print(deps::deps(&ws, &module), all, &ws))
        }
        Command::Find {
            pattern,
            scope,
            all,
        } => workspace().map_or_else(
            |c| c,
            |ws| print(find::find(&ws, &pattern, scope.as_deref(), all), all, &ws),
        ),
        Command::Search { words, all } => workspace().map_or_else(
            |c| c,
            |ws| print(search::search(&ws, &words.join(" "), all), all, &ws),
        ),
        Command::Build { full, budget } => {
            let ws = match workspace() {
                Ok(ws) => ws,
                Err(code) => return code,
            };
            match index::build(&ws, full, budget.map(Duration::from_secs_f64)) {
                Ok(report) => {
                    for (name, stats) in report {
                        println!("{}", stats_line(&name, &stats));
                    }
                    0
                }
                Err(e) => {
                    eprintln!("codemap: {e:#}");
                    1
                }
            }
        }
        Command::Update { files, from_event } => {
            let ws = match workspace() {
                Ok(ws) => ws,
                Err(code) => return code,
            };
            let paths: Vec<PathBuf> = if from_event {
                event_path().into_iter().collect()
            } else {
                files
            };
            let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
            match index::update(&ws, &refs) {
                Ok(Some(stats)) => {
                    println!("{}", stats_line(&ws.project.name, &stats));
                    0
                }
                Ok(None) => {
                    println!("nothing to do");
                    0
                }
                Err(e) => {
                    eprintln!("codemap: {e:#}");
                    1
                }
            }
        }
        Command::Emit { payload } => {
            // The printer a rewritten hook command runs: decodes base64 text and prints it, nothing else.
            use base64::Engine;
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload)
                && let Ok(text) = String::from_utf8(bytes)
            {
                print!("{text}");
            }
            0
        }
        Command::SessionStart => {
            // Builds the index inside a time budget and tells the agent to use codemap first. Silent
            // outside a repository.
            let Ok(cwd) = std::env::current_dir() else {
                return 0;
            };
            let Ok(ws) = discover(&cwd) else {
                return 0;
            };
            let budget = Duration::from_secs_f64(ws.config.settings.limits.start_budget_seconds);
            let _ = index::build(&ws, false, Some(budget));
            println!(
                "{}",
                serde_json::json!({"hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": ws.prompt(&ws.config.settings.prompts.session)
                }})
            );
            0
        }
        Command::Setup {
            agent,
            scope,
            dry_run,
        } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let (Some(home), Some(global_config)) =
                (dirs::home_dir(), crate::config::global_path())
            else {
                eprintln!("codemap setup: no home directory");
                return 2;
            };
            let project = index::git::toplevel(&cwd);
            let settings = match &project {
                Some(root) => crate::config::load(root).settings,
                None => crate::config::load(&cwd).settings,
            };
            let places = crate::agent::setup::Places {
                config: dirs::config_dir().unwrap_or_else(|| home.join(".config")),
                home,
                project,
                exe: std::env::current_exe().unwrap_or_else(|_| env!("CARGO_PKG_NAME").into()),
                global_config,
                agents: settings.agents,
            };
            let options = crate::agent::setup::Options {
                agents: agent,
                scope,
                shell: std::env::var("SHELL").ok(),
                dry_run,
            };
            for line in crate::agent::setup::run(&options, &places) {
                println!("{line}");
            }
            0
        }
        Command::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut crate::cli::completion_command(),
                env!("CARGO_PKG_NAME"),
                &mut std::io::stdout(),
            );
            0
        }
        Command::Mcp => {
            let cwd = std::env::current_dir().unwrap_or_default();
            match crate::agent::mcp::serve(&cwd) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("codemap mcp: {e}");
                    1
                }
            }
        }
        Command::Hook => {
            // Never fails and never panics out: anything not understood passes with an empty stdout.
            let mut stdin = String::new();
            let _ = std::io::stdin().read_to_string(&mut stdin);
            let cwd = std::env::current_dir().unwrap_or_default();
            let answer = std::panic::catch_unwind(|| crate::agent::hook::handle(&stdin, &cwd))
                .ok()
                .flatten();
            if let Some(answer) = answer {
                println!("{answer}");
            }
            0
        }
        Command::SelfUpdate { dry_run } => {
            // The installer checks the SHA-256 of the binary it downloads before replacing this one.
            let repository = crate::agent::setup::repository();
            let (default_owner, name) = repository.split_once('/').unwrap_or((repository, ""));
            let owner = std::env::var("CODEMAP_OWNER").unwrap_or_else(|_| default_owner.into());
            let script = format!(
                "curl -fsSL https://raw.githubusercontent.com/{owner}/{name}/main/install.sh | sh -s -- --no-setup"
            );
            println!("{script}");
            if dry_run {
                return 0;
            }
            match std::process::Command::new("sh")
                .args(["-c", &script])
                .status()
            {
                Ok(status) => status.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("codemap: cannot run the installer: {e}");
                    1
                }
            }
        }
        Command::Bench {
            file,
            configs,
            runs,
            readme,
            section,
            claude,
            plugin_dir,
            model,
        } => {
            let cfgs: Option<Vec<_>> = configs.split(',').map(crate::bench::Cfg::parse).collect();
            let Some(cfgs) = cfgs else {
                eprintln!("codemap bench: --configs takes A, B and C, comma-separated");
                return 2;
            };
            let options = crate::bench::Options {
                cfgs,
                runs,
                claude: claude.clone(),
                plugin_dir,
                model: model.clone(),
            };
            let results = match crate::bench::run(&file, &options) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("codemap bench: {e:#}");
                    return 1;
                }
            };
            let output_of = |program: &str, args: &[&str]| {
                std::process::Command::new(program)
                    .args(args)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default()
            };
            let header = format!(
                "codemap {}, {}, {}, model {}, {} runs per question",
                env!("CARGO_PKG_VERSION"),
                output_of("date", &["+%Y-%m-%d"]),
                output_of(&claude, &["--version"]),
                model.as_deref().unwrap_or("default"),
                runs
            );
            let table = crate::bench::table(&results, &header);
            print!("{table}");
            if let (Some(readme), Some(section)) = (readme, section)
                && let Err(e) = crate::bench::update_readme(&readme, &table, &section)
            {
                eprintln!("codemap bench: {e:#}");
                return 1;
            }
            0
        }
    }
}
