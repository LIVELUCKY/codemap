<p align="center"><img src="public/logo.svg" width="112" alt="codemap logo"></p>

# codemap

Code search for AI coding agents. codemap is a single Rust binary that indexes the definitions,
references and routes of a workspace, across repositories and languages, so an agent can find code
with one call instead of a chain of greps and file reads.

```sh
codemap BookRoute              # where it is defined, with its code
codemap token refresh          # a few words when you do not know the name
codemap uses BookPort          # who uses it, in every language
codemap impl BookPort          # what extends or implements it
codemap grep 'retry|backoff'   # a regex over every repository of the workspace
```

The index follows the git tree, so there is nothing to build or refresh by hand. Queries make no
network call and need no model server.

## Why

An agent that looks for something in a large code base usually greps, reads a file, greps again and
reads some more. Every step is a round trip to the model, and every result stays in its context,
most of it lines the agent did not need. With several repositories it also has to know where to
look, and grep alone cannot tell a definition from a mention or follow a name from Kotlin into
Swift.

codemap answers the question the agent actually has: where is this defined, who uses it, what
implements it. The answer is one short list with the code of the definition when there is only one,
ranked so that main code comes before tests and generated files. It also works when the agent only
has half a name, a typo or a description.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/LIVELUCKY/codemap/main/install.sh | sh
```

The script downloads the binary for your platform (macOS and Linux, arm64 and x86_64), checks its
SHA-256, puts it in `~/.local/bin` and connects the agents it finds on your machine. Two options:

```sh
... | sh -s -- --scope project   # connect the agents for this repository only
... | sh -s -- --no-setup        # install the binary and nothing else
```

If there is no release for your platform it builds from source, which needs a Rust toolchain.

From a clone:

```sh
git clone https://github.com/LIVELUCKY/codemap && cd codemap
sh install.sh            # builds this checkout, installs it and connects the agents
cargo install --path .   # or only the binary
```

You can also download `codemap-<target>.tar.gz` and `SHA256SUMS` from the
[latest release](https://github.com/LIVELUCKY/codemap/releases/latest) and put `codemap` on your
`PATH` yourself.

To update, run `codemap self-update`, and `claude plugin update codemap@codemap` for the Claude Code
plugin. The only thing codemap needs at run time is `git`.

## Connect it to your agent

```sh
codemap setup                        # every agent found on this machine, for all projects
codemap setup --scope project        # this repository only; the files can be committed
codemap setup --agent cursor,codex   # chosen agents, or --agent all
codemap setup --dry-run              # show what would change
```

| Agent | How | Global file | Project file |
|---|---|---|---|
| Claude Code | plugin (session prompt and hooks) | `claude plugin install -s user` | `-s project` |
| GitHub Copilot CLI | MCP server | `~/.copilot/mcp-config.json` | `.copilot/mcp-config.json` |
| VS Code (Copilot Chat) | MCP server | user `mcp.json` | `.vscode/mcp.json` |
| Codex | MCP server | `~/.codex/config.toml` | `.codex/config.toml` |
| Cursor | MCP server | `~/.cursor/mcp.json` | `.cursor/mcp.json` |
| Gemini CLI | MCP server | `~/.gemini/settings.json` | `.gemini/settings.json` |
| Antigravity | MCP server | `~/.gemini/config/mcp_config.json` | `.agents/mcp_config.json` |
| JetBrains Junie | MCP server | `~/.junie/mcp/mcp.json` | `.junie/mcp/mcp.json` |
| Windsurf | MCP server | `~/.codeium/windsurf/mcp_config.json` | none |
| Zed | MCP server | `~/.config/zed/settings.json` | none |

Setup only adds a `codemap` entry. It keeps the order of your other settings, leaves an existing
`codemap` entry alone, saves the old file as `<file>.codemap.bak`, and does not rewrite a file that
has comments (it prints the entry for you to add). Global entries point at the installed binary;
project entries say `codemap`, so they work for everyone on the team. Any other MCP client can start
`codemap mcp` (stdio) itself. The list of agents is a setting (`agents` in
[defaults/settings.toml](defaults/settings.toml)), so adding one does not need a release.

Setup also installs Tab completion for fish and bash. For zsh, add this line to `~/.zshrc`:

```sh
eval "$(codemap completions zsh)"
```

## Commands

| Question | Command |
|---|---|
| Where is `X` defined, and its code | `codemap X` |
| The same with a partial or misspelt name | `codemap BookRout`, `codemap BookRuote` |
| The same when you only know the topic | `codemap token refresh` |
| A file or an asset by name, with or without the extension | `codemap ic_logo`, `codemap build.gradle.kts` |
| The code of every definition of `X` | `codemap show X` |
| Who uses `X` | `codemap uses X` (`--lines` for every use) |
| What implements `X` | `codemap impl X` |
| The platform variants of `X` (Kotlin `expect`/`actual`) | `codemap variants X` |
| Which repositories carry a route or URL | `codemap link '/api/v1/books/{id}'` |
| A regular expression over every text file | `codemap grep 'retry\|backoff' src` |
| What a folder holds | `codemap outline src/main` |
| Module dependencies (Gradle, Cargo, npm) | `codemap deps :base:ui` |
| The gist of a log or of command output | `docker logs api 2>&1 \| codemap logs`, `codemap logs build.log` |

`codemap --help` lists every command with an example. Commands also answer to the names an agent
tends to try, such as `codemap definition X` or `codemap usages X`.

A name with exactly one definition comes back with its code. A name without a definition comes back
with the definitions of the closest names, and if nothing is close, with the files that hold its
words. `codemap logs` folds repeated lines into one entry with a count, puts errors first and keeps
stack traces with their line; `-e` and `--level` filter.

An answer starts with a header that names its source, then one line per hit:

```
# index (grammar)
features/cart/src/main/kotlin/com/example/cart/
  CartScreen.kt:82 type CartViewModel 1x
  CartNavigation.kt:40 type CartViewModel 1x
features/cart/src/test/kotlin/com/example/cart/CartViewModelTest.kt:12 call CartViewModel [test] 3x
../server/src/main/kotlin/server/Routes.kt:4 class CartRoute @Resource
```

Paths are relative to the repository you are in. A hit in a sibling repository starts with that
repository's path (`../server/...`), so every path can be opened as printed. Hits in the same folder
share one folder line. The scope is printed only for code that is not main code: `[test]`,
`[generated]` or `[vendored]`. Answers stop after 25 lines and say how many more there are; `--all`
prints the rest.

## What the agent gets

In Claude Code the plugin adds three things:

- A short session prompt that tells the agent to search with `codemap` instead of grep.
- A hook on Bash. A `grep`, `egrep`, `rg` or `git grep` over a folder is answered by codemap with
  the matches the real tool would print, minus gitignored files, ranked and capped. The regex dialect
  of each tool is kept, and the common options are supported (`-i`, `-w`, `-x`, `-F`, `-l`, `-S`,
  `--include`, `-g`, `-t`, `--exclude-dir`). A search codemap cannot reproduce exactly, for example
  one with PCRE, context lines or piped input, runs as usual.
- Read-only `codemap` commands run without a permission prompt, also when piped into `head`, `tail`,
  `grep`, `wc`, `sort`, `uniq` or `cut`. `setup`, `self-update` and `logs` still ask, because they
  write or can open any file. Only the bare `codemap` on your `PATH` is trusted this way, never a
  script of that name inside a repository, and a repository cannot change the list.

Claude's own Grep tool is left alone unless a repository opts in with `{"searchTools": ["Grep"]}`.
The plugin finds the binary by itself (`$CODEMAP_BIN`, `~/.cargo/bin`, `~/.local/bin`, or a cached
download) and does nothing if there is none.

Other agents get four MCP tools: `codemap_lookup`, `codemap_uses`, `codemap_impl` and
`codemap_grep`. The list is short on purpose, since every extra tool is one more choice for the
model.

## Configuration

codemap works without any. A repository can have a `.codemap.json` and a `.codemapignore`
(gitignore syntax) at its root or in `.claude/`. Your own `codemap/config.json`, in the platform
config folder, applies to every repository; the repository's file wins where both set a value.

```json
{
  "siblings": ["../server", "../web"],
  "settings": {
    "scope": {"testDirs": ["test", "e2e"], "testFiles": ["*Test.kt"]},
    "limits": {"answerLines": 40, "threads": 8}
  }
}
```

- `siblings`: other git repositories in the same parent folder that belong to this workspace. Paths
  that point anywhere else are ignored.
- `literals`: globs of contract files, such as OpenAPI documents, to scan for routes.
- `bridges`: extra rules for names that cross languages.
- `roles`: names for `outline` groups by supertype or name pattern, for example `{"Port": ["*Port"]}`.
- `searchTools`: tools besides Bash whose searches the hook answers.
- `settings` and `prompts`: overrides for any default.

The defaults are in [defaults/settings.toml](defaults/settings.toml) and
[defaults/prompts/](defaults/prompts). Tables merge key by key and lists replace. A typo is reported
and the defaults are used.

codemap does not guess about your code base. Generated and vendored files are taken from git's
`linguist-generated` and `linguist-vendored` attributes. Test folders, stop words, synonyms and the
keywords of the fallback parser are empty until you set them.

## How it works

```
src/
  main.rs, cli.rs, commands.rs   arguments, help, dispatch
  workspace.rs                   the repository of the working directory and its siblings
  config/                        .codemap.json, the user config, settings and their defaults
  index/                         git, the store, parsing, words, vectors, scopes
  query/                         def, uses, impl, variants, link, outline, show, find, search, deps
  output.rs                      the answer format
  agent/                         the hook, the MCP server, setup
tests/                           integration tests; tests/unit/ has the unit tests of each module
public/                          the project page and the logo
languages/                       the language registry and the tree-sitter queries
models/                          the embedding model that is compiled into the binary
defaults/                        settings and prompts, compiled into the binary
```

### The index

There is one SQLite database per repository, in your cache folder: `~/Library/Caches/codemap` on
macOS, `~/.cache/codemap` on Linux, `%LOCALAPPDATA%\codemap` on Windows. `CODEMAP_CACHE_DIR` moves
it. Parses are keyed by git blob sha, so a file that is the same on two branches is parsed once and
switching back to a branch parses nothing.

Before every query codemap asks git for the tracked tree and for what changed, and parses only the
files it has not seen. Sibling repositories are brought up to date at session start.

The index is rebuilt by itself when the parse settings change or when a new binary brings other
grammars or another model. Indexes of repositories that no longer exist, or that were not used for
14 days (`limits.cacheDays`), are deleted.

### Parsing

35 languages have a tree-sitter grammar compiled in: Kotlin, Swift, TypeScript, TSX, JavaScript,
Python, Go, Rust, Java, C, C++, C#, Objective-C, Ruby, PHP, Scala, Dart, Lua, Elixir, Haskell, OCaml,
Zig, Bash, Make, SQL, Protocol Buffers, GraphQL, HCL and Terraform, HTML, CSS, XML (including Android
resources, SVG and plist), Markdown, JSON, YAML and TOML. Each uses the grammar project's own
`tags.scm` plus codemap's additions in `languages/<language>/extra.scm`.

Any other text file is still found by grep and by its words. If you set
`parse.declarationKeywords`, a line scan finds its definitions too. In prose files
(`parse.proseExtensions`) the headings are the definitions. A file that is not text, such as an
image or a font, is indexed by its path: `codemap ic_logo` finds `ic_logo.png`, and
`codemap uses ic_logo` finds the layouts that show it.

### Links and bridges

String literals that are paths or URLs are indexed with the definition they sit in, so
`codemap link '/api/v1/books/{id}'` finds the client, the server and the tests of one endpoint
across repositories.

Bridges are rules for how a definition is reached from another language, so `uses` finds those
call sites. A Kotlin top-level function in `Platform.kt` is `PlatformKt` from Swift, a TypeScript
export is reached by an import whose path ends in its file, and a Python definition by
`from package.module import name`. They are settings and can be extended.

### Search by words

Identifiers are split on case, underscores and digits, and each word is reduced to its stem
([rust-stemmers](https://crates.io/crates/rust-stemmers)), so "refreshing tokens" finds
`refreshToken`. Files are ranked by how many of the query's words they hold, then by the name of
their best definition, then by BM25. The query is also searched as the identifiers of the code base
that contain its words.

A word that no file holds is replaced by the closest word from the code base's own names and paths,
and the answer says so: `notificaton icon` prints `searched for: notification icon`.

A name without a definition is matched against all definition names, first by letters in order
([nucleo](https://crates.io/crates/nucleo): `BRepoImpl` is `BookRepositoryImpl`), then by edit
distance ([strsim](https://crates.io/crates/strsim): `BookRuote`).

### Search by meaning

Words do not help when a description shares none with the code: "database access object for
orders" should find `OrderDao`. For that, every file also gets a vector from
[potion-code-16M-v2](https://huggingface.co/minishlab/potion-code-16M-v2), a static embedding model
made for code retrieval.

A static model is a table of token vectors that are averaged. No neural network runs, so embedding a
file costs about as much as tokenizing it. The model is 33 MB, MIT licensed, and compiled into the
binary from `models/`, so there is nothing to install and nothing to download. The tokenizer and
the table reader are part of codemap (`src/index/embed.rs`), so the model adds no dependency.

A file's vector is made from its path and the start of its text. Vectors are stored as 16-bit
numbers, 512 bytes per file, and compared straight from the database. With a few thousand files
per repository that is faster than keeping a vector index.

The closest files join the word ranking: files that both found come first, then the two take turns.
When no file holds any of the words, the answer lists the closest files and says that it did. A
single unknown name still answers `no match`. All of this is part of `codemap <a few words>`; there
is no extra command. `search.semanticFiles = 0` turns it off.

## Measured

On an Apple-silicon laptop with 18 cores and a private workspace of 5 repositories and about 5 300
files (Kotlin, TypeScript, Swift):

| Operation | Time |
|---|---|
| First index of all five repositories, vectors included | 1.1 s |
| Re-parse of one repository (1 550 files) | 0.7 s |
| Refresh with nothing changed | 0.07 s |
| A name lookup, including the refresh | 0.12 s |
| A lookup by words, including the refresh and loading the model | 0.22 to 0.27 s |

`codemap build` prints the git, parse and write time of each repository.

`codemap bench <questions.json>` runs a set of questions through headless Claude Code with and
without codemap and reports medians. An answer counts as correct when it names the expected
`path:line`.

The table below is from the same workspace: nine navigation questions, three runs each, Claude
Haiku, read-only tools, and no instructions that tell the model which tool to prefer. The questions
ask where something is defined, what implements an interface, who calls a function, which files use
an image, two descriptions that share no word with the code, and one misspelt name.

<!-- bench:real:start -->
codemap 0.1.0, 2026-11-10, 2.1.322 (Claude Code), model haiku, 3 runs per question

| Configuration | Correct | Turns | Seconds | Tokens | Cost |
|---|---|---|---|---|---|
| A | 27/27 | 3 | 5.3 | 126210 | $0.004 |
| C | 27/27 | 2 (x0.67) | 5.0 (x0.95) | 84112 (x0.67) | $0.005 (x1.19) |

- A: without codemap
- C: codemap
<!-- bench:real:end -->

With codemap the model needs a third fewer turns and reads a third fewer tokens. It is not more
correct on these questions, because grep and file reads get them right too, and it is about as fast.
It is not cheaper either: most tokens of a headless call are cached prompt, which costs far less
than fresh tokens, so the bill stays within a fraction of a cent per question both ways. Harder
questions and larger models are not measured here.

## Untidy code

- tree-sitter recovers from syntax errors and keeps what it can read. With fallback keywords set, a
  line scan also recovers the definitions after the broken spot.
- A file that is not UTF-8, has NUL bytes or is larger than 1 MiB is not parsed. It is still found
  by its name and path.
- Symbolic links are not followed.
- The hook never fails. If it does not understand a command it prints nothing and the tool runs as
  if codemap were not there.
- An unquoted glob in an option value (`--include=*.kt`) is refused with the quoted form, because
  the shell would expand it before the program sees it.

## Security

- Releases are built by GitHub Actions from a tagged commit, ship a `SHA256SUMS` file and carry
  build provenance (`gh attestation verify codemap-<target>.tar.gz --repo LIVELUCKY/codemap`). The
  installer and the plugin launcher check the checksum before they run anything.
- CI scans every pull request, every push to `main` and a weekly schedule with Trivy and
  `cargo audit`. Dependabot keeps dependencies and workflow actions current.
- A repository's own `.codemap.json` is not trusted with anything that widens access: it cannot
  change which commands run without a prompt, and siblings must be git repositories next to it.
- codemap makes no network call while answering a query. It reads your repository with `git` and
  keeps its index in your cache folder, readable only by you.
- To report a vulnerability privately, see [SECURITY.md](SECURITY.md).

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
scripts/sync-queries        # refresh the upstream tags.scm queries
```

A new language is an entry in `languages/registry.toml`, a grammar crate and an `extra.scm`; see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

MIT. The model in `models/` is potion-code-16M-v2 by MinishLab, also MIT; see the notice next to
it.
