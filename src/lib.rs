//! bckt, a static site generator for blogs.
//!
//! The crate is split in two along the `render` feature, which is on by
//! default. `post` is always available and carries the on-disk post contract
//! that companion tools need in order to write posts. Everything else — the
//! render pipeline, templates, themes, the dev server, the CLI — is gated
//! behind `render`, so a tool that only writes posts can depend on this crate
//! with `default-features = false` and pull in nothing but `anyhow` and `time`.

pub mod post;

#[cfg(feature = "render")]
pub mod cli;
#[cfg(feature = "render")]
pub mod commands;
#[cfg(feature = "render")]
pub mod config;
#[cfg(feature = "render")]
pub mod content;
#[cfg(feature = "render")]
pub mod language;
#[cfg(feature = "render")]
pub mod markdown;
#[cfg(feature = "render")]
pub mod render;
#[cfg(feature = "render")]
pub mod search;
#[cfg(feature = "render")]
pub mod template;
#[cfg(feature = "render")]
pub mod theme;
#[cfg(feature = "render")]
pub mod utils;
