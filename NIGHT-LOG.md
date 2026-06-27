# NIGHT-LOG

Reverse-chronological build notes.

## 2026-06-27 — slice 8: shareable permalinks (+ prompt standardization, relabel)

Three follow-ups after the initial build, all live:

- **Standardized the LLM prompt.** The web askthedeck has no free-text question
  — `buildPrompt` is a fixed template (cards + astrology). This port had added an
  open-ended `THE SEEKER'S QUESTION` field (unfaithful + a prompt-injection
  surface). Now `build_prompt(spread, cosmic)` is fixed; the typed text only
  seeds the draw and never reaches the LLM. Guard test strengthened to assert an
  injection-y typed string never appears in the prompt.
- **Relabelled** the entry from "Ask the deck (type your question)" to
  "Draw three cards" — the text shuffles the draw, it isn't a question answered.
- **Shareable permalinks (saving via bookmarks).** Every reading is persisted as
  a plain-text snapshot at `/r/<id>.txt` (served straight from the docroot via a
  writable `atd-shared` volume), with a copyable `gopher://` permalink printed in
  the reading. id = content hash of cards + UTC day (NOT the typed text), so it
  doubles as the cache key and identical draws collapse to one link — matching
  askthedeck's card-keyed cache. The cache now stores the header-free reading
  *core*; display prepends a header with the typed echo, the stored snapshot
  prepends one without it (so a link never leaks what someone typed). 30-day
  mtime GC. Deliberately NO cookie/account history — bookmarks are the history.

## 2026-06-27 — initial build, slices 1–7

Ported askthedeck into Gopherspace as `gopher-askthedeck`: a mostly-static deck
tree plus one dynamic `draw.dcgi`. Built in seven vertical slices, one commit
each, every slice green (`cargo test` + clippy + fmt), and green both with and
without the `net` feature.

- **Slice 1 — deck + draw.** Ported the 78-card universe (ids, names) from
  askthedeck's `cards.ts`. Added determinism a gopher draw needs: FNV-1a question
  hash → SplitMix64 → 3 distinct cards + reversal bits. Same seed ⇒ same spread,
  so the seed doubles as the cache key.
- **Slice 2 — cosmic.** Std-only ephemeris (Schlyter + the Moon's principal
  perturbations) replacing the astronomy-engine dependency. Validated bucket
  labels against ground truth captured from astronomy-engine across six fixture
  dates; sun within 0.1°, moon within 0.5°. `prompt_block()` is date-free by
  design (the ethical invariant).
- **Slice 3 — frames + static build.** Pure ASCII card renderer (pips by rank,
  courts, 22 major motifs, reversed flips the motif). Authored upright+reversed
  meanings for all 78 (askthedeck has none). `site` builds the menus + 78 card
  pages + about/caps/cosmic. Needed a type-7 menu item, so **gopher-core gained
  `ItemKind::Search` and was cut as v0.2.0** (additive; cta/blog stay on v0.1.0).
- **Slice 4 — offline reading.** `local_reading` assembles a real reading from
  meanings + positions + a cosmic-coloured focus line (waxing builds / waning
  releases; element sets the tone). `build_prompt` ported and trimmed.
- **Slice 5 — dcgi + the guard.** Verified geomyidae's call convention
  (`$search $arguments $host $port $traversal $selector`; question = argv[1];
  `.dcgi` ⇒ gophermap). Empty question → prompt; else a reading gophermap with
  real links. The prompt-guard release-gate test landed here.
- **Slice 6 — DeepSeek + controls.** `dcgi::handle` = per-IP throttle → seed
  cache → daily cap → DeepSeek (or local) → cache. LLM injected, so cache/cap/
  rate-limit are all unit-tested with a counting closure and tempdirs, no
  network. IP hashed at the edge from `$REMOTE_ADDR`.
- **Slice 7 — deploy + docs.** Immutable image (bakes the tree, builds
  geomyidae, drops in the `draw.dcgi` wrapper), compose on :7072 (independent
  hole, Watchtower-swapped), amd64-only CI (build/test/clippy/fmt × net+no-net,
  gitleaks, GHCR push), README/CLAUDE/this log, gitleaks + pre-commit.

### Deployed — live at gopher://gopher.debene.dev:7072/

Pushed to GitHub, image built by CI to GHCR (kept **private**; the VPS docker
is logged into ghcr.io to pull it), and brought up via compose on the RackNerd
box beside cta (:70) and blog (:7071). Verified end to end, externally:
root menu, card pages, and the dynamic draw all serve; a real DeepSeek reading
takes ~9s and the identical repeat is 0s + byte-identical (cache hit, zero LLM
calls). The DeepSeek key is live in the container.

Two geomyidae flag findings (the docs were thin/contradictory — verified
empirically on the box):

- **`-h gopher.debene.dev` is required.** Without it geomyidae substitutes the
  `.gph`/`.dcgi` `server` token with the container id, so the type-7 item and
  every link pointed external clients at an unreachable host. Passed via the
  compose `command` (keeps the image host-agnostic).
- **`-c` is chroot, NOT cgi-enable.** The usage string lists `-c`; adding it
  crash-loops as `nobody` ("chroot: Operation not permitted"). CGI/DCGI is
  enabled purely by the `.dcgi` extension + exec bit, exactly as CGI.md says —
  no flag. Do not pass `-c`.

CI note: the first push's gitleaks job went red on the known root-commit range
bug (`<root>^..HEAD` is an invalid range; it logged "no leaks found in partial
scan"). `test` + `image` were green and the image published; the next push
cleared it.

### Decisions / open questions

- **Topology:** chose an independent geomyidae container on **:7072** (mirrors
  blog, isolates the dcgi, no change to the live cta daemon) over sharing cta's
  :70 docroot. The shared-docroot path is supported via `--base-prefix`/`ATD_BASE`
  but deferred. Confirm this is what we want before pointing Floodgap at it.
- **DeepSeek key inheritance:** relies on geomyidae not clearing the environment
  (verified behaviour), with a wrapper fallback that sources
  `/etc/gopher-askthedeck.env`. Worth a smoke test on the VPS that a live key
  actually reaches the dcgi.

### Deferred (out of scope this round)

Big-endian/G5 (ring has no BE support); multi-step card picking; accounts/saved
history; non-English readings; a client-exposed reversals toggle; web gateway.
