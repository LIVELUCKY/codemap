# Contributing

## Pull requests

- Work on a branch and open a pull request; `main` accepts nothing else.
- The title follows conventional commits (`feat: ...`, `fix: ...`); CI checks it.
- Every required check must be green: fmt, clippy, tests on Linux and macOS, Trivy, `cargo audit`,
  dependency review.
- One review from a code owner. Squash merge, so each pull request is one commit on `main`.
- A pull request does one thing. Refactors go in their own pull request.

## Tests first

A bug fix comes with a test that fails without the fix. A feature comes with the test that describes
it. Parsing changes come with a fixture for the language and a robustness case (broken input, CRLF).

## Local checks

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

## Adding a language

1. `cargo add tree-sitter-<language>`
2. A `[[language]]` entry in `languages/registry.toml`
3. `cargo fetch && sh scripts/sync-queries` (copies the grammar's own `tags.scm`)
4. `languages/<language>/extra.scm` for what the upstream query lacks, and a fixture plus test in
   `src/index/parse/grammar.rs`

## Fixtures

Fixtures are invented. Never paste code from a private or employer repository into a fixture, a test
or an issue.

## Releases

Bump the version in `Cargo.toml` and `.claude-plugin/plugin.json` (the release workflow checks they
match the tag), merge, then push a tag `v<version>`.
