# gopher-askthedeck: a mostly-static gopher hole with one dynamic dcgi.
#
# The static deck tree (menus, 78 card pages, about, caps, cosmic) is baked at
# BUILD time. The only dynamic surface is /srv/draw.dcgi, a tiny wrapper that
# execs the same binary's `draw` subcommand per request; geomyidae runs it
# because of the .dcgi extension + exec bit (there is no daemon-wide CGI flag,
# so this changes nothing about how any sibling static tree is served).
#
#   docker build -t gopher-askthedeck .
#   docker run --rm -p 7072:7072 -e DEEPSEEK_API_KEY=sk-... gopher-askthedeck
#   lynx gopher://127.0.0.1:7072/
#
# With no key the hole still works end to end: every reading is the
# deterministic offline one.

# --- 1. Build the binary (with the `net` feature for the DeepSeek call) ----
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

# --- 2. Bake the static tree -----------------------------------------------
FROM build AS render
# Render, then dereference the `current` symlink so the image carries a plain
# tree at a fixed path.
RUN /src/target/release/gopher-askthedeck build --out /build/out \
 && cp -rL /build/out/current /export \
 && echo "baked $(find /export -type f | wc -l) files"

# --- 3. Build geomyidae (mirrors the sibling holes) ------------------------
# Not packaged in Debian; build from the canonical bitreich source over git://
# (the HTTPS "tarball" returns HTML). Build host needs port 9418 egress. TLS
# support disabled (no gophers://), so the runtime needs only libc.
FROM debian:bookworm-slim AS geo
RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates gcc make libc6-dev \
 && rm -rf /var/lib/apt/lists/*
ARG GEOMYIDAE_REF=v0.99
RUN git clone git://bitreich.org/geomyidae /g \
 && cd /g && git checkout "$GEOMYIDAE_REF" \
 && make TLS_CFLAGS= TLS_LDFLAGS=

# --- 4. Runtime: geomyidae serving the baked tree + the dcgi on :7072 ------
FROM debian:bookworm-slim
# ca-certificates: belt-and-suspenders for the DeepSeek TLS call (ureq+rustls
# bundles webpki roots, but this keeps the system store sane too).
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=geo /g/geomyidae /usr/local/bin/geomyidae
COPY --from=build /src/target/release/gopher-askthedeck /usr/local/bin/gopher-askthedeck
COPY --from=render /export /srv

# The dynamic entry: geomyidae calls this with
#   $search $arguments $host $port $traversal $selector
# We optionally source a secrets file mounted OUTSIDE the docroot, then exec the
# binary's draw path. (A .env inside /srv would be served over gopher — never do
# that; the key comes from the container environment or this mounted file.)
RUN printf '%s\n' \
      '#!/bin/sh' \
      '[ -r /etc/gopher-askthedeck.env ] && . /etc/gopher-askthedeck.env' \
      'exec /usr/local/bin/gopher-askthedeck draw "$@"' \
      > /srv/draw.dcgi \
 && chmod 0755 /srv/draw.dcgi

# Writable, sticky state dir for the cache, rate-limit buckets, and daily cap
# (geomyidae runs as nobody; 1777 lets it write).
RUN mkdir -p /var/cache/atd && chmod 1777 /var/cache/atd
ENV ATD_STATE_DIR=/var/cache/atd

USER nobody:nogroup
EXPOSE 7072
# -d: stay in the foreground; -b: serve the baked tree; -p: port.
ENTRYPOINT ["geomyidae", "-d", "-b", "/srv", "-p", "7072"]
