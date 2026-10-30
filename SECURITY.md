# Security policy

## Supported versions

The latest release receives fixes. Older releases do not.

## Reporting a vulnerability

Use GitHub's private reporting: **Security** tab, **Report a vulnerability**. Do not open a public
issue. You will get an answer within 7 days. Please include the version (`codemap --version`), the
command or hook event that triggers it, and what you expected.

## What counts

- Running anything a search did not ask for (the hook only reads and answers).
- Reading or writing outside the repository, its configured siblings and codemap's folder in the
  user's cache directory (or `CODEMAP_CACHE_DIR`).
- A way to make the installer or the launcher run an unverified binary.
- A crash on crafted input that takes the agent's search down with it (the hook must fail open).

## How releases are protected

- Built by GitHub Actions from a tagged commit; no manual uploads.
- Each release ships `SHA256SUMS` and build provenance attestations:
  `gh attestation verify codemap-<target>.tar.gz --repo LIVELUCKY/codemap`.
- `install.sh` and the plugin launcher verify the SHA-256 before running a downloaded binary.
- CI runs Trivy and `cargo audit` on every pull request and weekly; Dependabot updates dependencies.
