//! Abuse + cost controls: a per-client token bucket and a global daily call cap.
//!
//! Both persist to flat files so they survive the per-request dcgi process.
//!
//! The rate limiter is keyed by a **hash** of the client IP, never the IP
//! itself: the address is hashed at the IO edge and only the hash reaches here,
//! so nothing in this module (or its files) can reconstruct who someone is, and
//! the IP never travels near the LLM path. The daily cap bounds spend: once the
//! day's budget is gone, every reading falls back to the deterministic local one
//! — no silent overspend.

use std::fs;
use std::path::{Path, PathBuf};

// ---- per-IP token bucket ---------------------------------------------------

fn bucket_path(dir: &Path, ip_hash: u64) -> PathBuf {
    dir.join(format!("rl-{ip_hash:016x}"))
}

/// Try to spend one token for `ip_hash`. Returns `true` if allowed. The bucket
/// holds up to `capacity` tokens and refills at `refill_per_sec`. On any IO
/// error we fail **open** (allow) — a broken limiter must not take the hole
/// down; the daily cap is the backstop against runaway cost.
pub fn allow(dir: &Path, ip_hash: u64, now_unix: i64, capacity: f64, refill_per_sec: f64) -> bool {
    let path = bucket_path(dir, ip_hash);
    let (mut tokens, last) = match fs::read_to_string(&path) {
        Ok(s) => parse_bucket(&s).unwrap_or((capacity, now_unix)),
        Err(_) => (capacity, now_unix),
    };
    // refill for elapsed time
    let elapsed = now_unix.saturating_sub(last).max(0) as f64;
    tokens = (tokens + elapsed * refill_per_sec).min(capacity);

    let allowed = tokens >= 1.0;
    if allowed {
        tokens -= 1.0;
    }
    if fs::create_dir_all(dir).is_ok() {
        let _ = fs::write(&path, format!("{tokens} {now_unix}"));
    }
    allowed
}

fn parse_bucket(s: &str) -> Option<(f64, i64)> {
    let mut it = s.split_whitespace();
    let tokens: f64 = it.next()?.parse().ok()?;
    let last: i64 = it.next()?.parse().ok()?;
    Some((tokens, last))
}

// ---- global daily call cap -------------------------------------------------

fn cap_path(dir: &Path) -> PathBuf {
    dir.join("daily-cap")
}

/// Reserve one LLM call against today's budget. Returns `true` if there was room
/// (and records the call); `false` once `max` is reached for `day`. The counter
/// resets when `day` changes. Counts *attempts* on purpose: a transient upstream
/// outage then degrades to local readings for the day instead of hammering a
/// failing paid API. Fails **closed** (returns false) on IO error — when in
/// doubt, don't spend.
pub fn try_acquire_call(dir: &Path, day: &str, max: u32) -> bool {
    let path = cap_path(dir);
    let (stored_day, count) = match fs::read_to_string(&path) {
        Ok(s) => parse_cap(&s).unwrap_or((day.to_string(), 0)),
        Err(_) => (day.to_string(), 0),
    };
    let count = if stored_day == day { count } else { 0 };
    if count >= max {
        return false;
    }
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    fs::write(&path, format!("{day} {}", count + 1)).is_ok()
}

fn parse_cap(s: &str) -> Option<(String, u32)> {
    let mut it = s.split_whitespace();
    let day = it.next()?.to_string();
    let count: u32 = it.next()?.parse().ok()?;
    Some((day, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("atd-rl-test-{name}"));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn bucket_allows_capacity_then_throttles() {
        let d = dir("bucket");
        // capacity 3, no refill within the same second
        assert!(allow(&d, 42, 1000, 3.0, 0.1));
        assert!(allow(&d, 42, 1000, 3.0, 0.1));
        assert!(allow(&d, 42, 1000, 3.0, 0.1));
        assert!(
            !allow(&d, 42, 1000, 3.0, 0.1),
            "4th hit in the same second is throttled"
        );
    }

    #[test]
    fn bucket_refills_over_time() {
        let d = dir("refill");
        for _ in 0..3 {
            allow(&d, 7, 1000, 3.0, 1.0);
        }
        assert!(!allow(&d, 7, 1000, 3.0, 1.0), "drained");
        // 2 seconds later, ~2 tokens are back
        assert!(allow(&d, 7, 1002, 3.0, 1.0), "refilled after 2s");
    }

    #[test]
    fn buckets_are_per_ip_hash() {
        let d = dir("perip");
        for _ in 0..3 {
            allow(&d, 1, 1000, 3.0, 0.1);
        }
        assert!(!allow(&d, 1, 1000, 3.0, 0.1));
        // a different ip hash has its own full bucket
        assert!(allow(&d, 2, 1000, 3.0, 0.1));
    }

    #[test]
    fn daily_cap_counts_and_resets_next_day() {
        let d = dir("cap");
        assert!(try_acquire_call(&d, "2026-06-27", 2));
        assert!(try_acquire_call(&d, "2026-06-27", 2));
        assert!(!try_acquire_call(&d, "2026-06-27", 2), "over cap");
        // new day resets
        assert!(try_acquire_call(&d, "2026-06-28", 2));
    }
}
