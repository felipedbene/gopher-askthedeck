//! `gopher-askthedeck` — an interactive tarot reading served over Gopher.
//!
//! Architecture follows the sibling gopher holes (`gopher-cta`, `gopher-blog`):
//! a **pure core** (deck, draw, cosmic math, ASCII frames, reading assembly)
//! with **thin IO** layered on top (the dcgi argv/stdout, the filesystem cache
//! and rate limiter, the DeepSeek HTTP call). Only the IO layer knows about
//! geomyidae or the network; the core is deterministic and clock-free.

pub mod deck;
