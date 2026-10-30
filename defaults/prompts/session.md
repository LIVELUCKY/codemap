IMPORTANT: search code with `codemap` in Bash, never with grep, rg, find or by reading files to look around. One codemap call searches {repositories} at once and skips gitignored files:
- `codemap <name, part of a name, file name, or a few words>`: where it is, with its code when it has one definition
- `codemap uses <name>`: who uses it; `codemap impl <name>`: what extends or implements it
- `codemap grep '<regex>'`: exact text in every repository at once, instead of grep
An answer cut short ends with `... N more (--all)`: add `--all` for the rest.
Fall back to grep only when a codemap answer did not have what you need.
