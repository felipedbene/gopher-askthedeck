# gopher-askthedeck

A three-card tarot reading, drawn live and served over **Gopher** (RFC 1436) by
geomyidae. You select a type-7 "Ask the deck" item, type your question, and the
deck answers in three positions — each read against the **real sky** overhead
right now (moon phase, moon sign, zodiac season, planetary day, computed from the
server clock). A gopher port of [askthedeck](https://github.com/felipedbene/askthedeck),
sibling to [gopher-cta](https://github.com/felipedbene/gopher-cta) and
[gopher-blog](https://github.com/felipedbene/gopher-blog).

```
gopher://gopher.debene.dev:7072/
```

## Shape

Most of the hole is **static** (baked once at build time); only the reading is
**dynamic**. A small dynamic surface is a small attack/cost surface.

```
build (one-shot)  ── writes the static tree:
                       index.gph, about.txt, caps.txt, cosmic.txt,
                       cards/index.gph, cards/<slug>.txt   (78 ASCII frames + meanings)

draw.dcgi (per request, run by geomyidae on the type-7 item)
   1. question arrives as argv[1] (the type-7 search term)
   2. seed = hash(question + UTC day)          ← also the cache key
   3. rate-limit (per client-IP hash) → cache → daily cap
   4. draw 3 distinct cards (+ reversal) deterministically from the seed
   5. compute the cosmic context from the server clock
   6. DeepSeek (timeout) ── slow / down / over cap ──► deterministic local reading
   7. render: ASCII frames + narrative + links to each card's static page
   8. cache the result
```

There is no long-running daemon of our own and no fetch loop — geomyidae serves
the baked tree and execs `draw.dcgi` per request.

## The ethical invariant

The reading is built from **exactly three things**: your question, the three
cards you drew, and the sky. The assembled LLM prompt MUST NOT contain — directly
or laundered — the client **IP, hostname, port, selector path, user-agent,
geolocation, or a locating wall-clock timestamp**. This is structural:
`reading::build_prompt` only takes `(question, spread, cosmic)`, and the cosmic
block deliberately omits the calendar date (the moon phase + season already carry
the temporal context) and reports the *planet* (Saturn), never the weekday
(Saturday). A release-gate test (`prompt_never_contains_client_metadata`) fails
the build if any sentinel metadata could appear. The client IP is used only for
rate limiting, hashed at the edge, never logged in clear, never near the LLM.

No accounts, no cookies, no saved history, no tracking.

## Cost & abuse controls

A public dcgi that calls a paid LLM per hit is a cost hole, and this endpoint
gets crawled. So:

- **Cache** keyed by the draw seed: an identical question on the same UTC day
  returns the cached reading and makes **zero** LLM calls (24h flat-file TTL).
- **Daily call cap**: once the day's budget is spent, every draw falls back to
  the deterministic local reading. The slot is reserved *before* the call, so a
  transient outage degrades to local instead of hammering a failing paid API.
- **Per-IP token bucket** (keyed by a hash of the IP): a burst gets a polite
  "easy there" text item, not an error.
- **Deterministic local reading**: a real interpretation assembled from the
  static card meanings + positions + a cosmic-anchored line. The app **always**
  answers — no key, no network, or over budget.

## Quickstart

```bash
# Build the static tree locally and inspect it:
cargo run -- build --out public
lynx gopher://127.0.0.1:70/        # (after pointing a daemon at public/current)

# Try a reading on the command line (the dcgi calling convention):
#   draw  $search  $arguments  $host  $port  $traversal  $selector
cargo run -- draw "should I take the new job?"

# With a key (optional — offline works without it):
echo 'DEEPSEEK_API_KEY=sk-...' > .env
cargo run -- draw "what should I focus on?"

cargo test --all                          # core + IO + the prompt-guard gate
cargo test --all --no-default-features    # the same suite with no TLS stack
```

### Container

```bash
docker build -t gopher-askthedeck .
# -h sets the hostname geomyidae advertises in menu lines (so the type-7 item +
# links are dialable); pass your public name in real deploys.
docker run --rm -p 7072:7072 -e DEEPSEEK_API_KEY=sk-... gopher-askthedeck -h 127.0.0.1
lynx gopher://127.0.0.1:7072/
```

The image bakes the static tree, builds geomyidae from source, and drops in a
tiny `draw.dcgi` wrapper that execs the binary's `draw` path. geomyidae runs it
purely because of the `.dcgi` extension + exec bit — there is **no daemon-wide
CGI flag** (and note `-c` is *chroot*, not cgi-enable: it crash-loops as the
`nobody` user — don't add it), so enabling the dcgi here changes nothing about
how any sibling static hole is served.

## Deploy

`deploy/docker-compose.yml` runs it as an independent hole on **:7072** next to
gopher-cta (:70) and gopher-blog (:7071), with its own geomyidae container (so
the dcgi is fully isolated from the siblings). CI builds and pushes the image to
GHCR; the VPS's existing label-enabled Watchtower swaps it on a new digest — the
container recreate *is* the publish. Put the real key in a gitignored
`deploy/.env`. A persistent volume holds the cache + rate-limit + daily-cap
state across restarts.

> Alternative topology: it can instead share gopher-cta's docroot under a base
> path (build with `--base-prefix /tarot`, set `ATD_BASE=/tarot`), since CGI
> activation is per-file. The independent-container default is the lower-risk
> choice and matches the sibling holes.

Logs flow through geomyidae's access log (the same promtail→Loki→Grafana
`vclass` pipeline as the siblings). **The question text is never logged** — it's
the seeker's private intent.

## Architecture

Pure core / thin IO, like the siblings:

| Module | Pure? | Responsibility |
|---|---|---|
| `deck` | pure | the 78 cards + the deterministic seeded draw |
| `cosmic` | pure | the ephemeris (sun/moon longitude, sign, phase, planetary day) |
| `meanings` | pure | upright + reversed meaning for each card |
| `frame` | pure | the ASCII card-frame renderer |
| `reading` | pure | the spread description, the LLM prompt, the offline reading |
| `site` | pure | the static tree (menus + pages) → files |
| `cache` / `ratelimit` | IO (fs) | seed cache; per-IP bucket + daily cap |
| `deepseek` | IO (net) | the one HTTPS call (behind the `net` feature) |
| `dcgi` | IO | argv parsing + the orchestrated request path |

Menus and the `.gph` serializer come from
[`gopher-core`](https://github.com/felipedbene/gopher-core) (this needs its
type-7 `Search` kind, added in v0.2.0).

## Out of scope (this round)

- **G5 / ppc64 big-endian.** The DeepSeek HTTPS call uses ureq + rustls/ring,
  and ring has no big-endian support (the same gotcha as gopher-cta). amd64 only;
  a big-endian build would need an OpenSSL-backed TLS path.
- Multi-step card-picking flow; accounts/history/cookies; non-English readings;
  a client-exposed reversals toggle (it's a build flag); any non-tarot divination.

## Configuration

| Env var | Default | Meaning |
|---|---|---|
| `DEEPSEEK_API_KEY` | _(unset)_ | LLM key; unset ⇒ always the offline reading |
| `ATD_STATE_DIR` | temp dir | cache + rate-limit + daily-cap directory |
| `ATD_BASE` | _(empty)_ | selector base prefix (shared-docroot deploys) |
| `ATD_DAILY_CAP` | `500` | max LLM calls per UTC day |
| `ATD_RATE_CAPACITY` | `5` | per-IP token-bucket burst size |
| `ATD_RATE_REFILL` | `0.05` | tokens refilled per second (~1 / 20s) |
| `ATD_LLM_TIMEOUT` | `12` | DeepSeek connect+read timeout (seconds) |
