# NIGHT-LOG

Reverse-chronological build notes.

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
