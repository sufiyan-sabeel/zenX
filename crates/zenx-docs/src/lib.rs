//! zenX Docs — offline documentation, imported by the user and searched locally.
//!
//! zenX ships no documentation corpus of its own: importing is always an
//! explicit act on files the user already has, which keeps redistribution rules
//! with the person who knows them. Ranking is term frequency over the imported
//! text, computed here rather than by SQLite, so the behaviour is identical on
//! every platform and needs no FTS5 build flag.

use std::path::Path;

use rusqlite::params;
use serde::Serialize;
use walkdir::WalkDir;
use zenx_core::{Result, ZenxError};
use zenx_storage::{map_err, Db};

/// Extensions treated as documentation.
const EXTENSIONS: &[&str] = &["md", "markdown", "txt", "rst"];

#[derive(Debug, Clone, Serialize)]
pub struct AddReport {
    pub collection: String,
    pub version: String,
    pub source: String,
    pub imported: usize,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionSummary {
    pub collection: String,
    pub version: String,
    pub source: String,
    pub imported_at: String,
    pub doc_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub collection: String,
    pub version: String,
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: i64,
}

/// First Markdown heading, else the first non-empty line, else the file name.
pub fn extract_title(body: &str, fallback: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix('#') {
            let heading = heading.trim_start_matches('#').trim();
            if !heading.is_empty() {
                return heading.to_string();
            }
        }
        return zenx_core::util::truncate(trimmed, 120);
    }
    fallback.to_string()
}

fn is_documentation(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Import every documentation file under `source` into one collection version.
///
/// Re-importing the same collection and version replaces it, so an edited
/// corpus does not accumulate stale duplicates.
pub fn add_collection(
    db: &Db,
    source: &Path,
    collection: &str,
    version: &str,
    max_file_bytes: u64,
) -> Result<AddReport> {
    if collection.trim().is_empty() {
        return Err(ZenxError::Invalid(
            "collection name must not be empty".into(),
        ));
    }
    if version.trim().is_empty() {
        return Err(ZenxError::Invalid("version must not be empty".into()));
    }
    if !source.is_dir() {
        return Err(ZenxError::Invalid(format!(
            "{} is not a directory",
            source.display()
        )));
    }

    let mut imported = 0usize;
    let mut skipped = Vec::new();
    let mut documents: Vec<(String, String, String)> = Vec::new();

    for entry in WalkDir::new(source).follow_links(false) {
        let entry =
            entry.map_err(|e| ZenxError::Invalid(format!("walk {}: {e}", source.display())))?;
        if !entry.file_type().is_file() || !is_documentation(entry.path()) {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(source)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                skipped.push(format!("{relative}: {error}"));
                continue;
            }
        };
        if metadata.len() > max_file_bytes {
            skipped.push(format!(
                "{relative}: {} bytes exceeds the {} byte limit",
                metadata.len(),
                max_file_bytes
            ));
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(body) => {
                let title = extract_title(&body, &relative);
                documents.push((relative, title, body));
                imported += 1;
            }
            Err(error) => skipped.push(format!("{relative}: {error}")),
        }
    }

    if documents.is_empty() {
        return Err(ZenxError::Invalid(format!(
            "no Markdown or text documentation found under {}",
            source.display()
        )));
    }

    let transaction = db.conn().unchecked_transaction().map_err(map_err)?;
    transaction
        .execute(
            "DELETE FROM docs WHERE collection = ?1 AND version = ?2",
            params![collection, version],
        )
        .map_err(map_err)?;
    for (path, title, body) in &documents {
        transaction
            .execute(
                "INSERT INTO docs (collection, version, path, title, body) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![collection, version, path, title, body],
            )
            .map_err(map_err)?;
    }
    transaction
        .execute(
            "INSERT INTO doc_collections (collection, version, source, imported_at, doc_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(collection, version) DO UPDATE SET
                source = excluded.source,
                imported_at = excluded.imported_at,
                doc_count = excluded.doc_count",
            params![
                collection,
                version,
                source.to_string_lossy().to_string(),
                zenx_core::util::now_rfc3339(),
                imported as i64
            ],
        )
        .map_err(map_err)?;
    transaction.commit().map_err(map_err)?;

    Ok(AddReport {
        collection: collection.to_string(),
        version: version.to_string(),
        source: source.display().to_string(),
        imported,
        skipped,
    })
}

pub fn list_collections(db: &Db) -> Result<Vec<CollectionSummary>> {
    let mut statement = db
        .conn()
        .prepare(
            "SELECT collection, version, source, imported_at, doc_count
             FROM doc_collections ORDER BY collection ASC, version ASC",
        )
        .map_err(map_err)?;
    let rows = statement
        .query_map([], |row| {
            Ok(CollectionSummary {
                collection: row.get(0)?,
                version: row.get(1)?,
                source: row.get(2)?,
                imported_at: row.get(3)?,
                doc_count: row.get(4)?,
            })
        })
        .map_err(map_err)?;
    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(map_err)?);
    }
    Ok(collections)
}

pub fn remove_collection(db: &Db, collection: &str, version: &str) -> Result<u64> {
    let transaction = db.conn().unchecked_transaction().map_err(map_err)?;
    let removed = transaction
        .execute(
            "DELETE FROM docs WHERE collection = ?1 AND version = ?2",
            params![collection, version],
        )
        .map_err(map_err)? as u64;
    transaction
        .execute(
            "DELETE FROM doc_collections WHERE collection = ?1 AND version = ?2",
            params![collection, version],
        )
        .map_err(map_err)?;
    transaction.commit().map_err(map_err)?;
    if removed == 0 {
        return Err(ZenxError::NotFound(format!(
            "collection {collection} version {version}"
        )));
    }
    Ok(removed)
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|term| term.len() >= 2)
        .map(|term| term.to_string())
        .collect()
}

fn count_occurrences(haystack: &str, needle: &str) -> i64 {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count() as i64
}

fn snippet(body: &str, terms: &[String]) -> String {
    let lower = body.to_ascii_lowercase();
    let mut position = None;
    for term in terms {
        if let Some(found) = lower.find(term) {
            position = Some(match position {
                Some(existing) if existing <= found => existing,
                _ => found,
            });
        }
    }
    let Some(position) = position else {
        return zenx_core::util::truncate(body.trim(), 160);
    };
    let start = position.saturating_sub(80);
    let end = (position + 120).min(body.len());
    let mut start = start;
    while start > 0 && !body.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = end;
    while end < body.len() && !body.is_char_boundary(end) {
        end += 1;
    }
    let text = body[start..end].replace('\n', " ").trim().to_string();
    format!("…{}…", zenx_core::util::truncate(&text, 220))
}

/// Search imported documentation.
pub fn search(
    db: &Db,
    query: &str,
    collection: Option<&str>,
    version: Option<&str>,
    limit: usize,
) -> Result<Vec<Hit>> {
    let terms = tokenize(query);
    if terms.is_empty() {
        return Err(ZenxError::Invalid(
            "search needs at least one term of two or more characters".into(),
        ));
    }

    let mut statement = db
        .conn()
        .prepare(
            "SELECT collection, version, path, title, body FROM docs
             WHERE (COALESCE(?1, '') = '' OR collection = ?1)
               AND (COALESCE(?2, '') = '' OR version = ?2)",
        )
        .map_err(map_err)?;
    let rows = statement
        .query_map(params![collection, version], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(map_err)?;

    let mut hits = Vec::new();
    for row in rows {
        let (doc_collection, doc_version, path, title, body) = row.map_err(map_err)?;
        let title_lower = title.to_ascii_lowercase();
        let body_lower = body.to_ascii_lowercase();
        let mut score = 0i64;
        let mut matched_terms = 0usize;
        for term in &terms {
            let in_title = count_occurrences(&title_lower, term);
            let in_body = count_occurrences(&body_lower, term);
            if in_title > 0 || in_body > 0 {
                matched_terms += 1;
            }
            score += in_title * 5 + in_body;
        }
        if matched_terms == 0 {
            continue;
        }
        hits.push(Hit {
            collection: doc_collection,
            version: doc_version,
            path,
            title,
            snippet: snippet(&body, &terms),
            score,
        });
    }

    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    hits.truncate(limit.max(1));
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(tag: &str) -> (PathBuf, Db) {
        let root = std::env::temp_dir().join(format!("zenx-docs-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("guide")).unwrap();
        std::fs::write(
            root.join("guide/install.md"),
            "# Installing zenX\n\nRun cargo build to compile the workspace.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("guide/trace.md"),
            "# Agent tracing\n\nSessions record tool calls and durations.\n",
        )
        .unwrap();
        std::fs::write(root.join("notes.txt"), "plain notes about deployment\n").unwrap();
        let db = Db::open_in_memory().unwrap();
        (root, db)
    }

    #[test]
    fn import_counts_documents_and_titles() {
        let (root, db) = fixture("import");
        let report = add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        assert_eq!(report.imported, 3);
        assert!(report.skipped.is_empty());
        let collections = list_collections(&db).unwrap();
        assert_eq!(collections.len(), 1);
        assert_eq!(collections[0].doc_count, 3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reimport_replaces_rather_than_duplicates() {
        let (root, db) = fixture("reimport");
        add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM docs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn search_ranks_title_matches_first() {
        let (root, db) = fixture("search");
        add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        let hits = search(&db, "tracing", None, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Agent tracing");

        let hits = search(&db, "build", None, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("cargo build"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn search_filters_by_collection_and_version() {
        let (root, db) = fixture("filters");
        add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        assert_eq!(
            search(&db, "notes", Some("zenx"), None, 10).unwrap().len(),
            1
        );
        assert_eq!(
            search(&db, "notes", Some("other"), None, 10).unwrap().len(),
            0
        );
        assert_eq!(
            search(&db, "notes", None, Some("9.9.9"), 10).unwrap().len(),
            0
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn oversized_files_are_skipped_with_a_reason() {
        let (root, db) = fixture("oversize");
        std::fs::write(root.join("big.md"), "x".repeat(4096)).unwrap();
        let report = add_collection(&db, &root, "zenx", "0.1.0", 1024).unwrap();
        assert_eq!(report.imported, 3);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("exceeds"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_query_and_empty_source_are_rejected() {
        let (root, db) = fixture("errors");
        assert!(search(&db, "a", None, None, 10).is_err());
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(add_collection(&db, &empty, "zenx", "1", 1024).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_collection_clears_documents() {
        let (root, db) = fixture("remove");
        add_collection(&db, &root, "zenx", "0.1.0", 1024 * 1024).unwrap();
        assert_eq!(remove_collection(&db, "zenx", "0.1.0").unwrap(), 3);
        assert!(list_collections(&db).unwrap().is_empty());
        assert!(remove_collection(&db, "zenx", "0.1.0").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn titles_fall_back_to_the_first_line() {
        assert_eq!(extract_title("# Hello\n\nbody", "x"), "Hello");
        assert_eq!(extract_title("plain first line\n", "x"), "plain first line");
        assert_eq!(extract_title("\n\n", "fallback"), "fallback");
    }
}
