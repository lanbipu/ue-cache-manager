//! Zen-server wire format support.
//!
//! UECM only ever reads zen's responses (`/health/info`, `/stats`, `/stats/z$`)
//! and the `.lock` lockfile. Both are serialized in UE's Compact Binary (CB)
//! format. Submodules here implement a read-only mini-parser for that format;
//! we never produce CB ourselves.

pub mod cb_parser;
