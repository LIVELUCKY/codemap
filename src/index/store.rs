//! SQLite storage: one database per repository, keyed by git blob sha.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::{Connection, ErrorCode, Row, TransactionBehavior, params};

use crate::model::{Hit, Role, Symbol};

/// Separator for the list columns; never appears in an identifier.
const US: &str = "\x1f";

/// Bumped when the tables change; a store of another version is dropped and built again.
const SCHEMA_VERSION: &str = "9";

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS blobs(
        sha TEXT PRIMARY KEY, language TEXT NOT NULL, parsed_at INTEGER NOT NULL, length INTEGER NOT NULL,
        vector BLOB);
    CREATE TABLE IF NOT EXISTS files(path TEXT PRIMARY KEY, sha TEXT NOT NULL, scope TEXT NOT NULL, dirty_key TEXT);
    CREATE TABLE IF NOT EXISTS symbols(
        sha TEXT NOT NULL, name TEXT NOT NULL, role TEXT NOT NULL, kind TEXT NOT NULL,
        line INTEGER NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL,
        flags TEXT NOT NULL, supertypes TEXT NOT NULL);
    CREATE INDEX IF NOT EXISTS symbols_name ON symbols(name);
    CREATE INDEX IF NOT EXISTS symbols_sha ON symbols(sha);
    CREATE INDEX IF NOT EXISTS files_sha ON files(sha);
    CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS terms(
        word TEXT NOT NULL, sha TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY(word, sha)) WITHOUT ROWID;
";

const HIT_COLUMNS: &str = "s.name, s.role, s.kind, s.line, s.flags, s.supertypes, f.path, f.scope, b.language, s.start_line, s.end_line";
const HIT_FROM: &str = "FROM symbols s JOIN files f ON f.sha = s.sha JOIN blobs b ON b.sha = s.sha";

/// A search candidate: sha, distinct query words, query words in the best definition name,
/// occurrences, bm25, and the best definition (name and line).
type Scored = (String, usize, usize, usize, f64, Option<(String, u32)>);

/// The current tree: which blob sits at which path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub path: String,
    pub sha: String,
    pub scope: String,
    pub dirty_key: Option<String>,
}

/// What one blob parses to, ready to store.
#[derive(Debug, Clone)]
pub struct ParsedBlob {
    pub key: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    /// Each searchable word and how often it occurs (`text::terms`).
    pub terms: Vec<(String, u32)>,
    /// The semantic vector of the file (`embed`), when the model is installed.
    pub vector: Option<Vec<f32>>,
}

pub struct Store {
    conn: Connection,
    repo_name: String,
}

/// Deletes the stores in `cache_dir` that were not used for `max_age` or whose repository is gone,
/// and returns how many. Nothing else empties the cache folder, so
/// codemap cleans up after itself everywhere.
pub fn prune(cache_dir: &Path, max_age: Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return 0;
    };
    let mut removed = 0;
    for db in entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "db"))
    {
        let unused = std::fs::metadata(&db)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > max_age);
        let gone = std::fs::read_to_string(db.with_extension("root"))
            .is_ok_and(|root| !Path::new(root.trim()).exists());
        if unused || gone {
            for suffix in ["db", "db-wal", "db-shm", "root"] {
                let _ = std::fs::remove_file(db.with_extension(suffix));
            }
            removed += 1;
        }
    }
    removed
}

impl Store {
    /// Opens (or creates) the database of `repo_root` inside `cache_dir`.
    pub fn open(cache_dir: &Path, repo_root: &Path) -> Result<Store> {
        // Private to the user: an index says what is in a repository.
        let mut create = std::fs::DirBuilder::new();
        create.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut create, 0o700);
        create
            .create(cache_dir)
            .with_context(|| format!("creating {}", cache_dir.display()))?;
        let canonical = repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf());
        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(canonical.to_string_lossy().as_bytes());
        let db = cache_dir.join(format!("{}.db", hasher.digest()));
        let conn = Connection::open(&db).with_context(|| format!("opening {}", db.display()))?;
        // Which repository the store belongs to, and that it was just used: what `prune` reads.
        let _ = std::fs::write(
            db.with_extension("root"),
            canonical.to_string_lossy().as_bytes(),
        );
        if let Ok(file) = std::fs::File::options().write(true).open(&db) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        conn.busy_timeout(BUSY)?;
        // Switching to WAL and creating the schema both need the file to themselves. A second process
        // opening at the same moment (a session build next to a hook query) gets SQLITE_BUSY without
        // the busy handler being consulted, so these two steps retry explicitly.
        retry_busy(|| conn.pragma_update(None, "journal_mode", "WAL"))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // A store of another schema is a cache of another version: dropped and built again.
        let schema: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get(0)
            })
            .ok();
        let reset = if schema.is_some_and(|v| v != SCHEMA_VERSION) {
            "DROP TABLE IF EXISTS blobs; DROP TABLE IF EXISTS files; DROP TABLE IF EXISTS symbols;
             DROP TABLE IF EXISTS meta; DROP TABLE IF EXISTS text; DROP TABLE IF EXISTS terms;"
        } else {
            ""
        };
        retry_busy(|| {
            let result = conn.execute_batch(&format!(
                "BEGIN IMMEDIATE;{reset}{SCHEMA}\
                 INSERT OR IGNORE INTO meta(key, value) VALUES ('schema', '{SCHEMA_VERSION}');COMMIT;"
            ));
            if result.is_err() {
                let _ = conn.execute_batch("ROLLBACK;");
            }
            result
        })?;
        let repo_name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Store { conn, repo_name })
    }

    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }

    pub fn has_blob(&self, sha: &str) -> bool {
        self.conn
            .query_row("SELECT 1 FROM blobs WHERE sha = ?1", [sha], |_| Ok(()))
            .is_ok()
    }

    /// Records what a blob parses to, replacing any earlier parse of the same sha.
    pub fn insert_blob(&mut self, sha: &str, language: &str, symbols: &[Symbol]) -> Result<()> {
        let blob = ParsedBlob {
            key: sha.into(),
            language: language.into(),
            symbols: symbols.to_vec(),
            terms: Vec::new(),
            vector: None,
        };
        self.apply(std::slice::from_ref(&blob), &[], &[])
    }

    /// One reconcile's writes in one transaction: new parses, changed file rows and removed paths.
    /// A commit per file would pay the journal on every one of them.
    pub fn apply(
        &mut self,
        blobs: &[ParsedBlob],
        changed: &[FileRow],
        removed: &[String],
    ) -> Result<()> {
        if blobs.is_empty() && changed.is_empty() && removed.is_empty() {
            return Ok(());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let mut blob = tx.prepare_cached(
                "INSERT OR REPLACE INTO blobs(sha, language, parsed_at, length, vector) VALUES (?1, ?2, unixepoch(), ?3, ?4)",
            )?;
            let mut known = tx.prepare_cached("SELECT 1 FROM blobs WHERE sha = ?1")?;
            let mut forget_symbols = tx.prepare_cached("DELETE FROM symbols WHERE sha = ?1")?;
            let mut forget_terms = tx.prepare_cached("DELETE FROM terms WHERE sha = ?1")?;
            let mut term =
                tx.prepare_cached("INSERT INTO terms(word, sha, count) VALUES (?1, ?2, ?3)")?;
            let mut symbol = tx.prepare_cached(
                "INSERT INTO symbols(sha, name, role, kind, line, start_line, end_line, flags, supertypes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            // Rows in word order: the terms B-tree is filled front to back instead of at random.
            let mut terms: Vec<(&str, &str, u32)> = blobs
                .iter()
                .flat_map(|b| {
                    b.terms
                        .iter()
                        .map(|(w, n)| (w.as_str(), b.key.as_str(), *n))
                })
                .collect();
            terms.sort_unstable();
            for b in blobs {
                // Keys are content addresses, so a reconcile only ever adds new ones; replacing a known
                // blob (a test, a forced re-parse) is the rare case that pays for the deletes.
                if known.exists([&b.key])? {
                    forget_symbols.execute([&b.key])?;
                    forget_terms.execute([&b.key])?;
                }
                let length: u32 = b.terms.iter().map(|(_, n)| n).sum();
                let vector = b.vector.as_deref().map(super::embed::to_bytes);
                blob.execute(params![b.key, b.language, length, vector])?;
                for s in &b.symbols {
                    symbol.execute(params![
                        b.key,
                        s.name,
                        s.role.as_str(),
                        s.kind,
                        s.line,
                        s.start_line,
                        s.end_line,
                        s.flags.join(US),
                        s.supertypes.join(US)
                    ])?;
                }
            }
            for (word, sha, count) in terms {
                term.execute(params![word, sha, count])?;
            }
            let mut upsert = tx.prepare_cached(
                "INSERT OR REPLACE INTO files(path, sha, scope, dirty_key) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for f in changed {
                upsert.execute(params![f.path, f.sha, f.scope, f.dirty_key])?;
            }
            let mut delete = tx.prepare_cached("DELETE FROM files WHERE path = ?1")?;
            for path in removed {
                delete.execute([path])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn files(&self) -> Result<Vec<FileRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, sha, scope, dirty_key FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |r| {
            Ok(FileRow {
                path: r.get(0)?,
                sha: r.get(1)?,
                scope: r.get(2)?,
                dirty_key: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Replaces the whole tree in one transaction.
    pub fn replace_files(&mut self, rows: &[FileRow]) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM files", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO files(path, sha, scope, dirty_key) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for f in rows {
                insert.execute(params![f.path, f.sha, f.scope, f.dirty_key])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn by_name(&self, name: &str) -> Result<Vec<Hit>> {
        self.hits(
            &format!("SELECT {HIT_COLUMNS} {HIT_FROM} WHERE s.name = ?1 ORDER BY f.path, s.line"),
            name,
        )
    }

    pub fn by_supertype(&self, name: &str) -> Result<Vec<Hit>> {
        let candidates = self.hits(
            &format!(
                "SELECT {HIT_COLUMNS} {HIT_FROM} WHERE s.role = 'definition' AND instr(s.supertypes, ?1) > 0 ORDER BY f.path, s.line"
            ),
            name,
        )?;
        Ok(candidates
            .into_iter()
            .filter(|h| h.symbol.supertypes.iter().any(|s| s == name))
            .collect())
    }

    /// The literals that contain `literal`, so a route is found with or without its leading `/`, by
    /// its start (`/api/v1/books`) and inside a full URL.
    pub fn literals(&self, literal: &str) -> Result<Vec<Hit>> {
        self.hits(
            &format!("SELECT {HIT_COLUMNS} {HIT_FROM} WHERE s.role = 'literal' AND instr(s.name, ?1) > 0 ORDER BY f.path, s.line"),
            literal,
        )
    }

    /// Every symbol in files at or below `path_prefix`.
    pub fn under(&self, path_prefix: &str) -> Result<Vec<Hit>> {
        let prefix = path_prefix.trim_end_matches('/');
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {HIT_COLUMNS} {HIT_FROM} WHERE f.path = ?1 OR f.path LIKE ?2 ORDER BY f.path, s.line"
        ))?;
        let rows = stmt.query_map(params![prefix, format!("{prefix}/%")], |r| self.hit(r))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Forgets every parse and the tree; `meta` stays.
    pub fn clear(&mut self) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE; DELETE FROM symbols; DELETE FROM files; DELETE FROM blobs; DELETE FROM terms; COMMIT;")?;
        Ok(())
    }

    /// Every distinct definition name, for fuzzy suggestions.
    /// The `limit` files closest to `wanted` by their semantic vector, best first: similarity, path,
    /// scope. Every vector is compared as SQLite hands its bytes over, without a copy; a repository
    /// has thousands of files, where reading them all beats keeping a vector index up to date.
    pub fn nearest(&self, wanted: &[f32], limit: usize) -> Result<Vec<(f32, String, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT b.vector, f.path, f.scope FROM files f JOIN blobs b ON b.sha = f.sha
             WHERE b.vector IS NOT NULL",
        )?;
        let mut rows = stmt.query([])?;
        let mut best: Vec<(f32, String, String)> = Vec::new();
        while let Some(row) = rows.next()? {
            let score = super::embed::similarity(wanted, row.get_ref(0)?.as_blob()?);
            // Only a row that makes the list is read in full.
            if best.len() < limit || best.last().is_some_and(|worst| score > worst.0) {
                let at = best.partition_point(|b| b.0 >= score);
                best.insert(at, (score, row.get(1)?, row.get(2)?));
                best.truncate(limit);
            }
        }
        Ok(best)
    }

    /// Whether any file holds `word` as a search term.
    pub fn has_term(&self, word: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM terms WHERE word = ?1 LIMIT 1",
                [word],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn definition_names(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT name FROM symbols WHERE role = 'definition' ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Every blob of the current tree that holds a query word: (sha, distinct query words held, their
    /// occurrences, BM25), best first. BM25 with its usual constants (k1 1.2, b 0.75, as SQLite FTS5
    /// uses), computed from the stored counts.
    fn bm25(&self, words: &[String]) -> Result<Vec<(String, usize, usize, f64)>> {
        let (documents, average): (f64, f64) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(AVG(length), 0) FROM blobs WHERE sha IN (SELECT sha FROM files)",
            [],
            |r| Ok((r.get::<_, i64>(0)? as f64, r.get(1)?)),
        )?;
        let (k1, b) = (1.2, 0.75);
        let mut per_blob: std::collections::HashMap<String, (usize, usize, f64)> =
            std::collections::HashMap::new();
        let mut postings = self.conn.prepare_cached(
            "SELECT t.sha, t.count, bl.length FROM terms t JOIN blobs bl ON bl.sha = t.sha
             WHERE t.word = ?1 AND t.sha IN (SELECT sha FROM files)",
        )?;
        for word in words {
            let rows: Vec<(String, i64, i64)> = postings
                .query_map([word], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<_, _>>()?;
            let df = rows.len() as f64;
            let idf = ((documents - df + 0.5) / (df + 0.5) + 1.0).ln();
            for (sha, count, length) in rows {
                let tf = count as f64;
                let norm = 1.0 - b + b * length as f64 / average.max(1.0);
                let entry = per_blob.entry(sha).or_default();
                entry.0 += 1;
                entry.1 += count as usize;
                entry.2 += idf * tf * (k1 + 1.0) / (tf + k1 * norm);
            }
        }
        let mut ranked: Vec<(String, usize, usize, f64)> = per_blob
            .into_iter()
            .map(|(sha, (distinct, total, score))| (sha, distinct, total, score))
            .collect();
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.0.cmp(&b.0))
        });
        Ok(ranked)
    }

    /// Files whose words match best, one hit per file. Candidates are the blobs holding any query
    /// word, and are ordered by how many distinct query words they hold, then by how many query words
    /// their best definition name holds (the file that defines the thing beats the file that calls
    /// it), then by occurrences, then by BM25, because length normalisation alone lets a short file
    /// win. The best definition gives the line and name, else line 1 and the file name. Each hit
    /// comes with the number of distinct query words its file holds, which compares across stores.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(Hit, usize)>> {
        let words: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| w.to_ascii_lowercase())
            .collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let ranked = self.bm25(&words)?;
        let mut scored: Vec<Scored> = Vec::new();
        for (sha, distinct, total, score) in ranked.into_iter().take(limit * 4) {
            let mut defs = self.conn.prepare("SELECT name, line FROM symbols WHERE sha = ?1 AND role = 'definition' ORDER BY line")?;
            let best = defs
                .query_map([&sha], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
                })?
                .filter_map(Result::ok)
                .map(|(name, line)| {
                    let lower = name.to_ascii_lowercase();
                    let matched = words.iter().filter(|w| lower.contains(w.as_str())).count();
                    (matched, name, line)
                })
                .filter(|(matched, _, _)| *matched > 0)
                .max_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)));
            let def_words = best.as_ref().map(|b| b.0).unwrap_or(0);
            scored.push((
                sha,
                distinct,
                def_words,
                total,
                score,
                best.map(|(_, name, line)| (name, line)),
            ));
        }
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(b.2.cmp(&a.2))
                .then(b.3.cmp(&a.3))
                .then(b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal))
        });
        scored.truncate(limit);
        let mut out = Vec::new();
        for (sha, distinct, _, _, _, best) in scored {
            let mut files = self.conn.prepare("SELECT f.path, f.scope, b.language FROM files f JOIN blobs b ON b.sha = f.sha WHERE f.sha = ?1 ORDER BY f.path")?;
            let locations: Vec<(String, String, String)> = files
                .query_map([&sha], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<_, _>>()?;
            for (path, scope, language) in locations {
                let (name, line) = best
                    .clone()
                    .unwrap_or_else(|| (path.rsplit('/').next().unwrap_or(&path).to_string(), 1));
                out.push((
                    Hit {
                        repo: self.repo_name.clone(),
                        path,
                        scope,
                        language,
                        symbol: Symbol {
                            name,
                            role: Role::Reference,
                            kind: "text".into(),
                            line,
                            start_line: line,
                            end_line: line,
                            flags: Vec::new(),
                            supertypes: Vec::new(),
                        },
                    },
                    distinct,
                ));
            }
        }
        Ok(out)
    }

    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    fn hits(&self, sql: &str, arg: &str) -> Result<Vec<Hit>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([arg], |r| self.hit(r))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn hit(&self, r: &Row) -> rusqlite::Result<Hit> {
        let role: String = r.get(1)?;
        let flags: String = r.get(4)?;
        let supertypes: String = r.get(5)?;
        Ok(Hit {
            repo: self.repo_name.clone(),
            path: r.get(6)?,
            scope: r.get(7)?,
            language: r.get(8)?,
            symbol: Symbol {
                name: r.get(0)?,
                role: Role::parse(&role).unwrap_or(Role::Definition),
                kind: r.get(2)?,
                line: r.get(3)?,
                start_line: r.get(9)?,
                end_line: r.get(10)?,
                flags: split_list(&flags),
                supertypes: split_list(&supertypes),
            },
        })
    }
}

const BUSY: Duration = Duration::from_millis(5_000);

/// Writers take the write lock up front (immediate transactions), so the busy handler can wait for
/// them; the two setup steps in `open` cannot, hence this loop.
fn retry_busy<T>(mut op: impl FnMut() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    let start = Instant::now();
    loop {
        match op() {
            Err(rusqlite::Error::SqliteFailure(f, _))
                if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
                    && start.elapsed() < BUSY =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

fn split_list(text: &str) -> Vec<String> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split(US).map(str::to_string).collect()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/index/store.rs"]
mod tests;
