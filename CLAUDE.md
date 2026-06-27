# CLAUDE.md — working notes for gopher-askthedeck

A gopher port of askthedeck: a 3-card tarot reading with real astrological
context, served by geomyidae. Sibling to gopher-cta and gopher-blog.

## The non-negotiables

1. **The standardized-prompt + ethical invariant is a release gate.** The LLM
   prompt is a FIXED template (like askthedeck's `buildPrompt`): its only
   variable fields are the `Position: Card` lines and the cosmic block. The
   seeker's typed text only SEEDS THE DRAW — it must never enter the prompt (no
   open-ended field, no injection surface). `reading::build_prompt` takes only
   `(spread, cosmic)`; keep it that way. The prompt must also never carry client
   IP/hostname/port/selector/user-agent/geo or a locating timestamp, and
   `cosmic::Cosmic::prompt_block` must stay date-free. The test
   `reading::tests::prompt_is_standardized_and_leaks_nothing` enforces all of
   this — if you touch prompt assembly, it must stay green.

2. **The app must always answer.** No key, no network, or over the daily cap →
   the deterministic `reading::local_reading`. The whole suite is green with
   `--no-default-features` (no TLS stack); keep it that way.

3. **Cost controls are load-bearing.** This is a public dcgi calling a paid LLM.
   Don't weaken the cache (seed-keyed, zero-LLM on hit), the daily cap, or the
   per-IP rate limit. The client IP is hashed at the edge and never logged in
   clear or passed toward the LLM.

4. **Never log the question text.** It's the seeker's private intent. The same
   reason the shareable snapshot (`share::store`) is rendered with
   `render_header(None, ..)` — a permalink must never expose what someone typed.

5. **Saving = bookmarks, not accounts.** Sharing is a content-addressed
   permalink at `/r/<id>.txt` (id = cards + UTC day, not the typed text). Do NOT
   add cookies/accounts/server-side per-person history — it contradicts the
   no-tracking ethic. The cache stores the header-free reading *core* keyed by
   the same id; the display copy and the shared copy both prepend a header to it.

## Shape

- Pure core (`deck`, `cosmic`, `meanings`, `frame`, `reading`, `site`) — no clock,
  no fs, no net; deterministic; densely unit-tested.
- Thin IO (`cache`, `ratelimit`, `deepseek`, `dcgi` + `main`) — the only place
  that knows geomyidae, the filesystem, the clock, or the network.
- The clock is injected into pure code (`CivilTime`/`now_unix`) so everything is
  testable without a wall clock.

## geomyidae facts (verified against the man page / CGI.md)

- A dcgi is called `script $search $arguments $host $port $traversal $selector`.
  The draw is a **shuffle**, so the menu item is a plain **type-1** link and
  `$search`/`$arguments` are empty/ignored; the spread is seeded from
  server-clock entropy. `$host/$port` are the *server's* (used only for the share
  permalink). When the menu item was type-7 it forced a confusing "type a
  question" box — don't go back to that.
- `.dcgi` stdout is interpreted as a gophermap (`.gph`); `.cgi` is raw. We emit a
  gophermap (`gopher-core::render_menu_index`).
- CGI is activated by the `.cgi`/`.dcgi` extension + exec bit; there is **no
  daemon-wide enable flag**, so it can't change how a static tree is served.
- The client IP is in `$REMOTE_ADDR`; `$QUERY_STRING`/`$SELECTOR`/`$TRAVERSAL`
  are also set. geomyidae adds its vars without clearing the environment, so the
  container's `DEEPSEEK_API_KEY` is inherited by the dcgi (the wrapper also
  sources `/etc/gopher-askthedeck.env` if present).

## Build / test

```bash
cargo test --all
cargo test --all --no-default-features
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings
cargo fmt --all --check
cargo run -- build --out public
cargo run -- draw "" "" gopher.debene.dev 7072 0 /draw.dcgi   # a shuffle draw
```

## Dependencies

- `gopher-core` (tag, **v0.2.0**) — menu model + `.gph` serializer + atomic
  publish. (v0.2.0 added `ItemKind::Search`/type-7, which we no longer use now
  that the draw is a type-1 link; v0.1.0 would also work, but the pin is fine.)
  Bump the tag deliberately.
- `ureq` + `serde_json` (only under the `net` feature) — the DeepSeek call.
- `dotenvy` — load a gitignored `.env` in dev.

## Astrology

Ported from astronomy-engine to a std-only low-precision ephemeris (Schlyter +
the Moon's principal perturbation terms). Bucket labels match astronomy-engine on
the pinned fixture dates; if you touch `cosmic.rs`, keep
`labels_match_astronomy_engine` green. To regenerate ground truth, run the
fixture script in the commit history against the astronomy-engine npm package.

## Out of scope (this round)

Big-endian (ring has no BE support — amd64 only); multi-step picking;
accounts/history; non-English; client-exposed reversals toggle; non-tarot systems.
