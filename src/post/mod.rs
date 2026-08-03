//! The on-disk post contract, shared by bckt and its companion tools.
//!
//! This module owns everything a tool needs to *write* a post that bckt will
//! later render: how a slug is derived, what a front matter block looks like,
//! where a post directory goes, and how front matter dates are parsed. It
//! deliberately knows nothing about rendering, themes, or templates, and it is
//! the only part of the crate available with `default-features = false`.

pub mod datetime;
pub mod frontmatter;
pub mod project;
pub mod slug;

pub use datetime::{date_prefix, format_rfc3339, parse_datetime, parse_offset};
pub use frontmatter::{FrontMatter, yaml_quote};
pub use project::{find_project_root, post_dir, post_file};
pub use slug::{non_empty, normalize_tags, slugify};
