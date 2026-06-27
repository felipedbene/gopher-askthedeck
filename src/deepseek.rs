//! The DeepSeek call — the one network surface, behind the `net` feature.
//!
//! Blocking and synchronous on purpose: a dcgi is a short-lived process spawned
//! per request, so a tiny blocking client with no async runtime is the right
//! shape. Any failure — timeout, non-200, bad JSON, no key — returns `None`, and
//! the caller falls back to the deterministic local reading. The reading is
//! never blocked on the network.
//!
//! Only the assembled prompt and the API key cross this boundary; the prompt is
//! built by `reading::build_prompt` from {question, cards, cosmic} alone, so no
//! client metadata is here to leak.

use std::time::Duration;

/// Ask DeepSeek for a reading. `timeout_secs` bounds both connect and read.
/// Returns the model's text, or `None` on any error (caller falls back).
pub fn ask(api_key: &str, prompt: &str, timeout_secs: u64) -> Option<String> {
    if api_key.is_empty() {
        return None;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(timeout_secs))
        .timeout_read(Duration::from_secs(timeout_secs))
        .build();

    let resp = agent
        .post("https://api.deepseek.com/chat/completions")
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": "deepseek-chat",
            "messages": [{ "role": "user", "content": prompt }],
            "max_tokens": 2000
        }))
        .ok()?;

    let value: serde_json::Value = resp.into_json().ok()?;
    let content = value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?
        .trim()
        .to_string();

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}
