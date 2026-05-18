//! Zen-server wire format support.
//!
//! UECM only ever reads zen's responses (`/health/info`, `/stats`, `/stats/z$`)
//! and the `.lock` lockfile. Both are serialized in UE's Compact Binary (CB)
//! format. Submodules here implement a read-only mini-parser for that format;
//! we never produce CB ourselves.

pub mod cache_stats;
pub mod cb_parser;
pub mod lockfile;
pub mod probe;

#[cfg(test)]
pub mod test_http;
