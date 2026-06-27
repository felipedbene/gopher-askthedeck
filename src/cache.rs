//! Reading cache — flat files keyed by the draw seed.
//!
//! A public dcgi that pays an LLM per hit is a cost hole, and this endpoint will
//! be listed on Floodgap/Veronica-2 where bots crawl. The seed already folds in
//! the question and the UTC day, so it is the natural cache key: an identical
//! question on the same day returns the cached reading and makes ZERO LLM calls.
//!
//! Each entry is `<unix-ts>\n<reading bytes>`; the timestamp is stored in-band
//! (not read from filesystem mtime) so expiry is deterministic and testable.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 24-hour entry lifetime, mirroring askthedeck's same-day cache.
pub const TTL_SECONDS: i64 = 24 * 60 * 60;

fn entry_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.txt"))
}

/// Fetch a still-fresh cached reading, or `None` on miss/expiry/error.
pub fn get(dir: &Path, key: &str, now_unix: i64) -> Option<String> {
    let raw = fs::read_to_string(entry_path(dir, key)).ok()?;
    let (ts_line, body) = raw.split_once('\n')?;
    let ts: i64 = ts_line.trim().parse().ok()?;
    if now_unix.saturating_sub(ts) > TTL_SECONDS {
        return None;
    }
    Some(body.to_string())
}

/// Store a reading under `key`, stamped with `now_unix`. Best-effort: a cache
/// write failure must never break a reading, so callers ignore the error.
pub fn put(dir: &Path, key: &str, now_unix: i64, value: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(entry_path(dir, key), format!("{now_unix}\n{value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atd-cache-test-{name}"));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn miss_then_hit() {
        let d = dir("hit");
        assert_eq!(get(&d, "k1", 1000), None);
        put(&d, "k1", 1000, "the reading").unwrap();
        assert_eq!(get(&d, "k1", 1000), Some("the reading".to_string()));
    }

    #[test]
    fn multiline_body_round_trips() {
        let d = dir("multiline");
        let body = "line one\nline two\n\nline four";
        put(&d, "k", 5, body).unwrap();
        assert_eq!(get(&d, "k", 5).as_deref(), Some(body));
    }

    #[test]
    fn expires_after_ttl() {
        let d = dir("ttl");
        put(&d, "k", 1000, "old").unwrap();
        assert!(
            get(&d, "k", 1000 + TTL_SECONDS).is_some(),
            "exactly TTL is fresh"
        );
        assert!(
            get(&d, "k", 1000 + TTL_SECONDS + 1).is_none(),
            "past TTL is a miss"
        );
    }
}
