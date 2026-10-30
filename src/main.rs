use clap::Parser;
use codemap::cli::{Cli, Command};

// Parsing thousands of files on every core allocates heavily; mimalloc keeps those allocations
// thread-local and fast.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let cli = Cli::try_parse().unwrap_or_else(|e| {
        // A query that starts with a command word (`codemap find the login screen`) is still a
        // query. Help, version and anything with a flag keep clap's message.
        let words: Vec<String> = std::env::args().skip(1).collect();
        if !e.use_stderr() || words.is_empty() || words.iter().any(|w| w.starts_with('-')) {
            e.exit()
        }
        Cli {
            command: Command::Bare(words),
        }
    });
    std::process::exit(codemap::commands::run(cli.command));
}
