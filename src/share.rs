//! Shareable reading permalinks.
//!
//! Every reading is persisted as a plain-text snapshot under a stable id, served
//! by geomyidae as an ordinary text file at `/r/<id>.txt`. The dcgi prints a
//! copyable `gopher://` permalink in each reading; revisiting or bookmarking that
//! selector is how a reader "saves" a reading — no accounts, no cookies, no
//! server-side per-person history.
//!
//! The stored copy is rendered with the typed text omitted (see
//! `reading::render_header(None, ..)`), so a permalink never exposes what someone
//! typed. The id is content-derived from the cards + day (not the typed text), so
//! identical draws collapse to one permalink — matching askthedeck's card-keyed
//! cache.
//!
//! Files live on a writable volume (the docroot's `/r`), pruned by mtime so the
//! store doesn't grow without bound.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// How long a shared reading is retained before the GC sweep removes it.
pub const TTL_DAYS: u64 = 30;

fn entry(dir: &Path, id: &str) -> std::path::PathBuf {
    dir.join(format!("{id}.txt"))
}

/// Persist `body` as the shareable snapshot for `id` (idempotent — the same id
/// always holds the same content), then sweep expired entries. Best-effort: a
/// failure here must never break the reading, so callers ignore the error.
pub fn store(dir: &Path, id: &str, body: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(entry(dir, id), body)?;
    gc(dir, TTL_DAYS);
    Ok(())
}

/// Read back a shared reading (used by tests / any future dcgi server path).
pub fn load(dir: &Path, id: &str) -> Option<String> {
    fs::read_to_string(entry(dir, id)).ok()
}

/// The copyable gopher URL for a shared reading. Type `0` (text file). `host`
/// and `port` are the server's, taken from the dcgi argv (display only — never
/// the LLM). `selector` is the served path, e.g. `/r/<id>.txt`.
pub fn permalink(host: &str, port: &str, selector: &str) -> String {
    format!("gopher://{host}:{port}/0{selector}")
}

/// Remove snapshots older than `ttl_days` (by mtime). Cheap linear sweep — fine
/// for a niche hole's volume.
fn gc(dir: &Path, ttl_days: u64) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let now = SystemTime::now();
    let ttl = Duration::from_secs(ttl_days * 86_400);
    for e in rd.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if now
            .duration_since(modified)
            .map(|age| age > ttl)
            .unwrap_or(false)
        {
            let _ = fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("atd-share-test-{name}"));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn store_then_load() {
        let d = dir("roundtrip");
        store(&d, "abc123", "a shared reading\nwith lines").unwrap();
        assert_eq!(
            load(&d, "abc123").as_deref(),
            Some("a shared reading\nwith lines")
        );
        assert!(load(&d, "nope").is_none());
    }

    #[test]
    fn store_is_idempotent() {
        let d = dir("idem");
        store(&d, "x", "one").unwrap();
        store(&d, "x", "one").unwrap();
        assert_eq!(load(&d, "x").as_deref(), Some("one"));
    }

    #[test]
    fn fresh_entries_survive_gc() {
        let d = dir("gc");
        store(&d, "keep", "fresh").unwrap(); // store() runs gc() internally
        assert!(
            load(&d, "keep").is_some(),
            "a just-written file must not be GC'd"
        );
    }

    #[test]
    fn permalink_format() {
        assert_eq!(
            permalink("gopher.debene.dev", "7072", "/r/7f3a.txt"),
            "gopher://gopher.debene.dev:7072/0/r/7f3a.txt"
        );
    }
}
